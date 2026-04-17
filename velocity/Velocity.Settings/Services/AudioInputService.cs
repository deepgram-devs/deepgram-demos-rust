using System.Runtime.InteropServices;

namespace Velocity.Settings.Services;

public sealed class AudioInputService : IAsyncDisposable
{
    private const int SampleRate = 48_000;
    private const int BufferBytes = 19_200;
    private const int BufferCount = 4;
    private const int MeterUpdateIntervalMs = 100;
    private const int MeterHoldMilliseconds = 250;
    private const int WaveMapper = unchecked((int)0xFFFFFFFF);
    private const int CallbackNull = 0;
    private const int WhdrDone = 0x00000001;
    private const double MinDecibels = -60.0;
    private const double SilenceFloor = 0.001;
    private const double MeterDecayPerSecond = 16.0;

    private readonly List<WaveBuffer> _buffers = new();
    private CancellationTokenSource? _monitorCancellation;
    private Task? _monitorTask;
    private IntPtr _waveInHandle = IntPtr.Zero;

    public event EventHandler<double>? LevelChanged;

    public Task<IReadOnlyList<string>> GetInputDeviceNamesAsync()
    {
        var devices = new List<string>();
        var count = waveInGetNumDevs();
        for (var index = 0u; index < count; index++)
        {
            if (waveInGetDevCapsW(index, out var caps, (uint)Marshal.SizeOf<WAVEINCAPSW>()) == 0)
            {
                var name = caps.szPname.TrimEnd('\0');
                if (!string.IsNullOrWhiteSpace(name))
                {
                    devices.Add(name);
                }
            }
        }

        return Task.FromResult<IReadOnlyList<string>>(devices
            .Distinct(StringComparer.OrdinalIgnoreCase)
            .OrderBy(name => name, StringComparer.OrdinalIgnoreCase)
            .ToList());
    }

    public async Task StartMeterAsync(string? selectedDeviceName)
    {
        await StopMeterAsync();

        var deviceId = ResolveDeviceId(selectedDeviceName);
        var format = new WAVEFORMATEX
        {
            wFormatTag = 1,
            nChannels = 1,
            nSamplesPerSec = SampleRate,
            nAvgBytesPerSec = SampleRate * 2,
            nBlockAlign = 2,
            wBitsPerSample = 16,
            cbSize = 0
        };

        var result = waveInOpen(out _waveInHandle, deviceId, ref format, IntPtr.Zero, IntPtr.Zero, CallbackNull);
        if (result != 0 || _waveInHandle == IntPtr.Zero)
        {
            _waveInHandle = IntPtr.Zero;
            throw new InvalidOperationException($"Unable to access the selected microphone (waveInOpen={result}).");
        }

        for (var index = 0; index < BufferCount; index++)
        {
            var buffer = new WaveBuffer(BufferBytes);
            var prepareResult = waveInPrepareHeader(_waveInHandle, buffer.HeaderPointer, (uint)Marshal.SizeOf<WAVEHDR>());
            if (prepareResult != 0)
            {
                buffer.Dispose();
                throw new InvalidOperationException($"Unable to prepare microphone buffer (waveInPrepareHeader={prepareResult}).");
            }

            var addResult = waveInAddBuffer(_waveInHandle, buffer.HeaderPointer, (uint)Marshal.SizeOf<WAVEHDR>());
            if (addResult != 0)
            {
                buffer.Dispose();
                throw new InvalidOperationException($"Unable to queue microphone buffer (waveInAddBuffer={addResult}).");
            }

            _buffers.Add(buffer);
        }

        var startResult = waveInStart(_waveInHandle);
        if (startResult != 0)
        {
            throw new InvalidOperationException($"Unable to start microphone monitoring (waveInStart={startResult}).");
        }

        _monitorCancellation = new CancellationTokenSource();
        _monitorTask = Task.Run(() => MonitorAsync(_monitorCancellation.Token));
        LevelChanged?.Invoke(this, MinDecibels);
    }

    public async Task StopMeterAsync()
    {
        _monitorCancellation?.Cancel();
        if (_monitorTask is not null)
        {
            try
            {
                await _monitorTask;
            }
            catch (OperationCanceledException)
            {
            }
        }

        _monitorTask = null;
        _monitorCancellation?.Dispose();
        _monitorCancellation = null;

        if (_waveInHandle != IntPtr.Zero)
        {
            waveInStop(_waveInHandle);
            waveInReset(_waveInHandle);
        }

        foreach (var buffer in _buffers)
        {
            if (_waveInHandle != IntPtr.Zero)
            {
                waveInUnprepareHeader(_waveInHandle, buffer.HeaderPointer, (uint)Marshal.SizeOf<WAVEHDR>());
            }
            buffer.Dispose();
        }
        _buffers.Clear();

        if (_waveInHandle != IntPtr.Zero)
        {
            waveInClose(_waveInHandle);
            _waveInHandle = IntPtr.Zero;
        }
    }

    public async ValueTask DisposeAsync()
    {
        await StopMeterAsync();
    }

    private async Task MonitorAsync(CancellationToken cancellationToken)
    {
        var displayedLevel = MinDecibels;
        var heldLevel = MinDecibels;
        var holdUntilUtc = DateTime.UtcNow;
        var decayPerTick = MeterDecayPerSecond * MeterUpdateIntervalMs / 1000.0;

        while (!cancellationToken.IsCancellationRequested)
        {
            var measuredLevel = MinDecibels;
            var sawCompletedBuffer = false;
            foreach (var buffer in _buffers)
            {
                var header = Marshal.PtrToStructure<WAVEHDR>(buffer.HeaderPointer);
                if ((header.dwFlags & WhdrDone) == 0)
                {
                    continue;
                }

                sawCompletedBuffer = true;
                measuredLevel = Math.Max(measuredLevel, ComputeDecibels(buffer.DataPointer, (int)header.dwBytesRecorded));
                RearmBuffer(buffer);
            }

            var nowUtc = DateTime.UtcNow;
            if (sawCompletedBuffer && measuredLevel >= heldLevel)
            {
                heldLevel = measuredLevel;
                displayedLevel = measuredLevel;
                holdUntilUtc = nowUtc.AddMilliseconds(MeterHoldMilliseconds);
            }
            else if (sawCompletedBuffer && measuredLevel > displayedLevel)
            {
                displayedLevel = measuredLevel;
                heldLevel = measuredLevel;
                holdUntilUtc = nowUtc.AddMilliseconds(MeterHoldMilliseconds);
            }
            else if (nowUtc >= holdUntilUtc)
            {
                displayedLevel = Math.Max(MinDecibels, displayedLevel - decayPerTick);
                heldLevel = displayedLevel;
            }

            LevelChanged?.Invoke(this, displayedLevel);
            await Task.Delay(MeterUpdateIntervalMs, cancellationToken);
        }
    }

    private void RearmBuffer(WaveBuffer buffer)
    {
        if (_waveInHandle == IntPtr.Zero)
        {
            return;
        }

        waveInUnprepareHeader(_waveInHandle, buffer.HeaderPointer, (uint)Marshal.SizeOf<WAVEHDR>());
        buffer.ResetHeader();
        waveInPrepareHeader(_waveInHandle, buffer.HeaderPointer, (uint)Marshal.SizeOf<WAVEHDR>());
        waveInAddBuffer(_waveInHandle, buffer.HeaderPointer, (uint)Marshal.SizeOf<WAVEHDR>());
    }

    private static unsafe double ComputeDecibels(IntPtr dataPointer, int bytesRecorded)
    {
        if (bytesRecorded <= 0)
        {
            return MinDecibels;
        }

        var sampleCount = bytesRecorded / sizeof(short);
        if (sampleCount <= 0)
        {
            return MinDecibels;
        }

        var samples = (short*)dataPointer;
        double sumSquares = 0.0;
        for (var index = 0; index < sampleCount; index++)
        {
            var sample = samples[index] / 32768.0;
            sumSquares += sample * sample;
        }

        var rms = Math.Sqrt(sumSquares / sampleCount);
        var decibels = 20.0 * Math.Log10(Math.Max(rms, SilenceFloor));
        return Math.Clamp(decibels, MinDecibels, 0.0);
    }

    private static uint ResolveDeviceId(string? selectedDeviceName)
    {
        if (string.IsNullOrWhiteSpace(selectedDeviceName))
        {
            return unchecked((uint)WaveMapper);
        }

        var count = waveInGetNumDevs();
        for (var index = 0u; index < count; index++)
        {
            if (waveInGetDevCapsW(index, out var caps, (uint)Marshal.SizeOf<WAVEINCAPSW>()) == 0)
            {
                var name = caps.szPname.TrimEnd('\0');
                if (string.Equals(name, selectedDeviceName, StringComparison.OrdinalIgnoreCase))
                {
                    return index;
                }
            }
        }

        return unchecked((uint)WaveMapper);
    }

    private sealed class WaveBuffer : IDisposable
    {
        public WaveBuffer(int bufferBytes)
        {
            DataPointer = Marshal.AllocHGlobal(bufferBytes);
            HeaderPointer = Marshal.AllocHGlobal(Marshal.SizeOf<WAVEHDR>());
            BufferBytes = bufferBytes;
            ResetHeader();
        }

        public IntPtr DataPointer { get; }

        public IntPtr HeaderPointer { get; }

        private int BufferBytes { get; }

        public void ResetHeader()
        {
            var header = new WAVEHDR
            {
                lpData = DataPointer,
                dwBufferLength = (uint)BufferBytes,
                dwBytesRecorded = 0,
                dwUser = IntPtr.Zero,
                dwFlags = 0,
                dwLoops = 0,
                lpNext = IntPtr.Zero,
                reserved = IntPtr.Zero
            };
            Marshal.StructureToPtr(header, HeaderPointer, false);
        }

        public void Dispose()
        {
            if (HeaderPointer != IntPtr.Zero)
            {
                Marshal.FreeHGlobal(HeaderPointer);
            }

            if (DataPointer != IntPtr.Zero)
            {
                Marshal.FreeHGlobal(DataPointer);
            }
        }
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct WAVEFORMATEX
    {
        public ushort wFormatTag;
        public ushort nChannels;
        public uint nSamplesPerSec;
        public uint nAvgBytesPerSec;
        public ushort nBlockAlign;
        public ushort wBitsPerSample;
        public ushort cbSize;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct WAVEHDR
    {
        public IntPtr lpData;
        public uint dwBufferLength;
        public uint dwBytesRecorded;
        public IntPtr dwUser;
        public uint dwFlags;
        public uint dwLoops;
        public IntPtr lpNext;
        public IntPtr reserved;
    }

    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    private struct WAVEINCAPSW
    {
        public ushort wMid;
        public ushort wPid;
        public uint vDriverVersion;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 32)]
        public string szPname;
        public uint dwFormats;
        public ushort wChannels;
        public ushort wReserved1;
    }

    [DllImport("winmm.dll")]
    private static extern uint waveInGetNumDevs();

    [DllImport("winmm.dll", CharSet = CharSet.Unicode)]
    private static extern uint waveInGetDevCapsW(uint deviceId, out WAVEINCAPSW waveInCaps, uint waveInCapsSize);

    [DllImport("winmm.dll")]
    private static extern uint waveInOpen(out IntPtr waveInHandle, uint deviceId, ref WAVEFORMATEX format, IntPtr callback, IntPtr instance, uint flags);

    [DllImport("winmm.dll")]
    private static extern uint waveInPrepareHeader(IntPtr waveInHandle, IntPtr waveHeader, uint waveHeaderSize);

    [DllImport("winmm.dll")]
    private static extern uint waveInUnprepareHeader(IntPtr waveInHandle, IntPtr waveHeader, uint waveHeaderSize);

    [DllImport("winmm.dll")]
    private static extern uint waveInAddBuffer(IntPtr waveInHandle, IntPtr waveHeader, uint waveHeaderSize);

    [DllImport("winmm.dll")]
    private static extern uint waveInStart(IntPtr waveInHandle);

    [DllImport("winmm.dll")]
    private static extern uint waveInStop(IntPtr waveInHandle);

    [DllImport("winmm.dll")]
    private static extern uint waveInReset(IntPtr waveInHandle);

    [DllImport("winmm.dll")]
    private static extern uint waveInClose(IntPtr waveInHandle);
}

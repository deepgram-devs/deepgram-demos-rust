using Microsoft.UI.Windowing;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Media.Animation;
using Microsoft.UI;
using Windows.Graphics;
using Windows.UI;
using WinRT.Interop;
using Velocity.Settings.Models;
using Velocity.Settings.Services;

namespace Velocity.Settings;

public sealed partial class MainWindow : Window
{
    private const string DefaultAudioInputLabel = "Default system input";
    private const double MinMeterDecibels = -60.0;

    private readonly ConfigFileService _configFileService = new();
    private readonly AudioInputService _audioInputService = new();
    private readonly string _launchPage;
    private readonly DispatcherQueueTimer _configMonitorTimer;
    private DateTimeOffset? _lastLoadedConfigWriteTimeUtc;
    private bool _configChangedExternally;
    private bool _suppressAudioSelectionChanged;
    private double _currentMeterDecibels = MinMeterDecibels;

    public MainWindow(string launchPage)
    {
        _launchPage = launchPage;
        StartupLog.Write("MainWindow constructor");
        InitializeComponent();
        ConfigureWindow();
        SubtitleTextBlock.Text = _launchPage.Equals("api-key", StringComparison.OrdinalIgnoreCase)
            ? "API key onboarding"
            : "Application settings";
        LaunchModeTextBlock.Text = $"Launch mode: {_launchPage}";

        OutputModeComboBox.Items.Add("Type directly");
        OutputModeComboBox.Items.Add("Copy to clipboard");
        OutputModeComboBox.Items.Add("Paste clipboard");
        _audioInputService.LevelChanged += AudioInputService_LevelChanged;
        SetMeterLevel(MinMeterDecibels);

        _configMonitorTimer = DispatcherQueue.GetForCurrentThread().CreateTimer();
        _configMonitorTimer.Interval = TimeSpan.FromSeconds(2);
        _configMonitorTimer.Tick += ConfigMonitorTimer_Tick;
        _configMonitorTimer.Start();

        Activated += MainWindow_Activated;
        Closed += MainWindow_Closed;
    }

    private void ConfigureWindow()
    {
        var windowHandle = WindowNative.GetWindowHandle(this);
        var windowId = Microsoft.UI.Win32Interop.GetWindowIdFromWindow(windowHandle);
        var appWindow = AppWindow.GetFromWindowId(windowId);
        var iconPath = Path.Combine(AppContext.BaseDirectory, "Assets", "deepgram-icon.ico");
        if (File.Exists(iconPath))
        {
            appWindow.SetIcon(iconPath);
        }
        appWindow.Resize(new SizeInt32(760, 920));
    }

    private async void MainWindow_Activated(object sender, WindowActivatedEventArgs args)
    {
        StartupLog.Write("MainWindow activated event");
        Activated -= MainWindow_Activated;
        await LoadAudioInputsAsync();
        await LoadConfigAsync();
    }

    private void MainWindow_Closed(object sender, WindowEventArgs args)
    {
        _configMonitorTimer.Stop();
        _audioInputService.LevelChanged -= AudioInputService_LevelChanged;
        _ = _audioInputService.DisposeAsync();
        StartupLog.Write("MainWindow closed event");
    }

    private async void ReloadButton_Click(object sender, RoutedEventArgs e)
    {
        await LoadAudioInputsAsync(GetSelectedAudioInputName());
        await LoadConfigAsync();
    }

    private async void SaveButton_Click(object sender, RoutedEventArgs e)
    {
        await SaveConfigAsync();
    }

    private async void SaveKeyboardAccelerator_Invoked(KeyboardAccelerator sender, KeyboardAcceleratorInvokedEventArgs args)
    {
        args.Handled = true;
        await SaveConfigAsync();
    }

    private async Task SaveConfigAsync()
    {
        try
        {
            await _configFileService.SaveAsync(BuildConfig());
            _lastLoadedConfigWriteTimeUtc = _configFileService.GetConfigWriteTimeUtc();
            SetConfigChangedWarning(false);
            StatusTextBlock.Text = $"Saved {_configFileService.ConfigPath}";
        }
        catch (Exception error)
        {
            StatusTextBlock.Text = error.Message;
        }
    }

    private async Task LoadConfigAsync()
    {
        try
        {
            var config = await _configFileService.LoadAsync();
            await ApplyConfigAsync(config);
            _lastLoadedConfigWriteTimeUtc = _configFileService.GetConfigWriteTimeUtc();
            SetConfigChangedWarning(false);
            StatusTextBlock.Text = $"Loaded {_configFileService.ConfigPath}";
            StartupLog.Write("Config loaded");
        }
        catch (Exception error)
        {
            StatusTextBlock.Text = error.Message;
            StartupLog.Write($"Config load error: {error}");
        }
    }

    private async Task ApplyConfigAsync(VelocityConfig config)
    {
        ApiKeyBox.Password = config.ApiKey ?? string.Empty;
        ModelBox.Text = config.Model;
        SmartFormatToggle.IsOn = config.SmartFormat;
        KeyTermsBox.Text = FormatKeyTerms(config.KeyTerms);
        PushToTalkBox.Text = config.Hotkeys.PushToTalk;
        KeepTalkingBox.Text = config.Hotkeys.KeepTalking;
        StreamingBox.Text = config.Hotkeys.Streaming;
        ResendBox.Text = config.Hotkeys.ResendSelected;
        HistoryLimitBox.Value = config.HistoryLimit;
        AppendNewlineToggle.IsOn = config.AppendNewline;

        _suppressAudioSelectionChanged = true;
        AudioInputComboBox.SelectedItem = DefaultAudioInputLabel;
        if (!string.IsNullOrWhiteSpace(config.AudioInput))
        {
            if (!AudioInputComboBox.Items.Contains(config.AudioInput))
            {
                AudioInputComboBox.Items.Add(config.AudioInput);
            }
            AudioInputComboBox.SelectedItem = config.AudioInput;
        }
        _suppressAudioSelectionChanged = false;

        OutputModeComboBox.SelectedItem = config.OutputMode switch
        {
            "clipboard" => "Copy to clipboard",
            "paste" => "Paste clipboard",
            _ => "Type directly"
        };

        await RestartAudioMeterAsync(GetSelectedAudioInputName());
    }

    private VelocityConfig BuildConfig()
    {
        return new VelocityConfig
        {
            ApiKey = string.IsNullOrWhiteSpace(ApiKeyBox.Password) ? null : ApiKeyBox.Password.Trim(),
            Model = string.IsNullOrWhiteSpace(ModelBox.Text) ? "nova-3" : ModelBox.Text.Trim(),
            SmartFormat = SmartFormatToggle.IsOn,
            KeyTerms = ParseKeyTerms(KeyTermsBox.Text),
            Hotkeys = new HotkeyConfig
            {
                PushToTalk = PushToTalkBox.Text.Trim(),
                KeepTalking = KeepTalkingBox.Text.Trim(),
                Streaming = StreamingBox.Text.Trim(),
                ResendSelected = ResendBox.Text.Trim()
            },
            AudioInput = AudioInputComboBox.SelectedItem as string is { Length: > 0 } audioInput && audioInput != DefaultAudioInputLabel
                ? audioInput
                : null,
            HistoryLimit = Math.Max(1, Convert.ToInt32(HistoryLimitBox.Value)),
            OutputMode = (OutputModeComboBox.SelectedItem as string) switch
            {
                "Copy to clipboard" => "clipboard",
                "Paste clipboard" => "paste",
                _ => "direct-input"
            },
            AppendNewline = AppendNewlineToggle.IsOn
        };
    }

    private void ConfigMonitorTimer_Tick(DispatcherQueueTimer sender, object args)
    {
        var currentWriteTimeUtc = _configFileService.GetConfigWriteTimeUtc();
        if (currentWriteTimeUtc == _lastLoadedConfigWriteTimeUtc)
        {
            return;
        }

        if (currentWriteTimeUtc is null && _lastLoadedConfigWriteTimeUtc is null)
        {
            return;
        }

        if (_configChangedExternally)
        {
            return;
        }

        SetConfigChangedWarning(true);
    }

    private void SetConfigChangedWarning(bool changedExternally)
    {
        _configChangedExternally = changedExternally;
        ConfigChangedWarningTextBlock.Visibility = changedExternally
            ? Visibility.Visible
            : Visibility.Collapsed;
    }

    private async Task LoadAudioInputsAsync(string? preferredSelection = null)
    {
        try
        {
            var devices = await _audioInputService.GetInputDeviceNamesAsync();
            var selection = preferredSelection ?? GetSelectedAudioInputName();

            _suppressAudioSelectionChanged = true;
            AudioInputComboBox.Items.Clear();
            AudioInputComboBox.Items.Add(DefaultAudioInputLabel);
            foreach (var device in devices)
            {
                AudioInputComboBox.Items.Add(device);
            }

            AudioInputComboBox.SelectedItem = !string.IsNullOrWhiteSpace(selection) && AudioInputComboBox.Items.Contains(selection)
                ? selection
                : DefaultAudioInputLabel;
            _suppressAudioSelectionChanged = false;
        }
        catch (Exception error)
        {
            _suppressAudioSelectionChanged = false;
            StatusTextBlock.Text = $"Unable to enumerate audio inputs: {error.Message}";
            StartupLog.Write($"Audio input enumeration error: {error}");
        }
    }

    private async void AudioInputComboBox_SelectionChanged(object sender, SelectionChangedEventArgs e)
    {
        if (_suppressAudioSelectionChanged)
        {
            return;
        }

        await RestartAudioMeterAsync(GetSelectedAudioInputName());
    }

    private async Task RestartAudioMeterAsync(string? selectedAudioInput)
    {
        try
        {
            await _audioInputService.StartMeterAsync(selectedAudioInput);
            StatusTextBlock.Text = selectedAudioInput is null
                ? "Monitoring default system input"
                : $"Monitoring {selectedAudioInput}";
        }
        catch (Exception error)
        {
            SetMeterLevel(MinMeterDecibels);
            MicLevelTextBlock.Text = "Microphone monitoring unavailable";
            StatusTextBlock.Text = error.Message;
            StartupLog.Write($"Audio meter error: {error}");
        }
    }

    private string? GetSelectedAudioInputName() =>
        AudioInputComboBox.SelectedItem as string is { Length: > 0 } audioInput && audioInput != DefaultAudioInputLabel
            ? audioInput
            : null;

    private void AudioInputService_LevelChanged(object? sender, double decibels)
    {
        DispatcherQueue.TryEnqueue(() => SetMeterLevel(decibels));
    }

    private void MicLevelTrack_SizeChanged(object sender, SizeChangedEventArgs e)
    {
        UpdateMeterFillWidth(animate: false);
    }

    private void SetMeterLevel(double decibels)
    {
        var clamped = Math.Clamp(decibels, MinMeterDecibels, 0.0);
        _currentMeterDecibels = clamped;
        MicLevelTextBlock.Text = $"{clamped:0.0} dB";
        MicLevelFill.Background = new SolidColorBrush(GetMeterColor(clamped));
        UpdateMeterFillWidth(animate: true);
    }

    private void UpdateMeterFillWidth(bool animate)
    {
        var trackWidth = MicLevelTrack.ActualWidth;
        if (trackWidth <= 0)
        {
            return;
        }

        var normalized = (_currentMeterDecibels - MinMeterDecibels) / -MinMeterDecibels;
        var targetWidth = Math.Clamp(normalized, 0.0, 1.0) * trackWidth;
        if (!animate)
        {
            MicLevelFill.Width = targetWidth;
            return;
        }

        var animation = new DoubleAnimation
        {
            To = targetWidth,
            Duration = new Duration(TimeSpan.FromMilliseconds(90)),
            EnableDependentAnimation = true
        };

        var storyboard = new Storyboard();
        storyboard.Children.Add(animation);
        Storyboard.SetTarget(animation, MicLevelFill);
        Storyboard.SetTargetProperty(animation, nameof(Border.Width));
        storyboard.Begin();
    }

    private static Color GetMeterColor(double decibels) =>
        decibels > -10.0
            ? Colors.IndianRed
            : decibels > -20.0
                ? Colors.Goldenrod
                : Colors.YellowGreen;

    private static string FormatKeyTerms(IEnumerable<string> keyTerms) =>
        string.Join(", ", keyTerms.Where(term => !string.IsNullOrWhiteSpace(term)).Select(term => term.Trim()));

    private static List<string> ParseKeyTerms(string rawValue) =>
        rawValue
            .Split(',', StringSplitOptions.RemoveEmptyEntries | StringSplitOptions.TrimEntries)
            .ToList();
}

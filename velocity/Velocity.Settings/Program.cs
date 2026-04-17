using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using System.Threading;
using WinRT;

namespace Velocity.Settings;

public static class Program
{
    private static App? _app;

    [STAThread]
    public static void Main(string[] args)
    {
        StartupLog.Write("Program.Main enter");
        ComWrappersSupport.InitializeComWrappers();
        Application.Start(_launchArgs =>
        {
            StartupLog.Write("Application.Start callback");
            SynchronizationContext.SetSynchronizationContext(
                new DispatcherQueueSynchronizationContext(DispatcherQueue.GetForCurrentThread()));
            _app = new App();
            StartupLog.Write("App stored in static field");
        });
        StartupLog.Write("Program.Main exit");
    }
}

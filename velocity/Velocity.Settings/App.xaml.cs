using Microsoft.UI.Xaml;

namespace Velocity.Settings;

public sealed partial class App : Application
{
    private Window? _window;

    public App()
    {
        StartupLog.Write("App constructor");
        InitializeComponent();
        UnhandledException += App_UnhandledException;
    }

    protected override void OnLaunched(LaunchActivatedEventArgs args)
    {
        StartupLog.Write("OnLaunched");
        var launchPage = ParseLaunchPage(Environment.GetCommandLineArgs());
        _window = new MainWindow(launchPage);
        _window.Activate();
        StartupLog.Write("Window activated");
    }

    private static void App_UnhandledException(object sender, Microsoft.UI.Xaml.UnhandledExceptionEventArgs e)
    {
        StartupLog.Write($"Unhandled exception: {e.Exception}");
        try
        {
            var logPath = Path.Combine(AppContext.BaseDirectory, "Velocity.Settings.log");
            File.AppendAllText(
                logPath,
                $"{DateTimeOffset.Now:u} {e.Exception.Message}{Environment.NewLine}{e.Exception}{Environment.NewLine}{Environment.NewLine}");
        }
        catch
        {
        }
    }

    private static string ParseLaunchPage(string[] args)
    {
        for (var index = 0; index < args.Length - 1; index++)
        {
            if (string.Equals(args[index], "--page", StringComparison.OrdinalIgnoreCase))
            {
                return args[index + 1];
            }
        }

        return "settings";
    }
}

internal static class StartupLog
{
    private static readonly string LogPath = Path.Combine(AppContext.BaseDirectory, "Velocity.Settings.startup.log");

    public static void Write(string message)
    {
        try
        {
            File.AppendAllText(LogPath, $"{DateTimeOffset.Now:u} {message}{Environment.NewLine}");
        }
        catch
        {
        }
    }
}

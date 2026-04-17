using YamlDotNet.Serialization;

namespace Velocity.Settings.Models;

public sealed class VelocityConfig
{
    public string? ApiKey { get; set; }

    public bool SmartFormat { get; set; }

    public string Model { get; set; } = "nova-3";

    [YamlMember(Alias = "keyterms", ApplyNamingConventions = false)]
    public List<string> KeyTerms { get; set; } = [];

    public HotkeyConfig Hotkeys { get; set; } = new();

    public string? AudioInput { get; set; }

    public int HistoryLimit { get; set; } = 20;

    public string OutputMode { get; set; } = "direct-input";

    public bool AppendNewline { get; set; }
}

public sealed class HotkeyConfig
{
    public string PushToTalk { get; set; } = "Win+Ctrl+'";

    public string KeepTalking { get; set; } = "Win+Ctrl+Shift+'";

    public string Streaming { get; set; } = "Win+Ctrl+[";

    public string ResendSelected { get; set; } = "Win+Ctrl+]";
}

using System.Collections.ObjectModel;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using Microsoft.UI.Xaml.Controls;
using Velocity.Settings.Models;
using Velocity.Settings.Services;

namespace Velocity.Settings.ViewModels;

public partial class MainViewModel : ObservableObject
{
    private readonly ConfigFileService _configFileService;

    [ObservableProperty]
    private string _launchPage;

    [ObservableProperty]
    private string _subtitle;

    [ObservableProperty]
    private string _apiKey = string.Empty;

    [ObservableProperty]
    private string _model = "nova-3";

    [ObservableProperty]
    private string _selectedLanguage = DeepgramModelCatalog.DoNotSpecifyLanguageLabel;

    [ObservableProperty]
    private bool _smartFormat;

    [ObservableProperty]
    private string _keyTermsText = string.Empty;

    [ObservableProperty]
    private string _pushToTalkHotkey = "Win+Ctrl+'";

    [ObservableProperty]
    private string _keepTalkingHotkey = "Win+Ctrl+Shift+'";

    [ObservableProperty]
    private string _streamingHotkey = "Win+Ctrl+[";

    [ObservableProperty]
    private string _resendHotkey = "Win+Ctrl+]";

    [ObservableProperty]
    private string _selectedAudioInput = "Default system input";

    [ObservableProperty]
    private int _historyLimit = 20;

    [ObservableProperty]
    private string _selectedOutputMode = "Type directly";

    [ObservableProperty]
    private bool _appendNewline;

    [ObservableProperty]
    private double _micLevelPercent = 28;

    [ObservableProperty]
    private string _micLevelLabel = "Placeholder meter until Rust IPC is wired";

    [ObservableProperty]
    private string _statusMessage = string.Empty;

    [ObservableProperty]
    private InfoBarSeverity _statusSeverity = InfoBarSeverity.Informational;

    public bool HasLoaded { get; private set; }

    public bool HasStatusMessage => !string.IsNullOrWhiteSpace(StatusMessage);

    public ObservableCollection<string> AudioInputOptions { get; } =
    [
        "Default system input"
    ];

    public ObservableCollection<string> OutputModeOptions { get; } =
    [
        "Type directly",
        "Copy to clipboard",
        "Paste clipboard"
    ];

    public ObservableCollection<string> ModelOptions { get; } =
    [
        .. DeepgramModelCatalog.SupportedModels
    ];

    public ObservableCollection<string> LanguageOptions { get; } =
    [
        DeepgramModelCatalog.DoNotSpecifyLanguageLabel
    ];

    public IAsyncRelayCommand ReloadCommand { get; }

    public IAsyncRelayCommand SaveCommand { get; }

    public MainViewModel(ConfigFileService configFileService, string launchPage)
    {
        _configFileService = configFileService;
        _launchPage = launchPage;
        _subtitle = launchPage.Equals("api-key", StringComparison.OrdinalIgnoreCase)
            ? "API key onboarding"
            : "Application settings";

        ReloadCommand = new AsyncRelayCommand(LoadAsync);
        SaveCommand = new AsyncRelayCommand(SaveAsync);
    }

    public async Task InitializeAsync()
    {
        if (HasLoaded)
        {
            return;
        }

        await LoadAsync();
        HasLoaded = true;
    }

    partial void OnStatusMessageChanged(string value)
    {
        OnPropertyChanged(nameof(HasStatusMessage));
    }

    private async Task LoadAsync()
    {
        try
        {
            var config = await _configFileService.LoadAsync();
            Apply(config);
            StatusSeverity = InfoBarSeverity.Informational;
            StatusMessage = $"Loaded {_configFileService.ConfigPath}";
        }
        catch (Exception error)
        {
            StatusSeverity = InfoBarSeverity.Error;
            StatusMessage = error.Message;
        }
    }

    private async Task SaveAsync()
    {
        try
        {
            var config = BuildConfig();
            await _configFileService.SaveAsync(config);
            StatusSeverity = InfoBarSeverity.Success;
            StatusMessage = $"Saved {_configFileService.ConfigPath}";
        }
        catch (Exception error)
        {
            StatusSeverity = InfoBarSeverity.Error;
            StatusMessage = error.Message;
        }
    }

    private void Apply(VelocityConfig config)
    {
        ApiKey = config.ApiKey ?? string.Empty;
        Model = DeepgramModelCatalog.NormalizeModel(config.Model);
        RefreshLanguageOptions(config.Language);
        SmartFormat = config.SmartFormat;
        KeyTermsText = string.Join(Environment.NewLine, config.KeyTerms);
        PushToTalkHotkey = config.Hotkeys.PushToTalk;
        KeepTalkingHotkey = config.Hotkeys.KeepTalking;
        StreamingHotkey = config.Hotkeys.Streaming;
        ResendHotkey = config.Hotkeys.ResendSelected;
        HistoryLimit = config.HistoryLimit;
        AppendNewline = config.AppendNewline;

        SelectedAudioInput = string.IsNullOrWhiteSpace(config.AudioInput)
            ? "Default system input"
            : config.AudioInput;

        EnsureAudioChoice(SelectedAudioInput);
        SelectedOutputMode = config.OutputMode switch
        {
            "clipboard" => "Copy to clipboard",
            "paste" => "Paste clipboard",
            _ => "Type directly"
        };
    }

    private VelocityConfig BuildConfig()
    {
        var outputMode = SelectedOutputMode switch
        {
            "Copy to clipboard" => "clipboard",
            "Paste clipboard" => "paste",
            _ => "direct-input"
        };

        return new VelocityConfig
        {
            ApiKey = string.IsNullOrWhiteSpace(ApiKey) ? null : ApiKey.Trim(),
            Model = DeepgramModelCatalog.NormalizeModel(Model),
            Language = DeepgramModelCatalog.LanguageCodeFromDisplay(Model, SelectedLanguage),
            SmartFormat = SmartFormat,
            KeyTerms = KeyTermsText
                .Split(Environment.NewLine, StringSplitOptions.RemoveEmptyEntries | StringSplitOptions.TrimEntries)
                .ToList(),
            Hotkeys = new HotkeyConfig
            {
                PushToTalk = PushToTalkHotkey.Trim(),
                KeepTalking = KeepTalkingHotkey.Trim(),
                Streaming = StreamingHotkey.Trim(),
                ResendSelected = ResendHotkey.Trim()
            },
            AudioInput = SelectedAudioInput == "Default system input" ? null : SelectedAudioInput,
            HistoryLimit = HistoryLimit,
            OutputMode = outputMode,
            AppendNewline = AppendNewline
        };
    }

    private void EnsureAudioChoice(string selectedValue)
    {
        if (!AudioInputOptions.Contains(selectedValue))
        {
            AudioInputOptions.Add(selectedValue);
        }
    }

    partial void OnModelChanged(string value)
    {
        RefreshLanguageOptions(DeepgramModelCatalog.LanguageCodeFromDisplay(value, SelectedLanguage));
    }

    private void RefreshLanguageOptions(string? selectedLanguageCode)
    {
        var model = DeepgramModelCatalog.NormalizeModel(Model);
        var normalizedLanguage = DeepgramModelCatalog.NormalizeLanguage(model, selectedLanguageCode);

        LanguageOptions.Clear();
        LanguageOptions.Add(DeepgramModelCatalog.DoNotSpecifyLanguageLabel);
        foreach (var option in DeepgramModelCatalog.LanguagesForModel(model))
        {
            LanguageOptions.Add(option.DisplayText);
        }

        SelectedLanguage = normalizedLanguage is null
            ? DeepgramModelCatalog.DoNotSpecifyLanguageLabel
            : DeepgramModelCatalog.LanguagesForModel(model)
                .First(option => string.Equals(option.Code, normalizedLanguage, StringComparison.OrdinalIgnoreCase))
                .DisplayText;
    }
}

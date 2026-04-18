namespace Velocity.Settings.Services;

public sealed record DeepgramLanguageOption(string Code, string Label)
{
    public string DisplayText => $"{Label} ({Code})";
}

public static class DeepgramModelCatalog
{
    public const string DefaultModel = "nova-3";
    public const string DoNotSpecifyLanguageLabel = "Do not specify";

    public static IReadOnlyList<string> SupportedModels { get; } = ["nova-2", "nova-3"];

    private static readonly IReadOnlyList<DeepgramLanguageOption> Nova2Languages =
    [
        new("multi", "Multilingual (English + Spanish)"),
        new("bg", "Bulgarian"),
        new("ca", "Catalan"),
        new("zh", "Chinese (Mandarin, Simplified)"),
        new("zh-CN", "Chinese (Mandarin, Simplified, China)"),
        new("zh-Hans", "Chinese (Mandarin, Simplified Script)"),
        new("zh-TW", "Chinese (Mandarin, Traditional, Taiwan)"),
        new("zh-Hant", "Chinese (Mandarin, Traditional Script)"),
        new("zh-HK", "Chinese (Cantonese, Traditional, Hong Kong)"),
        new("cs", "Czech"),
        new("da", "Danish"),
        new("da-DK", "Danish (Denmark)"),
        new("nl", "Dutch"),
        new("en", "English"),
        new("en-US", "English (United States)"),
        new("en-AU", "English (Australia)"),
        new("en-GB", "English (United Kingdom)"),
        new("en-NZ", "English (New Zealand)"),
        new("en-IN", "English (India)"),
        new("et", "Estonian"),
        new("fi", "Finnish"),
        new("nl-BE", "Flemish"),
        new("fr", "French"),
        new("fr-CA", "French (Canada)"),
        new("de", "German"),
        new("de-CH", "German (Switzerland)"),
        new("el", "Greek"),
        new("hi", "Hindi"),
        new("hu", "Hungarian"),
        new("id", "Indonesian"),
        new("it", "Italian"),
        new("ja", "Japanese"),
        new("ko", "Korean"),
        new("ko-KR", "Korean (South Korea)"),
        new("lv", "Latvian"),
        new("lt", "Lithuanian"),
        new("ms", "Malay"),
        new("no", "Norwegian"),
        new("pl", "Polish"),
        new("pt", "Portuguese"),
        new("pt-BR", "Portuguese (Brazil)"),
        new("pt-PT", "Portuguese (Portugal)"),
        new("ro", "Romanian"),
        new("ru", "Russian"),
        new("sk", "Slovak"),
        new("es", "Spanish"),
        new("es-419", "Spanish (Latin America)"),
        new("sv", "Swedish"),
        new("sv-SE", "Swedish (Sweden)"),
        new("th", "Thai"),
        new("th-TH", "Thai (Thailand)"),
        new("tr", "Turkish"),
        new("uk", "Ukrainian"),
        new("vi", "Vietnamese"),
    ];

    private static readonly IReadOnlyList<DeepgramLanguageOption> Nova3Languages =
    [
        new("multi", "Multilingual"),
        new("ar", "Arabic"),
        new("ar-AE", "Arabic (United Arab Emirates)"),
        new("ar-SA", "Arabic (Saudi Arabia)"),
        new("ar-QA", "Arabic (Qatar)"),
        new("ar-KW", "Arabic (Kuwait)"),
        new("ar-SY", "Arabic (Syria)"),
        new("ar-LB", "Arabic (Lebanon)"),
        new("ar-PS", "Arabic (Palestine)"),
        new("ar-JO", "Arabic (Jordan)"),
        new("ar-EG", "Arabic (Egypt)"),
        new("ar-SD", "Arabic (Sudan)"),
        new("ar-TD", "Arabic (Chad)"),
        new("ar-MA", "Arabic (Morocco)"),
        new("ar-DZ", "Arabic (Algeria)"),
        new("ar-TN", "Arabic (Tunisia)"),
        new("ar-IQ", "Arabic (Iraq)"),
        new("ar-IR", "Arabic (Iran)"),
        new("be", "Belarusian"),
        new("bn", "Bengali"),
        new("bs", "Bosnian"),
        new("bg", "Bulgarian"),
        new("ca", "Catalan"),
        new("hr", "Croatian"),
        new("cs", "Czech"),
        new("da", "Danish"),
        new("da-DK", "Danish (Denmark)"),
        new("nl", "Dutch"),
        new("en", "English"),
        new("en-US", "English (United States)"),
        new("en-AU", "English (Australia)"),
        new("en-GB", "English (United Kingdom)"),
        new("en-IN", "English (India)"),
        new("en-NZ", "English (New Zealand)"),
        new("et", "Estonian"),
        new("fi", "Finnish"),
        new("nl-BE", "Flemish"),
        new("fr", "French"),
        new("fr-CA", "French (Canada)"),
        new("de", "German"),
        new("de-CH", "German (Switzerland)"),
        new("el", "Greek"),
        new("he", "Hebrew"),
        new("hi", "Hindi"),
        new("hu", "Hungarian"),
        new("id", "Indonesian"),
        new("it", "Italian"),
        new("ja", "Japanese"),
        new("kn", "Kannada"),
        new("ko", "Korean"),
        new("ko-KR", "Korean (South Korea)"),
        new("lv", "Latvian"),
        new("lt", "Lithuanian"),
        new("mk", "Macedonian"),
        new("ms", "Malay"),
        new("mr", "Marathi"),
        new("no", "Norwegian"),
        new("fa", "Persian"),
        new("pl", "Polish"),
        new("pt", "Portuguese"),
        new("pt-BR", "Portuguese (Brazil)"),
        new("pt-PT", "Portuguese (Portugal)"),
        new("ro", "Romanian"),
        new("ru", "Russian"),
        new("sr", "Serbian"),
        new("sk", "Slovak"),
        new("sl", "Slovenian"),
        new("es", "Spanish"),
        new("es-419", "Spanish (Latin America)"),
        new("sv", "Swedish"),
        new("sv-SE", "Swedish (Sweden)"),
        new("tl", "Tagalog"),
        new("ta", "Tamil"),
        new("te", "Telugu"),
        new("tr", "Turkish"),
        new("uk", "Ukrainian"),
        new("ur", "Urdu"),
        new("vi", "Vietnamese"),
    ];

    public static string NormalizeModel(string? rawModel) =>
        SupportedModels.FirstOrDefault(model => string.Equals(model, rawModel?.Trim(), StringComparison.OrdinalIgnoreCase))
        ?? DefaultModel;

    public static IReadOnlyList<DeepgramLanguageOption> LanguagesForModel(string? rawModel) =>
        NormalizeModel(rawModel) == "nova-2" ? Nova2Languages : Nova3Languages;

    public static string? NormalizeLanguage(string? rawModel, string? rawLanguage)
    {
        var trimmed = rawLanguage?.Trim();
        if (string.IsNullOrWhiteSpace(trimmed))
        {
            return null;
        }

        return LanguagesForModel(rawModel)
            .FirstOrDefault(option => string.Equals(option.Code, trimmed, StringComparison.OrdinalIgnoreCase))
            ?.Code;
    }

    public static string? LanguageCodeFromDisplay(string? rawModel, string? displayText)
    {
        var trimmed = displayText?.Trim();
        if (string.IsNullOrWhiteSpace(trimmed) || string.Equals(trimmed, DoNotSpecifyLanguageLabel, StringComparison.OrdinalIgnoreCase))
        {
            return null;
        }

        var lastOpenParen = trimmed.LastIndexOf('(');
        if (lastOpenParen >= 0 && trimmed.EndsWith(')'))
        {
            var code = trimmed[(lastOpenParen + 1)..^1];
            return NormalizeLanguage(rawModel, code);
        }

        return NormalizeLanguage(rawModel, trimmed);
    }
}

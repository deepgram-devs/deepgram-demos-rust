using Velocity.Settings.Models;
using YamlDotNet.Serialization;
using YamlDotNet.Serialization.NamingConventions;

namespace Velocity.Settings.Services;

public sealed class ConfigFileService
{
    private const string ConfigSubdirectory = "deepgram";
    private readonly IDeserializer _deserializer;
    private readonly ISerializer _serializer;

    public ConfigFileService()
    {
        _deserializer = new DeserializerBuilder()
            .WithNamingConvention(UnderscoredNamingConvention.Instance)
            .IgnoreUnmatchedProperties()
            .Build();

        _serializer = new SerializerBuilder()
            .WithNamingConvention(UnderscoredNamingConvention.Instance)
            .ConfigureDefaultValuesHandling(DefaultValuesHandling.OmitNull)
            .Build();
    }

    public string ConfigPath => Path.Combine(ConfigDirectory, "velocity.yml");

    public string BackupPath => Path.Combine(ConfigDirectory, "velocity.backup.yml");

    private static string ConfigDirectory =>
        Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.UserProfile), ".config", ConfigSubdirectory);

    public async Task<VelocityConfig> LoadAsync()
    {
        if (!File.Exists(ConfigPath))
        {
            return new VelocityConfig();
        }

        await using var stream = File.OpenRead(ConfigPath);
        using var reader = new StreamReader(stream);
        var yaml = NormalizeLegacyYaml(await reader.ReadToEndAsync());
        return _deserializer.Deserialize<VelocityConfig>(yaml) ?? new VelocityConfig();
    }

    public async Task SaveAsync(VelocityConfig config)
    {
        Directory.CreateDirectory(ConfigDirectory);

        var normalized = Normalize(config);
        var yaml = _serializer.Serialize(normalized);

        await File.WriteAllTextAsync(ConfigPath, yaml);
        await File.WriteAllTextAsync(BackupPath, yaml);
    }

    public DateTimeOffset? GetConfigWriteTimeUtc()
    {
        if (!File.Exists(ConfigPath))
        {
            return null;
        }

        return File.GetLastWriteTimeUtc(ConfigPath);
    }

    private static string NormalizeLegacyYaml(string yaml) =>
        yaml.Replace(Environment.NewLine + "key_terms:", Environment.NewLine + "keyterms:")
            .Replace("key_terms:" + Environment.NewLine, "keyterms:" + Environment.NewLine);

    private static VelocityConfig Normalize(VelocityConfig config)
    {
        config.Model = string.IsNullOrWhiteSpace(config.Model) ? "nova-3" : config.Model.Trim();
        config.ApiKey = string.IsNullOrWhiteSpace(config.ApiKey) ? null : config.ApiKey.Trim();
        config.AudioInput = string.IsNullOrWhiteSpace(config.AudioInput) ? null : config.AudioInput.Trim();
        config.HistoryLimit = config.HistoryLimit <= 0 ? 20 : config.HistoryLimit;
        config.KeyTerms = config.KeyTerms
            .Select(term => term.Trim())
            .Where(term => !string.IsNullOrWhiteSpace(term))
            .Distinct(StringComparer.OrdinalIgnoreCase)
            .ToList();

        return config;
    }
}

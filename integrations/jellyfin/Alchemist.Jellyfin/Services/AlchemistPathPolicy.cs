using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using dev.bybrooklyn.alchemist.Configuration;

namespace dev.bybrooklyn.alchemist.Services;

/// <summary>
/// File filtering and path translation rules for Jellyfin library events.
/// </summary>
public static class AlchemistPathPolicy
{
    /// <summary>
    /// Attempts to turn a Jellyfin item path into an Alchemist-visible media path.
    /// </summary>
    /// <param name="itemPath">Path reported by Jellyfin.</param>
    /// <param name="configuration">Plugin configuration.</param>
    /// <param name="translatedPath">Translated path when accepted.</param>
    /// <returns>True when the path is eligible.</returns>
    public static bool TryGetCandidatePath(string? itemPath, PluginConfiguration configuration, out string translatedPath)
    {
        translatedPath = string.Empty;
        if (string.IsNullOrWhiteSpace(itemPath))
        {
            return false;
        }

        var extension = Path.GetExtension(itemPath);
        if (string.IsNullOrWhiteSpace(extension) || !AllowedExtensions(configuration).Contains(extension, StringComparer.OrdinalIgnoreCase))
        {
            return false;
        }

        translatedPath = ApplyTranslations(itemPath, configuration.PathTranslations);
        return !string.IsNullOrWhiteSpace(translatedPath);
    }

    /// <summary>
    /// Attempts to turn an Alchemist job path back into a Jellyfin-visible directory path.
    /// </summary>
    /// <param name="inputPath">Alchemist job input path.</param>
    /// <param name="outputPath">Alchemist job output path.</param>
    /// <param name="configuration">Plugin configuration.</param>
    /// <param name="refreshPath">Jellyfin-visible containing directory path.</param>
    /// <returns>True when a refresh path could be produced.</returns>
    public static bool TryGetRefreshPath(
        string? inputPath,
        string? outputPath,
        PluginConfiguration configuration,
        out string refreshPath)
    {
        refreshPath = string.Empty;
        var sourcePath = !string.IsNullOrWhiteSpace(outputPath) ? outputPath : inputPath;
        if (string.IsNullOrWhiteSpace(sourcePath))
        {
            return false;
        }

        var translatedPath = ApplyTranslations(sourcePath, ReverseTranslations(configuration));
        var containingDirectory = GetContainingDirectory(translatedPath);
        if (string.IsNullOrWhiteSpace(containingDirectory))
        {
            return false;
        }

        refreshPath = containingDirectory;
        return true;
    }

    /// <summary>
    /// Gets the containing directory for a path, falling back to the original path when no directory can be parsed.
    /// </summary>
    /// <param name="path">Source path.</param>
    /// <returns>Containing directory or the source path.</returns>
    public static string GetContainingDirectory(string path)
    {
        if (string.IsNullOrWhiteSpace(path))
        {
            return string.Empty;
        }

        var normalizedPath = path.Trim();
        var directory = Path.GetDirectoryName(normalizedPath);
        return string.IsNullOrWhiteSpace(directory) ? normalizedPath : directory;
    }

    private static HashSet<string> AllowedExtensions(PluginConfiguration configuration)
    {
        return configuration.AllowedExtensions
            .Split(',', StringSplitOptions.RemoveEmptyEntries | StringSplitOptions.TrimEntries)
            .Select(extension => extension.StartsWith(".", StringComparison.Ordinal) ? extension : "." + extension)
            .ToHashSet(StringComparer.OrdinalIgnoreCase);
    }

    /// <summary>
    /// Applies the longest matching path translation to a path.
    /// </summary>
    /// <param name="path">Source path.</param>
    /// <param name="translations">Newline-separated translation rules.</param>
    /// <returns>Translated path, or the original path when no rule matches.</returns>
    public static string ApplyTranslations(string path, string translations)
    {
        var bestSource = string.Empty;
        var bestTarget = string.Empty;

        foreach (var rule in ParseTranslations(translations))
        {
            if (PathMatchesRule(path, rule.From) && rule.From.Length > bestSource.Length)
            {
                bestSource = rule.From.TrimEnd('/', '\\');
                bestTarget = rule.To.TrimEnd('/', '\\');
            }
        }

        if (bestSource.Length == 0)
        {
            return path;
        }

        return bestTarget + path[bestSource.Length..];
    }

    private static bool PathMatchesRule(string path, string ruleFrom)
    {
        var source = ruleFrom.TrimEnd('/', '\\');
        if (source.Length == 0 || !path.StartsWith(source, StringComparison.OrdinalIgnoreCase))
        {
            return false;
        }

        return path.Length == source.Length || path[source.Length] is '/' or '\\';
    }

    private static string ReverseTranslations(PluginConfiguration configuration)
    {
        if (!string.IsNullOrWhiteSpace(configuration.ReversePathTranslations))
        {
            return configuration.ReversePathTranslations;
        }

        return string.Join(
            Environment.NewLine,
            ParseTranslations(configuration.PathTranslations).Select(rule => $"{rule.To}={rule.From}"));
    }

    private static IEnumerable<PathTranslation> ParseTranslations(string translations)
    {
        foreach (var line in translations.Split('\n', StringSplitOptions.RemoveEmptyEntries | StringSplitOptions.TrimEntries))
        {
            var separator = line.Contains("=>", StringComparison.Ordinal) ? "=>" : "=";
            var parts = line.Split(separator, 2, StringSplitOptions.TrimEntries);
            if (parts.Length != 2 || parts[0].Length == 0 || parts[1].Length == 0)
            {
                continue;
            }

            yield return new PathTranslation(parts[0], parts[1]);
        }
    }

    private readonly record struct PathTranslation(string From, string To);
}

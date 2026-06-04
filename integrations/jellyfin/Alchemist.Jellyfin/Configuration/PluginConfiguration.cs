using MediaBrowser.Model.Plugins;

namespace dev.bybrooklyn.alchemist.Configuration;

/// <summary>
/// Alchemist plugin configuration persisted by Jellyfin.
/// </summary>
public sealed class PluginConfiguration : BasePluginConfiguration
{
    /// <summary>
    /// Gets or sets the Alchemist server base URL.
    /// </summary>
    public string AlchemistUrl { get; set; } = "http://localhost:3000";

    /// <summary>
    /// Gets or sets an Alchemist Jellyfin-scoped API token.
    /// </summary>
    public string ApiToken { get; set; } = string.Empty;

    /// <summary>
    /// Gets or sets a value indicating whether Jellyfin library events should enqueue media.
    /// </summary>
    public bool AutoEnqueueEnabled { get; set; }

    /// <summary>
    /// Gets or sets a value indicating whether the plugin should listen for Alchemist job events.
    /// </summary>
    public bool EventListenerEnabled { get; set; }

    /// <summary>
    /// Gets or sets a value indicating whether completed Alchemist jobs should refresh Jellyfin paths.
    /// </summary>
    public bool RefreshOnCompletionEnabled { get; set; }

    /// <summary>
    /// Gets or sets a value indicating whether events should be logged without contacting Alchemist.
    /// </summary>
    public bool DryRun { get; set; } = true;

    /// <summary>
    /// Gets or sets the duplicate event suppression window in seconds.
    /// </summary>
    public int DedupeWindowSeconds { get; set; } = 300;

    /// <summary>
    /// Gets or sets a comma-separated list of file extensions to forward.
    /// </summary>
    public string AllowedExtensions { get; set; } = ".mkv,.mp4,.mov,.avi,.ts,.m2ts,.webm";

    /// <summary>
    /// Gets or sets newline-separated path translations in the form /jellyfin/path=/alchemist/path.
    /// </summary>
    public string PathTranslations { get; set; } = string.Empty;

    /// <summary>
    /// Gets or sets newline-separated path translations in the form /alchemist/path=/jellyfin/path.
    /// </summary>
    public string ReversePathTranslations { get; set; } = string.Empty;
}

using System;

namespace dev.bybrooklyn.alchemist.Services;

/// <summary>
/// Stores runtime status shown on the Jellyfin plugin configuration page.
/// </summary>
public sealed class AlchemistPluginRuntimeState
{
    private readonly object _lock = new();
    private string _eventListenerState = "Stopped";
    private string _eventListenerMessage = "Event listener has not started.";
    private string _lastRefreshResult = "No refresh has run.";
    private DateTimeOffset? _lastRefreshUtc;

    /// <summary>
    /// Updates the event listener status.
    /// </summary>
    /// <param name="state">State label.</param>
    /// <param name="message">Status details.</param>
    public void SetEventListenerState(string state, string message)
    {
        lock (_lock)
        {
            _eventListenerState = state;
            _eventListenerMessage = message;
        }
    }

    /// <summary>
    /// Updates the last refresh result.
    /// </summary>
    /// <param name="message">Refresh result details.</param>
    public void SetRefreshResult(string message)
    {
        lock (_lock)
        {
            _lastRefreshResult = message;
            _lastRefreshUtc = DateTimeOffset.UtcNow;
        }
    }

    /// <summary>
    /// Returns a consistent runtime status snapshot.
    /// </summary>
    /// <returns>Runtime status snapshot.</returns>
    public AlchemistPluginRuntimeStatus Snapshot()
    {
        lock (_lock)
        {
            return new AlchemistPluginRuntimeStatus
            {
                EventListenerState = _eventListenerState,
                EventListenerMessage = _eventListenerMessage,
                LastRefreshResult = _lastRefreshResult,
                LastRefreshUtc = _lastRefreshUtc
            };
        }
    }
}

/// <summary>
/// Runtime status returned to the Jellyfin dashboard.
/// </summary>
public sealed class AlchemistPluginRuntimeStatus
{
    /// <summary>
    /// Gets or sets the event listener state label.
    /// </summary>
    public string EventListenerState { get; set; } = string.Empty;

    /// <summary>
    /// Gets or sets the event listener status details.
    /// </summary>
    public string EventListenerMessage { get; set; } = string.Empty;

    /// <summary>
    /// Gets or sets the latest refresh result.
    /// </summary>
    public string LastRefreshResult { get; set; } = string.Empty;

    /// <summary>
    /// Gets or sets when the latest refresh result was recorded.
    /// </summary>
    public DateTimeOffset? LastRefreshUtc { get; set; }
}

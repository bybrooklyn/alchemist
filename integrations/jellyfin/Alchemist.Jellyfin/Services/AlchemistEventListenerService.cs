using System;
using System.Collections.Concurrent;
using System.IO;
using System.Text;
using System.Threading;
using System.Threading.Tasks;
using dev.bybrooklyn.alchemist.Configuration;
using Microsoft.Extensions.Hosting;
using Microsoft.Extensions.Logging;

namespace dev.bybrooklyn.alchemist.Services;

/// <summary>
/// Listens to Alchemist job events and refreshes Jellyfin when jobs complete.
/// </summary>
public sealed class AlchemistEventListenerService : BackgroundService
{
    private static readonly TimeSpan DisabledPollInterval = TimeSpan.FromSeconds(30);
    private static readonly TimeSpan InitialBackoff = TimeSpan.FromSeconds(5);
    private static readonly TimeSpan MaxBackoff = TimeSpan.FromMinutes(5);
    private static readonly TimeSpan CompletedJobDedupWindow = TimeSpan.FromHours(24);

    private readonly AlchemistClient _client;
    private readonly AlchemistRefreshCoordinator _refreshCoordinator;
    private readonly AlchemistPluginRuntimeState _runtimeState;
    private readonly ILogger<AlchemistEventListenerService> _logger;
    private readonly ConcurrentDictionary<long, DateTimeOffset> _completedJobs = new();

    /// <summary>
    /// Initializes a new instance of the <see cref="AlchemistEventListenerService"/> class.
    /// </summary>
    /// <param name="client">Alchemist HTTP client.</param>
    /// <param name="refreshCoordinator">Jellyfin refresh coordinator.</param>
    /// <param name="runtimeState">Shared plugin runtime state.</param>
    /// <param name="logger">Logger.</param>
    public AlchemistEventListenerService(
        AlchemistClient client,
        AlchemistRefreshCoordinator refreshCoordinator,
        AlchemistPluginRuntimeState runtimeState,
        ILogger<AlchemistEventListenerService> logger)
    {
        _client = client;
        _refreshCoordinator = refreshCoordinator;
        _runtimeState = runtimeState;
        _logger = logger;
    }

    /// <inheritdoc />
    protected override async Task ExecuteAsync(CancellationToken stoppingToken)
    {
        var backoff = InitialBackoff;

        while (!stoppingToken.IsCancellationRequested)
        {
            try
            {
                var configuration = Plugin.Instance?.Configuration;
                if (!ShouldListen(configuration))
                {
                    await Task.Delay(DisabledPollInterval, stoppingToken).ConfigureAwait(false);
                    continue;
                }

                await RunEventStreamAsync(configuration!, stoppingToken).ConfigureAwait(false);
                _runtimeState.SetEventListenerState("Disconnected", "Alchemist event stream ended.");
                await Task.Delay(backoff, stoppingToken).ConfigureAwait(false);
                backoff = NextBackoff(backoff);
            }
            catch (OperationCanceledException) when (stoppingToken.IsCancellationRequested)
            {
                break;
            }
            catch (Exception ex)
            {
                _logger.LogWarning(ex, "Alchemist event listener failed; reconnecting in {Delay}", backoff);
                _runtimeState.SetEventListenerState("Reconnecting", $"Event stream failed: {ex.Message}");
                await Task.Delay(backoff, stoppingToken).ConfigureAwait(false);
                backoff = NextBackoff(backoff);
            }
        }

        _runtimeState.SetEventListenerState("Stopped", "Event listener stopped.");
    }

    private bool ShouldListen(PluginConfiguration? configuration)
    {
        if (configuration is null)
        {
            _runtimeState.SetEventListenerState("Waiting", "Plugin configuration is not loaded.");
            return false;
        }

        if (!configuration.EventListenerEnabled)
        {
            _runtimeState.SetEventListenerState("Disabled", "Event listener is disabled.");
            return false;
        }

        if (!configuration.RefreshOnCompletionEnabled)
        {
            _runtimeState.SetEventListenerState("Disabled", "Refresh on completion is disabled.");
            return false;
        }

        if (string.IsNullOrWhiteSpace(configuration.AlchemistUrl) || string.IsNullOrWhiteSpace(configuration.ApiToken))
        {
            _runtimeState.SetEventListenerState("Waiting", "Alchemist URL or API token is not configured.");
            return false;
        }

        return true;
    }

    private async Task RunEventStreamAsync(
        PluginConfiguration configuration,
        CancellationToken stoppingToken)
    {
        using var eventStream = await _client.OpenEventStreamAsync(configuration, stoppingToken).ConfigureAwait(false);
        using var reader = new StreamReader(eventStream.Stream, Encoding.UTF8);
        _runtimeState.SetEventListenerState("Connected", "Listening to Alchemist job events.");

        string? eventName = null;
        var dataBuilder = new StringBuilder();

        while (!stoppingToken.IsCancellationRequested)
        {
            var line = await reader.ReadLineAsync(stoppingToken).ConfigureAwait(false);
            if (line is null)
            {
                return;
            }

            if (line.Length == 0)
            {
                await DispatchEventAsync(
                        configuration,
                        eventName,
                        dataBuilder.ToString().TrimEnd('\r', '\n'),
                        stoppingToken)
                    .ConfigureAwait(false);
                eventName = null;
                dataBuilder.Clear();
                continue;
            }

            if (line.StartsWith("event:", StringComparison.Ordinal))
            {
                eventName = line["event:".Length..].Trim();
                continue;
            }

            if (line.StartsWith("data:", StringComparison.Ordinal))
            {
                dataBuilder.AppendLine(line["data:".Length..].TrimStart());
            }
        }
    }

    private async Task DispatchEventAsync(
        PluginConfiguration configuration,
        string? eventName,
        string data,
        CancellationToken stoppingToken)
    {
        if (!AlchemistEventParser.TryParseCompletedJobId(eventName, data, out var jobId)
            || !TryMarkCompletedJob(jobId))
        {
            return;
        }

        var details = await _client.GetJobDetailsAsync(configuration, jobId, stoppingToken).ConfigureAwait(false);
        if (details is null)
        {
            _runtimeState.SetRefreshResult($"Could not load details for completed Alchemist job {jobId}.");
            return;
        }

        _refreshCoordinator.RefreshForJob(configuration, details);
    }

    private bool TryMarkCompletedJob(long jobId)
    {
        var now = DateTimeOffset.UtcNow;
        foreach (var entry in _completedJobs)
        {
            if (now - entry.Value > CompletedJobDedupWindow)
            {
                _completedJobs.TryRemove(entry.Key, out _);
            }
        }

        return _completedJobs.TryAdd(jobId, now);
    }

    private static TimeSpan NextBackoff(TimeSpan current)
    {
        var nextTicks = Math.Min(current.Ticks * 2, MaxBackoff.Ticks);
        return TimeSpan.FromTicks(nextTicks);
    }
}

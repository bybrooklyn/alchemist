using System;
using dev.bybrooklyn.alchemist.Configuration;
using MediaBrowser.Controller.Library;
using Microsoft.Extensions.Logging;

namespace dev.bybrooklyn.alchemist.Services;

/// <summary>
/// Refreshes Jellyfin paths after Alchemist completes a job.
/// </summary>
public sealed class AlchemistRefreshCoordinator
{
    private readonly ILibraryMonitor _libraryMonitor;
    private readonly AlchemistPluginRuntimeState _runtimeState;
    private readonly ILogger<AlchemistRefreshCoordinator> _logger;

    /// <summary>
    /// Initializes a new instance of the <see cref="AlchemistRefreshCoordinator"/> class.
    /// </summary>
    /// <param name="libraryMonitor">Jellyfin library monitor.</param>
    /// <param name="runtimeState">Shared plugin runtime state.</param>
    /// <param name="logger">Logger.</param>
    public AlchemistRefreshCoordinator(
        ILibraryMonitor libraryMonitor,
        AlchemistPluginRuntimeState runtimeState,
        ILogger<AlchemistRefreshCoordinator> logger)
    {
        _libraryMonitor = libraryMonitor;
        _runtimeState = runtimeState;
        _logger = logger;
    }

    /// <summary>
    /// Refreshes the Jellyfin-visible containing directory for a completed Alchemist job.
    /// </summary>
    /// <param name="configuration">Plugin configuration.</param>
    /// <param name="details">Alchemist job details.</param>
    /// <returns>Refresh result.</returns>
    public AlchemistConnectionResult RefreshForJob(
        PluginConfiguration configuration,
        AlchemistJobDetailResponse details)
    {
        if (!configuration.RefreshOnCompletionEnabled)
        {
            return RecordResult("Refresh on completion is disabled.");
        }

        if (!AlchemistPathPolicy.TryGetRefreshPath(
                details.Job.InputPath,
                details.Job.OutputPath,
                configuration,
                out var refreshPath))
        {
            return RecordResult($"No refresh path found for completed Alchemist job {details.Job.Id}.");
        }

        try
        {
            _libraryMonitor.ReportFileSystemChanged(refreshPath);
            _logger.LogInformation(
                "Requested Jellyfin refresh for Alchemist job {JobId} at {Path}",
                details.Job.Id,
                refreshPath);
            return RecordResult($"Requested Jellyfin refresh for {refreshPath} from job {details.Job.Id}.");
        }
        catch (Exception ex)
        {
            _logger.LogWarning(ex, "Failed to request Jellyfin refresh for Alchemist job {JobId}", details.Job.Id);
            return RecordResult($"Failed to refresh Jellyfin path for job {details.Job.Id}: {ex.Message}", false);
        }
    }

    private AlchemistConnectionResult RecordResult(string message, bool success = true)
    {
        _runtimeState.SetRefreshResult(message);
        return success ? AlchemistConnectionResult.Ok(message) : AlchemistConnectionResult.Fail(message);
    }
}

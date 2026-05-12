using System;
using System.Collections.Concurrent;
using System.Threading;
using System.Threading.Tasks;
using Alchemist.Jellyfin.Configuration;
using MediaBrowser.Controller.Entities;
using MediaBrowser.Controller.Library;
using Microsoft.Extensions.Hosting;
using Microsoft.Extensions.Logging;

namespace Alchemist.Jellyfin.Services;

/// <summary>
/// Subscribes to Jellyfin library events and forwards eligible media paths to Alchemist.
/// </summary>
public sealed class AlchemistLibraryHookService : IHostedService, IDisposable
{
    private readonly ILibraryManager _libraryManager;
    private readonly AlchemistClient _client;
    private readonly ILogger<AlchemistLibraryHookService> _logger;
    private readonly ConcurrentDictionary<string, DateTimeOffset> _recentSubmissions = new(StringComparer.OrdinalIgnoreCase);
    private bool _started;

    /// <summary>
    /// Initializes a new instance of the <see cref="AlchemistLibraryHookService"/> class.
    /// </summary>
    /// <param name="libraryManager">Jellyfin library manager.</param>
    /// <param name="client">Alchemist client.</param>
    /// <param name="logger">Logger.</param>
    public AlchemistLibraryHookService(
        ILibraryManager libraryManager,
        AlchemistClient client,
        ILogger<AlchemistLibraryHookService> logger)
    {
        _libraryManager = libraryManager;
        _client = client;
        _logger = logger;
    }

    /// <inheritdoc />
    public Task StartAsync(CancellationToken cancellationToken)
    {
        if (_started)
        {
            return Task.CompletedTask;
        }

        _libraryManager.ItemAdded += OnItemAdded;
        _libraryManager.ItemUpdated += OnItemUpdated;
        _started = true;
        _logger.LogInformation("Alchemist Jellyfin library hook started.");
        return Task.CompletedTask;
    }

    /// <inheritdoc />
    public Task StopAsync(CancellationToken cancellationToken)
    {
        if (!_started)
        {
            return Task.CompletedTask;
        }

        _libraryManager.ItemAdded -= OnItemAdded;
        _libraryManager.ItemUpdated -= OnItemUpdated;
        _started = false;
        _logger.LogInformation("Alchemist Jellyfin library hook stopped.");
        return Task.CompletedTask;
    }

    /// <inheritdoc />
    public void Dispose()
    {
        _libraryManager.ItemAdded -= OnItemAdded;
        _libraryManager.ItemUpdated -= OnItemUpdated;
    }

    private void OnItemAdded(object? sender, ItemChangeEventArgs args)
    {
        QueueItem(args.Item, "added");
    }

    private void OnItemUpdated(object? sender, ItemChangeEventArgs args)
    {
        QueueItem(args.Item, "updated");
    }

    private void QueueItem(BaseItem item, string eventName)
    {
        var configuration = Plugin.Instance?.Configuration;
        if (configuration is null || !configuration.AutoEnqueueEnabled)
        {
            return;
        }

        if (!AlchemistPathPolicy.TryGetCandidatePath(item.Path, configuration, out var translatedPath))
        {
            return;
        }

        if (IsDuplicate(translatedPath, configuration.DedupeWindowSeconds))
        {
            _logger.LogDebug("Skipping duplicate Alchemist enqueue event for {Path}", translatedPath);
            return;
        }

        _ = Task.Run(
            async () =>
            {
                var result = await _client.EnqueueAsync(configuration, translatedPath, CancellationToken.None).ConfigureAwait(false);
                if (result.Success)
                {
                    _logger.LogInformation("Alchemist {EventName} hook accepted {Path}: {Message}", eventName, translatedPath, result.Message);
                }
                else
                {
                    _logger.LogWarning("Alchemist {EventName} hook failed for {Path}: {Message}", eventName, translatedPath, result.Message);
                }
            });
    }

    private bool IsDuplicate(string path, int windowSeconds)
    {
        var window = TimeSpan.FromSeconds(Math.Clamp(windowSeconds, 1, 86_400));
        var now = DateTimeOffset.UtcNow;
        PruneRecent(now, window);

        if (_recentSubmissions.TryGetValue(path, out var previous) && now - previous < window)
        {
            return true;
        }

        _recentSubmissions[path] = now;
        return false;
    }

    private void PruneRecent(DateTimeOffset now, TimeSpan window)
    {
        var maxAge = TimeSpan.FromTicks(window.Ticks * 4);
        foreach (var entry in _recentSubmissions)
        {
            if (now - entry.Value > maxAge)
            {
                _recentSubmissions.TryRemove(entry.Key, out _);
            }
        }
    }
}

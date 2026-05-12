using Alchemist.Jellyfin.Services;
using MediaBrowser.Controller;
using MediaBrowser.Controller.Plugins;
using Microsoft.Extensions.DependencyInjection;

namespace Alchemist.Jellyfin;

/// <summary>
/// Registers Alchemist plugin services with Jellyfin.
/// </summary>
public sealed class AlchemistPluginServiceRegistrator : IPluginServiceRegistrator
{
    /// <inheritdoc />
    public void RegisterServices(IServiceCollection serviceCollection, IServerApplicationHost applicationHost)
    {
        serviceCollection.AddSingleton<AlchemistClient>();
        serviceCollection.AddSingleton<AlchemistPluginRuntimeState>();
        serviceCollection.AddSingleton<AlchemistRefreshCoordinator>();
        serviceCollection.AddHostedService<AlchemistLibraryHookService>();
        serviceCollection.AddHostedService<AlchemistEventListenerService>();
    }
}

using Alchemist.Jellyfin.Configuration;
using Alchemist.Jellyfin.Services;
using Xunit;

namespace Alchemist.Jellyfin.Tests;

public sealed class AlchemistPathPolicyTests
{
    [Fact]
    public void TryGetCandidatePath_UsesLongestForwardTranslation()
    {
        var configuration = new PluginConfiguration
        {
            PathTranslations = "/jellyfin=/alchemist\n/jellyfin/movies=/alchemist-movies"
        };

        var accepted = AlchemistPathPolicy.TryGetCandidatePath(
            "/jellyfin/movies/Movie.mkv",
            configuration,
            out var translatedPath);

        Assert.True(accepted);
        Assert.Equal("/alchemist-movies/Movie.mkv", translatedPath);
    }

    [Fact]
    public void TryGetCandidatePath_DoesNotMatchSiblingPrefix()
    {
        var configuration = new PluginConfiguration
        {
            PathTranslations = "/jellyfin/media=/alchemist/media"
        };

        var accepted = AlchemistPathPolicy.TryGetCandidatePath(
            "/jellyfin/media-other/Movie.mkv",
            configuration,
            out var translatedPath);

        Assert.True(accepted);
        Assert.Equal("/jellyfin/media-other/Movie.mkv", translatedPath);
    }

    [Fact]
    public void TryGetRefreshPath_UsesExplicitReverseTranslation()
    {
        var configuration = new PluginConfiguration
        {
            PathTranslations = "/jellyfin=/alchemist",
            ReversePathTranslations = "/alchemist-output=/jellyfin-output"
        };

        var accepted = AlchemistPathPolicy.TryGetRefreshPath(
            "/alchemist/input/Movie.mkv",
            "/alchemist-output/Movie/Movie.mkv",
            configuration,
            out var refreshPath);

        Assert.True(accepted);
        Assert.Equal("/jellyfin-output/Movie", refreshPath);
    }

    [Fact]
    public void TryGetRefreshPath_InvertsForwardTranslationWhenReverseIsEmpty()
    {
        var configuration = new PluginConfiguration
        {
            PathTranslations = "/jellyfin/media=/alchemist/media"
        };

        var accepted = AlchemistPathPolicy.TryGetRefreshPath(
            "/alchemist/media/Show/Episode.mkv",
            null,
            configuration,
            out var refreshPath);

        Assert.True(accepted);
        Assert.Equal("/jellyfin/media/Show", refreshPath);
    }
}

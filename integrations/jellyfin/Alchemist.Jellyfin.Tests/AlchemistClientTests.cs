using System.Net;
using System.Net.Http;
using System.Text;
using System.Threading;
using System.Threading.Tasks;
using dev.bybrooklyn.alchemist.Configuration;
using dev.bybrooklyn.alchemist.Services;
using Microsoft.Extensions.Logging.Abstractions;
using Xunit;

namespace dev.bybrooklyn.alchemist.tests;

public sealed class AlchemistClientTests
{
    [Fact]
    public async Task TestConnectionAsync_UsesScopedTokenAndSystemInfo()
    {
        using var handler = new RecordingHandler(HttpStatusCode.OK, """{"version":"0.3.4"}""");
        using var httpClient = new HttpClient(handler);
        using var client = new AlchemistClient(httpClient, NullLogger<AlchemistClient>.Instance);
        var configuration = Configuration(dryRun: false);

        var result = await client.TestConnectionAsync(configuration, CancellationToken.None);

        Assert.True(result.Success);
        Assert.Equal(HttpMethod.Get, handler.Method);
        Assert.Equal("/api/v1/system/info", handler.Path);
        Assert.Equal("Bearer test-token", handler.Authorization);
    }

    [Fact]
    public async Task EnqueueAsync_PostsTranslatedPathWhenDryRunIsDisabled()
    {
        using var handler = new RecordingHandler(HttpStatusCode.OK, """{"status":"queued"}""");
        using var httpClient = new HttpClient(handler);
        using var client = new AlchemistClient(httpClient, NullLogger<AlchemistClient>.Instance);
        var configuration = Configuration(dryRun: false);

        var result = await client.EnqueueAsync(
            configuration,
            "/alchemist/media/Movie.mkv",
            CancellationToken.None);

        Assert.True(result.Success);
        Assert.Equal(HttpMethod.Post, handler.Method);
        Assert.Equal("/api/v1/jobs/enqueue", handler.Path);
        Assert.Contains("/alchemist/media/Movie.mkv", handler.Body);
    }

    [Fact]
    public async Task EnqueueAsync_DryRunDoesNotContactAlchemist()
    {
        using var handler = new RecordingHandler(HttpStatusCode.InternalServerError, "unexpected");
        using var httpClient = new HttpClient(handler);
        using var client = new AlchemistClient(httpClient, NullLogger<AlchemistClient>.Instance);
        var configuration = Configuration(dryRun: true);

        var result = await client.EnqueueAsync(
            configuration,
            "/alchemist/media/Movie.mkv",
            CancellationToken.None);

        Assert.True(result.Success);
        Assert.Contains("Dry run", result.Message);
        Assert.Null(handler.Method);
    }

    [Fact]
    public async Task TestEventAccessAsync_OpensScopedEventStream()
    {
        using var handler = new RecordingHandler(HttpStatusCode.OK, "event: status\n\n");
        using var httpClient = new HttpClient(handler);
        using var client = new AlchemistClient(httpClient, NullLogger<AlchemistClient>.Instance);
        var configuration = Configuration(dryRun: false);

        var result = await client.TestEventAccessAsync(configuration, CancellationToken.None);

        Assert.True(result.Success);
        Assert.Equal(HttpMethod.Get, handler.Method);
        Assert.Equal("/api/v1/events", handler.Path);
    }

    private static PluginConfiguration Configuration(bool dryRun)
    {
        return new PluginConfiguration
        {
            AlchemistUrl = "http://127.0.0.1:3000",
            ApiToken = "test-token",
            DryRun = dryRun
        };
    }

    private sealed class RecordingHandler : HttpMessageHandler
    {
        private readonly HttpStatusCode _statusCode;
        private readonly string _responseBody;

        public RecordingHandler(HttpStatusCode statusCode, string responseBody)
        {
            _statusCode = statusCode;
            _responseBody = responseBody;
        }

        public HttpMethod? Method { get; private set; }

        public string? Path { get; private set; }

        public string? Authorization { get; private set; }

        public string Body { get; private set; } = string.Empty;

        protected override async Task<HttpResponseMessage> SendAsync(
            HttpRequestMessage request,
            CancellationToken cancellationToken)
        {
            Method = request.Method;
            Path = request.RequestUri?.AbsolutePath;
            Authorization = request.Headers.Authorization?.ToString();
            if (request.Content is not null)
            {
                Body = await request.Content.ReadAsStringAsync(cancellationToken);
            }

            return new HttpResponseMessage(_statusCode)
            {
                Content = new StringContent(_responseBody, Encoding.UTF8, "application/json")
            };
        }
    }
}

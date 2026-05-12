using System;
using System.IO;
using System.Net.Http;
using System.Net.Http.Headers;
using System.Text;
using System.Text.Json;
using System.Text.Json.Serialization;
using System.Threading;
using System.Threading.Tasks;
using Alchemist.Jellyfin.Configuration;
using Microsoft.Extensions.Logging;

namespace Alchemist.Jellyfin.Services;

/// <summary>
/// Minimal Alchemist HTTP client used by Jellyfin plugin services.
/// </summary>
public sealed class AlchemistClient : IDisposable
{
    private static readonly JsonSerializerOptions JsonOptions = new(JsonSerializerDefaults.Web);
    private readonly HttpClient _httpClient = new();
    private readonly ILogger<AlchemistClient> _logger;

    /// <summary>
    /// Initializes a new instance of the <see cref="AlchemistClient"/> class.
    /// </summary>
    /// <param name="logger">Logger.</param>
    public AlchemistClient(ILogger<AlchemistClient> logger)
    {
        _logger = logger;
    }

    /// <summary>
    /// Tests connectivity with Alchemist using a read-only endpoint.
    /// </summary>
    /// <param name="configuration">Plugin configuration.</param>
    /// <param name="cancellationToken">Cancellation token.</param>
    /// <returns>Connection result.</returns>
    public Task<AlchemistConnectionResult> TestConnectionAsync(
        PluginConfiguration configuration,
        CancellationToken cancellationToken)
    {
        return SendAsync(configuration, HttpMethod.Get, "/api/v1/system/info", null, cancellationToken);
    }

    /// <summary>
    /// Tests whether the configured token can access Alchemist's event stream.
    /// </summary>
    /// <param name="configuration">Plugin configuration.</param>
    /// <param name="cancellationToken">Cancellation token.</param>
    /// <returns>Connection result.</returns>
    public async Task<AlchemistConnectionResult> TestEventAccessAsync(
        PluginConfiguration configuration,
        CancellationToken cancellationToken)
    {
        if (!TryBuildUri(configuration.AlchemistUrl, "/api/v1/events", out var uri, out var message))
        {
            return AlchemistConnectionResult.Fail(message);
        }

        using var request = CreateRequest(configuration, HttpMethod.Get, uri);
        try
        {
            using var response = await _httpClient.SendAsync(
                    request,
                    HttpCompletionOption.ResponseHeadersRead,
                    cancellationToken)
                .ConfigureAwait(false);
            if (!response.IsSuccessStatusCode)
            {
                var body = await response.Content.ReadAsStringAsync(cancellationToken).ConfigureAwait(false);
                return AlchemistConnectionResult.Fail(
                    $"Alchemist event stream returned {(int)response.StatusCode} {response.ReasonPhrase}: {SummarizeBody(body)}");
            }

            return AlchemistConnectionResult.Ok("Event stream accepted the configured token.");
        }
        catch (OperationCanceledException) when (cancellationToken.IsCancellationRequested)
        {
            throw;
        }
        catch (Exception ex) when (ex is HttpRequestException or TaskCanceledException or InvalidOperationException)
        {
            _logger.LogWarning(ex, "Failed to access Alchemist events at {Url}", configuration.AlchemistUrl);
            return AlchemistConnectionResult.Fail(ex.Message);
        }
    }

    /// <summary>
    /// Enqueues a local media path in Alchemist.
    /// </summary>
    /// <param name="configuration">Plugin configuration.</param>
    /// <param name="path">Path visible to Alchemist.</param>
    /// <param name="cancellationToken">Cancellation token.</param>
    /// <returns>Connection result.</returns>
    public Task<AlchemistConnectionResult> EnqueueAsync(
        PluginConfiguration configuration,
        string path,
        CancellationToken cancellationToken)
    {
        if (configuration.DryRun)
        {
            return Task.FromResult(AlchemistConnectionResult.Ok($"Dry run: would enqueue {path}"));
        }

        return SendAsync(
            configuration,
            HttpMethod.Post,
            "/api/v1/jobs/enqueue",
            JsonSerializer.Serialize(new { path }, JsonOptions),
            cancellationToken);
    }

    /// <summary>
    /// Opens the Alchemist event stream.
    /// </summary>
    /// <param name="configuration">Plugin configuration.</param>
    /// <param name="cancellationToken">Cancellation token.</param>
    /// <returns>Open event stream.</returns>
    public async Task<AlchemistEventStream> OpenEventStreamAsync(
        PluginConfiguration configuration,
        CancellationToken cancellationToken)
    {
        if (!TryBuildUri(configuration.AlchemistUrl, "/api/v1/events", out var uri, out var message))
        {
            throw new InvalidOperationException(message);
        }

        using var request = CreateRequest(configuration, HttpMethod.Get, uri);
        var response = await _httpClient.SendAsync(
                request,
                HttpCompletionOption.ResponseHeadersRead,
                cancellationToken)
            .ConfigureAwait(false);
        if (!response.IsSuccessStatusCode)
        {
            var body = await response.Content.ReadAsStringAsync(cancellationToken).ConfigureAwait(false);
            var failure = $"Alchemist event stream returned {(int)response.StatusCode} {response.ReasonPhrase}: {SummarizeBody(body)}";
            response.Dispose();
            throw new InvalidOperationException(failure);
        }

        var stream = await response.Content.ReadAsStreamAsync(cancellationToken).ConfigureAwait(false);
        return new AlchemistEventStream(response, stream);
    }

    /// <summary>
    /// Fetches Alchemist job details.
    /// </summary>
    /// <param name="configuration">Plugin configuration.</param>
    /// <param name="jobId">Alchemist job id.</param>
    /// <param name="cancellationToken">Cancellation token.</param>
    /// <returns>Job details, or null when unavailable.</returns>
    public async Task<AlchemistJobDetailResponse?> GetJobDetailsAsync(
        PluginConfiguration configuration,
        long jobId,
        CancellationToken cancellationToken)
    {
        if (!TryBuildUri(configuration.AlchemistUrl, $"/api/v1/jobs/{jobId}/details", out var uri, out var message))
        {
            _logger.LogWarning("Cannot fetch Alchemist job {JobId}: {Message}", jobId, message);
            return null;
        }

        using var request = CreateRequest(configuration, HttpMethod.Get, uri);
        try
        {
            using var response = await _httpClient.SendAsync(request, cancellationToken).ConfigureAwait(false);
            var body = await response.Content.ReadAsStringAsync(cancellationToken).ConfigureAwait(false);
            if (!response.IsSuccessStatusCode)
            {
                _logger.LogWarning(
                    "Alchemist job detail request for {JobId} returned {Status}: {Body}",
                    jobId,
                    response.StatusCode,
                    SummarizeBody(body));
                return null;
            }

            return JsonSerializer.Deserialize<AlchemistJobDetailResponse>(body, JsonOptions);
        }
        catch (JsonException ex)
        {
            _logger.LogWarning(ex, "Failed to parse Alchemist job details for {JobId}", jobId);
            return null;
        }
        catch (OperationCanceledException) when (cancellationToken.IsCancellationRequested)
        {
            throw;
        }
        catch (Exception ex) when (ex is HttpRequestException or TaskCanceledException or InvalidOperationException)
        {
            _logger.LogWarning(ex, "Failed to fetch Alchemist job details for {JobId}", jobId);
            return null;
        }
    }

    /// <inheritdoc />
    public void Dispose()
    {
        _httpClient.Dispose();
    }

    private async Task<AlchemistConnectionResult> SendAsync(
        PluginConfiguration configuration,
        HttpMethod method,
        string relativePath,
        string? jsonBody,
        CancellationToken cancellationToken)
    {
        if (!TryBuildUri(configuration.AlchemistUrl, relativePath, out var uri, out var message))
        {
            return AlchemistConnectionResult.Fail(message);
        }

        using var request = CreateRequest(configuration, method, uri);

        if (jsonBody is not null)
        {
            request.Content = new StringContent(jsonBody, Encoding.UTF8, "application/json");
        }

        try
        {
            using var response = await _httpClient.SendAsync(request, cancellationToken).ConfigureAwait(false);
            var body = await response.Content.ReadAsStringAsync(cancellationToken).ConfigureAwait(false);
            if (!response.IsSuccessStatusCode)
            {
                return AlchemistConnectionResult.Fail(
                    $"Alchemist returned {(int)response.StatusCode} {response.ReasonPhrase}: {SummarizeBody(body)}");
            }

            return AlchemistConnectionResult.Ok(SummarizeBody(body));
        }
        catch (OperationCanceledException) when (cancellationToken.IsCancellationRequested)
        {
            throw;
        }
        catch (Exception ex) when (ex is HttpRequestException or TaskCanceledException or InvalidOperationException)
        {
            _logger.LogWarning(ex, "Failed to call Alchemist at {Url}", configuration.AlchemistUrl);
            return AlchemistConnectionResult.Fail(ex.Message);
        }
    }

    private static bool TryBuildUri(string baseUrl, string relativePath, out Uri uri, out string message)
    {
        uri = new Uri("http://127.0.0.1/");
        message = string.Empty;

        if (string.IsNullOrWhiteSpace(baseUrl))
        {
            message = "Alchemist URL is empty.";
            return false;
        }

        if (!Uri.TryCreate(baseUrl.Trim().TrimEnd('/') + relativePath, UriKind.Absolute, out var parsed))
        {
            message = "Alchemist URL is invalid.";
            return false;
        }

        uri = parsed;
        return true;
    }

    private static HttpRequestMessage CreateRequest(
        PluginConfiguration configuration,
        HttpMethod method,
        Uri uri)
    {
        var request = new HttpRequestMessage(method, uri);
        if (!string.IsNullOrWhiteSpace(configuration.ApiToken))
        {
            request.Headers.Authorization = new AuthenticationHeaderValue("Bearer", configuration.ApiToken.Trim());
        }

        return request;
    }

    private static string SummarizeBody(string body)
    {
        if (string.IsNullOrWhiteSpace(body))
        {
            return "OK";
        }

        return body.Length > 240 ? body[..240] : body;
    }
}

/// <summary>
/// Open Alchemist event stream and its owning response.
/// </summary>
public sealed class AlchemistEventStream : IDisposable
{
    private readonly HttpResponseMessage _response;

    /// <summary>
    /// Initializes a new instance of the <see cref="AlchemistEventStream"/> class.
    /// </summary>
    /// <param name="response">Owning HTTP response.</param>
    /// <param name="stream">Event stream content.</param>
    public AlchemistEventStream(HttpResponseMessage response, Stream stream)
    {
        _response = response;
        Stream = stream;
    }

    /// <summary>
    /// Gets the event stream content.
    /// </summary>
    public Stream Stream { get; }

    /// <inheritdoc />
    public void Dispose()
    {
        Stream.Dispose();
        _response.Dispose();
    }
}

/// <summary>
/// Alchemist job detail response.
/// </summary>
public sealed class AlchemistJobDetailResponse
{
    /// <summary>
    /// Gets or sets the job.
    /// </summary>
    [JsonPropertyName("job")]
    public AlchemistJob Job { get; set; } = new();
}

/// <summary>
/// Alchemist job fields used by the Jellyfin plugin.
/// </summary>
public sealed class AlchemistJob
{
    /// <summary>
    /// Gets or sets the job id.
    /// </summary>
    [JsonPropertyName("id")]
    public long Id { get; set; }

    /// <summary>
    /// Gets or sets the source path.
    /// </summary>
    [JsonPropertyName("input_path")]
    public string InputPath { get; set; } = string.Empty;

    /// <summary>
    /// Gets or sets the output path.
    /// </summary>
    [JsonPropertyName("output_path")]
    public string OutputPath { get; set; } = string.Empty;

    /// <summary>
    /// Gets or sets the job status.
    /// </summary>
    [JsonPropertyName("status")]
    public string Status { get; set; } = string.Empty;
}

/// <summary>
/// Result returned by Alchemist plugin connection attempts.
/// </summary>
public sealed class AlchemistConnectionResult
{
    /// <summary>
    /// Gets or sets a value indicating whether the request succeeded.
    /// </summary>
    public bool Success { get; set; }

    /// <summary>
    /// Gets or sets a human-readable result message.
    /// </summary>
    public string Message { get; set; } = string.Empty;

    /// <summary>
    /// Creates a success result.
    /// </summary>
    /// <param name="message">Message.</param>
    /// <returns>Result.</returns>
    public static AlchemistConnectionResult Ok(string message) => new() { Success = true, Message = message };

    /// <summary>
    /// Creates a failure result.
    /// </summary>
    /// <param name="message">Message.</param>
    /// <returns>Result.</returns>
    public static AlchemistConnectionResult Fail(string message) => new() { Success = false, Message = message };
}

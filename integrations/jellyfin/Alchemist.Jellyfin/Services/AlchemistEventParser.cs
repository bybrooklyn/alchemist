using System;
using System.Text.Json;

namespace dev.bybrooklyn.alchemist.Services;

/// <summary>
/// Parses Alchemist server-sent events used by the Jellyfin plugin.
/// </summary>
public static class AlchemistEventParser
{
    /// <summary>
    /// Attempts to extract a completed job id from an Alchemist status event.
    /// </summary>
    /// <param name="eventName">SSE event name.</param>
    /// <param name="data">SSE data payload.</param>
    /// <param name="jobId">Completed job id.</param>
    /// <returns>True when the event is a completed job status event.</returns>
    public static bool TryParseCompletedJobId(string? eventName, string data, out long jobId)
    {
        jobId = 0;
        if (!string.Equals(eventName, "status", StringComparison.Ordinal) || string.IsNullOrWhiteSpace(data))
        {
            return false;
        }

        try
        {
            using var document = JsonDocument.Parse(data);
            var root = document.RootElement;
            if (!root.TryGetProperty("status", out var status)
                || !string.Equals(status.GetString(), "completed", StringComparison.OrdinalIgnoreCase))
            {
                return false;
            }

            if (!root.TryGetProperty("job_id", out var idProperty))
            {
                return false;
            }

            if (idProperty.ValueKind == JsonValueKind.Number && idProperty.TryGetInt64(out var numericId))
            {
                jobId = numericId;
                return jobId > 0;
            }

            if (idProperty.ValueKind == JsonValueKind.String
                && long.TryParse(idProperty.GetString(), out var stringId)
                && stringId > 0)
            {
                jobId = stringId;
                return true;
            }
        }
        catch (JsonException)
        {
            return false;
        }

        return false;
    }
}

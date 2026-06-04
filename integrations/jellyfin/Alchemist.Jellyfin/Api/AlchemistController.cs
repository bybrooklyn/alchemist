using System.Threading;
using System.Threading.Tasks;
using dev.bybrooklyn.alchemist.Services;
using Microsoft.AspNetCore.Mvc;

namespace dev.bybrooklyn.alchemist.Api;

/// <summary>
/// Jellyfin dashboard API endpoints for the Alchemist plugin.
/// </summary>
[ApiController]
[Route("Alchemist")]
public sealed class AlchemistController : ControllerBase
{
    private readonly AlchemistClient _client;
    private readonly AlchemistPluginRuntimeState _runtimeState;

    /// <summary>
    /// Initializes a new instance of the <see cref="AlchemistController"/> class.
    /// </summary>
    /// <param name="client">Alchemist client.</param>
    /// <param name="runtimeState">Shared plugin runtime state.</param>
    public AlchemistController(AlchemistClient client, AlchemistPluginRuntimeState runtimeState)
    {
        _client = client;
        _runtimeState = runtimeState;
    }

    /// <summary>
    /// Tests the configured Alchemist connection.
    /// </summary>
    /// <param name="cancellationToken">Cancellation token.</param>
    /// <returns>Connection result.</returns>
    [HttpPost("TestConnection")]
    public async Task<ActionResult<AlchemistConnectionResult>> TestConnection(CancellationToken cancellationToken)
    {
        var configuration = Plugin.Instance?.Configuration;
        if (configuration is null)
        {
            return StatusCode(503, AlchemistConnectionResult.Fail("Alchemist plugin is not initialized."));
        }

        return Ok(await _client.TestConnectionAsync(configuration, cancellationToken).ConfigureAwait(false));
    }

    /// <summary>
    /// Tests access to the configured Alchemist event stream.
    /// </summary>
    /// <param name="cancellationToken">Cancellation token.</param>
    /// <returns>Connection result.</returns>
    [HttpPost("TestEventAccess")]
    public async Task<ActionResult<AlchemistConnectionResult>> TestEventAccess(CancellationToken cancellationToken)
    {
        var configuration = Plugin.Instance?.Configuration;
        if (configuration is null)
        {
            return StatusCode(503, AlchemistConnectionResult.Fail("Alchemist plugin is not initialized."));
        }

        return Ok(await _client.TestEventAccessAsync(configuration, cancellationToken).ConfigureAwait(false));
    }

    /// <summary>
    /// Gets plugin runtime status.
    /// </summary>
    /// <returns>Runtime status.</returns>
    [HttpGet("Status")]
    public ActionResult<AlchemistPluginRuntimeStatus> Status()
    {
        return Ok(_runtimeState.Snapshot());
    }
}

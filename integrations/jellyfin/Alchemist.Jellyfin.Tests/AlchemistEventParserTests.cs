using dev.bybrooklyn.alchemist.Services;
using Xunit;

namespace dev.bybrooklyn.alchemist.tests;

public sealed class AlchemistEventParserTests
{
    [Fact]
    public void TryParseCompletedJobId_AcceptsCompletedStatusEvent()
    {
        var parsed = AlchemistEventParser.TryParseCompletedJobId(
            "status",
            """{"job_id":42,"status":"completed"}""",
            out var jobId);

        Assert.True(parsed);
        Assert.Equal(42, jobId);
    }

    [Fact]
    public void TryParseCompletedJobId_IgnoresOtherEvents()
    {
        Assert.False(AlchemistEventParser.TryParseCompletedJobId(
            "progress",
            """{"job_id":42,"status":"completed"}""",
            out _));
        Assert.False(AlchemistEventParser.TryParseCompletedJobId(
            "status",
            """{"job_id":42,"status":"encoding"}""",
            out _));
    }
}

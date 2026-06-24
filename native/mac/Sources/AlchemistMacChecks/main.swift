import AlchemistMacCore
import Foundation
import SwiftUI

enum CheckFailure: Error, CustomStringConvertible {
    case failed(String)

    var description: String {
        switch self {
        case .failed(let message): message
        }
    }
}

func expect(_ condition: @autoclosure () -> Bool, _ message: String) throws {
    if !condition() {
        throw CheckFailure.failed(message)
    }
}

@main
struct AlchemistMacChecks {
    static func main() async throws {
        try await endpointComposesVersionedAPIPath()
        try await endpointSupportsNestedBasePathForFutureAPIProxy()
        try sseParserUsesBackendEventNames()
        try sseParserSplitsFramesFromRawBytes()
        try themeMapsColorSchemes()
        try sidebarMatchesWebUISurface()
        try supportPathsUseNativeApplicationSupport()
        try jobProgressClampsToFraction()
        try sessionCookieExtractionKeepsNativeActionsAuthenticated()
        try await jobParityRoutesUseVersionedAPI()
        try await jobTableQueryMatchesWebUIContract()
        try jobDetailPayloadDecodesWebUIParityFields()
        try savedJobViewsDecodeFromPreferencePayload()
        try await sessionTokenRoundTripsThroughActor()
        try connectionStateRejectsInvalidURL()
        try bundledModeUsesPrivatePort()
        try navigationRouterTogglesPresentationState()
        try jobStateAppliesSavedViews()
        print("AlchemistMacChecks passed")
    }

    static func endpointComposesVersionedAPIPath() async throws {
        let client = AlchemistAPIClient(baseURL: URL(string: "http://127.0.0.1:3000")!)
        let url = try await client.endpoint(AlchemistAPIRoute.jobs)
        try expect(url.absoluteString == "http://127.0.0.1:3000/api/v1/jobs", "versioned /api/v1 path changed")
    }

    static func endpointSupportsNestedBasePathForFutureAPIProxy() async throws {
        let client = AlchemistAPIClient(baseURL: URL(string: "http://127.0.0.1:3000/alchemist")!)
        let url = try await client.endpoint(AlchemistAPIRoute.jobs, queryItems: [
            URLQueryItem(name: "limit", value: "12")
        ])
        try expect(
            url.absoluteString == "http://127.0.0.1:3000/alchemist/api/v1/jobs?limit=12",
            "nested base path did not compose"
        )
    }

    static func sseParserUsesBackendEventNames() throws {
        var parser = AlchemistSSEParser()
        try expect(parser.parse(line: "event: progress") == nil, "event line should wait for data")
        try expect(parser.parse(line: #"data: {"job_id":42,"percentage":37.5,"time":12}"#) == nil, "data line should wait for frame end")

        guard case .progress(let jobID, let percentage, let time) = parser.parse(line: "") else {
            throw CheckFailure.failed("progress SSE event did not parse")
        }
        try expect(jobID == 42, "progress job id did not parse")
        try expect(percentage == 37.5, "progress percentage did not parse")
        try expect(time == 12, "progress time did not parse")

        var systemParser = AlchemistSSEParser()
        _ = systemParser.parse(line: "event: engine_status_changed")
        _ = systemParser.parse(line: "data: {}")
        try expect(systemParser.parse(line: "") == .engineStatusChanged, "system event name was ignored")
    }

    static func sseParserSplitsFramesFromRawBytes() throws {
        // Drive two complete frames through the real byte-splitting path. The blank
        // lines between frames are the delimiters `AsyncBytes.lines` used to drop.
        var parser = AlchemistSSEParser()
        let fixture = Data(
            (
                "event: progress\n" +
                #"data: {"job_id":7,"percentage":50}"# + "\n" +
                "\n" +
                "event: status\n" +
                #"data: {"job_id":7,"status":"completed"}"# + "\n" +
                "\n"
            ).utf8
        )

        let events = parser.consume(data: fixture)
        try expect(events.count == 2, "two SSE frames should decode from raw bytes, got \(events.count)")

        guard case .progress(let jobID, let percentage, _) = events.first else {
            throw CheckFailure.failed("first raw-byte frame should be a progress event")
        }
        try expect(jobID == 7, "raw-byte progress job id did not parse")
        try expect(percentage == 50, "raw-byte progress percentage did not parse")

        guard case .status(let statusJobID, let status) = events.last else {
            throw CheckFailure.failed("second raw-byte frame should be a status event")
        }
        try expect(statusJobID == 7, "raw-byte status job id did not parse")
        try expect(status == "completed", "raw-byte status value did not parse")
    }

    static func themeMapsColorSchemes() throws {
        try expect(AppTheme.system.colorScheme == nil, "system theme should defer to macOS")
        try expect(AppTheme.light.colorScheme == ColorScheme.light, "light theme did not map to SwiftUI")
        try expect(AppTheme.dark.colorScheme == ColorScheme.dark, "dark theme did not map to SwiftUI")
        try expect(ThemeModel.prototypeDefault.theme == .system, "prototype theme default changed")
    }

    static func sidebarMatchesWebUISurface() throws {
        let labels = AppSection.allCases.map(\.label)
        try expect(
            labels == ["Dashboard", "Jobs", "Logs", "Statistics", "Intelligence", "Convert", "System"],
            "native sidebar drifted from WebUI navigation"
        )
    }

    static func supportPathsUseNativeApplicationSupport() throws {
        try expect(
            AlchemistSupportPaths.root.path.contains("Library/Application Support/Alchemist"),
            "support root is not native Application Support"
        )
        try expect(AlchemistSupportPaths.config.lastPathComponent == "config.toml", "config path changed")
        try expect(AlchemistSupportPaths.database.lastPathComponent == "alchemist.db", "database path changed")
    }

    static func jobProgressClampsToFraction() throws {
        let json = """
        {
          "id": 7,
          "input_path": "/Movies/Input.mkv",
          "output_path": "/Movies/Input.alchemist.mkv",
          "status": "encoding",
          "priority": 0,
          "progress": 150,
          "created_at": "2026-04-30T00:00:00Z",
          "updated_at": "2026-04-30T00:01:00Z",
          "attempt_count": 1,
          "encoder": "videotoolbox"
        }
        """.data(using: .utf8)!

        let job = try JSONDecoder().decode(Job.self, from: json)
        try expect(job.fileName == "Input.mkv", "job file name parsing failed")
        try expect(job.progressFraction == 1, "job progress did not clamp")
        try expect(job.isActive, "active job status was not detected")
    }

    static func sessionCookieExtractionKeepsNativeActionsAuthenticated() throws {
        let cookie = "alchemist_session=abc123; HttpOnly; SameSite=Lax; Path=/; Max-Age=2592000"
        try expect(
            AlchemistAPIClient.sessionToken(fromSetCookieHeader: cookie) == "abc123",
            "login session cookie was not captured"
        )
        try expect(
            AlchemistAPIClient.sessionToken(fromSetCookieHeader: "foo=bar; Path=/") == nil,
            "unrelated cookies should not become session tokens"
        )
    }

    static func jobParityRoutesUseVersionedAPI() async throws {
        let client = AlchemistAPIClient(baseURL: URL(string: "http://127.0.0.1:3000")!)
        let routeExpectations: [(String, String)] = [
            (AlchemistAPIRoute.jobDetails(id: 44), "http://127.0.0.1:3000/api/v1/jobs/44/details"),
            (AlchemistAPIRoute.cancelJob(id: 44), "http://127.0.0.1:3000/api/v1/jobs/44/cancel"),
            (AlchemistAPIRoute.restartJob(id: 44), "http://127.0.0.1:3000/api/v1/jobs/44/restart"),
            (AlchemistAPIRoute.job(id: 44), "http://127.0.0.1:3000/api/v1/jobs/44"),
            (AlchemistAPIRoute.jobPriority(id: 44), "http://127.0.0.1:3000/api/v1/jobs/44/priority"),
            (AlchemistAPIRoute.batchJobs, "http://127.0.0.1:3000/api/v1/jobs/batch"),
            (AlchemistAPIRoute.restartFailedJobs, "http://127.0.0.1:3000/api/v1/jobs/restart-failed"),
            (AlchemistAPIRoute.clearCompletedJobs, "http://127.0.0.1:3000/api/v1/jobs/clear-completed"),
            (AlchemistAPIRoute.clearHistory, "http://127.0.0.1:3000/api/v1/jobs/clear-history"),
            (AlchemistAPIRoute.setupStatus, "http://127.0.0.1:3000/api/v1/setup/status"),
            (AlchemistAPIRoute.processorStatus, "http://127.0.0.1:3000/api/v1/processor/status"),
            (AlchemistAPIRoute.preference(key: "saved_job_views"), "http://127.0.0.1:3000/api/v1/settings/preferences/saved_job_views"),
        ]

        for (route, expectedURL) in routeExpectations {
            let url = try await client.endpoint(route)
            try expect(url.absoluteString == expectedURL, "route changed: \(route)")
        }
    }

    static func jobTableQueryMatchesWebUIContract() async throws {
        let client = AlchemistAPIClient(baseURL: URL(string: "http://127.0.0.1:3000")!)
        let url = try await client.endpoint(AlchemistAPIRoute.jobs, queryItems: [
            URLQueryItem(name: "limit", value: "50"),
            URLQueryItem(name: "page", value: "2"),
            URLQueryItem(name: "sort", value: "updated_at"),
            URLQueryItem(name: "sort_by", value: "updated_at"),
            URLQueryItem(name: "sort_desc", value: "true"),
            URLQueryItem(name: "archived", value: "false"),
            URLQueryItem(name: "status", value: "failed,cancelled"),
            URLQueryItem(name: "search", value: "movie"),
        ])
        let components = URLComponents(url: url, resolvingAgainstBaseURL: false)!
        try expect(queryValue("limit", in: components) == "50", "jobs query limit changed")
        try expect(queryValue("page", in: components) == "2", "jobs query page changed")
        try expect(queryValue("sort", in: components) == "updated_at", "jobs query sort changed")
        try expect(queryValue("sort_by", in: components) == "updated_at", "jobs query sort_by changed")
        try expect(queryValue("sort_desc", in: components) == "true", "jobs query sort_desc changed")
        try expect(queryValue("archived", in: components) == "false", "jobs query archived changed")
        try expect(queryValue("status", in: components) == "failed,cancelled", "jobs query status changed")
        try expect(queryValue("search", in: components) == "movie", "jobs query search changed")
    }

    static func jobDetailPayloadDecodesWebUIParityFields() throws {
        let json = """
        {
          "job": {
            "id": 42,
            "input_path": "/Movies/Input.mkv",
            "output_path": "/Movies/Input.alchemist.mkv",
            "status": "completed",
            "priority": 10,
            "progress": 100,
            "created_at": "2026-04-30T00:00:00Z",
            "updated_at": "2026-04-30T01:00:00Z",
            "attempt_count": 1,
            "encoder": "videotoolbox",
            "vmaf_score": 95.4,
            "decision_explanation": {
              "category": "decision",
              "code": "TRANSCODE",
              "summary": "Transcode recommended",
              "detail": "The file exceeds the target bitrate.",
              "operator_guidance": "No action required.",
              "measured": { "source_bitrate": 24000, "hdr": true },
              "legacy_reason": "Bitrate exceeds target"
            }
          },
          "metadata": {
            "duration_secs": 5400,
            "codec_name": "h264",
            "width": 3840,
            "height": 2160,
            "bit_depth": 10,
            "size_bytes": 12000000000,
            "video_bitrate_bps": 24000000,
            "container_bitrate_bps": 25000000,
            "fps": 23.976,
            "container": "matroska",
            "audio_codec": "aac",
            "audio_channels": 6,
            "dynamic_range": "HDR10"
          },
          "encode_stats": {
            "input_size_bytes": 12000000000,
            "output_size_bytes": 4000000000,
            "compression_ratio": 0.333,
            "encode_time_seconds": 7200,
            "encode_speed": 0.75,
            "avg_bitrate_kbps": 6500,
            "vmaf_score": 95.4
          },
          "encode_attempts": [
            {
              "id": 5,
              "attempt_number": 1,
              "started_at": "2026-04-30T00:00:00Z",
              "finished_at": "2026-04-30T01:00:00Z",
              "outcome": "completed",
              "failure_code": null,
              "failure_summary": null,
              "input_size_bytes": 12000000000,
              "output_size_bytes": 4000000000,
              "encode_time_seconds": 7200
            }
          ],
          "encode_history_runs": [
            {
              "run_number": 1,
              "current": true,
              "outcome": "completed",
              "started_at": "2026-04-30T00:00:00Z",
              "finished_at": "2026-04-30T01:00:00Z",
              "failure_summary": null,
              "input_size_bytes": 12000000000,
              "output_size_bytes": 4000000000,
              "encode_time_seconds": 7200,
              "attempts": [
                {
                  "id": 5,
                  "attempt_number": 1,
                  "started_at": "2026-04-30T00:00:00Z",
                  "finished_at": "2026-04-30T01:00:00Z",
                  "outcome": "completed",
                  "failure_code": null,
                  "failure_summary": null,
                  "input_size_bytes": 12000000000,
                  "output_size_bytes": 4000000000,
                  "encode_time_seconds": 7200
                }
              ]
            }
          ],
          "job_logs": [
            { "id": 9, "level": "info", "job_id": 42, "message": "Encode completed", "created_at": "2026-04-30T01:00:00Z" }
          ],
          "job_failure_summary": null,
          "decision_explanation": null,
          "failure_explanation": null,
          "queue_position": null
        }
        """.data(using: .utf8)!

        let detail = try JSONDecoder().decode(JobDetail.self, from: json)
        try expect(detail.job.id == 42, "job detail id did not decode")
        try expect(detail.metadata?.width == 3840, "job metadata did not decode")
        try expect(detail.encodeStats?.vmafScore == 95.4, "encode stats did not decode")
        try expect(detail.encodeHistoryRuns.first?.attempts.count == 1, "encode history attempts did not decode")
        try expect(detail.job.decisionExplanation?.measured["hdr"]?.displayString == "true", "decision explanation measured values did not decode")
        try expect(AlchemistFormatter.duration(detail.metadata?.durationSecs) == "1h 30m", "duration formatter changed")
        try expect(AlchemistFormatter.shortDate(detail.job.updatedAt) != "-", "date formatter failed")
    }

    static func savedJobViewsDecodeFromPreferencePayload() throws {
        let json = """
        [
          {
            "id": "custom-movies",
            "label": "Movies",
            "activeTab": "completed",
            "sortBy": "updated_at",
            "sortDesc": true,
            "search": "movie"
          }
        ]
        """.data(using: .utf8)!

        let views = try JSONDecoder().decode([SavedJobView].self, from: json)
        try expect(views.first?.label == "Movies", "saved job view label did not decode")
        try expect(views.first?.activeTab == .completed, "saved job view tab did not decode")
        try expect(views.first?.sortBy == .updatedAt, "saved job view sort did not decode")
    }

    static func sessionTokenRoundTripsThroughActor() async throws {
        let client = AlchemistAPIClient(baseURL: URL(string: "http://127.0.0.1:3000")!)
        await client.restoreSessionToken("abc123")
        let restoredToken = await client.currentSessionToken()
        try expect(restoredToken == "abc123", "session token restore failed")
        await client.clearSessionToken()
        let clearedToken = await client.currentSessionToken()
        try expect(clearedToken == nil, "session token clear failed")
        try expect(AlchemistAPIError.unauthorized.errorDescription == "Session expired. Login required.", "unauthorized description changed")
    }

    @MainActor
    static func connectionStateRejectsInvalidURL() throws {
        let state = ConnectionState()
        state.baseURLString = "not a url"
        state.rebuildClient()
        try expect(state.apiClient == nil, "invalid base URL should not create client")
        try expect(state.lastError == .connectionFailed("not a url"), "invalid base URL should produce connection error")
    }

    @MainActor
    static func bundledModeUsesPrivatePort() throws {
        try expect(DaemonController.bundledPort == 41737, "bundled daemon port changed from the private 41737")
        try expect(
            DaemonController.bundledBaseURLString == "http://127.0.0.1:41737",
            "bundled base URL changed"
        )
        let state = ConnectionState()
        try expect(
            state.baseURLString == DaemonController.bundledBaseURLString,
            "default base URL drifted from the bundled port"
        )
        try expect(!state.isRemote, "default connection mode should be bundled")
    }

    @MainActor
    static func navigationRouterTogglesPresentationState() throws {
        let router = NavigationRouter()
        try expect(router.selectedSection == AppSection.dashboard, "router default section changed")
        try expect(router.showingInspector, "inspector should default visible")
        router.toggleInspector()
        router.presentLogin()
        router.toggleCommandPalette()
        router.navigate(to: AppSection.queue)
        try expect(router.selectedSection == AppSection.queue, "router navigate failed")
        try expect(!router.showingInspector, "toggle inspector failed")
        try expect(router.showingLogin, "present login failed")
        try expect(router.showingCommandPalette, "toggle command palette failed")
        router.dismissLogin()
        try expect(!router.showingLogin, "dismiss login failed")
    }

    @MainActor
    static func jobStateAppliesSavedViews() throws {
        let state = JobState()
        state.jobs = [
            Job(id: 1, inputPath: "/tmp/a.mkv", outputPath: nil, status: "queued", priority: 0, progress: nil, createdAt: nil, updatedAt: nil, attemptCount: nil, encoder: nil),
            Job(id: 2, inputPath: "/tmp/b.mkv", outputPath: nil, status: "completed", priority: 0, progress: nil, createdAt: nil, updatedAt: nil, attemptCount: nil, encoder: nil),
        ]
        state.toggleAllVisible()
        try expect(state.selectedIDs == Set([1, 2]), "toggleAllVisible should select all visible jobs")
        state.toggleAllVisible()
        try expect(state.selectedIDs.isEmpty, "toggleAllVisible should clear visible jobs on second pass")

        let view = SavedJobView(
            id: "custom-view",
            label: "Failures",
            activeTab: JobTab.failed,
            sortBy: JobSortField.size,
            sortDesc: false,
            search: "movie"
        )
        state.applySavedView(view)
        try expect(state.activeTab == JobTab.failed, "saved view tab did not apply")
        try expect(state.sortField == JobSortField.size, "saved view sort field did not apply")
        try expect(!state.sortDescending, "saved view sort direction did not apply")
        try expect(state.searchText == "movie", "saved view search did not apply")
        try expect(state.activeSavedViewID == "custom-view", "saved view id did not persist")
    }

    private static func queryValue(_ name: String, in components: URLComponents) -> String? {
        components.queryItems?.first { $0.name == name }?.value
    }
}

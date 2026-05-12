import Foundation

@Observable
@MainActor
public final class LogState {
    public var logs: [LogEntry] = []
    public var query = ""
    public var levelFilter = "all"
    public var lastError: AlchemistUIError?

    public init() {}

    public var filteredLogs: [LogEntry] {
        logs.filter { log in
            let matchesLevel = levelFilter == "all" || log.level.lowercased().contains(levelFilter)
            let needle = query.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
            let matchesQuery = needle.isEmpty
                || log.message.lowercased().contains(needle)
                || log.level.lowercased().contains(needle)
                || String(log.jobID ?? 0).contains(needle)
            return matchesLevel && matchesQuery
        }
    }

    public func load(apiClient: AlchemistAPIClient?) async {
        guard let apiClient else { return }
        do {
            let history = try await apiClient.fetchLogsHistory()
            logs = Array(history.reversed())
            lastError = nil
        } catch {
            if let api = error as? AlchemistAPIError, api == .unauthorized {
                lastError = .authenticationRequired
            } else {
                lastError = .apiError(code: "logs_load_failed", message: error.localizedDescription)
            }
        }
    }

    public func clear(apiClient: AlchemistAPIClient?) async {
        guard let apiClient else { return }
        do {
            try await apiClient.clearLogs()
            logs = []
            lastError = nil
        } catch {
            if let api = error as? AlchemistAPIError, api == .unauthorized {
                lastError = .authenticationRequired
            } else {
                lastError = .apiError(code: "logs_clear_failed", message: error.localizedDescription)
            }
        }
    }
}

import Foundation

@Observable
@MainActor
public final class EngineState {
    public var status = EngineStatus.offline
    public var lastError: AlchemistUIError?

    public init() {}

    public var isPaused: Bool { status.status == "paused" }
    public var isRunning: Bool { status.status == "running" }

    public func refresh(apiClient: AlchemistAPIClient?) async {
        guard let apiClient else { return }
        do {
            self.status = try await apiClient.fetchEngineStatus()
            lastError = nil
        } catch {
            // Engine status errors are non-fatal during startup
        }
    }

    public func pause(apiClient: AlchemistAPIClient?) async {
        guard let apiClient else { return }
        do {
            try await apiClient.pauseQueue()
            await refresh(apiClient: apiClient)
            lastError = nil
        } catch {
            lastError = .apiError(code: "pause_failed", message: error.localizedDescription)
        }
    }

    public func resume(apiClient: AlchemistAPIClient?) async {
        guard let apiClient else { return }
        do {
            try await apiClient.resumeQueue()
            await refresh(apiClient: apiClient)
            lastError = nil
        } catch {
            lastError = .apiError(code: "resume_failed", message: error.localizedDescription)
        }
    }
}

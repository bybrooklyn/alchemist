import Foundation

@Observable
@MainActor
public final class IntelligenceState {
    public var intelligence: IntelligenceResponse?
    public var lastError: AlchemistUIError?

    public init() {}

    public func load(apiClient: AlchemistAPIClient?) async {
        guard let apiClient else { return }
        do {
            intelligence = try await apiClient.fetchLibraryIntelligence()
            lastError = nil
        } catch {
            if let api = error as? AlchemistAPIError, api == .unauthorized {
                lastError = .authenticationRequired
            } else {
                lastError = .apiError(code: "intelligence_load_failed", message: error.localizedDescription)
            }
        }
    }

    public func enqueuePath(_ path: String, apiClient: AlchemistAPIClient?) async {
        guard let apiClient else { return }
        do {
            _ = try await apiClient.enqueueFile(path: path)
        } catch {
            if let api = error as? AlchemistAPIError, api == .unauthorized {
                lastError = .authenticationRequired
            } else {
                lastError = .apiError(code: "enqueue_failed", message: error.localizedDescription)
            }
        }
    }
}

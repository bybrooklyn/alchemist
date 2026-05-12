import Foundation

public enum SSEConnectionState: Equatable, Sendable {
    case disconnected
    case connecting
    case connected
    case reconnecting(attempt: Int)
}

@Observable
@MainActor
public final class ConnectionState {
    public var apiClient: AlchemistAPIClient?
    public var baseURLString = "http://127.0.0.1:3000"
    public var connectionMode: ConnectionMode = .bundled
    public var sseState: SSEConnectionState = .disconnected
    public var lastError: AlchemistUIError?

    private var eventTask: Task<Void, Never>?
    private var pollingTask: Task<Void, Never>?
    private var reconnectAttempt = 0
    private let maxReconnectDelay: UInt64 = 30_000_000_000

    public init() {}

    public func rebuildClient() {
        let trimmed = baseURLString.trimmingCharacters(in: .whitespacesAndNewlines)
        guard
            let baseURL = URL(string: trimmed),
            let scheme = baseURL.scheme?.lowercased(),
            ["http", "https"].contains(scheme),
            baseURL.host != nil
        else {
            lastError = .connectionFailed(baseURLString)
            apiClient = nil
            return
        }
        apiClient = AlchemistAPIClient(baseURL: baseURL)
        lastError = nil
    }

    public func startEventStream(onEvent: @escaping (AlchemistEvent) -> Void) {
        eventTask?.cancel()
        reconnectAttempt = 0
        sseState = .connecting

        eventTask = Task { [weak self] in
            guard let self else { return }
            while !Task.isCancelled {
                guard let client = self.apiClient else {
                    self.sseState = .disconnected
                    return
                }
                self.sseState = .connected
                self.reconnectAttempt = 0
                do {
                    let stream = await client.streamEvents()
                    for try await event in stream {
                        if Task.isCancelled { break }
                        self.lastError = nil
                        onEvent(event)
                    }
                } catch {
                    if Task.isCancelled { break }
                    if let apiError = error as? AlchemistAPIError, apiError == .unauthorized {
                        self.lastError = .authenticationRequired
                    } else {
                        self.lastError = .connectionFailed(error.localizedDescription)
                    }
                }
                self.reconnectAttempt += 1
                self.sseState = .reconnecting(attempt: self.reconnectAttempt)
                let delay = min(
                    UInt64(1_000_000_000) * UInt64(1 << min(self.reconnectAttempt, 5)),
                    self.maxReconnectDelay
                )
                try? await Task.sleep(nanoseconds: delay)
            }
        }
    }

    public func startResourcePolling(interval: TimeInterval = 2.0, onPoll: @escaping () async -> Void) {
        pollingTask?.cancel()
        pollingTask = Task { [weak self] in
            while !Task.isCancelled {
                guard self != nil else { return }
                await onPoll()
                try? await Task.sleep(nanoseconds: UInt64(interval * 1_000_000_000))
            }
        }
    }

    public func stopAll() {
        eventTask?.cancel()
        eventTask = nil
        pollingTask?.cancel()
        pollingTask = nil
        sseState = .disconnected
    }

    deinit {
        // Tasks are cancelled via stopAll() before deallocation
    }
}

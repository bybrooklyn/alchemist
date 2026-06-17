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
    public var baseURLString = DaemonController.bundledBaseURLString
    public var connectionMode: ConnectionMode = .bundled
    public var sseState: SSEConnectionState = .disconnected
    public var lastError: AlchemistUIError?

    private var eventTask: Task<Void, Never>?
    private var pollingTask: Task<Void, Never>?
    private var reconnectAttempt = 0
    private let maxReconnectDelay: UInt64 = 30_000_000_000

    /// In remote mode the app talks only to a configured server it does not manage, so
    /// Mac-local file paths must not be sent to it (audit FG-6).
    public var isRemote: Bool { connectionMode == .remote }

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
                // Report the true pre-request state. The previous code flipped to
                // `.connected` here unconditionally, which made the banner always claim
                // "connected", reset the backoff every loop (so it never grew past
                // attempt 1), and hid the real connecting/down state (audit P2-38).
                self.sseState = self.reconnectAttempt == 0
                    ? .connecting
                    : .reconnecting(attempt: self.reconnectAttempt)
                do {
                    let stream = await client.streamEvents()
                    for try await event in stream {
                        if Task.isCancelled { break }
                        if case .connected = event {
                            // Only a real established stream flips us to connected and
                            // resets the backoff; the synthetic marker isn't forwarded.
                            self.sseState = .connected
                            self.reconnectAttempt = 0
                            self.lastError = nil
                            continue
                        }
                        self.lastError = nil
                        onEvent(event)
                    }
                } catch {
                    if Task.isCancelled { break }
                    if let apiError = error as? AlchemistAPIError, apiError == .unauthorized {
                        // Dead session: stop hammering the endpoint every ~2s and let
                        // RootView present login on the lastError change (audit P2-38).
                        self.lastError = .authenticationRequired
                        self.stopAll()
                        return
                    }
                    self.lastError = .connectionFailed(error.localizedDescription)
                }
                if Task.isCancelled { break }
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

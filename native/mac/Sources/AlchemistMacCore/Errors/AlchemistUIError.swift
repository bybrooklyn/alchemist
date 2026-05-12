import Foundation

public enum AlchemistUIError: Identifiable, Error, LocalizedError, Equatable {
    case connectionFailed(String)
    case authenticationRequired
    case apiError(code: String, message: String)
    case jobActionFailed(action: String, reason: String)
    case daemonFailed(reason: String)

    public var id: String {
        switch self {
        case .connectionFailed(let url): "connectionFailed-\(url)"
        case .authenticationRequired: "authenticationRequired"
        case .apiError(let code, _): "apiError-\(code)"
        case .jobActionFailed(let action, _): "jobActionFailed-\(action)"
        case .daemonFailed(let reason): "daemonFailed-\(reason)"
        }
    }

    public var errorDescription: String? {
        switch self {
        case .connectionFailed(let url): "Cannot connect to Alchemist at \(url)"
        case .authenticationRequired: "Login required"
        case .apiError(_, let message): message
        case .jobActionFailed(_, let reason): reason
        case .daemonFailed(let reason): "Daemon failed: \(reason)"
        }
    }

    public var recoverySuggestion: String? {
        switch self {
        case .connectionFailed: "Check that Alchemist is running and the URL is correct."
        case .authenticationRequired: "Log in to continue."
        case .apiError: nil
        case .jobActionFailed: "Retry the action or check job status."
        case .daemonFailed: "Restart the app or check logs."
        }
    }

    public var canRetry: Bool {
        switch self {
        case .connectionFailed, .jobActionFailed: true
        case .authenticationRequired, .apiError, .daemonFailed: false
        }
    }

    public var sfSymbol: String {
        switch self {
        case .connectionFailed: "wifi.slash"
        case .authenticationRequired: "lock.fill"
        case .apiError: "exclamationmark.triangle"
        case .jobActionFailed: "xmark.circle"
        case .daemonFailed: "bolt.slash"
        }
    }
}

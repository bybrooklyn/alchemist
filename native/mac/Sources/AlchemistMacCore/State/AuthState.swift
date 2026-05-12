import Foundation

@Observable
@MainActor
public final class AuthState {
    public var isAuthenticated = false
    public var lastError: AlchemistUIError?

    public init() {}

    public func login(apiClient: AlchemistAPIClient?, username: String, password: String) async {
        guard let apiClient else {
            lastError = .connectionFailed("No API client configured")
            return
        }
        do {
            try await apiClient.login(username: username, password: password)
            isAuthenticated = true
            lastError = nil
        } catch {
            isAuthenticated = false
            if let apiError = error as? AlchemistAPIError {
                if apiError == .unauthorized {
                    lastError = .authenticationRequired
                } else {
                    lastError = .apiError(code: "login_failed", message: apiError.errorDescription ?? "Login failed")
                }
            } else {
                lastError = .apiError(code: "login_failed", message: error.localizedDescription)
            }
        }
    }

    public func logout(apiClient: AlchemistAPIClient?) async {
        guard let apiClient else { return }
        do {
            try await apiClient.logout()
        } catch {
            // Logout errors are non-fatal
        }
        isAuthenticated = false
        lastError = nil
    }

    public func clearError() {
        lastError = nil
    }
}

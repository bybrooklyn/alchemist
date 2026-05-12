import Foundation

@Observable
@MainActor
public final class DashboardState {
    public var stats = JobStats.empty
    public var savings = SavingsSummary.empty
    public var dailyStats: [DailyStat] = []
    public var resources = SystemResources.empty
    public var settingsBundle: SettingsBundleResponse?
    public var systemInfo: SystemInfo?
    public var profiles: [LibraryProfile] = []
    public var isRefreshing = false
    public var lastError: AlchemistUIError?

    public init() {}

    public func refresh(apiClient: AlchemistAPIClient?) async {
        guard let apiClient else { return }
        isRefreshing = true
        defer { isRefreshing = false }

        do {
            async let stats = apiClient.fetchStats()
            async let savings = apiClient.fetchSavings()
            async let system = apiClient.fetchSystemInfo()
            async let profiles = apiClient.fetchProfiles()
            async let dailyStats = apiClient.fetchDailyStats()
            async let settingsBundle = apiClient.fetchSettingsBundle()

            self.stats = try await stats
            self.savings = try await savings
            self.systemInfo = try await system
            self.profiles = try await profiles
            if let loaded = try? await dailyStats { self.dailyStats = loaded }
            if let loaded = try? await settingsBundle { self.settingsBundle = loaded }
            lastError = nil
        } catch {
            lastError = mapError(error)
        }
    }

    public func refreshResources(apiClient: AlchemistAPIClient?) async {
        guard let apiClient else { return }
        do {
            self.resources = try await apiClient.fetchResources()
        } catch {
            // Resource polling errors are silent
        }
    }

    private func mapError(_ error: Error) -> AlchemistUIError {
        if let api = error as? AlchemistAPIError {
            if api == .unauthorized {
                return .authenticationRequired
            }
            return .apiError(code: "api_error", message: api.errorDescription ?? "Unknown error")
        }
        return .connectionFailed(error.localizedDescription)
    }
}

import Foundation
import SwiftUI

@Observable
@MainActor
public final class AppModel {
    // Domain state holders
    public let auth = AuthState()
    public let jobs = JobState()
    public let dashboard = DashboardState()
    public let engine = EngineState()
    public let logs = LogState()
    public let intelligence = IntelligenceState()
    public let setup = SetupState()
    public let connection = ConnectionState()
    public let navigation = NavigationRouter()
    public let tasks = TaskRegistry()
    public let notifications = NotificationCenterState()
    public let daemon = DaemonController()
    public var theme = ThemeModel.prototypeDefault
    public var setupRequired = false
    public var toast: ToastMessage?

    // MARK: - Convenience accessors (backward compat during migration)

    public var isAuthenticated: Bool { auth.isAuthenticated }
    public var selectedSection: AppSection? {
        get { navigation.selectedSection }
        set { navigation.selectedSection = newValue ?? .dashboard }
    }
    public var isRefreshing: Bool { dashboard.isRefreshing || jobs.isRefreshing }
    public var lastError: AlchemistUIError? {
        setup.lastError ?? auth.lastError ?? jobs.lastError ?? dashboard.lastError ?? engine.lastError ?? connection.lastError
    }

    // MARK: - Init

    public init() {
        connection.rebuildClient()
        daemon.startBundledDaemon()
        Task {
            await notifications.requestAuthorizationIfNeeded()
            await bootstrap()
        }
    }

    // MARK: - Bootstrap

    /// Single startup/reconnect routine shared by `init`, `startBundledDaemon`, and
    /// `reconnect` (audit TD-13). Waits for the daemon to actually answer before
    /// checking setup/session, replacing three divergent hand-rolled sequences and the
    /// magic 700 ms sleep that lost on slow disks / first-run migrations.
    public func bootstrap() async {
        await waitForDaemonReady()
        await refreshSetupStatus()
        guard !setupRequired else { return }
        await restoreSessionIfAvailable()
        if auth.isAuthenticated {
            await refreshAll()
        }
    }

    /// Poll setup status with short backoff until the daemon responds (or we give up).
    private func waitForDaemonReady() async {
        guard let client = connection.apiClient else { return }
        let deadline = Date().addingTimeInterval(15)
        var delay: UInt64 = 150_000_000
        while Date() < deadline {
            if Task.isCancelled { return }
            if (try? await client.fetchSetupStatus()) != nil {
                return
            }
            try? await Task.sleep(nanoseconds: delay)
            delay = min(delay * 2, 1_500_000_000)
        }
    }

    // MARK: - Toast

    public func showToast(_ type: ToastType, _ message: String) {
        withAnimation { toast = ToastMessage(type: type, message: message) }
    }

    // MARK: - Top-level refresh

    public func refreshAll() async {
        guard !setupRequired else { return }
        guard let client = connection.apiClient else { return }
        do {
            dashboard.isRefreshing = true
            jobs.isRefreshing = true
            engine.status = try await client.fetchEngineStatus()
            auth.isAuthenticated = true
            navigation.dismissLogin()
        } catch {
            dashboard.isRefreshing = false
            jobs.isRefreshing = false
            handleSessionError(error)
            return
        }

        await withTaskGroup(of: Void.self) { group in
            group.addTask { await self.dashboard.refresh(apiClient: client) }
            group.addTask { await self.jobs.refresh(apiClient: client) }
        }
    }

    // MARK: - SSE event handling

    private func startEventStream() {
        connection.startEventStream { [weak self] event in
            guard let self else { return }
            self.handleEvent(event)
        }
    }

    private func handleEvent(_ event: AlchemistEvent) {
        switch event {
        case .progress(let jobID, let percentage, _):
            jobs.handleProgress(jobID: jobID, percentage: percentage)
        case .status(let jobID, let status):
            jobs.handleStatusChange(jobID: jobID, status: status)
            postStatusNotification(jobID: jobID, status: status)
            if jobs.focusedDetail?.job.id == jobID {
                // Keep the open inspector's status/stats/logs current (audit RG-13).
                tasks.run("job-detail-refresh") { [weak self] in
                    guard let self else { return }
                    await self.jobs.loadDetails(id: jobID, apiClient: self.connection.apiClient)
                }
            }
            tasks.run("jobs-refresh") { [weak self] in
                guard let self else { return }
                await self.jobs.refresh(apiClient: self.connection.apiClient, silent: true)
            }
        case .decision(let jobID, _, _), .log(let jobID, _, _):
            if jobs.focusedDetail?.job.id == jobID {
                tasks.run("job-detail-refresh") { [weak self] in
                    guard let self else { return }
                    await self.jobs.loadDetails(id: jobID, apiClient: self.connection.apiClient)
                }
            }
            tasks.run("jobs-refresh") { [weak self] in
                guard let self else { return }
                await self.jobs.refresh(apiClient: self.connection.apiClient, silent: true)
            }
        case .engineStatusChanged, .engineIdle:
            tasks.run("engine-refresh") { [weak self] in
                guard let self else { return }
                await self.engine.refresh(apiClient: self.connection.apiClient)
            }
        case .configUpdated, .watchFolderAdded, .watchFolderRemoved:
            tasks.run("dashboard-refresh") { [weak self] in
                guard let self else { return }
                await self.dashboard.refresh(apiClient: self.connection.apiClient)
            }
        case .scanCompleted, .hardwareStateChanged, .lagged:
            tasks.run("full-refresh") { [weak self] in
                guard let self else { return }
                await self.refreshAll()
            }
        default:
            break
        }
    }

    private func startResourcePolling() {
        connection.startResourcePolling { [weak self] in
            guard let self else { return }
            await self.dashboard.refreshResources(apiClient: self.connection.apiClient)
        }
    }

    // MARK: - Auth

    public func login(username: String, password: String) async {
        await auth.login(apiClient: connection.apiClient, username: username, password: password)
        if auth.isAuthenticated {
            if let token = await connection.apiClient?.currentSessionToken() {
                try? KeychainHelper.saveSessionToken(token)
            }
            startEventStream()
            startResourcePolling()
            navigation.dismissLogin()
            await refreshAll()
        }
    }

    public func logout() async {
        await auth.logout(apiClient: connection.apiClient)
        try? KeychainHelper.deleteSessionToken()
        await connection.apiClient?.clearSessionToken()
        connection.stopAll()
        tasks.cancelAll()
        jobs.cancelPendingProgressUpdates()
        jobs.jobs = []
        logs.logs = []
        intelligence.intelligence = nil
        dashboard.stats = .empty
        dashboard.savings = .empty
        navigation.presentLogin()
    }

    // MARK: - Daemon

    public func startBundledDaemon() {
        connection.connectionMode = .bundled
        connection.baseURLString = DaemonController.bundledBaseURLString
        daemon.startBundledDaemon()
        connection.rebuildClient()
        Task { await bootstrap() }
    }

    /// Switch between the bundled daemon and a remote server (audit FG-6). Bundled mode
    /// owns the local daemon lifecycle; remote mode stops it and talks only to the
    /// configured URL.
    public func setConnectionMode(_ mode: ConnectionMode) {
        guard connection.connectionMode != mode else { return }
        switch mode {
        case .bundled:
            startBundledDaemon()
        case .remote:
            connection.connectionMode = .remote
            daemon.stopBundledDaemon()
            connection.stopAll()
            connection.rebuildClient()
            Task { await bootstrap() }
        }
    }

    // MARK: - Queue control

    public func pauseQueue() async {
        await engine.pause(apiClient: connection.apiClient)
    }

    public func resumeQueue() async {
        await engine.resume(apiClient: connection.apiClient)
    }

    // MARK: - File import

    public func enqueueFiles(_ urls: [URL]) async {
        guard !connection.isRemote else {
            jobs.lastError = .apiError(
                code: "remote_local_path",
                message: "Local file paths can't be sent to a remote server. Use Convert (upload) instead."
            )
            return
        }
        for url in urls {
            await jobs.enqueuePath(url.path, apiClient: connection.apiClient)
        }
    }

    public func addWatchFolders(_ urls: [URL]) async {
        guard !connection.isRemote else {
            jobs.lastError = .apiError(
                code: "remote_local_path",
                message: "Watch folders use Mac-local paths a remote server can't see."
            )
            return
        }
        guard let client = connection.apiClient else { return }
        for url in urls {
            do {
                _ = try await client.addWatchFolder(path: url.path)
            } catch {
                jobs.lastError = .apiError(code: "watch_folder_failed", message: error.localizedDescription)
            }
        }
        await refreshAll()
    }

    public func uploadConversion(_ url: URL) async {
        guard let client = connection.apiClient else { return }
        do {
            _ = try await client.uploadConversion(fileURL: url)
        } catch {
            jobs.lastError = .apiError(code: "upload_failed", message: error.localizedDescription)
        }
        await refreshAll()
    }

    // MARK: - Reconnect

    public func reconnect() {
        connection.rebuildClient()
        Task { await bootstrap() }
    }

    public func refreshSetupStatus() async {
        guard let client = connection.apiClient else { return }
        do {
            let status = try await client.fetchSetupStatus()
            setupRequired = status.setupRequired
            setup.setupStatus = status
            connection.lastError = nil
            if setupRequired {
                auth.isAuthenticated = false
                connection.stopAll()
                navigation.dismissLogin()
                await setup.loadBootstrap(apiClient: client)
            }
        } catch {
            // The daemon isn't reachable yet — surface a connection error and leave
            // setupRequired unchanged rather than silently defaulting a fresh install
            // to the login screen instead of the wizard (audit P2-37).
            connection.lastError = .connectionFailed(error.localizedDescription)
        }
    }

    public func completeNativeSetup() async {
        let completed = await setup.completeSetup(apiClient: connection.apiClient)
        guard completed else { return }
        if let token = await connection.apiClient?.currentSessionToken() {
            try? KeychainHelper.saveSessionToken(token)
        }
        auth.isAuthenticated = true
        setupRequired = false
        navigation.dismissLogin()
        startEventStream()
        startResourcePolling()
        await refreshAll()
    }

    private func restoreSessionIfAvailable() async {
        guard let token = try? KeychainHelper.loadSessionToken(), let client = connection.apiClient else {
            navigation.presentLogin()
            return
        }
        await client.restoreSessionToken(token)
        do {
            _ = try await client.fetchEngineStatus()
            auth.isAuthenticated = true
            navigation.dismissLogin()
            startEventStream()
            startResourcePolling()
        } catch {
            if let apiError = error as? AlchemistAPIError, apiError == .unauthorized {
                // Only a real auth rejection should drop the persisted session.
                await client.clearSessionToken()
                try? KeychainHelper.deleteSessionToken()
                handleSessionError(error)
            } else {
                // Daemon not ready / transient connection failure — keep the token so the
                // user isn't forced to re-login on every cold launch (audit P2-37).
                connection.lastError = .connectionFailed(error.localizedDescription)
            }
        }
    }

    private func handleSessionError(_ error: Error) {
        if let apiError = error as? AlchemistAPIError, apiError == .unauthorized {
            auth.isAuthenticated = false
            navigation.presentLogin()
            auth.lastError = .authenticationRequired
            return
        }
        auth.lastError = .connectionFailed(error.localizedDescription)
    }

    private func postStatusNotification(jobID: Int64, status: String) {
        // Don't drop notifications for jobs outside the loaded page (audit UX-7).
        let name = jobs.jobs.first(where: { $0.id == jobID })?.fileName ?? "Job #\(jobID)"
        switch status {
        case "completed":
            notifications.postJobNotification(title: "Encode completed", body: name)
        case "failed":
            notifications.postJobNotification(title: "Encode failed", body: name)
        case "cancelled":
            // A user-initiated cancel is not a failure.
            notifications.postJobNotification(title: "Encode cancelled", body: name)
        default:
            break
        }
    }
}

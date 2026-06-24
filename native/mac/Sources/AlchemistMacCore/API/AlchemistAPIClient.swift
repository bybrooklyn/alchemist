import Foundation

public enum AlchemistAPIError: LocalizedError, Equatable {
    case invalidBaseURL(String)
    case invalidResponse
    case missingSessionCookie
    case unauthorized
    case server(code: String, message: String)
    case httpStatus(Int, String)

    public var errorDescription: String? {
        switch self {
        case .invalidBaseURL(let value):
            "Invalid Alchemist URL: \(value)"
        case .invalidResponse:
            "Alchemist returned an invalid response."
        case .missingSessionCookie:
            "Alchemist login succeeded but did not return a session cookie."
        case .unauthorized:
            "Session expired. Login required."
        case .server(_, let message):
            message
        case .httpStatus(let status, let body):
            "Alchemist returned HTTP \(status): \(body)"
        }
    }
}

public enum AlchemistAPIRoute {
    public static let login = "/api/v1/auth/login"
    public static let logout = "/api/v1/auth/logout"
    public static let stats = "/api/v1/stats"
    public static let dailyStats = "/api/v1/stats/daily"
    public static let savings = "/api/v1/stats/savings"
    public static let engineStatus = "/api/v1/engine/status"
    public static let systemInfo = "/api/v1/system/info"
    public static let systemResources = "/api/v1/system/resources"
    public static let jobs = "/api/v1/jobs"
    public static let batchJobs = "/api/v1/jobs/batch"
    public static let restartFailedJobs = "/api/v1/jobs/restart-failed"
    public static let clearCompletedJobs = "/api/v1/jobs/clear-completed"
    public static let clearHistory = "/api/v1/jobs/clear-history"
    public static let processorStatus = "/api/v1/processor/status"
    public static let profiles = "/api/v1/profiles"
    public static let profilePresets = "/api/v1/profiles/presets"
    public static let enqueueJob = "/api/v1/jobs/enqueue"
    public static let watchDirectories = "/api/v1/settings/watch-dirs"
    public static let preferences = "/api/v1/settings/preferences"
    public static let settingsBundle = "/api/v1/settings/bundle"
    public static let setupStatus = "/api/v1/setup/status"
    public static let setupComplete = "/api/v1/setup/complete"
    public static let pauseEngine = "/api/v1/engine/pause"
    public static let resumeEngine = "/api/v1/engine/resume"
    public static let systemHardware = "/api/v1/system/hardware"
    public static let conversionUploads = "/api/v1/conversion/uploads"
    public static let libraryIntelligence = "/api/v1/library/intelligence"
    public static let logsHistory = "/api/v1/logs/history"
    public static let logs = "/api/v1/logs"
    public static let events = "/api/v1/events"
    public static let fsRecommendations = "/api/v1/fs/recommendations"
    public static let fsPreview = "/api/v1/fs/preview"
    public static let selftest = "/api/v1/system/selftest"
    public static let backup = "/api/v1/system/backup"
    public static let notificationTargets = "/api/v1/settings/notifications"
    public static let notificationTest = "/api/v1/settings/notifications/test"
    public static let apiTokens = "/api/v1/settings/api-tokens"

    public static func job(id: Int64) -> String {
        "/api/v1/jobs/\(id)"
    }

    public static func restartJob(id: Int64) -> String {
        "/api/v1/jobs/\(id)/restart"
    }

    public static func cancelJob(id: Int64) -> String {
        "/api/v1/jobs/\(id)/cancel"
    }

    public static func jobDetails(id: Int64) -> String {
        "/api/v1/jobs/\(id)/details"
    }

    public static func jobPriority(id: Int64) -> String {
        "/api/v1/jobs/\(id)/priority"
    }

    public static func watchDir(id: Int64) -> String {
        "/api/v1/settings/watch-dirs/\(id)"
    }

    public static func notificationTarget(id: Int64) -> String {
        "/api/v1/settings/notifications/\(id)"
    }

    public static func apiToken(id: Int64) -> String {
        "/api/v1/settings/api-tokens/\(id)"
    }

    public static func preference(key: String) -> String {
        "/api/v1/settings/preferences/\(key)"
    }
}

public actor AlchemistAPIClient {
    public let baseURL: URL
    private let session: URLSession
    private let decoder: JSONDecoder
    private var sessionToken: String?

    public init(baseURL: URL, session: URLSession? = nil) {
        self.baseURL = baseURL
        self.session = session ?? Self.makeDefaultSession()
        self.decoder = JSONDecoder()
    }

    /// A cookie-free session. `URLSession.shared` stores the backend's
    /// `Set-Cookie: alchemist_session=…` and can re-inject a stale cookie that overrides
    /// the manually managed `Cookie`/`Bearer` headers after logout or a user switch,
    /// making which credential wins undefined (audit RG-12).
    private static func makeDefaultSession() -> URLSession {
        let config = URLSessionConfiguration.ephemeral
        config.httpShouldSetCookies = false
        config.httpCookieAcceptPolicy = .never
        return URLSession(configuration: config)
    }

    public func endpoint(_ path: String, queryItems: [URLQueryItem] = []) throws -> URL {
        let cleanPath = path.hasPrefix("/") ? String(path.dropFirst()) : path
        guard var components = URLComponents(url: baseURL, resolvingAgainstBaseURL: false) else {
            throw AlchemistAPIError.invalidBaseURL(baseURL.absoluteString)
        }
        let basePath = components.path.trimmingCharacters(in: CharacterSet(charactersIn: "/"))
        components.path = "/" + [basePath, cleanPath].filter { !$0.isEmpty }.joined(separator: "/")
        if !queryItems.isEmpty {
            components.queryItems = queryItems
        }
        guard let url = components.url else {
            throw AlchemistAPIError.invalidBaseURL(baseURL.absoluteString)
        }
        return url
    }

    public func login(username: String, password: String) async throws {
        let payload = ["username": username, "password": password]
        var request = URLRequest(url: try endpoint(AlchemistAPIRoute.login))
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.httpBody = try JSONEncoder().encode(payload)
        let (data, response) = try await session.data(for: request)
        let _: EmptyResponse = try decode(data: data, response: response)

        guard let http = response as? HTTPURLResponse,
              let token = Self.sessionToken(fromSetCookieHeader: http.value(forHTTPHeaderField: "Set-Cookie")) else {
            throw AlchemistAPIError.missingSessionCookie
        }
        sessionToken = token
    }

    public func completeSetup(username: String, password: String, settings: SetupSettings) async throws -> SetupCompleteResponse {
        var request = URLRequest(url: try endpoint(AlchemistAPIRoute.setupComplete))
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.httpBody = try JSONEncoder().encode(SetupCompletePayload(username: username, password: password, settings: settings))
        let (data, response) = try await session.data(for: request)
        let decoded: SetupCompleteResponse = try decode(data: data, response: response)

        guard let http = response as? HTTPURLResponse,
              let token = Self.sessionToken(fromSetCookieHeader: http.value(forHTTPHeaderField: "Set-Cookie")) else {
            throw AlchemistAPIError.missingSessionCookie
        }
        sessionToken = token
        return decoded
    }

    public func logout() async throws {
        let _: EmptyResponse = try await postJSON(AlchemistAPIRoute.logout, body: EmptyPayload())
        sessionToken = nil
    }

    public func restoreSessionToken(_ token: String) {
        sessionToken = token
    }

    public func clearSessionToken() {
        sessionToken = nil
    }

    public func currentSessionToken() -> String? {
        sessionToken
    }

    public func fetchStats() async throws -> JobStats {
        try await getJSON(AlchemistAPIRoute.stats)
    }

    public func fetchDailyStats() async throws -> [DailyStat] {
        try await getJSON(AlchemistAPIRoute.dailyStats)
    }

    public func fetchSavings() async throws -> SavingsSummary {
        try await getJSON(AlchemistAPIRoute.savings)
    }

    public func fetchEngineStatus() async throws -> EngineStatus {
        try await getJSON(AlchemistAPIRoute.engineStatus)
    }

    public func fetchSystemInfo() async throws -> SystemInfo {
        try await getJSON(AlchemistAPIRoute.systemInfo)
    }

    public func fetchResources() async throws -> SystemResources {
        try await getJSON(AlchemistAPIRoute.systemResources)
    }

    public func fetchJobs(limit: Int = 12) async throws -> [Job] {
        try await getJSON(
            AlchemistAPIRoute.jobs,
            queryItems: [
                URLQueryItem(name: "limit", value: String(limit)),
                URLQueryItem(name: "sort", value: "created_at"),
                URLQueryItem(name: "sort_by", value: "created_at"),
                URLQueryItem(name: "sort_desc", value: "true")
            ]
        )
    }

    public func fetchJobs(
        tab: JobTab,
        search: String,
        page: Int,
        limit: Int = JobState.pageSize,
        sortBy: JobSortField,
        sortDescending: Bool
    ) async throws -> [Job] {
        var queryItems = [
            URLQueryItem(name: "limit", value: String(limit)),
            URLQueryItem(name: "page", value: String(max(1, page))),
            URLQueryItem(name: "sort", value: sortBy.rawValue),
            URLQueryItem(name: "sort_by", value: sortBy.rawValue),
            URLQueryItem(name: "sort_desc", value: String(sortDescending)),
            URLQueryItem(name: "archived", value: String(tab.includesArchived)),
        ]

        let statuses = tab.statusFilter
        if !statuses.isEmpty {
            queryItems.append(URLQueryItem(name: "status", value: statuses.joined(separator: ",")))
        }

        let trimmedSearch = search.trimmingCharacters(in: .whitespacesAndNewlines)
        if !trimmedSearch.isEmpty {
            queryItems.append(URLQueryItem(name: "search", value: trimmedSearch))
        }

        return try await getJSON(AlchemistAPIRoute.jobs, queryItems: queryItems)
    }

    public func fetchJobDetails(id: Int64) async throws -> JobDetail {
        try await getJSON(AlchemistAPIRoute.jobDetails(id: id))
    }

    public func fetchProcessorStatus() async throws -> ProcessorStatus {
        try await getJSON(AlchemistAPIRoute.processorStatus)
    }

    public func fetchProfiles() async throws -> [LibraryProfile] {
        try await getJSON(AlchemistAPIRoute.profiles)
    }

    public func fetchProfilePresets() async throws -> [LibraryProfile] {
        try await getJSON(AlchemistAPIRoute.profilePresets)
    }

    public func enqueueFile(path: String) async throws -> EnqueueJobResponse {
        try await postJSON(AlchemistAPIRoute.enqueueJob, body: ["path": path])
    }

    public func addWatchFolder(path: String, recursive: Bool = true) async throws -> WatchDirectory {
        try await postJSON(
            AlchemistAPIRoute.watchDirectories,
            body: AddWatchFolderPayload(path: path, isRecursive: recursive)
        )
    }

    public func deleteWatchFolder(id: Int64) async throws {
        let _: EmptyResponse = try await deleteJSON(AlchemistAPIRoute.watchDir(id: id))
    }

    public func fetchWatchDirectories() async throws -> [WatchDirectory] {
        try await getJSON(AlchemistAPIRoute.watchDirectories)
    }

    public func runSelftest() async throws -> SelftestResponse {
        try await postJSON(AlchemistAPIRoute.selftest, body: EmptyPayload())
    }

    public func backupDatabase() async throws -> Data {
        var request = URLRequest(url: try endpoint(AlchemistAPIRoute.backup))
        request.httpMethod = "POST"
        applyAuth(to: &request)
        let (data, response) = try await session.data(for: request)
        guard let http = response as? HTTPURLResponse, (200..<300).contains(http.statusCode) else {
            throw AlchemistAPIError.invalidResponse
        }
        return data
    }

    public func pauseQueue() async throws {
        let _: EmptyResponse = try await postJSON(AlchemistAPIRoute.pauseEngine, body: EmptyPayload())
    }

    public func resumeQueue() async throws {
        let _: EmptyResponse = try await postJSON(AlchemistAPIRoute.resumeEngine, body: EmptyPayload())
    }

    public func cancelJob(id: Int64) async throws {
        let _: EmptyResponse = try await postJSON(AlchemistAPIRoute.cancelJob(id: id), body: EmptyPayload())
    }

    public func restartJob(id: Int64) async throws {
        let _: EmptyResponse = try await postJSON(AlchemistAPIRoute.restartJob(id: id), body: EmptyPayload())
    }

    public func deleteJob(id: Int64) async throws {
        let _: EmptyResponse = try await deleteJSON(AlchemistAPIRoute.job(id: id))
    }

    public func updateJobPriority(id: Int64, priority: Int) async throws {
        let _: PriorityResponse = try await postJSON(
            AlchemistAPIRoute.jobPriority(id: id),
            body: PriorityPayload(priority: priority)
        )
    }

    public func batchJobs(ids: [Int64], action: JobBatchAction) async throws -> CountMessageResponse {
        try await postJSON(
            AlchemistAPIRoute.batchJobs,
            body: BatchActionPayload(action: action.rawValue, ids: ids)
        )
    }

    public func restartFailedJobs() async throws -> CountMessageResponse {
        try await postJSON(AlchemistAPIRoute.restartFailedJobs, body: EmptyPayload())
    }

    public func clearCompletedJobs() async throws -> CountMessageResponse {
        try await postJSON(AlchemistAPIRoute.clearCompletedJobs, body: EmptyPayload())
    }

    public func clearHistory() async throws -> CountMessageResponse {
        try await postJSON(AlchemistAPIRoute.clearHistory, body: EmptyPayload())
    }

    public func fetchPreference(key: String) async throws -> PreferenceResponse {
        try await getJSON(AlchemistAPIRoute.preference(key: key))
    }

    public func savePreference(key: String, value: String) async throws -> PreferenceResponse {
        try await postJSON(AlchemistAPIRoute.preferences, body: PreferencePayload(key: key, value: value))
    }

    public func fetchSettingsBundle() async throws -> SettingsBundleResponse {
        try await getJSON(AlchemistAPIRoute.settingsBundle)
    }

    public func fetchSetupStatus() async throws -> SetupStatusResponse {
        try await getJSON(AlchemistAPIRoute.setupStatus)
    }

    public func fetchHardwareInfo() async throws -> HardwareInfo {
        try await getJSON(AlchemistAPIRoute.systemHardware)
    }

    public func fetchFolderRecommendations() async throws -> FsRecommendationsResponse {
        try await getJSON(AlchemistAPIRoute.fsRecommendations)
    }

    public func previewFolders(_ directories: [String]) async throws -> FsPreviewResponse {
        try await postJSON(AlchemistAPIRoute.fsPreview, body: FsPreviewRequest(directories: directories))
    }

    public func fetchLibraryIntelligence() async throws -> IntelligenceResponse {
        try await getJSON(AlchemistAPIRoute.libraryIntelligence)
    }

    public func fetchLogsHistory(limit: Int = 200) async throws -> [LogEntry] {
        try await getJSON(
            AlchemistAPIRoute.logsHistory,
            queryItems: [URLQueryItem(name: "limit", value: String(limit))]
        )
    }

    public func clearLogs() async throws {
        let _: EmptyResponse = try await deleteJSON(AlchemistAPIRoute.logs)
    }

    // MARK: - Notification Targets

    public func fetchNotificationTargets() async throws -> NotificationsSettingsResponse {
        try await getJSON(AlchemistAPIRoute.notificationTargets)
    }

    public func addNotificationTarget(
        name: String,
        targetType: String,
        configJSON: [String: JSONValue],
        events: [String],
        enabled: Bool
    ) async throws -> NotificationTargetResponse {
        try await postJSON(
            AlchemistAPIRoute.notificationTargets,
            body: AddNotificationTargetPayload(
                name: name,
                targetType: targetType,
                configJSON: configJSON,
                events: events,
                enabled: enabled
            )
        )
    }

    public func deleteNotificationTarget(id: Int64) async throws {
        let _: EmptyResponse = try await deleteJSON(AlchemistAPIRoute.notificationTarget(id: id))
    }

    public func testNotificationTarget(
        name: String,
        targetType: String,
        configJSON: [String: JSONValue],
        events: [String],
        enabled: Bool
    ) async throws {
        let _: EmptyResponse = try await postJSON(
            AlchemistAPIRoute.notificationTest,
            body: AddNotificationTargetPayload(
                name: name,
                targetType: targetType,
                configJSON: configJSON,
                events: events,
                enabled: enabled
            )
        )
    }

    // MARK: - API Tokens

    public func fetchApiTokens() async throws -> [ApiTokenResponse] {
        try await getJSON(AlchemistAPIRoute.apiTokens)
    }

    public func createApiToken(name: String, accessLevel: String) async throws -> CreatedApiTokenResponse {
        try await postJSON(
            AlchemistAPIRoute.apiTokens,
            body: CreateApiTokenPayload(name: name, accessLevel: accessLevel)
        )
    }

    public func revokeApiToken(id: Int64) async throws {
        let _: EmptyResponse = try await deleteJSON(AlchemistAPIRoute.apiToken(id: id))
    }

    public func uploadConversion(fileURL: URL) async throws -> ConversionUploadResponse {
        let boundary = "AlchemistBoundary-\(UUID().uuidString)"
        var request = URLRequest(url: try endpoint(AlchemistAPIRoute.conversionUploads))
        request.httpMethod = "POST"
        request.setValue("multipart/form-data; boundary=\(boundary)", forHTTPHeaderField: "Content-Type")
        applyAuth(to: &request)

        let body = try multipartBody(fileURL: fileURL, boundary: boundary)
        let (data, response) = try await session.upload(for: request, from: body)
        return try decode(data: data, response: response)
    }

    public func streamEvents() -> AsyncThrowingStream<AlchemistEvent, Error> {
        let token = sessionToken
        return AsyncThrowingStream<AlchemistEvent, Error> { continuation in
            let task = Task {
                do {
                    let url = try endpoint(AlchemistAPIRoute.events)
                    var request = URLRequest(url: url)
                    request.timeoutInterval = 3600
                    Self.applyAuthToken(token, to: &request)

                    let (stream, response) = try await session.bytes(for: request)

                    guard let http = response as? HTTPURLResponse, (200..<300).contains(http.statusCode) else {
                        let status = (response as? HTTPURLResponse)?.statusCode
                        continuation.finish(throwing: status == 401
                            ? AlchemistAPIError.unauthorized
                            : AlchemistAPIError.invalidResponse)
                        return
                    }

                    // Signal a live connection so the reconnect state machine can flip to
                    // `.connected` and reset its backoff only once the stream is real.
                    continuation.yield(.connected)

                    // Frame over raw bytes: `AsyncBytes.lines` drops the blank-line SSE
                    // delimiters, which makes the whole live-update layer dead (audit P1-12).
                    var parser = AlchemistSSEParser()
                    for try await byte in stream {
                        if let event = parser.consume(byte: byte) {
                            continuation.yield(event)
                        }
                    }
                    if let event = parser.finish() {
                        continuation.yield(event)
                    }
                    continuation.finish()
                } catch {
                    continuation.finish(throwing: error)
                }
            }
            continuation.onTermination = { _ in
                task.cancel()
            }
        }
    }

    private func getJSON<T: Decodable>(_ path: String, queryItems: [URLQueryItem] = []) async throws -> T {
        var request = URLRequest(url: try endpoint(path, queryItems: queryItems))
        applyAuth(to: &request)
        let (data, response) = try await session.data(for: request)
        return try decode(data: data, response: response)
    }

    private func postJSON<T: Decodable, Body: Encodable>(_ path: String, body: Body) async throws -> T {
        var request = URLRequest(url: try endpoint(path))
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.httpBody = try JSONEncoder().encode(body)
        applyAuth(to: &request)
        let (data, response) = try await session.data(for: request)
        return try decode(data: data, response: response)
    }

    private func deleteJSON<T: Decodable>(_ path: String) async throws -> T {
        var request = URLRequest(url: try endpoint(path))
        request.httpMethod = "DELETE"
        applyAuth(to: &request)
        let (data, response) = try await session.data(for: request)
        return try decode(data: data, response: response)
    }

    private func decode<T: Decodable>(data: Data, response: URLResponse) throws -> T {
        guard let http = response as? HTTPURLResponse else {
            throw AlchemistAPIError.invalidResponse
        }

        if (200..<300).contains(http.statusCode) {
            if T.self == EmptyResponse.self {
                return EmptyResponse() as! T
            }
            return try decoder.decode(T.self, from: data)
        }

        if http.statusCode == 401 {
            throw AlchemistAPIError.unauthorized
        }

        if let envelope = try? decoder.decode(APIErrorEnvelope.self, from: data) {
            throw AlchemistAPIError.server(code: envelope.error.code, message: envelope.error.message)
        }

        let body = String(data: data, encoding: .utf8) ?? ""
        throw AlchemistAPIError.httpStatus(http.statusCode, body)
    }

    private func multipartBody(fileURL: URL, boundary: String) throws -> Data {
        var body = Data()
        let fileName = fileURL.lastPathComponent
        body.append("--\(boundary)\r\n")
        body.append("Content-Disposition: form-data; name=\"file\"; filename=\"\(fileName)\"\r\n")
        body.append("Content-Type: application/octet-stream\r\n\r\n")
        body.append(try Data(contentsOf: fileURL))
        body.append("\r\n--\(boundary)--\r\n")
        return body
    }

    private func applyAuth(to request: inout URLRequest) {
        Self.applyAuthToken(sessionToken, to: &request)
    }

    private static func applyAuthToken(_ token: String?, to request: inout URLRequest) {
        guard let token, !token.isEmpty else { return }
        request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
        request.setValue("alchemist_session=\(token)", forHTTPHeaderField: "Cookie")
    }

    public static func sessionToken(fromSetCookieHeader header: String?) -> String? {
        guard let header else { return nil }
        let token = header
            .split(separator: ";")
            .map { $0.trimmingCharacters(in: .whitespacesAndNewlines) }
            .first { $0.hasPrefix("alchemist_session=") }?
            .dropFirst("alchemist_session=".count)
        guard let token, !token.isEmpty else { return nil }
        return String(token)
    }
}

private struct EmptyPayload: Encodable {}
private struct EmptyResponse: Decodable {}

public enum JobBatchAction: String, Sendable {
    case cancel
    case restart
    case delete
}

private struct BatchActionPayload: Encodable {
    let action: String
    let ids: [Int64]
}

private struct PriorityPayload: Encodable {
    let priority: Int
}

private struct PriorityResponse: Decodable {
    let id: Int64
    let priority: Int
}

private struct PreferencePayload: Encodable {
    let key: String
    let value: String
}

private struct AddWatchFolderPayload: Encodable {
    let path: String
    let isRecursive: Bool

    enum CodingKeys: String, CodingKey {
        case path
        case isRecursive = "is_recursive"
    }
}

private struct AddNotificationTargetPayload: Encodable {
    let name: String
    let targetType: String
    let configJSON: [String: JSONValue]
    let events: [String]
    let enabled: Bool

    enum CodingKeys: String, CodingKey {
        case name
        case targetType = "target_type"
        case configJSON = "config_json"
        case events, enabled
    }
}

private struct CreateApiTokenPayload: Encodable {
    let name: String
    let accessLevel: String

    enum CodingKeys: String, CodingKey {
        case name
        case accessLevel = "access_level"
    }
}

private extension Data {
    mutating func append(_ string: String) {
        if let data = string.data(using: .utf8) {
            append(data)
        }
    }
}

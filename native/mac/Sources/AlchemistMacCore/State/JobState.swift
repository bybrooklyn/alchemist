import Foundation

@Observable
@MainActor
public final class JobState {
    /// Server page size for the jobs table. Used for both the fetch limit and the
    /// "can page forward" heuristic so the two can't drift (audit P2-39). `nonisolated`
    /// so the non-MainActor API client can use it as a default argument.
    public nonisolated static let pageSize = 50

    public var jobs: [Job] = []
    public var activeTab: JobTab = .all
    public var sortField: JobSortField = .updatedAt
    public var sortDescending = true
    public var searchText = ""
    public var page = 1
    public var selectedIDs: Set<Int64> = []
    public var savedViews: [SavedJobView] = []
    public var activeSavedViewID: String?
    public var focusedDetail: JobDetail?
    public var focusedProcessorStatus: ProcessorStatus?
    public var detailLoading = false
    public var actionMessage: String?
    public var isRefreshing = false
    public var lastError: AlchemistUIError?
    public var isPerformingAction = false

    private var pendingProgressUpdates: [Int64: Double] = [:]
    private var progressFlushTask: Task<Void, Never>?

    public init() {}

    // MARK: - Computed

    public var selectedJobs: [Job] {
        jobs.filter { selectedIDs.contains($0.id) }
    }

    public var hasSelectedActiveJobs: Bool {
        selectedJobs.contains(where: \.isActive)
    }

    public var canGoToPreviousPage: Bool { page > 1 }
    public var canGoToNextPage: Bool { jobs.count == Self.pageSize }

    public var activeCount: Int { jobs.filter(\.isActive).count }
    public var failedCount: Int { jobs.filter { ["failed", "cancelled"].contains($0.status) }.count }
    public var completedCount: Int { jobs.filter { $0.status == "completed" }.count }

    // MARK: - Refresh

    public func refresh(apiClient: AlchemistAPIClient?, silent: Bool = false) async {
        guard let apiClient else { return }
        if !silent { isRefreshing = true }
        defer { if !silent { isRefreshing = false } }

        do {
            let fetched = try await apiClient.fetchJobs(
                tab: activeTab,
                search: searchText,
                page: page,
                sortBy: sortField,
                sortDescending: sortDescending
            )
            jobs = fetched
            selectedIDs = selectedIDs.intersection(Set(jobs.map(\.id)))
            lastError = nil
            actionMessage = nil
        } catch {
            lastError = mapError(error)
            actionMessage = error.localizedDescription
        }
    }

    public func refreshQuick(apiClient: AlchemistAPIClient?) async {
        guard let apiClient else { return }
        do {
            jobs = try await apiClient.fetchJobs(limit: 5)
        } catch {
            // Silent
        }
    }

    // MARK: - Tab / Sort / Search

    public func setTab(_ tab: JobTab) {
        activeTab = tab
        page = 1
        activeSavedViewID = nil
    }

    public func setSortField(_ field: JobSortField) {
        sortField = field
        page = 1
        activeSavedViewID = nil
    }

    public func toggleSortDirection() {
        sortDescending.toggle()
        page = 1
        activeSavedViewID = nil
    }

    public func prepareSearchRefresh() {
        page = 1
        activeSavedViewID = nil
    }

    public func movePage(by delta: Int) {
        page = max(1, page + delta)
    }

    // MARK: - Selection

    public func toggleSelection(id: Int64) {
        if selectedIDs.contains(id) {
            selectedIDs.remove(id)
        } else {
            selectedIDs.insert(id)
        }
    }

    public func toggleAllVisible() {
        let visibleIDs = Set(jobs.map(\.id))
        if !visibleIDs.isEmpty && visibleIDs.isSubset(of: selectedIDs) {
            selectedIDs.subtract(visibleIDs)
        } else {
            selectedIDs.formUnion(visibleIDs)
        }
    }

    public func clearSelection() {
        selectedIDs.removeAll()
    }

    // MARK: - Job Details

    public func loadDetails(id: Int64, apiClient: AlchemistAPIClient?) async {
        guard let apiClient else { return }
        detailLoading = true
        defer { detailLoading = false }
        do {
            let detail = try await apiClient.fetchJobDetails(id: id)
            focusedDetail = detail
            if detail.job.status == "queued" {
                focusedProcessorStatus = try? await apiClient.fetchProcessorStatus()
            } else {
                focusedProcessorStatus = nil
            }
            lastError = nil
        } catch {
            actionMessage = error.localizedDescription
            lastError = mapError(error)
        }
    }

    public func closeDetails() {
        focusedDetail = nil
        focusedProcessorStatus = nil
    }

    // MARK: - Job Actions

    public func performAction(id: Int64, action: JobBatchAction, apiClient: AlchemistAPIClient?) async -> (ToastType, String)? {
        guard let apiClient else {
            actionMessage = "No API connection."
            return nil
        }
        isPerformingAction = true
        defer { isPerformingAction = false }
        do {
            switch action {
            case .cancel: try await apiClient.cancelJob(id: id)
            case .restart: try await apiClient.restartJob(id: id)
            case .delete:
                try await apiClient.deleteJob(id: id)
                if focusedDetail?.job.id == id { closeDetails() }
            }
            lastError = nil
            await refresh(apiClient: apiClient, silent: true)
            return (.success, "Job #\(id) \(action.rawValue)d.")
        } catch {
            actionMessage = error.localizedDescription
            lastError = mapError(error)
            return (.error, error.localizedDescription)
        }
    }

    public func performBatch(_ action: JobBatchAction, apiClient: AlchemistAPIClient?) async -> (ToastType, String)? {
        let ids = Array(selectedIDs)
        guard !ids.isEmpty, let apiClient else { return nil }
        isPerformingAction = true
        defer { isPerformingAction = false }
        do {
            let response = try await apiClient.batchJobs(ids: ids, action: action)
            selectedIDs.removeAll()
            actionMessage = response.message ?? "\(action.rawValue.capitalized) request sent."
            lastError = nil
            await refresh(apiClient: apiClient, silent: true)
            return (.success, actionMessage ?? "Done.")
        } catch {
            actionMessage = error.localizedDescription
            lastError = mapError(error)
            return (.error, error.localizedDescription)
        }
    }

    public func restartFailed(apiClient: AlchemistAPIClient?) async -> (ToastType, String)? {
        guard let apiClient else { return nil }
        do {
            let response = try await apiClient.restartFailedJobs()
            actionMessage = response.message ?? "Retry sent."
            lastError = nil
            await refresh(apiClient: apiClient, silent: true)
            return (.success, actionMessage ?? "Done.")
        } catch {
            actionMessage = error.localizedDescription
            lastError = mapError(error)
            return (.error, error.localizedDescription)
        }
    }

    public func clearCompleted(apiClient: AlchemistAPIClient?) async -> (ToastType, String)? {
        guard let apiClient else { return nil }
        do {
            let response = try await apiClient.clearCompletedJobs()
            actionMessage = response.message ?? "Completed jobs cleared."
            lastError = nil
            await refresh(apiClient: apiClient, silent: true)
            return (.success, actionMessage ?? "Done.")
        } catch {
            actionMessage = error.localizedDescription
            lastError = mapError(error)
            return (.error, error.localizedDescription)
        }
    }

    public func clearHistory(apiClient: AlchemistAPIClient?) async -> (ToastType, String)? {
        guard let apiClient else { return nil }
        do {
            let response = try await apiClient.clearHistory()
            actionMessage = "Purged \(response.count) jobs."
            lastError = nil
            await refresh(apiClient: apiClient, silent: true)
            return (.success, actionMessage ?? "Done.")
        } catch {
            actionMessage = error.localizedDescription
            lastError = mapError(error)
            return (.error, error.localizedDescription)
        }
    }

    public func updatePriority(_ job: Job, priority: Int, label: String, apiClient: AlchemistAPIClient?) async {
        guard let apiClient else { return }
        do {
            _ = try await apiClient.updateJobPriority(id: job.id, priority: priority)
            if let index = jobs.firstIndex(where: { $0.id == job.id }) {
                jobs[index] = jobs[index].withPriority(priority)
            }
            if let detail = focusedDetail, detail.job.id == job.id {
                focusedDetail = detail.withJob(detail.job.withPriority(priority))
            }
            actionMessage = "\(label) for job #\(job.id)."
            lastError = nil
        } catch {
            actionMessage = error.localizedDescription
            lastError = mapError(error)
        }
    }

    public func enqueuePath(_ path: String, apiClient: AlchemistAPIClient?) async {
        let trimmed = path.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty, let apiClient else {
            actionMessage = "File path is required."
            return
        }
        do {
            let response = try await apiClient.enqueueFile(path: trimmed)
            actionMessage = response.message
            lastError = nil
            await refresh(apiClient: apiClient, silent: true)
        } catch {
            actionMessage = error.localizedDescription
            lastError = mapError(error)
        }
    }

    // MARK: - Saved Views

    public func applySavedView(_ view: SavedJobView) {
        activeTab = view.activeTab
        sortField = view.sortBy
        sortDescending = view.sortDesc
        searchText = view.search ?? ""
        activeSavedViewID = view.id
        page = 1
    }

    public func saveCurrentView(label: String, apiClient: AlchemistAPIClient?) async {
        let trimmed = label.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty, let apiClient else {
            actionMessage = "View name is required."
            return
        }
        let view = SavedJobView(
            id: "custom-\(UUID().uuidString)",
            label: trimmed,
            activeTab: activeTab,
            sortBy: sortField,
            sortDesc: sortDescending,
            search: searchText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ? nil : searchText.trimmingCharacters(in: .whitespacesAndNewlines)
        )
        savedViews.append(view)
        activeSavedViewID = view.id
        await persistSavedViews(apiClient: apiClient)
    }

    public func deleteSavedView(id: String, apiClient: AlchemistAPIClient?) async {
        savedViews.removeAll { $0.id == id }
        if activeSavedViewID == id { activeSavedViewID = nil }
        if let apiClient { await persistSavedViews(apiClient: apiClient) }
    }

    public func loadSavedViews(apiClient: AlchemistAPIClient?) async {
        guard let apiClient else { return }
        do {
            let pref = try await apiClient.fetchPreference(key: "saved_job_views")
            let data = Data(pref.value.utf8)
            savedViews = (try? JSONDecoder().decode([SavedJobView].self, from: data)) ?? []
        } catch {
            savedViews = []
        }
    }

    private func persistSavedViews(apiClient: AlchemistAPIClient?) async {
        guard let apiClient else { return }
        do {
            let data = try JSONEncoder().encode(savedViews)
            let value = String(data: data, encoding: .utf8) ?? "[]"
            _ = try await apiClient.savePreference(key: "saved_job_views", value: value)
        } catch {
            actionMessage = error.localizedDescription
        }
    }

    // MARK: - Event Handling

    public func handleProgress(jobID: Int64, percentage: Double) {
        pendingProgressUpdates[jobID] = percentage
        if progressFlushTask != nil {
            return
        }

        progressFlushTask = Task { @MainActor [weak self] in
            try? await Task.sleep(nanoseconds: 100_000_000)
            self?.flushPendingProgressUpdates()
        }
    }

    private func flushPendingProgressUpdates() {
        for (jobID, percentage) in pendingProgressUpdates {
            if let index = jobs.firstIndex(where: { $0.id == jobID }) {
                jobs[index] = jobs[index].withProgress(percentage)
            }
            if let detail = focusedDetail, detail.job.id == jobID {
                focusedDetail = detail.withJob(detail.job.withProgress(percentage))
            }
        }
        pendingProgressUpdates.removeAll(keepingCapacity: true)
        progressFlushTask = nil
    }

    public func cancelPendingProgressUpdates() {
        progressFlushTask?.cancel()
        progressFlushTask = nil
        pendingProgressUpdates.removeAll()
    }

    public func handleImmediateProgress(jobID: Int64, percentage: Double) {
        if let index = jobs.firstIndex(where: { $0.id == jobID }) {
            jobs[index] = jobs[index].withProgress(percentage)
        }
        if let detail = focusedDetail, detail.job.id == jobID {
            focusedDetail = detail.withJob(detail.job.withProgress(percentage))
        }
    }

    public func handleStatusChange(jobID: Int64, status: String) {
        if let index = jobs.firstIndex(where: { $0.id == jobID }) {
            jobs[index] = jobs[index].withStatus(status)
        }
        // The open inspector is refreshed by AppModel.handleEvent's `.status` case.
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

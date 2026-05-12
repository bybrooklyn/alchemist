import AppKit
import SwiftUI

struct JobsWorkspaceView: View {
    @Environment(AppModel.self) private var model
    @State private var saveViewName = ""
    @State private var showingSaveView = false
    @State private var enqueuePath = ""
    @State private var showingEnqueuePath = false
    @State private var confirmation: JobsConfirmation?
    @State private var searchTask: Task<Void, Never>?
    @FocusState private var searchFocused: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            header
            savedViews
            jobsToolbar

            if let message = model.jobs.actionMessage {
                JobNoticePanel(message: message, isError: model.jobs.lastError != nil)
            } else if let error = model.jobs.lastError {
                ErrorPanel(message: error.localizedDescription)
            }

            if !model.jobs.selectedIDs.isEmpty {
                selectionBar
            }

            HSplitView {
                jobsTable
                    .frame(minWidth: 620)

                if model.navigation.showingInspector {
                    JobInspectorView(
                        confirmation: $confirmation,
                        showingEnqueuePath: $showingEnqueuePath
                    )
                    .frame(minWidth: 340, idealWidth: 410, maxWidth: 520)
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)

            footer
        }
        .padding(28)
        .task {
            await model.jobs.loadSavedViews(apiClient: model.connection.apiClient)
            await model.jobs.refresh(apiClient: model.connection.apiClient)
        }
        .onChange(of: model.jobs.selectedIDs) { _, selection in
            if selection.count == 1, let id = selection.first {
                Task { await model.jobs.loadDetails(id: id, apiClient: model.connection.apiClient) }
            }
        }
        .onChange(of: model.jobs.searchText) { _, _ in
            model.jobs.prepareSearchRefresh()
            searchTask?.cancel()
            searchTask = Task {
                try? await Task.sleep(nanoseconds: 350_000_000)
                if !Task.isCancelled {
                    await model.jobs.refresh(apiClient: model.connection.apiClient)
                }
            }
        }
        .onDisappear {
            searchTask?.cancel()
        }
        .sheet(isPresented: $showingSaveView) {
            SaveJobViewSheet(name: $saveViewName) {
                let label = saveViewName
                saveViewName = ""
                showingSaveView = false
                Task { await model.jobs.saveCurrentView(label: label, apiClient: model.connection.apiClient) }
            } onCancel: {
                saveViewName = ""
                showingSaveView = false
            }
            .frame(width: 380)
            .padding(24)
        }
        .sheet(isPresented: $showingEnqueuePath) {
            EnqueuePathSheet(path: $enqueuePath) {
                let path = enqueuePath
                enqueuePath = ""
                showingEnqueuePath = false
                Task { await model.jobs.enqueuePath(path, apiClient: model.connection.apiClient) }
            } onCancel: {
                enqueuePath = ""
                showingEnqueuePath = false
            }
            .frame(width: 520)
            .padding(24)
        }
        .alert(
            confirmation?.title ?? "Confirm",
            isPresented: Binding(
                get: { confirmation != nil },
                set: { if !$0 { confirmation = nil } }
            )
        ) {
            Button(confirmation?.confirmLabel ?? "Confirm", role: confirmation?.role) {
                let action = confirmation?.action
                confirmation = nil
                action?()
            }
            Button("Cancel", role: .cancel) {
                confirmation = nil
            }
        } message: {
            Text(confirmation?.message ?? "")
        }
    }

    private var header: some View {
        HStack(alignment: .center) {
            VStack(alignment: .leading, spacing: 4) {
                Text("Jobs")
                    .font(.system(size: 34, weight: .bold, design: .rounded))
                Text("Exact queue control with native Mac speed")
                    .font(.title3)
                    .foregroundStyle(.secondary)
            }
            Spacer()
            HStack(spacing: 10) {
                jobMetric("\(model.jobs.activeCount)", "active", Color.heliosAccent)
                jobMetric("\(model.jobs.failedCount)", "failed", Color.heliosError)
                jobMetric("\(model.jobs.completedCount)", "completed", Color.heliosSuccess)
            }
        }
    }

    private func jobMetric(_ value: String, _ label: String, _ color: Color) -> some View {
        VStack(spacing: 2) {
            Text(value)
                .font(.title3.bold().monospacedDigit())
                .foregroundStyle(color)
            Text(label)
                .font(.caption.weight(.semibold))
                .foregroundStyle(.secondary)
        }
        .frame(minWidth: 86)
        .padding(.vertical, 10)
        .background(.ultraThinMaterial, in: RoundedRectangle(cornerRadius: 14, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 14, style: .continuous).stroke(color.opacity(0.18), lineWidth: 1))
    }

    private var savedViews: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 8) {
                ForEach(SavedJobView.builtIn) { view in
                    SavedViewChip(view: view, active: model.jobs.activeSavedViewID == view.id) {
                        model.jobs.applySavedView(view)
                    }
                }

                ForEach(model.jobs.savedViews) { view in
                    SavedViewChip(view: view, active: model.jobs.activeSavedViewID == view.id) {
                        model.jobs.applySavedView(view)
                    } onDelete: {
                        Task { await model.jobs.deleteSavedView(id: view.id, apiClient: model.connection.apiClient) }
                    }
                }

                Button {
                    showingSaveView = true
                } label: {
                    Label("Save View", systemImage: "plus")
                }
                .buttonStyle(.glass)
            }
            .padding(.vertical, 1)
        }
    }

    private var jobsToolbar: some View {
        VStack(spacing: 12) {
            jobTabBar
            jobFilterBar
        }
    }

    private var jobTabBar: some View {
        HStack(spacing: 8) {
            ForEach(JobTab.allCases) { tab in
                JobTabButton(tab: tab)
            }
        }
    }

    private var jobFilterBar: some View {
        HStack(spacing: 10) {
            searchField
            sortPicker
            sortDirectionButton

            Divider()
                .frame(height: 24)

            Button {
                showingEnqueuePath = true
            } label: {
                Label("Add File", systemImage: "plus")
            }
            .buttonStyle(.glass)

            Button {
                Task { await model.jobs.restartFailed(apiClient: model.connection.apiClient) }
            } label: {
                Label("Retry Failed", systemImage: "arrow.counterclockwise")
            }
            .buttonStyle(.glass)

            Button {
                confirmation = JobsConfirmation(
                    title: "Clear completed jobs",
                    message: "Remove all completed jobs from the visible queue while preserving historical stats?",
                    confirmLabel: "Clear",
                    role: .destructive
                ) {
                    Task { await model.jobs.clearCompleted(apiClient: model.connection.apiClient) }
                }
            } label: {
                Label("Clear Completed", systemImage: "archivebox")
            }
            .buttonStyle(.glass)

            Button {
                Task { await model.jobs.refresh(apiClient: model.connection.apiClient) }
            } label: {
                Label("Refresh", systemImage: "arrow.clockwise")
            }
            .labelStyle(.iconOnly)
            .buttonStyle(.glass)
            .disabled(model.jobs.isRefreshing)
            .help("Refresh jobs")
        }
        .padding(12)
        .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 18, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 18, style: .continuous).stroke(Color.white.opacity(0.10), lineWidth: 1))
    }

    private var jobSearchBinding: Binding<String> {
        Binding(
            get: { model.jobs.searchText },
            set: { model.jobs.searchText = $0 }
        )
    }

    private var jobSortFieldBinding: Binding<JobSortField> {
        Binding(
            get: { model.jobs.sortField },
            set: { model.jobs.setSortField($0) }
        )
    }

    private var searchField: some View {
        HStack(spacing: 8) {
            Label("Search", systemImage: "magnifyingglass")
                .labelStyle(.iconOnly)
                .foregroundStyle(.secondary)

            TextField("Search files", text: jobSearchBinding)
                .textFieldStyle(.plain)
                .focused($searchFocused)
                .onSubmit { Task { await model.jobs.refresh(apiClient: model.connection.apiClient) } }
        }
        .frame(minWidth: 180)
    }

    private var sortPicker: some View {
        Picker("Sort", selection: jobSortFieldBinding) {
            ForEach(JobSortField.allCases) { field in
                Text(field.label).tag(field)
            }
        }
        .labelsHidden()
        .frame(width: 168)
    }

    private var sortDirectionButton: some View {
        Button {
            model.jobs.toggleSortDirection()
        } label: {
            Label(model.jobs.sortDescending ? "Descending" : "Ascending", systemImage: model.jobs.sortDescending ? "arrow.down" : "arrow.up")
        }
        .labelStyle(.iconOnly)
        .buttonStyle(.glass)
        .help(model.jobs.sortDescending ? "Sort descending" : "Sort ascending")
    }

    private var selectionBar: some View {
        HStack {
            VStack(alignment: .leading, spacing: 2) {
                Text("\(model.jobs.selectedIDs.count) jobs selected")
                    .font(.subheadline.bold())
                    .foregroundStyle(Color.heliosAccent)
                if model.jobs.hasSelectedActiveJobs {
                    Text("Active jobs must be cancelled before restart or delete.")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
            }
            Spacer()
            Button {
                confirmation = JobsConfirmation(
                    title: "Restart jobs",
                    message: "Restart \(model.jobs.selectedIDs.count) selected jobs?",
                    confirmLabel: "Restart"
                ) {
                    Task { await model.jobs.performBatch(.restart, apiClient: model.connection.apiClient) }
                }
            } label: {
                Label("Restart", systemImage: "arrow.counterclockwise")
            }
            .buttonStyle(.glass)
            .disabled(model.jobs.hasSelectedActiveJobs)

            Button {
                confirmation = JobsConfirmation(
                    title: "Cancel jobs",
                    message: "Cancel \(model.jobs.selectedIDs.count) selected jobs?",
                    confirmLabel: "Cancel",
                    role: .destructive
                ) {
                    Task { await model.jobs.performBatch(.cancel, apiClient: model.connection.apiClient) }
                }
            } label: {
                Label("Cancel", systemImage: "xmark.circle")
            }
            .buttonStyle(.glass)

            Button {
                confirmation = JobsConfirmation(
                    title: "Delete jobs",
                    message: "Delete \(model.jobs.selectedIDs.count) selected jobs from history?",
                    confirmLabel: "Delete",
                    role: .destructive
                ) {
                    Task { await model.jobs.performBatch(.delete, apiClient: model.connection.apiClient) }
                }
            } label: {
                Label("Delete", systemImage: "trash")
            }
            .buttonStyle(.glass)
            .disabled(model.jobs.hasSelectedActiveJobs)

            Button {
                model.jobs.clearSelection()
            } label: {
                Label("Clear Selection", systemImage: "xmark")
            }
            .labelStyle(.iconOnly)
            .buttonStyle(.glass)
        }
        .padding(14)
        .background(model.theme.accent.color.opacity(0.12), in: RoundedRectangle(cornerRadius: 16, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 16, style: .continuous).stroke(model.theme.accent.color.opacity(0.24), lineWidth: 1))
    }

    private var allVisibleJobsSelected: Bool {
        let visibleIDs = Set(model.jobs.jobs.map(\.id))
        return !visibleIDs.isEmpty && visibleIDs.isSubset(of: model.jobs.selectedIDs)
    }

    private var jobsTable: some View {
        VStack(spacing: 0) {
            JobsTableHeader(allSelected: allVisibleJobsSelected)

            ScrollView {
                LazyVStack(spacing: 0) {
                    if model.jobs.jobs.isEmpty {
                        EmptyHeroCard(title: "No jobs found", detail: "Adjust filters or add media to populate the queue.")
                            .padding(18)
                    } else {
                        ForEach(model.jobs.jobs) { job in
                            JobTableRow(
                                job: job,
                                selected: model.jobs.selectedIDs.contains(job.id),
                                focused: model.jobs.focusedDetail?.job.id == job.id,
                                confirmation: $confirmation
                            )
                        }
                    }
                }
            }
        }
        .background(.thinMaterial, in: RoundedRectangle(cornerRadius: 18, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 18, style: .continuous).stroke(Color.white.opacity(0.08), lineWidth: 1))
    }

    private var footer: some View {
        HStack {
            Text("Showing \(model.jobs.jobs.count) jobs · Limit 50")
                .font(.caption)
                .foregroundStyle(.secondary)
            Spacer()
            HStack(spacing: 8) {
                Button {
                    model.jobs.movePage(by: -1)
                } label: {
                    Label("Previous Page", systemImage: "chevron.left")
                }
                .labelStyle(.iconOnly)
                .buttonStyle(.glass)
                .disabled(!model.jobs.canGoToPreviousPage)

                Text("Page \(model.jobs.page)")
                    .font(.caption.monospacedDigit())
                    .foregroundStyle(.secondary)

                Button {
                    model.jobs.movePage(by: 1)
                } label: {
                    Label("Next Page", systemImage: "chevron.right")
                }
                .labelStyle(.iconOnly)
                .buttonStyle(.glass)
                .disabled(!model.jobs.canGoToNextPage)
            }
            Button(role: .destructive) {
                confirmation = JobsConfirmation(
                    title: "Clear all job history",
                    message: "Purge matching job history from Alchemist? This is broader than clearing completed jobs.",
                    confirmLabel: "Clear History",
                    role: .destructive
                ) {
                    Task { await model.jobs.clearHistory(apiClient: model.connection.apiClient) }
                }
            } label: {
                Label("Clear History", systemImage: "trash")
            }
            .buttonStyle(.glass)
        }
    }
}

import SwiftUI

struct JobInspectorView: View {
    @Environment(AppModel.self) private var model
    @Binding var confirmation: JobsConfirmation?
    @Binding var showingEnqueuePath: Bool

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 18) {
                if model.jobs.detailLoading {
                    ProgressView("Loading job details...")
                        .frame(maxWidth: .infinity, alignment: .center)
                        .padding(32)
                } else if let detail = model.jobs.focusedDetail {
                    inspectorHeader(detail)
                    queuedBanner(detail)
                    explanationBlock("Decision", detail.decisionExplanation ?? detail.job.decisionExplanation)
                    explanationBlock("Failure", detail.failureExplanation)
                    metadataBlock(detail)
                    encodeResultsBlock(detail)
                    attemptHistoryBlock(detail)
                    logsBlock(detail)
                } else {
                    EmptyHeroCard(title: "Select a job", detail: "Choose a row to inspect media metadata, encode results, attempt history, and logs.")
                    Button {
                        showingEnqueuePath = true
                    } label: {
                        Label("Add File", systemImage: "plus")
                    }
                    .buttonStyle(.glassProminent)
                    .tint(model.theme.accent.color)
                }
            }
            .padding(18)
        }
        .background(.ultraThinMaterial, in: RoundedRectangle(cornerRadius: 18, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 18, style: .continuous).stroke(Color.white.opacity(0.10), lineWidth: 1))
    }

    private func inspectorHeader(_ detail: JobDetail) -> some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack(alignment: .top) {
                VStack(alignment: .leading, spacing: 4) {
                    StatusPill(status: detail.job.status)
                    Text(detail.job.fileName)
                        .font(.title3.bold())
                        .lineLimit(2)
                    Text(detail.job.inputPath)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                        .lineLimit(2)
                }
                Spacer()
                Button {
                    model.jobs.closeDetails()
                } label: {
                    Image(systemName: "xmark")
                }
                .buttonStyle(.glass)
            }

            HStack {
                Button {
                    confirmation = JobsConfirmation(title: "Retry job", message: "Retry job #\(detail.job.id)?", confirmLabel: "Retry") {
                        Task { await model.jobs.performAction(id: detail.job.id, action: .restart, apiClient: model.connection.apiClient) }
                    }
                } label: {
                    Label("Retry", systemImage: "arrow.counterclockwise")
                }
                .buttonStyle(.glass)
                .disabled(!["failed", "cancelled"].contains(detail.job.status))

                Button(role: .destructive) {
                    confirmation = JobsConfirmation(title: "Cancel job", message: "Stop job #\(detail.job.id) immediately?", confirmLabel: "Cancel", role: .destructive) {
                        Task { await model.jobs.performAction(id: detail.job.id, action: .cancel, apiClient: model.connection.apiClient) }
                    }
                } label: {
                    Label("Cancel", systemImage: "xmark.circle")
                }
                .buttonStyle(.glass)
                .disabled(!(detail.job.isActive || detail.job.status == "queued"))

                Button(role: .destructive) {
                    confirmation = JobsConfirmation(title: "Delete job", message: "Delete job #\(detail.job.id) from history?", confirmLabel: "Delete", role: .destructive) {
                        Task { await model.jobs.performAction(id: detail.job.id, action: .delete, apiClient: model.connection.apiClient) }
                    }
                } label: {
                    Label("Delete", systemImage: "trash")
                }
                .buttonStyle(.glass)
                .disabled(detail.job.isActive)
            }

            HStack {
                Text("Priority \(detail.job.priority ?? 0)")
                    .font(.caption.bold())
                Spacer()
                Button("-10") {
                    Task { await model.jobs.updatePriority(detail.job, priority: (detail.job.priority ?? 0) - 10, label: "Priority lowered", apiClient: model.connection.apiClient) }
                }
                Button("Reset") {
                    Task { await model.jobs.updatePriority(detail.job, priority: 0, label: "Priority reset", apiClient: model.connection.apiClient) }
                }
                Button("+10") {
                    Task { await model.jobs.updatePriority(detail.job, priority: (detail.job.priority ?? 0) + 10, label: "Priority boosted", apiClient: model.connection.apiClient) }
                }
            }
            .buttonStyle(.glass)
        }
    }

    @ViewBuilder
    private func queuedBanner(_ detail: JobDetail) -> some View {
        if detail.job.status == "queued" {
            VStack(alignment: .leading, spacing: 8) {
                if let position = detail.queuePosition {
                    Text("Queue position: #\(position)")
                        .font(.headline)
                }
                if let processor = model.jobs.focusedProcessorStatus {
                    Text(processor.message)
                        .font(.subheadline)
                        .foregroundStyle(processor.blockedReason == nil ? Color.heliosSuccess : Color.heliosWarning)
                    Text("\(processor.activeJobs) / \(processor.concurrentLimit) workers active")
                        .font(.caption.monospacedDigit())
                        .foregroundStyle(.secondary)
                }
            }
            .padding(14)
            .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 14, style: .continuous))
        }
    }

    @ViewBuilder
    private func explanationBlock(_ title: String, _ explanation: ExplanationPayload?) -> some View {
        if let explanation {
            VStack(alignment: .leading, spacing: 8) {
                Label(title, systemImage: title == "Failure" ? "exclamationmark.triangle" : "lightbulb")
                    .font(.headline)
                Text(explanation.summary)
                    .font(.subheadline.bold())
                Text(explanation.detail)
                    .font(.caption)
                    .foregroundStyle(.secondary)
                if let guidance = explanation.operatorGuidance {
                    Text(guidance)
                        .font(.caption)
                        .padding(10)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .background(Color.heliosAccent.opacity(0.10), in: RoundedRectangle(cornerRadius: 10, style: .continuous))
                }
                if !explanation.measured.isEmpty {
                    VStack(alignment: .leading, spacing: 4) {
                        ForEach(explanation.measured.keys.sorted(), id: \.self) { key in
                            HStack {
                                Text(key.replacingOccurrences(of: "_", with: " ").capitalized)
                                    .foregroundStyle(.secondary)
                                Spacer()
                                Text(explanation.measured[key]?.displayString ?? "-")
                                    .font(.caption.monospacedDigit())
                            }
                        }
                    }
                    .font(.caption)
                }
            }
            .padding(14)
            .background(title == "Failure" ? Color.heliosError.opacity(0.10) : Color.heliosAccent.opacity(0.08), in: RoundedRectangle(cornerRadius: 14, style: .continuous))
            .overlay(RoundedRectangle(cornerRadius: 14, style: .continuous).stroke((title == "Failure" ? Color.heliosError : Color.heliosAccent).opacity(0.22), lineWidth: 1))
        }
    }

    private func metadataBlock(_ detail: JobDetail) -> some View {
        VStack(alignment: .leading, spacing: 12) {
            Label("Media Metadata", systemImage: "film")
                .font(.headline)
            if let metadata = detail.metadata {
                detailGrid([
                    ("Codec", metadata.codecName ?? "Unknown"),
                    ("Resolution", resolution(metadata)),
                    ("Duration", AlchemistFormatter.duration(metadata.durationSecs)),
                    ("FPS", metadata.fps.map { $0.formatted(.number.precision(.fractionLength(2))) } ?? "-"),
                    ("Container", metadata.container?.uppercased() ?? "-"),
                    ("Audio", audio(metadata)),
                    ("Dynamic Range", metadata.dynamicRange ?? "-"),
                    ("Input Size", metadata.sizeBytes.map(AlchemistFormatter.bytes) ?? "-"),
                ])
            } else {
                Text(JobDetailEmptyState.forStatus(detail.job.status).detail)
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
        }
        .padding(14)
        .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 14, style: .continuous))
    }

    @ViewBuilder
    private func encodeResultsBlock(_ detail: JobDetail) -> some View {
        if let stats = detail.encodeStats {
            VStack(alignment: .leading, spacing: 12) {
                Label("Encode Results", systemImage: "bolt")
                    .font(.headline)
                detailGrid([
                    ("Input", AlchemistFormatter.bytes(stats.inputSizeBytes)),
                    ("Output", AlchemistFormatter.bytes(stats.outputSizeBytes)),
                    ("Reduction", "\(max(0, (1 - Double(stats.outputSizeBytes) / Double(max(1, stats.inputSizeBytes))) * 100).formatted(.number.precision(.fractionLength(1))))%"),
                    ("Speed", "\(stats.encodeSpeed.formatted(.number.precision(.fractionLength(2))))x"),
                    ("Bitrate", "\(stats.avgBitrateKbps.formatted(.number.precision(.fractionLength(0)))) kbps"),
                    ("VMAF", stats.vmafScore.map { $0.formatted(.number.precision(.fractionLength(1))) } ?? "-"),
                ])
            }
            .padding(14)
            .background(Color.heliosSuccess.opacity(0.08), in: RoundedRectangle(cornerRadius: 14, style: .continuous))
        }
    }

    @ViewBuilder
    private func attemptHistoryBlock(_ detail: JobDetail) -> some View {
        if !detail.encodeAttempts.isEmpty || !detail.encodeHistoryRuns.isEmpty {
            VStack(alignment: .leading, spacing: 10) {
                Label("Attempt History", systemImage: "clock.arrow.circlepath")
                    .font(.headline)
                ForEach(detail.encodeHistoryRuns) { run in
                    VStack(alignment: .leading, spacing: 4) {
                        HStack {
                            Text("Run \(run.runNumber)")
                                .font(.subheadline.bold())
                            if run.current {
                                Text("Current")
                                    .font(.caption2.bold())
                                    .padding(.horizontal, 6)
                                    .padding(.vertical, 2)
                                    .background(Color.heliosAccent.opacity(0.15), in: Capsule())
                            }
                            Spacer()
                            StatusPill(status: run.outcome)
                        }
                        if let failure = run.failureSummary {
                            Text(failure)
                                .font(.caption)
                                .foregroundStyle(Color.heliosError)
                        }
                    }
                    .padding(10)
                    .background(.thinMaterial, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
                }
                if detail.encodeHistoryRuns.isEmpty {
                    ForEach(detail.encodeAttempts) { attempt in
                        HStack {
                            Text("#\(attempt.attemptNumber)")
                                .font(.caption.bold())
                            Text(attempt.outcome.capitalized)
                            Spacer()
                            Text(AlchemistFormatter.shortDate(attempt.finishedAt))
                                .font(.caption.monospacedDigit())
                                .foregroundStyle(.secondary)
                        }
                    }
                }
            }
            .padding(14)
            .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 14, style: .continuous))
        }
    }

    @ViewBuilder
    private func logsBlock(_ detail: JobDetail) -> some View {
        if !detail.jobLogs.isEmpty {
            VStack(alignment: .leading, spacing: 10) {
                Label("Logs", systemImage: "terminal")
                    .font(.headline)
                ForEach(detail.jobLogs.suffix(20)) { log in
                    VStack(alignment: .leading, spacing: 3) {
                        HStack {
                            Text(log.level.uppercased())
                                .font(.caption2.bold())
                                .foregroundStyle(log.level.lowercased().contains("error") ? Color.heliosError : Color.heliosWarning)
                            Spacer()
                            Text(AlchemistFormatter.shortDate(log.createdAt))
                                .font(.caption2.monospacedDigit())
                                .foregroundStyle(.secondary)
                        }
                        Text(log.message)
                            .font(.caption.monospaced())
                            .foregroundStyle(.secondary)
                            .textSelection(.enabled)
                    }
                    .padding(9)
                    .background(Color.black.opacity(0.14), in: RoundedRectangle(cornerRadius: 8, style: .continuous))
                }
            }
            .padding(14)
            .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 14, style: .continuous))
        }
    }

    private func detailGrid(_ rows: [(String, String)]) -> some View {
        Grid(alignment: .leading, horizontalSpacing: 14, verticalSpacing: 7) {
            ForEach(rows, id: \.0) { label, value in
                GridRow {
                    Text(label)
                        .foregroundStyle(.secondary)
                    Text(value)
                        .font(.caption.monospacedDigit())
                        .frame(maxWidth: .infinity, alignment: .trailing)
                }
            }
        }
        .font(.caption)
    }

    private func resolution(_ metadata: JobMetadata) -> String {
        if let width = metadata.width, let height = metadata.height {
            return "\(width)x\(height)"
        }
        return "-"
    }

    private func audio(_ metadata: JobMetadata) -> String {
        let codec = metadata.audioCodec ?? "N/A"
        let channels = metadata.audioChannels ?? 0
        return "\(codec) (\(channels)ch)"
    }
}

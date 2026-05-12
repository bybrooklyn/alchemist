import SwiftUI

struct JobTableRow: View {
    @Environment(AppModel.self) private var model
    let job: Job
    let selected: Bool
    let focused: Bool
    @Binding var confirmation: JobsConfirmation?

    var body: some View {
        HStack(spacing: 12) {
            Button {
                model.jobs.toggleSelection(id: job.id)
            } label: {
                Image(systemName: selected ? "checkmark.square.fill" : "square")
                    .foregroundStyle(selected ? model.theme.accent.color : .secondary)
            }
            .buttonStyle(.plain)
            .frame(width: 26)

            VStack(alignment: .leading, spacing: 3) {
                Text(job.fileName)
                    .font(.headline)
                    .lineLimit(1)
                HStack(spacing: 8) {
                    Text(job.inputPath)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
                    Text("P\(job.priority ?? 0)")
                        .font(.caption2.bold())
                        .padding(.horizontal, 6)
                        .padding(.vertical, 2)
                        .background(.thinMaterial, in: Capsule())
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)

            StatusPill(status: job.status)
                .frame(width: 118, alignment: .leading)

            VStack(alignment: .leading, spacing: 4) {
                if job.isActive {
                    ProgressView(value: job.progressFraction)
                        .tint(model.theme.accent.color)
                    HStack {
                        Text("\((job.progress ?? 0).formatted(.number.precision(.fractionLength(1))))%")
                        if let encoder = job.encoder {
                            Text(encoder)
                        }
                    }
                    .font(.caption2.monospaced())
                    .foregroundStyle(.secondary)
                } else if let vmaf = job.vmafScore {
                    Text("VMAF \(vmaf.formatted(.number.precision(.fractionLength(1))))")
                        .font(.caption.monospacedDigit())
                        .foregroundStyle(.secondary)
                } else {
                    Text("-")
                        .font(.caption)
                        .foregroundStyle(.tertiary)
                }
            }
            .frame(width: 126, alignment: .leading)

            Text(AlchemistFormatter.shortDate(job.updatedAt))
                .font(.caption.monospacedDigit())
                .foregroundStyle(.secondary)
                .frame(width: 138, alignment: .leading)

            Menu {
                Button("View Details") {
                    Task { await model.jobs.loadDetails(id: job.id, apiClient: model.connection.apiClient) }
                }
                Button("Reveal in Finder") {
                    FinderHelpers.reveal(path: job.outputPath ?? job.inputPath)
                }
                Button("Boost priority (+10)") {
                    Task { await model.jobs.updatePriority(job, priority: (job.priority ?? 0) + 10, label: "Priority boosted", apiClient: model.connection.apiClient) }
                }
                Button("Lower priority (-10)") {
                    Task { await model.jobs.updatePriority(job, priority: (job.priority ?? 0) - 10, label: "Priority lowered", apiClient: model.connection.apiClient) }
                }
                Button("Reset priority") {
                    Task { await model.jobs.updatePriority(job, priority: 0, label: "Priority reset", apiClient: model.connection.apiClient) }
                }
                if ["failed", "cancelled"].contains(job.status) {
                    Button("Retry") {
                        confirmation = JobsConfirmation(title: "Retry job", message: "Retry job #\(job.id)?", confirmLabel: "Retry") {
                            Task { await model.jobs.performAction(id: job.id, action: .restart, apiClient: model.connection.apiClient) }
                        }
                    }
                }
                if job.isActive || job.status == "queued" {
                    Button("Stop / Cancel", role: .destructive) {
                        confirmation = JobsConfirmation(title: "Cancel job", message: "Stop job #\(job.id) immediately?", confirmLabel: "Cancel", role: .destructive) {
                            Task { await model.jobs.performAction(id: job.id, action: .cancel, apiClient: model.connection.apiClient) }
                        }
                    }
                }
                if !job.isActive {
                    Button("Delete", role: .destructive) {
                        confirmation = JobsConfirmation(title: "Delete job", message: "Delete job #\(job.id) from history?", confirmLabel: "Delete", role: .destructive) {
                            Task { await model.jobs.performAction(id: job.id, action: .delete, apiClient: model.connection.apiClient) }
                        }
                    }
                }
            } label: {
                Image(systemName: "ellipsis.circle")
            }
            .menuStyle(.button)
            .buttonStyle(.plain)
            .frame(width: 34)
        }
        .padding(.horizontal, 14)
        .padding(.vertical, 12)
        .background(rowBackground)
        .contentShape(Rectangle())
        .onTapGesture {
            model.jobs.selectedIDs = [job.id]
            Task { await model.jobs.loadDetails(id: job.id, apiClient: model.connection.apiClient) }
        }
        .contextMenu {
            Button("View Details") {
                Task { await model.jobs.loadDetails(id: job.id, apiClient: model.connection.apiClient) }
            }
            Button("Reveal in Finder") {
                FinderHelpers.reveal(path: job.outputPath ?? job.inputPath)
            }
            Button("Boost priority (+10)") {
                Task { await model.jobs.updatePriority(job, priority: (job.priority ?? 0) + 10, label: "Priority boosted", apiClient: model.connection.apiClient) }
            }
            Button("Lower priority (-10)") {
                Task { await model.jobs.updatePriority(job, priority: (job.priority ?? 0) - 10, label: "Priority lowered", apiClient: model.connection.apiClient) }
            }
            if job.isActive || job.status == "queued" {
                Button("Stop / Cancel", role: .destructive) {
                    confirmation = JobsConfirmation(title: "Cancel job", message: "Stop job #\(job.id) immediately?", confirmLabel: "Cancel", role: .destructive) {
                        Task { await model.jobs.performAction(id: job.id, action: .cancel, apiClient: model.connection.apiClient) }
                    }
                }
            }
            if !job.isActive {
                Button("Delete", role: .destructive) {
                    confirmation = JobsConfirmation(title: "Delete job", message: "Delete job #\(job.id) from history?", confirmLabel: "Delete", role: .destructive) {
                        Task { await model.jobs.performAction(id: job.id, action: .delete, apiClient: model.connection.apiClient) }
                    }
                }
            }
        }
    }

    private var rowBackground: some ShapeStyle {
        if focused {
            return AnyShapeStyle(model.theme.accent.color.opacity(0.14))
        }
        if selected {
            return AnyShapeStyle(Color.heliosAccent.opacity(0.08))
        }
        return AnyShapeStyle(Color.clear)
    }
}

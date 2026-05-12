import SwiftUI

public struct MenuBarStatusView: View {
    @Environment(AppModel.self) private var model

    public init() {}

    public var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            HStack {
                VStack(alignment: .leading, spacing: 2) {
                    Text("Alchemist")
                        .font(.headline)
                        .foregroundStyle(.primary)
                    Text(model.engine.status.status.capitalized)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                Spacer()
                StatusPill(status: model.engine.status.status)
            }

            Divider().background(.white.opacity(0.1))

            if let activeJob = model.jobs.jobs.first(where: \.isActive) {
                VStack(alignment: .leading, spacing: 8) {
                    Text(activeJob.fileName)
                        .font(.caption.bold())
                        .foregroundStyle(.primary)
                        .lineLimit(1)
                    ProgressView(value: activeJob.progressFraction)
                        .tint(Color.heliosAccent)
                        .controlSize(.small)
                }
            } else {
                Text("No active encode")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }

            Divider().background(.white.opacity(0.1))

            HStack {
                Button {
                    Task { await model.refreshAll() }
                } label: {
                    Label("Refresh", systemImage: "arrow.clockwise")
                }
                .buttonStyle(.glass)

                if model.engine.isPaused {
                    Button {
                        Task { await model.resumeQueue() }
                    } label: {
                        Label("Start", systemImage: "play.fill")
                    }
                    .buttonStyle(.glassProminent)
                    .tint(Color.heliosAccent)
                } else {
                    Button {
                        Task { await model.pauseQueue() }
                    } label: {
                        Label("Pause", systemImage: "pause.fill")
                    }
                    .buttonStyle(.glass)
                }
            }
        }
        .padding(16)
        .frame(width: 320)
        .background(.ultraThinMaterial)
    }
}

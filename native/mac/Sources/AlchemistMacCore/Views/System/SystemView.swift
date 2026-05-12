import SwiftUI

struct SystemView: View {
    @Environment(AppModel.self) private var model

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 32) {
                VStack(alignment: .leading, spacing: 4) {
                    Text("System")
                        .font(.system(size: 34, weight: .bold, design: .rounded))
                    Text("Hardware monitoring and daemon control")
                        .font(.title3)
                        .foregroundStyle(.secondary)
                }

                SystemHealthMonitor()

                VStack(alignment: .leading, spacing: 16) {
                    Text("Daemon Information")
                        .font(.headline)
                        .foregroundStyle(.secondary)

                    VStack(spacing: 0) {
                        systemRow(label: "Status", value: model.daemon.status.capitalized)
                        Divider().background(.white.opacity(0.05)).padding(.vertical, 8)
                        systemRow(label: "Version", value: model.dashboard.systemInfo?.version ?? "Unknown")
                        Divider().background(.white.opacity(0.05)).padding(.vertical, 8)
                        systemRow(label: "OS Version", value: model.dashboard.systemInfo?.osVersion ?? "Unknown")
                        Divider().background(.white.opacity(0.05)).padding(.vertical, 8)
                        systemRow(label: "FFmpeg", value: model.dashboard.systemInfo?.ffmpegVersion ?? "Unknown")
                        Divider().background(.white.opacity(0.05)).padding(.vertical, 8)
                        systemRow(label: "Docker", value: model.dashboard.systemInfo?.isDocker == true ? "Yes" : "No")
                    }
                    .padding(20)
                    .background(.ultraThinMaterial, in: RoundedRectangle(cornerRadius: 16, style: .continuous))
                    .overlay(RoundedRectangle(cornerRadius: 16, style: .continuous).stroke(Color.white.opacity(0.05), lineWidth: 1))
                }

                VStack(alignment: .leading, spacing: 16) {
                    Text("Host Paths")
                        .font(.headline)
                        .foregroundStyle(.secondary)

                    VStack(alignment: .leading, spacing: 16) {
                        pathRow(label: "Root", value: AlchemistSupportPaths.root.path)
                        pathRow(label: "Config", value: AlchemistSupportPaths.config.path)
                        pathRow(label: "Database", value: AlchemistSupportPaths.database.path)
                    }
                    .padding(20)
                    .background(.ultraThinMaterial, in: RoundedRectangle(cornerRadius: 16, style: .continuous))
                    .overlay(RoundedRectangle(cornerRadius: 16, style: .continuous).stroke(Color.white.opacity(0.05), lineWidth: 1))
                }
            }
            .padding(32)
        }
    }

    private func systemRow(label: String, value: String) -> some View {
        HStack {
            Text(label)
                .font(.subheadline)
                .foregroundStyle(.secondary)
            Spacer()
            Text(value)
                .font(.subheadline.monospacedDigit().bold())
                .foregroundStyle(.primary)
        }
    }

    private func pathRow(label: String, value: String) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(label)
                .font(.caption.bold())
                .foregroundStyle(.secondary)
            Text(value)
                .font(.system(size: 11, weight: .medium, design: .monospaced))
                .foregroundStyle(.primary)
                .lineLimit(1)
                .help(value)
        }
    }
}

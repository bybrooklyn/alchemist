import AppKit
import SwiftUI

struct SystemView: View {
    @Environment(AppModel.self) private var model
    @State private var selftestResult: SelftestResponse?
    @State private var isRunningSelftest = false

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

                daemonSection
                hardwareSection
                resourceSection
                selftestSection
                backupSection
                hostPathsSection
            }
            .padding(32)
        }
    }

    private var daemonSection: some View {
        VStack(alignment: .leading, spacing: 16) {
            Text("Daemon")
                .font(.headline)
                .foregroundStyle(.secondary)

            VStack(spacing: 0) {
                HStack {
                    Text("Status")
                        .font(.subheadline)
                        .foregroundStyle(.secondary)
                    Spacer()
                    HStack(spacing: 6) {
                        Circle()
                            .fill(daemonStatusColor)
                            .frame(width: 8, height: 8)
                        Text(model.daemon.status)
                            .font(.subheadline.monospacedDigit().bold())
                            .foregroundStyle(.primary)
                    }
                }

                if model.daemon.lastError != nil {
                    Divider().background(.white.opacity(0.05)).padding(.vertical, 8)
                    VStack(alignment: .leading, spacing: 4) {
                        Text("Error")
                            .font(.caption.bold())
                            .foregroundStyle(Color.heliosError)
                        Text(model.daemon.lastError ?? "")
                            .font(.caption.monospaced())
                            .foregroundStyle(.secondary)
                            .textSelection(.enabled)
                    }
                }

                Divider().background(.white.opacity(0.05)).padding(.vertical, 8)

                HStack {
                    if model.daemon.status.contains("Running") {
                        Button {
                            model.daemon.stopBundledDaemon()
                        } label: {
                            Label("Stop Daemon", systemImage: "stop.fill")
                        }
                        .buttonStyle(.glass)
                    } else {
                        Button {
                            model.daemon.startBundledDaemon()
                        } label: {
                            Label("Start Daemon", systemImage: "play.fill")
                        }
                        .buttonStyle(.glassProminent)
                        .tint(model.theme.accent.color)
                    }

                    Button {
                        Task { await model.bootstrap() }
                    } label: {
                        Label("Reconnect", systemImage: "arrow.clockwise")
                    }
                    .buttonStyle(.glass)
                }
            }
            .padding(20)
            .background(.ultraThinMaterial, in: RoundedRectangle(cornerRadius: 16, style: .continuous))
            .overlay(RoundedRectangle(cornerRadius: 16, style: .continuous).stroke(Color.white.opacity(0.05), lineWidth: 1))
        }
    }

    private var daemonStatusColor: Color {
        let status = model.daemon.status.lowercased()
        if status.contains("running") { return Color.heliosSuccess }
        if status.contains("starting") { return Color.heliosWarning }
        return Color.heliosError
    }

    private var hardwareSection: some View {
        VStack(alignment: .leading, spacing: 16) {
            Text("Hardware")
                .font(.headline)
                .foregroundStyle(.secondary)

            VStack(spacing: 0) {
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
    }

    private var resourceSection: some View {
        VStack(alignment: .leading, spacing: 16) {
            Text("Resources")
                .font(.headline)
                .foregroundStyle(.secondary)

            VStack(spacing: 12) {
                ResourceBar(label: "CPU", value: model.dashboard.resources.cpuPercent, max: 100, unit: "%")
                ResourceBar(label: "Memory", value: model.dashboard.resources.memoryPercent, max: 100, unit: "%")
                if let gpu = model.dashboard.resources.gpuUtilization {
                    ResourceBar(label: "GPU", value: gpu, max: 100, unit: "%")
                }
                HStack {
                    Text("Uptime")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                    Spacer()
                    Text(model.dashboard.resources.uptimeDescription)
                        .font(.caption.monospacedDigit())
                }
            }
            .padding(20)
            .background(.ultraThinMaterial, in: RoundedRectangle(cornerRadius: 16, style: .continuous))
            .overlay(RoundedRectangle(cornerRadius: 16, style: .continuous).stroke(Color.white.opacity(0.05), lineWidth: 1))
        }
    }

    private var selftestSection: some View {
        VStack(alignment: .leading, spacing: 16) {
            Text("Selftest")
                .font(.headline)
                .foregroundStyle(.secondary)

            VStack(alignment: .leading, spacing: 12) {
                Button {
                    Task {
                        isRunningSelftest = true
                        defer { isRunningSelftest = false }
                        do {
                            selftestResult = try await model.connection.apiClient?.runSelftest()
                            if selftestResult?.success == true {
                                model.showToast(.success, "Selftest passed")
                            } else {
                                model.showToast(.error, "Selftest failed")
                            }
                        } catch {
                            model.showToast(.error, error.localizedDescription)
                        }
                    }
                } label: {
                    HStack {
                        if isRunningSelftest {
                            ProgressView()
                                .controlSize(.small)
                        }
                        Label("Run Selftest", systemImage: "stethoscope")
                    }
                }
                .buttonStyle(.glass)
                .disabled(isRunningSelftest)

                if let result = selftestResult {
                    ForEach(result.stages, id: \.name) { stage in
                        HStack {
                            Image(systemName: stage.success ? "checkmark.circle.fill" : "xmark.circle.fill")
                                .foregroundStyle(stage.success ? Color.heliosSuccess : Color.heliosError)
                            Text(stage.name)
                                .font(.caption.bold())
                            Spacer()
                            Text("\(stage.durationMs)ms")
                                .font(.caption2.monospaced())
                                .foregroundStyle(.secondary)
                        }
                        if !stage.message.isEmpty {
                            Text(stage.message)
                                .font(.caption2)
                                .foregroundStyle(.secondary)
                                .padding(.leading, 22)
                        }
                    }
                }
            }
            .padding(20)
            .background(.ultraThinMaterial, in: RoundedRectangle(cornerRadius: 16, style: .continuous))
            .overlay(RoundedRectangle(cornerRadius: 16, style: .continuous).stroke(Color.white.opacity(0.05), lineWidth: 1))
        }
    }

    private var backupSection: some View {
        VStack(alignment: .leading, spacing: 16) {
            Text("Backup")
                .font(.headline)
                .foregroundStyle(.secondary)

            VStack(alignment: .leading, spacing: 12) {
                Button {
                    Task {
                        do {
                            let data = try await model.connection.apiClient?.backupDatabase()
                            guard let data else { return }
                            let panel = NSSavePanel()
                            panel.nameFieldStringValue = "alchemist-backup.db.gz"
                            panel.allowedContentTypes = [.gzip]
                            if panel.runModal() == .OK, let url = panel.url {
                                try data.write(to: url)
                                model.showToast(.success, "Backup saved")
                            }
                        } catch {
                            model.showToast(.error, error.localizedDescription)
                        }
                    }
                } label: {
                    Label("Backup Database", systemImage: "arrow.down.doc.fill")
                }
                .buttonStyle(.glass)

                Text("Downloads a consistent, gzip-compressed SQLite snapshot safe to run while encodes are in flight.")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
            .padding(20)
            .background(.ultraThinMaterial, in: RoundedRectangle(cornerRadius: 16, style: .continuous))
            .overlay(RoundedRectangle(cornerRadius: 16, style: .continuous).stroke(Color.white.opacity(0.05), lineWidth: 1))
        }
    }

    private var hostPathsSection: some View {
        VStack(alignment: .leading, spacing: 16) {
            Text("Host Paths")
                .font(.headline)
                .foregroundStyle(.secondary)

            VStack(alignment: .leading, spacing: 16) {
                pathRow(label: "Root", value: AlchemistSupportPaths.root.path)
                pathRow(label: "Config", value: AlchemistSupportPaths.config.path)
                pathRow(label: "Database", value: AlchemistSupportPaths.database.path)
                pathRow(label: "Daemon Log", value: AlchemistSupportPaths.daemonLog.path)
            }
            .padding(20)
            .background(.ultraThinMaterial, in: RoundedRectangle(cornerRadius: 16, style: .continuous))
            .overlay(RoundedRectangle(cornerRadius: 16, style: .continuous).stroke(Color.white.opacity(0.05), lineWidth: 1))
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
                .textSelection(.enabled)
        }
    }
}

private struct ResourceBar: View {
    let label: String
    let value: Float
    let max: Float
    let unit: String

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack {
                Text(label)
                    .font(.caption)
                    .foregroundStyle(.secondary)
                Spacer()
                Text("\(Int(value))\(unit)")
                    .font(.caption.monospacedDigit())
            }
            ProgressView(value: Double(value), total: Double(max))
                .tint(Color.heliosAccent)
        }
    }
}

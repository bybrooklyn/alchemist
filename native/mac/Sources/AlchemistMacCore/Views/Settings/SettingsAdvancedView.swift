import SwiftUI

struct SettingsAdvancedView: View {
    @Environment(AppModel.self) private var model
    @State private var selftestResult: SelftestResponse?
    @State private var isRunningSelftest = false

    var body: some View {
        Form {
            Section("System") {
                LabeledContent("Telemetry") {
                    Text(model.dashboard.settingsBundle?.settings.system.enableTelemetry == true ? "Enabled" : "Disabled")
                }
                LabeledContent("Monitoring Poll Interval") {
                    Text("\(Int(model.dashboard.settingsBundle?.settings.system.monitoringPollInterval ?? 2))s")
                        .monospaced()
                }
                LabeledContent("Upload Limit") {
                    Text("\(model.dashboard.settingsBundle?.settings.system.conversionUploadLimitGB ?? 8) GB")
                        .monospaced()
                }
            }

            Section("Backup") {
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
                                model.showToast(.success, "Backup saved to \(url.lastPathComponent)")
                            }
                        } catch {
                            model.showToast(.error, error.localizedDescription)
                        }
                    }
                } label: {
                    Label("Backup Database", systemImage: "arrow.down.doc.fill")
                }

                Text("Downloads a consistent, gzip-compressed SQLite snapshot safe to run while encodes are in flight.")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }

            Section("Selftest") {
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
                .disabled(isRunningSelftest)

                if let result = selftestResult {
                    VStack(alignment: .leading, spacing: 8) {
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
                    .padding(10)
                    .background(.thinMaterial, in: RoundedRectangle(cornerRadius: 10))
                }
            }
        }
        .formStyle(.grouped)
        .padding(20)
    }
}

import AppKit

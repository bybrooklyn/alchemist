import SwiftUI

struct SettingsNotificationsView: View {
    @Environment(AppModel.self) private var model

    var body: some View {
        Form {
            Section("Notification Targets") {
                if model.dashboard.settingsBundle?.settings.notifications.targets.isEmpty ?? true {
                    Text("No notification targets configured.")
                        .foregroundStyle(.secondary)
                    Text("Set up Discord, Telegram, Gotify, ntfy, email, or webhook notifications to get alerted when jobs complete or fail.")
                        .font(.caption)
                        .foregroundStyle(.tertiary)
                } else {
                    ForEach(model.dashboard.settingsBundle?.settings.notifications.targets ?? [], id: \.localID) { target in
                        HStack {
                            VStack(alignment: .leading, spacing: 2) {
                                Text(target.name)
                                    .font(.body)
                                HStack(spacing: 6) {
                                    Text(target.targetType)
                                        .font(.caption2)
                                        .padding(.horizontal, 6)
                                        .padding(.vertical, 2)
                                        .background(.thinMaterial, in: Capsule())
                                    Text(target.enabled ? "Enabled" : "Disabled")
                                        .font(.caption2)
                                        .foregroundStyle(target.enabled ? Color.heliosSuccess : .secondary)
                                }
                            }
                            Spacer()
                        }
                    }
                }
            }

            Section("Events") {
                Text("Configure which events trigger notifications in the web UI or via the API.")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                LabeledContent("Available Events") {
                    VStack(alignment: .trailing, spacing: 2) {
                        Text("encode.queued")
                        Text("encode.started")
                        Text("encode.completed")
                        Text("encode.failed")
                        Text("scan.completed")
                        Text("engine.idle")
                        Text("daily.summary")
                    }
                    .font(.caption.monospaced())
                    .foregroundStyle(.secondary)
                }
            }
        }
        .formStyle(.grouped)
        .padding(20)
    }
}

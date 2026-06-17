import SwiftUI

struct SettingsNetworkView: View {
    @Environment(AppModel.self) private var model

    var body: some View {
        Form {
            Section("API Tokens") {
                Text("Create and manage API tokens for automation, integrations, and third-party access.")
                    .font(.caption)
                    .foregroundStyle(.secondary)

                Text("Token management is available in the web UI under Settings → API Tokens.")
                    .font(.caption)
                    .foregroundStyle(.tertiary)
            }

            Section("Token Classes") {
                LabeledContent("Read Only") {
                    Text("Monitoring and observability routes")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                LabeledContent("ARR Webhook") {
                    Text("Sonarr/Radarr download webhooks")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                LabeledContent("Jellyfin") {
                    Text("Jellyfin plugin integration")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                LabeledContent("Full Access") {
                    Text("All API endpoints")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
            }

            Section("Connection") {
                LabeledContent("Base URL") {
                    Text(model.connection.baseURLString)
                        .font(.caption.monospaced())
                        .foregroundStyle(.secondary)
                }
                LabeledContent("Mode") {
                    Text(model.connection.connectionMode.label)
                }
                LabeledContent("SSE State") {
                    HStack(spacing: 6) {
                        Circle()
                            .fill(model.connection.sseState == .connected ? Color.heliosSuccess : Color.heliosError)
                            .frame(width: 8, height: 8)
                        Text(String(describing: model.connection.sseState))
                            .font(.caption)
                    }
                }
            }
        }
        .formStyle(.grouped)
        .padding(20)
    }
}

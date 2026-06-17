import SwiftUI

struct SettingsGeneralView: View {
    @Environment(AppModel.self) private var model

    private var connectionModeBinding: Binding<ConnectionMode> {
        Binding(get: { model.connection.connectionMode }, set: { model.setConnectionMode($0) })
    }
    private var baseURLBinding: Binding<String> {
        Binding(get: { model.connection.baseURLString }, set: { model.connection.baseURLString = $0 })
    }

    var body: some View {
        Form {
            Section("Connection") {
                Picker("Mode", selection: connectionModeBinding) {
                    ForEach(ConnectionMode.allCases) { mode in
                        Text(mode.label).tag(mode)
                    }
                }

                TextField("API URL", text: baseURLBinding)
                    .disabled(!model.connection.isRemote)
                    .onSubmit { model.reconnect() }

                LabeledContent("Status") {
                    HStack(spacing: 6) {
                        Circle()
                            .fill(model.daemon.status.contains("Running") ? Color.heliosSuccess : Color.heliosError)
                            .frame(width: 8, height: 8)
                        Text(model.daemon.status)
                            .font(.caption)
                    }
                }

                if model.connection.isRemote {
                    Button("Reconnect") { model.reconnect() }
                    Button("Use Bundled Daemon") { model.setConnectionMode(.bundled) }
                }
            }

            Section("Startup") {
                Text(model.connection.isRemote
                    ? "Connected to a remote server. Local file import is disabled; use Convert (upload) instead."
                    : "The bundled daemon runs locally on \(DaemonController.bundledBaseURLString).")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
        }
        .formStyle(.grouped)
        .padding(20)
    }
}

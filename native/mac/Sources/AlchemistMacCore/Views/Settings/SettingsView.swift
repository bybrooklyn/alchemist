import SwiftUI

public struct SettingsView: View {
    @Environment(AppModel.self) private var model

    public init() {}

    private var themeBinding: Binding<AppTheme> {
        Binding(get: { model.theme.theme }, set: { model.theme.theme = $0 })
    }
    private var accentBinding: Binding<AppAccent> {
        Binding(get: { model.theme.accent }, set: { model.theme.accent = $0 })
    }
    private var materialBinding: Binding<MaterialIntensity> {
        Binding(get: { model.theme.material }, set: { model.theme.material = $0 })
    }
    private var densityBinding: Binding<AppDensity> {
        Binding(get: { model.theme.density }, set: { model.theme.density = $0 })
    }
    private var connectionModeBinding: Binding<ConnectionMode> {
        Binding(get: { model.connection.connectionMode }, set: { model.connection.connectionMode = $0 })
    }
    private var baseURLBinding: Binding<String> {
        Binding(get: { model.connection.baseURLString }, set: { model.connection.baseURLString = $0 })
    }

    public var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 32) {
                VStack(alignment: .leading, spacing: 4) {
                    Text("Settings")
                        .font(.system(size: 34, weight: .bold, design: .rounded))
                    Text("Configure appearance and daemon connection")
                        .font(.title3)
                        .foregroundStyle(.secondary)
                }

                VStack(alignment: .leading, spacing: 24) {
                    settingsSection("Appearance") {
                        Picker("Theme", selection: themeBinding) {
                            ForEach(AppTheme.allCases) { theme in
                                Text(theme.label).tag(theme)
                            }
                        }
                        Picker("Accent", selection: accentBinding) {
                            ForEach(AppAccent.allCases) { accent in
                                Text(accent.label).tag(accent)
                            }
                        }
                        Picker("Material", selection: materialBinding) {
                            ForEach(MaterialIntensity.allCases) { material in
                                Text(material.label).tag(material)
                            }
                        }
                        Picker("Density", selection: densityBinding) {
                            ForEach(AppDensity.allCases) { density in
                                Text(density.label).tag(density)
                            }
                        }
                    }

                    settingsSection("Connection") {
                        Picker("Mode", selection: connectionModeBinding) {
                            ForEach(ConnectionMode.allCases) { mode in
                                Text(mode.label).tag(mode)
                            }
                        }

                        TextField("API URL", text: baseURLBinding)
                            .textFieldStyle(.plain)
                            .padding(12)
                            .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
                            .overlay(RoundedRectangle(cornerRadius: 10, style: .continuous).stroke(Color.white.opacity(0.05), lineWidth: 1))
                            .onSubmit { model.reconnect() }

                        HStack {
                            Button("Reconnect") {
                                model.reconnect()
                            }
                            .buttonStyle(.glass)

                            Button("Use Bundled Daemon") {
                                model.startBundledDaemon()
                            }
                            .buttonStyle(.glass)
                        }
                    }
                }
            }
            .padding(32)
        }
    }

    private func settingsSection<Content: View>(_ title: String, @ViewBuilder content: () -> Content) -> some View {
        VStack(alignment: .leading, spacing: 16) {
            Text(title)
                .font(.headline)
                .foregroundStyle(.secondary)

            VStack(alignment: .leading, spacing: 16) {
                content()
            }
            .padding(20)
            .background(.ultraThinMaterial, in: RoundedRectangle(cornerRadius: 16, style: .continuous))
            .overlay(RoundedRectangle(cornerRadius: 16, style: .continuous).stroke(Color.white.opacity(0.05), lineWidth: 1))
        }
    }
}

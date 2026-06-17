import SwiftUI

struct SettingsAppearanceView: View {
    @Environment(AppModel.self) private var model

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

    var body: some View {
        Form {
            Section("Theme") {
                Picker("Appearance", selection: themeBinding) {
                    ForEach(AppTheme.allCases) { theme in
                        Text(theme.label).tag(theme)
                    }
                }

                Picker("Accent Color", selection: accentBinding) {
                    ForEach(AppAccent.allCases) { accent in
                        HStack(spacing: 6) {
                            Circle()
                                .fill(accent.color)
                                .frame(width: 10, height: 10)
                            Text(accent.label)
                        }
                        .tag(accent)
                    }
                }
            }

            Section("Material") {
                Picker("Intensity", selection: materialBinding) {
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
        }
        .formStyle(.grouped)
        .padding(20)
    }
}

import SwiftUI

struct ConfigPanel: View {
    let settingsBundle: SettingsBundleResponse?

    var body: some View {
        let bundle = settingsBundle?.settings
        VStack(alignment: .leading, spacing: 16) {
            Text("Configuration")
                .font(.headline)
                .foregroundStyle(.secondary)

            VStack(alignment: .leading, spacing: 12) {
                summaryRow(label: "Library roots", value: "\(bundle?.scanner.directories.count ?? 0)")
                summaryRow(label: "Notification targets", value: "\(bundle?.notifications.targets.count ?? 0)")
                summaryRow(label: "Schedule windows", value: "\(bundle?.schedule.windows.count ?? 0)")
            }
            .padding(18)
            .background(.ultraThinMaterial, in: RoundedRectangle(cornerRadius: 16, style: .continuous))
            .overlay(RoundedRectangle(cornerRadius: 16, style: .continuous).stroke(Color.white.opacity(0.05), lineWidth: 1))
        }
    }

    private func summaryRow(label: String, value: String) -> some View {
        HStack {
            Text(label)
                .font(.subheadline)
                .foregroundStyle(.secondary)
            Spacer()
            Text(value)
                .font(.subheadline.monospacedDigit().bold())
        }
    }
}

import SwiftUI

struct WeekPanel: View {
    let weeklyBytesSaved: Int64
    let weeklyJobsCompleted: Int

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            Label("Last 7 Days", systemImage: "externaldrive.fill")
                .font(.headline)
                .foregroundStyle(.secondary)

            VStack(alignment: .leading, spacing: 12) {
                summaryRow(label: "Space recovered", value: AlchemistFormatter.bytes(weeklyBytesSaved))
                summaryRow(label: "Jobs completed", value: "\(weeklyJobsCompleted)")
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

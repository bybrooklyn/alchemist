import SwiftUI

struct LogRow: View {
    let log: LogEntry

    var body: some View {
        HStack(alignment: .top, spacing: 12) {
            Text(AlchemistFormatter.shortDate(log.createdAt))
                .font(.caption2.monospacedDigit())
                .foregroundStyle(.tertiary)
                .frame(width: 118, alignment: .trailing)
            Text(log.level.uppercased())
                .font(.caption2.bold())
                .foregroundStyle(levelColor)
                .frame(width: 52, alignment: .leading)
            Text(log.message)
                .font(.caption.monospaced())
                .textSelection(.enabled)
                .frame(maxWidth: .infinity, alignment: .leading)
            if let jobID = log.jobID {
                Text("#\(jobID)")
                    .font(.caption2.monospacedDigit())
                    .foregroundStyle(.secondary)
            }
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 7)
        .overlay(Rectangle().fill(Color.white.opacity(0.04)).frame(height: 1), alignment: .bottom)
    }

    private var levelColor: Color {
        if log.level.lowercased().contains("error") {
            return Color.heliosError
        }
        if log.level.lowercased().contains("warn") {
            return Color.heliosWarning
        }
        return Color.heliosAccent
    }
}

import SwiftUI

struct DashboardActivityRow: View {
    let job: Job

    var body: some View {
        HStack(spacing: 12) {
            Circle()
                .fill(statusColor)
                .frame(width: 7, height: 7)

            VStack(alignment: .leading, spacing: 3) {
                Text(job.fileName)
                    .font(.subheadline.weight(.medium))
                    .lineLimit(1)
                Text("\(job.status) · \(relativeTime(job.createdAt))")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }

            Spacer()

            Text("#\(job.id)")
                .font(.caption.monospacedDigit())
                .foregroundStyle(.tertiary)
        }
        .padding(.horizontal, 14)
        .padding(.vertical, 10)
        .background(Color.clear)
    }

    private var statusColor: Color {
        switch job.status {
        case "completed":
            Color.heliosSuccess
        case "failed", "cancelled":
            Color.heliosError
        case "encoding", "analyzing", "remuxing", "resuming":
            Color.heliosAccent
        default:
            .secondary.opacity(0.45)
        }
    }

    private func relativeTime(_ iso: String?) -> String {
        guard let iso else { return "Just now" }
        let fractional = ISO8601DateFormatter()
        fractional.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        let standard = ISO8601DateFormatter()
        standard.formatOptions = [.withInternetDateTime]
        guard let date = fractional.date(from: iso) ?? standard.date(from: iso) else {
            return "Just now"
        }
        let minutes = max(0, Int(Date().timeIntervalSince(date) / 60))
        if minutes < 1 { return "Just now" }
        if minutes < 60 { return "\(minutes)m ago" }
        let hours = minutes / 60
        if hours < 24 { return "\(hours)h ago" }
        return "\(hours / 24)d ago"
    }
}

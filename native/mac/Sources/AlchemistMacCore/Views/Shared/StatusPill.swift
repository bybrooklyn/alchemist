import SwiftUI

struct StatusPill: View {
    let status: String

    var body: some View {
        Label(status.capitalized, systemImage: symbol)
            .font(.caption.bold())
            .padding(.horizontal, 10)
            .padding(.vertical, 6)
            .background(color.opacity(0.2), in: Capsule())
            .foregroundStyle(color)
            .overlay(Capsule().stroke(color.opacity(0.5), lineWidth: 1))
    }

    private var color: Color {
        switch status.lowercased() {
        case "running", "encoding", "remuxing", "analyzing", "resuming":
            return Color.heliosAccent
        case "completed", "profile":
            return Color.heliosSuccess
        case "failed", "cancelled":
            return Color.heliosError
        case "paused", "queued", "draining", "skipped":
            return Color.heliosWarning
        default:
            return .secondary
        }
    }

    private var symbol: String {
        switch status.lowercased() {
        case "running", "encoding", "remuxing", "analyzing", "resuming":
            return "bolt.fill"
        case "completed":
            return "checkmark"
        case "failed", "cancelled":
            return "exclamationmark.triangle"
        case "paused":
            return "pause.fill"
        case "queued":
            return "clock"
        case "skipped":
            return "forward.end"
        default:
            return "circle"
        }
    }
}

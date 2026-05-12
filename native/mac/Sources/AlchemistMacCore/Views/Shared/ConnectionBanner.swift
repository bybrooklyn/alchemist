import SwiftUI

struct ConnectionBanner: View {
    let state: SSEConnectionState

    var body: some View {
        HStack(spacing: 10) {
            Image(systemName: symbol)
            Text(message)
                .font(.subheadline)
            Spacer()
        }
        .foregroundStyle(color)
        .padding(12)
        .background(color.opacity(0.10), in: RoundedRectangle(cornerRadius: 12, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 12, style: .continuous).stroke(color.opacity(0.25), lineWidth: 1))
    }

    private var color: Color {
        switch state {
        case .connected:
            .heliosSuccess
        case .connecting, .reconnecting:
            .heliosWarning
        case .disconnected:
            .heliosError
        }
    }

    private var symbol: String {
        switch state {
        case .connected:
            "checkmark.circle.fill"
        case .connecting:
            "arrow.triangle.2.circlepath"
        case .reconnecting:
            "wifi.exclamationmark"
        case .disconnected:
            "wifi.slash"
        }
    }

    private var message: String {
        switch state {
        case .connected:
            "Live updates connected."
        case .connecting:
            "Connecting to live updates..."
        case .reconnecting(let attempt):
            "Live updates interrupted. Reconnecting (attempt \(attempt))."
        case .disconnected:
            "Live updates disconnected."
        }
    }
}

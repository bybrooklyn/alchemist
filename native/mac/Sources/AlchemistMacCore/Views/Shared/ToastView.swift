import SwiftUI

public enum ToastType: Equatable, Sendable {
    case success
    case error
    case info

    var icon: String {
        switch self {
        case .success: "checkmark.circle.fill"
        case .error: "exclamationmark.triangle.fill"
        case .info: "info.circle.fill"
        }
    }
}

public struct ToastMessage: Equatable {
    public let type: ToastType
    public let message: String

    public init(type: ToastType, message: String) {
        self.type = type
        self.message = message
    }
}

struct ToastView: View {
    let toast: ToastMessage
    let onDismiss: () -> Void

    private var color: Color {
        switch toast.type {
        case .success: .heliosSuccess
        case .error: .heliosError
        case .info: .heliosAccent
        }
    }

    var body: some View {
        HStack(spacing: 10) {
            Image(systemName: toast.type.icon)
                .foregroundStyle(color)
            Text(toast.message)
                .font(.subheadline)
                .foregroundStyle(.primary)
            Spacer()
            Button {
                onDismiss()
            } label: {
                Image(systemName: "xmark")
                    .font(.caption2)
                    .foregroundStyle(.secondary)
            }
            .buttonStyle(.plain)
        }
        .padding(12)
        .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 12, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 12, style: .continuous).stroke(color.opacity(0.3), lineWidth: 1))
        .shadow(color: .black.opacity(0.15), radius: 8, y: 4)
        .padding(.horizontal, 20)
        .transition(.move(edge: .top).combined(with: .opacity))
    }
}

struct ToastModifier: ViewModifier {
    @Binding var toast: ToastMessage?

    func body(content: Content) -> some View {
        ZStack(alignment: .top) {
            content

            if let toast {
                ToastView(toast: toast) {
                    withAnimation { self.toast = nil }
                }
                .padding(.top, 8)
                .zIndex(1)
                .onAppear {
                    Task {
                        try? await Task.sleep(nanoseconds: 3_500_000_000)
                        withAnimation { self.toast = nil }
                    }
                }
            }
        }
    }
}

extension View {
    func toast(_ toast: Binding<ToastMessage?>) -> some View {
        modifier(ToastModifier(toast: toast))
    }
}

import SwiftUI

struct ActionButton<Label: View>: View {
    let action: () async throws -> Void
    @ViewBuilder var label: () -> Label

    @State private var isLoading = false
    @State private var showSuccess = false
    @State private var errorMessage: String?

    var body: some View {
        Button {
            Task {
                isLoading = true
                errorMessage = nil
                showSuccess = false
                do {
                    try await action()
                    withAnimation { showSuccess = true }
                    try? await Task.sleep(nanoseconds: 1_200_000_000)
                    withAnimation { showSuccess = false }
                } catch {
                    errorMessage = error.localizedDescription
                    try? await Task.sleep(nanoseconds: 3_000_000_000)
                    errorMessage = nil
                }
                isLoading = false
            }
        } label: {
            HStack(spacing: 6) {
                if isLoading {
                    ProgressView()
                        .controlSize(.small)
                } else if showSuccess {
                    Image(systemName: "checkmark.circle.fill")
                        .foregroundStyle(Color.heliosSuccess)
                } else if errorMessage != nil {
                    Image(systemName: "exclamationmark.circle.fill")
                        .foregroundStyle(Color.heliosError)
                }
                label()
            }
        }
        .disabled(isLoading)
        .help(errorMessage ?? "")
    }
}

import SwiftUI

struct LoginView: View {
    @Environment(AppModel.self) private var model
    @Environment(\.dismiss) private var dismiss
    @State private var username = "admin"
    @State private var password = ""

    var body: some View {
        VStack(alignment: .leading, spacing: 24) {
            VStack(alignment: .leading, spacing: 8) {
                Text("Welcome Back")
                    .font(.title.bold())
                    .foregroundStyle(.primary)
                Text("Login to your Alchemist node")
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
            }

            VStack(spacing: 16) {
                VStack(alignment: .leading, spacing: 8) {
                    Text("USERNAME")
                        .font(.caption.bold())
                        .foregroundStyle(.secondary)
                    TextField("", text: $username)
                        .textFieldStyle(.plain)
                        .padding(12)
                        .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
                        .overlay(RoundedRectangle(cornerRadius: 10).stroke(Color.white.opacity(0.1), lineWidth: 1))
                }

                VStack(alignment: .leading, spacing: 8) {
                    Text("PASSWORD")
                        .font(.caption.bold())
                        .foregroundStyle(.secondary)
                    SecureField("", text: $password)
                        .textFieldStyle(.plain)
                        .padding(12)
                        .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
                        .overlay(RoundedRectangle(cornerRadius: 10).stroke(Color.white.opacity(0.1), lineWidth: 1))
                }
            }

            if let error = model.lastError {
                ErrorPanel(message: error.localizedDescription)
            }

            HStack(spacing: 16) {
                Spacer()
                Button("Cancel") { dismiss() }
                    .buttonStyle(.glass)

                Button("Login") {
                    Task {
                        await model.login(username: username, password: password)
                        if model.isAuthenticated {
                            dismiss()
                        }
                    }
                }
                .buttonStyle(.glassProminent)
                .tint(Color.heliosAccent)
            }
            .frame(maxWidth: .infinity)
        }
        .padding(32)
        .frame(width: 440)
        .background(.ultraThinMaterial)
    }
}

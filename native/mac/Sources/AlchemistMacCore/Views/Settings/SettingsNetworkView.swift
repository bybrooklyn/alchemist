import SwiftUI

struct SettingsNetworkView: View {
    @Environment(AppModel.self) private var model
    @State private var showingCreateToken = false
    @State private var createdTokenPlaintext: String?
    @State private var tokenToRevoke: ApiTokenResponse?

    var body: some View {
        Form {
            Section("API Tokens") {
                if model.dashboard.apiTokens.isEmpty {
                    Text("No API tokens configured.")
                        .foregroundStyle(.secondary)
                    Text("Create tokens for automation, integrations, and third-party access.")
                        .font(.caption)
                        .foregroundStyle(.tertiary)
                } else {
                    ForEach(model.dashboard.apiTokens.filter { $0.isActive }) { token in
                        ApiTokenRow(token: token) {
                            tokenToRevoke = token
                        }
                    }
                }

                Button {
                    showingCreateToken = true
                } label: {
                    Label("Create Token", systemImage: "key.fill")
                }
            }

            Section("Token Classes") {
                LabeledContent("Read Only") {
                    Text("Monitoring and observability routes")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                LabeledContent("ARR Webhook") {
                    Text("Sonarr/Radarr download webhooks")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                LabeledContent("Jellyfin") {
                    Text("Jellyfin plugin integration")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                LabeledContent("Full Access") {
                    Text("All API endpoints")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
            }

            Section("Connection") {
                LabeledContent("Base URL") {
                    Text(model.connection.baseURLString)
                        .font(.caption.monospaced())
                        .foregroundStyle(.secondary)
                }
                LabeledContent("Mode") {
                    Text(model.connection.connectionMode.label)
                }
                LabeledContent("SSE State") {
                    HStack(spacing: 6) {
                        Circle()
                            .fill(model.connection.sseState == .connected ? Color.heliosSuccess : Color.heliosError)
                            .frame(width: 8, height: 8)
                        Text(String(describing: model.connection.sseState))
                            .font(.caption)
                    }
                }
            }
        }
        .formStyle(.grouped)
        .padding(20)
        .sheet(isPresented: $showingCreateToken) {
            CreateApiTokenSheet(apiClient: model.connection.apiClient) { plaintext in
                createdTokenPlaintext = plaintext
                Task { await model.dashboard.refresh(apiClient: model.connection.apiClient) }
            }
        }
        .alert("Token Created", isPresented: Binding(
            get: { createdTokenPlaintext != nil },
            set: { if !$0 { createdTokenPlaintext = nil } }
        )) {
            Button("Copy") {
                if let token = createdTokenPlaintext {
                    NSPasteboard.general.clearContents()
                    NSPasteboard.general.setString(token, forType: .string)
                }
                createdTokenPlaintext = nil
            }
            Button("Done") { createdTokenPlaintext = nil }
        } message: {
            Text("Copy this token now — it won't be shown again.\n\n\(createdTokenPlaintext ?? "")")
        }
        .alert("Revoke Token?", isPresented: Binding(
            get: { tokenToRevoke != nil },
            set: { if !$0 { tokenToRevoke = nil } }
        )) {
            Button("Cancel", role: .cancel) {}
            Button("Revoke", role: .destructive) {
                guard let token = tokenToRevoke else { return }
                Task {
                    do {
                        try await model.connection.apiClient?.revokeApiToken(id: token.id)
                        model.showToast(.success, "Revoked \(token.name)")
                        await model.dashboard.refresh(apiClient: model.connection.apiClient)
                    } catch {
                        model.showToast(.error, error.localizedDescription)
                    }
                }
            }
        } message: {
            Text("Are you sure you want to revoke \"\(tokenToRevoke?.name ?? "")\"? This cannot be undone.")
        }
    }
}

private struct ApiTokenRow: View {
    let token: ApiTokenResponse
    let onRevoke: () -> Void

    var body: some View {
        HStack {
            VStack(alignment: .leading, spacing: 2) {
                Text(token.name)
                    .font(.body)
                HStack(spacing: 6) {
                    Text(token.accessLevel.replacingOccurrences(of: "_", with: " ").capitalized)
                        .font(.caption2)
                        .padding(.horizontal, 6)
                        .padding(.vertical, 2)
                        .background(.thinMaterial, in: Capsule())
                    Text(AlchemistFormatter.shortDate(token.createdAt))
                        .font(.caption2)
                        .foregroundStyle(.secondary)
                }
            }
            Spacer()
            Button(role: .destructive, action: onRevoke) {
                Image(systemName: "xmark.circle")
            }
            .buttonStyle(.plain)
            .foregroundStyle(.secondary)
        }
    }
}

struct CreateApiTokenSheet: View {
    let apiClient: AlchemistAPIClient?
    let onCreated: (String) -> Void
    @Environment(\.dismiss) private var dismiss

    @State private var name = ""
    @State private var accessLevel = "full_access"
    @State private var isCreating = false
    @State private var errorMessage: String?

    private static let accessLevels = [
        ("read_only", "Read Only"),
        ("arr_webhook", "ARR Webhook"),
        ("jellyfin", "Jellyfin"),
        ("full_access", "Full Access"),
    ]

    var body: some View {
        VStack(spacing: 16) {
            Text("Create API Token")
                .font(.headline)

            Form {
                TextField("Token Name", text: $name)
                Picker("Access Level", selection: $accessLevel) {
                    ForEach(Self.accessLevels, id: \.0) { level in
                        Text(level.1).tag(level.0)
                    }
                }
            }
            .formStyle(.grouped)

            if let errorMessage {
                Text(errorMessage)
                    .font(.caption)
                    .foregroundStyle(Color.heliosError)
            }

            HStack {
                Button("Cancel") { dismiss() }
                    .keyboardShortcut(.cancelAction)
                Spacer()
                Button("Create") {
                    Task { await createToken() }
                }
                .keyboardShortcut(.defaultAction)
                .disabled(name.trimmingCharacters(in: .whitespaces).isEmpty || isCreating)
            }
        }
        .padding(24)
        .frame(width: 400)
    }

    private func createToken() async {
        isCreating = true
        errorMessage = nil
        do {
            let response = try await apiClient?.createApiToken(name: name.trimmingCharacters(in: .whitespaces), accessLevel: accessLevel)
            if let plaintext = response?.plaintextToken {
                onCreated(plaintext)
            }
            dismiss()
        } catch {
            errorMessage = error.localizedDescription
            isCreating = false
        }
    }
}

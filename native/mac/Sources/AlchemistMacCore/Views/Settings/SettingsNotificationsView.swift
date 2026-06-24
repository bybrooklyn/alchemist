import SwiftUI

struct SettingsNotificationsView: View {
    @Environment(AppModel.self) private var model
    @State private var showingAddTarget = false
    @State private var targetToDelete: NotificationTargetResponse?

    var body: some View {
        Form {
            Section("Notification Targets") {
                if model.dashboard.notificationTargets.isEmpty {
                    Text("No notification targets configured.")
                        .foregroundStyle(.secondary)
                    Text("Set up Discord, Telegram, Gotify, ntfy, email, or webhook notifications to get alerted when jobs complete or fail.")
                        .font(.caption)
                        .foregroundStyle(.tertiary)
                } else {
                    ForEach(model.dashboard.notificationTargets) { target in
                        NotificationTargetRow(target: target) {
                            targetToDelete = target
                        }
                    }
                }

                Button {
                    showingAddTarget = true
                } label: {
                    Label("Add Target", systemImage: "bell.badge.plus")
                }
            }

            Section("Events") {
                Text("Configure which events trigger notifications when adding a target.")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                LabeledContent("Available Events") {
                    VStack(alignment: .trailing, spacing: 2) {
                        Text("encode.queued")
                        Text("encode.started")
                        Text("encode.completed")
                        Text("encode.failed")
                        Text("scan.completed")
                        Text("engine.idle")
                        Text("daily.summary")
                    }
                    .font(.caption.monospaced())
                    .foregroundStyle(.secondary)
                }
            }
        }
        .formStyle(.grouped)
        .padding(20)
        .sheet(isPresented: $showingAddTarget) {
            AddNotificationTargetSheet(apiClient: model.connection.apiClient) { name, targetType, configJSON, events in
                Task {
                    do {
                        _ = try await model.connection.apiClient?.addNotificationTarget(
                            name: name,
                            targetType: targetType,
                            configJSON: configJSON,
                            events: events,
                            enabled: true
                        )
                        model.showToast(.success, "Added \(name)")
                        await model.dashboard.refresh(apiClient: model.connection.apiClient)
                    } catch {
                        model.showToast(.error, error.localizedDescription)
                    }
                }
            }
        }
        .alert("Delete Target?", isPresented: Binding(
            get: { targetToDelete != nil },
            set: { if !$0 { targetToDelete = nil } }
        )) {
            Button("Cancel", role: .cancel) {}
            Button("Delete", role: .destructive) {
                guard let target = targetToDelete else { return }
                Task {
                    do {
                        try await model.connection.apiClient?.deleteNotificationTarget(id: target.id)
                        model.showToast(.success, "Removed \(target.name)")
                        await model.dashboard.refresh(apiClient: model.connection.apiClient)
                    } catch {
                        model.showToast(.error, error.localizedDescription)
                    }
                }
            }
        } message: {
            Text("Are you sure you want to delete \"\(targetToDelete?.name ?? "")\"? This cannot be undone.")
        }
    }
}

private struct NotificationTargetRow: View {
    let target: NotificationTargetResponse
    let onDelete: () -> Void

    var body: some View {
        HStack {
            VStack(alignment: .leading, spacing: 2) {
                Text(target.name)
                    .font(.body)
                HStack(spacing: 6) {
                    Text(target.targetType.replacingOccurrences(of: "_", with: " ").capitalized)
                        .font(.caption2)
                        .padding(.horizontal, 6)
                        .padding(.vertical, 2)
                        .background(.thinMaterial, in: Capsule())
                    Text(target.enabled ? "Enabled" : "Disabled")
                        .font(.caption2)
                        .foregroundStyle(target.enabled ? Color.heliosSuccess : .secondary)
                }
            }
            Spacer()
            Button(role: .destructive, action: onDelete) {
                Image(systemName: "trash")
            }
            .buttonStyle(.plain)
            .foregroundStyle(.secondary)
        }
    }
}

struct AddNotificationTargetSheet: View {
    let apiClient: AlchemistAPIClient?
    let onSave: (String, String, [String: JSONValue], [String]) -> Void
    @Environment(\.dismiss) private var dismiss

    @State private var name = ""
    @State private var targetType = "discord_webhook"
    @State private var webhookURL = ""
    @State private var serverURL = ""
    @State private var appToken = ""
    @State private var botToken = ""
    @State private var chatID = ""
    @State private var topic = ""
    @State private var authToken = ""
    @State private var selectedEvents: Set<String> = ["encode.completed", "encode.failed"]
    @State private var isTesting = false
    @State private var testResult: String?

    private static let allEvents = [
        "encode.queued", "encode.started", "encode.completed", "encode.failed",
        "scan.completed", "engine.idle", "daily.summary",
    ]

    private static let targetTypes = [
        ("discord_webhook", "Discord"),
        ("gotify", "Gotify"),
        ("ntfy", "ntfy"),
        ("webhook", "Webhook"),
        ("telegram", "Telegram"),
    ]

    var body: some View {
        VStack(spacing: 16) {
            Text("Add Notification Target")
                .font(.headline)

            Form {
                Section("Target") {
                    TextField("Name", text: $name)
                    Picker("Type", selection: $targetType) {
                        ForEach(Self.targetTypes, id: \.0) { type in
                            Text(type.1).tag(type.0)
                        }
                    }
                }

                Section("Configuration") {
                    switch targetType {
                    case "discord_webhook":
                        TextField("Webhook URL", text: $webhookURL)
                            .textFieldStyle(.roundedBorder)
                    case "gotify":
                        TextField("Server URL", text: $serverURL)
                            .textFieldStyle(.roundedBorder)
                        SecureField("App Token", text: $appToken)
                            .textFieldStyle(.roundedBorder)
                    case "ntfy":
                        TextField("Topic", text: $topic)
                            .textFieldStyle(.roundedBorder)
                        TextField("Server URL (optional)", text: $serverURL)
                            .textFieldStyle(.roundedBorder)
                    case "webhook":
                        TextField("URL", text: $webhookURL)
                            .textFieldStyle(.roundedBorder)
                        SecureField("Auth Token (optional)", text: $authToken)
                            .textFieldStyle(.roundedBorder)
                    case "telegram":
                        SecureField("Bot Token", text: $botToken)
                            .textFieldStyle(.roundedBorder)
                        TextField("Chat ID", text: $chatID)
                            .textFieldStyle(.roundedBorder)
                    default:
                        EmptyView()
                    }
                }

                Section("Events") {
                    ForEach(Self.allEvents, id: \.self) { event in
                        Toggle(event, isOn: Binding(
                            get: { selectedEvents.contains(event) },
                            set: { if $0 { selectedEvents.insert(event) } else { selectedEvents.remove(event) } }
                        ))
                    }
                }
            }
            .formStyle(.grouped)

            if let testResult {
                Text(testResult)
                    .font(.caption)
                    .foregroundStyle(testResult.contains("Failed") ? Color.heliosError : Color.heliosSuccess)
            }

            HStack {
                Button("Cancel") { dismiss() }
                    .keyboardShortcut(.cancelAction)
                Spacer()
                Button("Test") {
                    Task { await testNotification() }
                }
                .disabled(name.isEmpty || !isValid || isTesting)
                Button("Add") {
                    let config = buildConfigJSON()
                    let events = Array(selectedEvents).sorted()
                    onSave(name, targetType, config, events)
                    dismiss()
                }
                .keyboardShortcut(.defaultAction)
                .disabled(name.isEmpty || !isValid)
            }
        }
        .padding(24)
        .frame(width: 480)
    }

    private var isValid: Bool {
        switch targetType {
        case "discord_webhook": return !webhookURL.isEmpty
        case "gotify": return !serverURL.isEmpty && !appToken.isEmpty
        case "ntfy": return !topic.isEmpty
        case "webhook": return !webhookURL.isEmpty
        case "telegram": return !botToken.isEmpty && !chatID.isEmpty
        default: return false
        }
    }

    private func buildConfigJSON() -> [String: JSONValue] {
        switch targetType {
        case "discord_webhook":
            return ["webhook_url": .string(webhookURL)]
        case "gotify":
            return ["server_url": .string(serverURL), "app_token": .string(appToken)]
        case "ntfy":
            var config: [String: JSONValue] = ["topic": .string(topic)]
            if !serverURL.isEmpty { config["server_url"] = .string(serverURL) }
            return config
        case "webhook":
            var config: [String: JSONValue] = ["url": .string(webhookURL)]
            if !authToken.isEmpty { config["auth_token"] = .string(authToken) }
            return config
        case "telegram":
            return ["bot_token": .string(botToken), "chat_id": .string(chatID)]
        default:
            return [:]
        }
    }

    private func testNotification() async {
        isTesting = true
        testResult = nil
        do {
            let config = buildConfigJSON()
            let events = Array(selectedEvents).sorted()
            try await apiClient?.testNotificationTarget(
                name: name,
                targetType: targetType,
                configJSON: config,
                events: events,
                enabled: true
            )
            testResult = "Test sent successfully"
        } catch {
            testResult = "Failed: \(error.localizedDescription)"
        }
        isTesting = false
    }
}

import Foundation

public enum SetupStep: Int, CaseIterable, Identifiable, Sendable {
    case account
    case library
    case processing
    case runtime
    case review

    public var id: Int { rawValue }

    public var title: String {
        switch self {
        case .account: "Admin"
        case .library: "Library"
        case .processing: "Processing"
        case .runtime: "Runtime"
        case .review: "Review"
        }
    }

    public var subtitle: String {
        switch self {
        case .account: "Create first admin account"
        case .library: "Choose folders to scan"
        case .processing: "Set default transcode behavior"
        case .runtime: "Hardware and telemetry"
        case .review: "Confirm and finish setup"
        }
    }
}

@Observable
@MainActor
public final class SetupState {
    public var step: SetupStep = .account
    public var username = ""
    public var password = ""
    public var confirmPassword = ""
    public var settings = SetupSettings.defaultValue
    public var setupStatus: SetupStatusResponse?
    public var hardwareInfo: HardwareInfo?
    public var recommendations: [FsRecommendation] = []
    public var preview: FsPreviewResponse?
    public var isLoading = false
    public var isSubmitting = false
    public var lastError: AlchemistUIError?

    public init() {}

    public var configMutable: Bool {
        setupStatus?.configMutable ?? true
    }

    public var canGoBack: Bool {
        step != .account
    }

    public var canAdvance: Bool {
        step != .review
    }

    public func loadBootstrap(apiClient: AlchemistAPIClient?) async {
        guard let apiClient else { return }
        isLoading = true
        defer { isLoading = false }

        do {
            async let status = apiClient.fetchSetupStatus()
            async let bundle = apiClient.fetchSettingsBundle()
            async let recommendations = apiClient.fetchFolderRecommendations()

            let statusValue = try await status
            let bundleValue = try await bundle
            let recommendationValue = try await recommendations

            self.setupStatus = statusValue
            self.settings = bundleValue.settings.mergedWithDefaults(enableTelemetryOverride: statusValue.enableTelemetry)
            self.recommendations = recommendationValue.recommendations
            self.lastError = nil
        } catch {
            self.lastError = mapError(error, fallbackCode: "setup_bootstrap_failed")
        }

        do {
            self.hardwareInfo = try await apiClient.fetchHardwareInfo()
        } catch {
            // Hardware can still be warming up during setup.
            self.hardwareInfo = nil
        }
    }

    public func previewDirectories(apiClient: AlchemistAPIClient?) async {
        guard let apiClient else { return }
        do {
            self.preview = try await apiClient.previewFolders(settings.scanner.directories)
            self.lastError = nil
        } catch {
            self.lastError = mapError(error, fallbackCode: "setup_preview_failed")
        }
    }

    public func addDirectory(_ path: String) {
        let trimmed = path.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        if !settings.scanner.directories.contains(trimmed) {
            settings.scanner.directories.append(trimmed)
        }
    }

    public func removeDirectory(_ path: String) {
        settings.scanner.directories.removeAll { $0 == path }
    }

    public func applyRecommendation(_ recommendation: FsRecommendation) {
        addDirectory(recommendation.path)
    }

    public func addNotificationTarget() {
        settings.notifications.targets.append(
            SetupNotificationTarget(
                name: "",
                targetType: "discord_webhook",
                endpointURL: "",
                authToken: nil,
                events: ["completed", "failed"],
                enabled: true
            )
        )
        settings.notifications.enabled = true
    }

    public func removeNotificationTarget(id: UUID) {
        settings.notifications.targets.removeAll { $0.id == id }
        if settings.notifications.targets.isEmpty {
            settings.notifications.enabled = false
        }
    }

    public func toggleNotificationEvent(targetID: UUID, event: String) {
        guard let index = settings.notifications.targets.firstIndex(where: { $0.id == targetID }) else {
            return
        }

        if settings.notifications.targets[index].events.contains(event) {
            settings.notifications.targets[index].events.removeAll { $0 == event }
        } else {
            settings.notifications.targets[index].events.append(event)
        }

        settings.notifications.targets[index].events.sort()
    }

    public func addScheduleWindow() {
        settings.schedule.windows.append(
            SetupScheduleWindow(
                startTime: "22:00",
                endTime: "06:00",
                daysOfWeek: [0, 1, 2, 3, 4, 5, 6],
                enabled: true
            )
        )
    }

    public func removeScheduleWindow(id: UUID) {
        settings.schedule.windows.removeAll { $0.id == id }
    }

    public func toggleScheduleDay(windowID: UUID, day: Int) {
        guard let index = settings.schedule.windows.firstIndex(where: { $0.id == windowID }) else {
            return
        }

        if settings.schedule.windows[index].daysOfWeek.contains(day) {
            settings.schedule.windows[index].daysOfWeek.removeAll { $0 == day }
        } else {
            settings.schedule.windows[index].daysOfWeek.append(day)
        }

        settings.schedule.windows[index].daysOfWeek.sort()
    }

    public func goBack() {
        guard let previous = SetupStep(rawValue: max(0, step.rawValue - 1)) else { return }
        step = previous
    }

    public func goNext() {
        guard let next = SetupStep(rawValue: min(SetupStep.review.rawValue, step.rawValue + 1)) else { return }
        step = next
    }

    public func validateCurrentStep() -> String? {
        switch step {
        case .account:
            if username.trimmingCharacters(in: .whitespacesAndNewlines).count < 3 {
                return "Username must be at least 3 characters."
            }
            if password.count < 8 {
                return "Password must be at least 8 characters."
            }
            if password != confirmPassword {
                return "Passwords do not match."
            }
        case .library:
            if settings.scanner.directories.isEmpty {
                return "Add at least one library folder."
            }
        case .processing:
            if settings.transcode.concurrentJobs < 1 {
                return "Concurrent jobs must be at least 1."
            }
            if !(0...1).contains(settings.transcode.sizeReductionThreshold) {
                return "Size reduction threshold must be between 0 and 1."
            }
            if settings.transcode.minBppThreshold < 0 {
                return "Bits-per-pixel threshold must be 0 or greater."
            }
        case .runtime:
            if !configMutable {
                return "Configuration writes are disabled for this Alchemist instance."
            }
            if settings.notifications.enabled {
                for target in settings.notifications.targets {
                    if target.name.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                        return "Each notification target needs a name."
                    }
                    if target.endpointURL.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                        return "Each notification target needs an endpoint URL."
                    }
                    if target.events.isEmpty {
                        return "Each notification target needs at least one event."
                    }
                }
            }
            for window in settings.schedule.windows {
                if window.startTime.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || window.endTime.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                    return "Schedule windows need start and end times."
                }
                if window.daysOfWeek.isEmpty {
                    return "Schedule windows need at least one day selected."
                }
            }
        case .review:
            return validateAll()
        }
        return nil
    }

    public func validateAll() -> String? {
        for candidate in SetupStep.allCases {
            let current = step
            step = candidate
            let error = validateCurrentStep()
            step = current
            if error != nil {
                return error
            }
        }
        return nil
    }

    public func completeSetup(apiClient: AlchemistAPIClient?) async -> Bool {
        guard let apiClient else { return false }
        if let error = validateAll() {
            lastError = .apiError(code: "setup_validation_failed", message: error)
            return false
        }

        isSubmitting = true
        defer { isSubmitting = false }

        do {
            let preparedSettings = normalizedSettingsForSubmit()
            _ = try await apiClient.completeSetup(username: username, password: password, settings: preparedSettings)
            self.settings = preparedSettings
            self.lastError = nil
            return true
        } catch {
            self.lastError = mapError(error, fallbackCode: "setup_complete_failed")
            return false
        }
    }

    private func normalizedSettingsForSubmit() -> SetupSettings {
        var prepared = settings.mergedWithDefaults(enableTelemetryOverride: settings.system.enableTelemetry)
        prepared.notifications.targets = prepared.notifications.targets.map { target in
            var normalized = target
            var configJSON = normalized.configJSON
            switch normalized.targetType {
            case "discord", "discord_webhook":
                normalized.targetType = "discord_webhook"
                configJSON["webhook_url"] = .string(normalized.endpointURL)
            case "webhook":
                configJSON["url"] = .string(normalized.endpointURL)
                if let token = normalized.authToken, !token.isEmpty {
                    configJSON["auth_token"] = .string(token)
                }
            case "gotify":
                configJSON["server_url"] = .string(normalized.endpointURL)
                if let token = normalized.authToken, !token.isEmpty {
                    configJSON["app_token"] = .string(token)
                }
            case "ntfy":
                configJSON["server_url"] = .string(normalized.endpointURL)
                if let token = normalized.authToken, !token.isEmpty {
                    configJSON["access_token"] = .string(token)
                }
            default:
                break
            }
            normalized.configJSON = configJSON
            return normalized
        }
        return prepared
    }

    private func mapError(_ error: Error, fallbackCode: String) -> AlchemistUIError {
        if let apiError = error as? AlchemistAPIError {
            switch apiError {
            case .unauthorized:
                return .authenticationRequired
            case .server(let code, let message):
                return .apiError(code: code, message: message)
            default:
                return .apiError(code: fallbackCode, message: apiError.errorDescription ?? "Unknown error")
            }
        }
        return .connectionFailed(error.localizedDescription)
    }
}

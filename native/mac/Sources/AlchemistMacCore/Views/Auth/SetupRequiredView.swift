import AppKit
import SwiftUI

struct SetupRequiredView: View {
    @Environment(AppModel.self) private var model

    private var setupURL: URL? {
        URL(string: "\(model.connection.baseURLString.trimmingCharacters(in: .whitespacesAndNewlines))/setup")
    }

    private var settings: SetupSettings { model.setup.settings }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 24) {
                header
                stepRail
                stepContent

                if let error = model.lastError {
                    ErrorPanel(message: error.localizedDescription)
                }

                footer
            }
            .padding(32)
        }
        .task {
            await model.setup.loadBootstrap(apiClient: model.connection.apiClient)
        }
    }

    private var header: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("Native Setup")
                .font(.system(size: 36, weight: .bold, design: .rounded))
            Text("Finish first-run configuration without dropping back to browser UI.")
                .font(.title3)
                .foregroundStyle(.secondary)

            if !model.setup.configMutable {
                ErrorPanel(message: "This Alchemist instance has config writes disabled. Native setup cannot continue until config becomes mutable.")
            }
        }
    }

    private var stepRail: some View {
        HStack(spacing: 10) {
            ForEach(SetupStep.allCases) { step in
                VStack(alignment: .leading, spacing: 4) {
                    Text(step.title)
                        .font(.caption.bold())
                    Text(step.subtitle)
                        .font(.caption2)
                        .lineLimit(1)
                }
                .foregroundStyle(step == model.setup.step ? Color.heliosAccent : .secondary)
                .padding(.horizontal, 12)
                .padding(.vertical, 10)
                .background(step == model.setup.step ? Color.heliosAccent.opacity(0.12) : Color.white.opacity(0.04), in: RoundedRectangle(cornerRadius: 12, style: .continuous))
                .overlay(RoundedRectangle(cornerRadius: 12, style: .continuous).stroke(step == model.setup.step ? Color.heliosAccent.opacity(0.35) : Color.white.opacity(0.06), lineWidth: 1))
            }
        }
    }

    @ViewBuilder
    private var stepContent: some View {
        switch model.setup.step {
        case .account:
            accountStep
        case .library:
            libraryStep
        case .processing:
            processingStep
        case .runtime:
            runtimeStep
        case .review:
            reviewStep
        }
    }

    private var accountStep: some View {
        VStack(alignment: .leading, spacing: 18) {
            Text("Create first admin account")
                .font(.title2.bold())

            VStack(alignment: .leading, spacing: 12) {
                labeledField("Username") {
                    TextField("admin", text: usernameBinding)
                        .textFieldStyle(.roundedBorder)
                }

                labeledField("Password") {
                    SecureField("At least 8 characters", text: passwordBinding)
                        .textFieldStyle(.roundedBorder)
                }

                labeledField("Confirm Password") {
                    SecureField("Repeat password", text: confirmPasswordBinding)
                        .textFieldStyle(.roundedBorder)
                }
            }
            .padding(18)
            .background(.thinMaterial, in: RoundedRectangle(cornerRadius: 16, style: .continuous))
        }
    }

    private var libraryStep: some View {
        VStack(alignment: .leading, spacing: 18) {
            HStack {
                Text("Choose library folders")
                    .font(.title2.bold())
                Spacer()
                Button {
                    let panel = NSOpenPanel()
                    panel.canChooseFiles = false
                    panel.canChooseDirectories = true
                    panel.allowsMultipleSelection = true
                    if panel.runModal() == .OK {
                        for url in panel.urls {
                            model.setup.addDirectory(url.path)
                        }
                    }
                } label: {
                    Label("Add Folder", systemImage: "folder.badge.plus")
                }
                .buttonStyle(.glass)

                Button {
                    Task { await model.setup.previewDirectories(apiClient: model.connection.apiClient) }
                } label: {
                    Label("Preview", systemImage: "eye")
                }
                .buttonStyle(.glass)
            }

            HStack(alignment: .top, spacing: 20) {
                VStack(alignment: .leading, spacing: 12) {
                    Text("Selected folders")
                        .font(.headline)

                    if settings.scanner.directories.isEmpty {
                        EmptyHeroCard(title: "No folders yet", detail: "Add one or more media folders for Alchemist to scan.")
                    } else {
                        VStack(spacing: 0) {
                            ForEach(settings.scanner.directories, id: \.self) { path in
                                HStack(spacing: 10) {
                                    Image(systemName: "folder")
                                        .foregroundStyle(Color.heliosAccent)
                                    Text(path)
                                        .font(.caption.monospaced())
                                        .lineLimit(1)
                                    Spacer()
                                    Button(role: .destructive) {
                                        model.setup.removeDirectory(path)
                                    } label: {
                                        Image(systemName: "trash")
                                    }
                                    .buttonStyle(.plain)
                                }
                                .padding(12)
                                .overlay(Rectangle().fill(Color.white.opacity(0.05)).frame(height: 1), alignment: .bottom)
                            }
                        }
                        .background(.thinMaterial, in: RoundedRectangle(cornerRadius: 16, style: .continuous))
                    }
                }
                .frame(maxWidth: .infinity, alignment: .topLeading)

                VStack(alignment: .leading, spacing: 12) {
                    Text("Recommended folders")
                        .font(.headline)

                    if model.setup.recommendations.isEmpty {
                        EmptyHeroCard(title: "No recommendations", detail: "Add folders manually if Alchemist cannot infer likely media roots.")
                    } else {
                        VStack(spacing: 8) {
                            ForEach(model.setup.recommendations.prefix(8)) { recommendation in
                                Button {
                                    model.setup.applyRecommendation(recommendation)
                                } label: {
                                    VStack(alignment: .leading, spacing: 4) {
                                        HStack {
                                            Text(recommendation.label)
                                                .font(.subheadline.bold())
                                            Spacer()
                                            Text(recommendation.mediaHint.uppercased())
                                                .font(.caption2.bold())
                                                .foregroundStyle(Color.heliosAccent)
                                        }
                                        Text(recommendation.reason)
                                            .font(.caption)
                                            .foregroundStyle(.secondary)
                                        Text(recommendation.path)
                                            .font(.caption2.monospaced())
                                            .foregroundStyle(.tertiary)
                                            .lineLimit(1)
                                    }
                                    .padding(10)
                                    .frame(maxWidth: .infinity, alignment: .leading)
                                    .background(.thinMaterial, in: RoundedRectangle(cornerRadius: 12, style: .continuous))
                                }
                                .buttonStyle(.plain)
                            }
                        }
                    }
                }
                .frame(width: 340, alignment: .topLeading)
            }

            if let preview = model.setup.preview {
                VStack(alignment: .leading, spacing: 10) {
                    Text("Preview")
                        .font(.headline)
                    Text("Detected \(preview.totalMediaFiles) media files across \(preview.directories.count) folders.")
                        .font(.subheadline)
                        .foregroundStyle(.secondary)
                    ForEach(preview.directories.prefix(4)) { directory in
                        VStack(alignment: .leading, spacing: 4) {
                            HStack {
                                Text(directory.path)
                                    .font(.caption.monospaced())
                                    .lineLimit(1)
                                Spacer()
                                Text("\(directory.mediaFiles) files")
                                    .font(.caption.bold())
                            }
                            if !directory.warnings.isEmpty {
                                Text(directory.warnings.joined(separator: " "))
                                    .font(.caption)
                                    .foregroundStyle(Color.heliosWarning)
                            }
                        }
                        .padding(10)
                        .background(.thinMaterial, in: RoundedRectangle(cornerRadius: 12, style: .continuous))
                    }
                }
            }
        }
    }

    private var processingStep: some View {
        VStack(alignment: .leading, spacing: 18) {
            Text("Default processing policy")
                .font(.title2.bold())

            HStack(alignment: .top, spacing: 20) {
                VStack(alignment: .leading, spacing: 14) {
                    Stepper("Concurrent jobs: \(settings.transcode.concurrentJobs)", value: transcodeConcurrentJobsBinding, in: 1...12)
                    Stepper("Minimum file size: \(settings.transcode.minFileSizeMB) MB", value: transcodeMinFileSizeBinding, in: 50...5000, step: 50)

                    VStack(alignment: .leading, spacing: 6) {
                        Text("Size reduction threshold: \(Int(settings.transcode.sizeReductionThreshold * 100))%")
                        Slider(value: transcodeReductionThresholdBinding, in: 0...1, step: 0.05)
                    }

                    VStack(alignment: .leading, spacing: 6) {
                        Text("Bits-per-pixel floor: \(settings.transcode.minBppThreshold.formatted(.number.precision(.fractionLength(2))))")
                        Slider(value: transcodeMinBppBinding, in: 0...0.5, step: 0.01)
                    }
                }
                .frame(maxWidth: .infinity, alignment: .topLeading)

                VStack(alignment: .leading, spacing: 14) {
                    Picker("Output codec", selection: transcodeOutputCodecBinding) {
                        Text("AV1").tag("av1")
                        Text("HEVC").tag("hevc")
                        Text("H.264").tag("h264")
                    }

                    Picker("Quality profile", selection: transcodeQualityProfileBinding) {
                        Text("Quality").tag("quality")
                        Text("Balanced").tag("balanced")
                        Text("Speed").tag("speed")
                    }

                    Picker("Subtitle mode", selection: transcodeSubtitleModeBinding) {
                        Text("Copy").tag("copy")
                        Text("Burn").tag("burn")
                        Text("Extract").tag("extract")
                        Text("None").tag("none")
                    }

                    Toggle("Allow encoder fallback", isOn: transcodeAllowFallbackBinding)
                    Toggle("Allow quality verification (VMAF)", isOn: qualityEnableVMAFBinding)
                    Toggle("Revert low quality outputs", isOn: qualityRevertBinding)
                }
                .frame(width: 320, alignment: .topLeading)
            }
            .padding(18)
            .background(.thinMaterial, in: RoundedRectangle(cornerRadius: 16, style: .continuous))
        }
    }

    private var runtimeStep: some View {
        VStack(alignment: .leading, spacing: 18) {
            Text("Runtime defaults")
                .font(.title2.bold())

            HStack(alignment: .top, spacing: 20) {
                VStack(alignment: .leading, spacing: 14) {
                    Picker("Theme", selection: appearanceThemeBinding) {
                        ForEach(themeOptions, id: \.id) { option in
                            Text(option.name).tag(Optional(option.id))
                        }
                    }

                    Toggle("Enable telemetry", isOn: systemEnableTelemetryBinding)
                    Toggle("Allow CPU encoding", isOn: hardwareAllowCPUEncodingBinding)
                    Toggle("Allow CPU fallback", isOn: hardwareAllowCPUFallbackBinding)

                    Picker("CPU preset", selection: hardwareCPUPresetBinding) {
                        Text("Slow").tag("slow")
                        Text("Medium").tag("medium")
                        Text("Fast").tag("fast")
                        Text("Faster").tag("faster")
                    }

                    Toggle("Delete source after successful encode", isOn: filesDeleteSourceBinding)
                    TextField("Output suffix", text: filesOutputSuffixBinding)
                        .textFieldStyle(.roundedBorder)
                }
                .frame(maxWidth: .infinity, alignment: .topLeading)

                VStack(alignment: .leading, spacing: 12) {
                    Text("Detected hardware")
                        .font(.headline)
                    if let hardware = model.setup.hardwareInfo {
                        VStack(alignment: .leading, spacing: 6) {
                            Text(hardware.vendor.uppercased())
                                .font(.title3.bold())
                            if let path = hardware.devicePath {
                                Text(path)
                                    .font(.caption.monospaced())
                                    .foregroundStyle(.secondary)
                                    .lineLimit(2)
                            }
                            Text(hardware.supportedCodecs.joined(separator: ", ").uppercased())
                                .font(.caption)
                                .foregroundStyle(.secondary)
                        }
                        .padding(14)
                        .background(.thinMaterial, in: RoundedRectangle(cornerRadius: 14, style: .continuous))
                    } else {
                        EmptyHeroCard(title: "Hardware still loading", detail: "Alchemist can finish setup even if hardware detection is still warming up.")
                    }
                }
                .frame(width: 340, alignment: .topLeading)
            }
            .padding(18)
            .background(.thinMaterial, in: RoundedRectangle(cornerRadius: 16, style: .continuous))

            VStack(alignment: .leading, spacing: 16) {
                HStack {
                    Toggle("Enable notifications", isOn: notificationsEnabledBinding)
                    Spacer()
                    Button {
                        model.setup.addNotificationTarget()
                    } label: {
                        Label("Add Target", systemImage: "bell.badge")
                    }
                    .buttonStyle(.glass)
                }

                if settings.notifications.targets.isEmpty {
                    EmptyHeroCard(title: "No notification targets", detail: "Add Discord, webhook, Gotify, or another destination if you want setup-time alerts.")
                } else {
                    VStack(spacing: 12) {
                        ForEach(Array(settings.notifications.targets.enumerated()), id: \.element.id) { index, target in
                            notificationTargetEditor(index: index, target: target)
                        }
                    }
                }
            }
            .padding(18)
            .background(.thinMaterial, in: RoundedRectangle(cornerRadius: 16, style: .continuous))

            VStack(alignment: .leading, spacing: 16) {
                HStack {
                    Text("Schedule windows")
                        .font(.headline)
                    Spacer()
                    Button {
                        model.setup.addScheduleWindow()
                    } label: {
                        Label("Add Window", systemImage: "calendar.badge.plus")
                    }
                    .buttonStyle(.glass)
                }

                if settings.schedule.windows.isEmpty {
                    EmptyHeroCard(title: "No schedule windows", detail: "Leave empty to let the queue run anytime, or add quiet-hour windows now.")
                } else {
                    VStack(spacing: 12) {
                        ForEach(Array(settings.schedule.windows.enumerated()), id: \.element.id) { index, window in
                            scheduleWindowEditor(index: index, window: window)
                        }
                    }
                }
            }
            .padding(18)
            .background(.thinMaterial, in: RoundedRectangle(cornerRadius: 16, style: .continuous))
        }
    }

    private var reviewStep: some View {
        VStack(alignment: .leading, spacing: 18) {
            Text("Review setup")
                .font(.title2.bold())

            VStack(alignment: .leading, spacing: 10) {
                summaryRow("Admin", model.setup.username)
                summaryRow("Folders", "\(settings.scanner.directories.count)")
                summaryRow("Codec", settings.transcode.outputCodec.uppercased())
                summaryRow("Quality", settings.transcode.qualityProfile.capitalized)
                summaryRow("Concurrent jobs", "\(settings.transcode.concurrentJobs)")
                summaryRow("Theme", settings.appearance.activeThemeID ?? "system")
                summaryRow("Telemetry", settings.system.enableTelemetry ? "Enabled" : "Disabled")
                summaryRow("Notification targets", "\(settings.notifications.targets.count)")
                summaryRow("Schedule windows", "\(settings.schedule.windows.count)")
                if let preview = model.setup.preview {
                    summaryRow("Previewed media files", "\(preview.totalMediaFiles)")
                }
            }
            .padding(18)
            .background(.thinMaterial, in: RoundedRectangle(cornerRadius: 16, style: .continuous))

            Text("Submitting will save config, create the first account, start the initial scan, and sign this native app in using the new session.")
                .font(.subheadline)
                .foregroundStyle(.secondary)

            if let setupURL {
                Text("Fallback URL: \(setupURL.absoluteString)")
                    .font(.caption.monospaced())
                    .foregroundStyle(.tertiary)
            }
        }
    }

    private func notificationTargetEditor(index: Int, target: SetupNotificationTarget) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack {
                Text(target.name.isEmpty ? "Notification Target" : target.name)
                    .font(.headline)
                Spacer()
                Toggle("Enabled", isOn: notificationEnabledBinding(index: index))
                    .toggleStyle(.switch)
                    .labelsHidden()
                Button(role: .destructive) {
                    model.setup.removeNotificationTarget(id: target.id)
                } label: {
                    Image(systemName: "trash")
                }
                .buttonStyle(.plain)
            }

            HStack(spacing: 12) {
                TextField("Name", text: notificationNameBinding(index: index))
                    .textFieldStyle(.roundedBorder)
                Picker("Type", selection: notificationTypeBinding(index: index)) {
                    Text("Discord").tag("discord_webhook")
                    Text("Webhook").tag("webhook")
                    Text("Gotify").tag("gotify")
                }
                .frame(width: 160)
            }

            TextField("Endpoint URL / server", text: notificationEndpointBinding(index: index))
                .textFieldStyle(.roundedBorder)

            TextField("Optional auth token", text: notificationAuthTokenBinding(index: index))
                .textFieldStyle(.roundedBorder)

            HStack(spacing: 8) {
                ForEach(notificationEventOptions, id: \.self) { event in
                    let selected = target.events.contains(event)
                    Button {
                        model.setup.toggleNotificationEvent(targetID: target.id, event: event)
                    } label: {
                        Text(event.capitalized)
                            .font(.caption.bold())
                            .padding(.horizontal, 10)
                            .padding(.vertical, 6)
                            .background(selected ? Color.heliosAccent.opacity(0.18) : Color.white.opacity(0.05), in: Capsule())
                            .foregroundStyle(selected ? Color.heliosAccent : .secondary)
                    }
                    .buttonStyle(.plain)
                }
            }
        }
        .padding(14)
        .background(.ultraThinMaterial, in: RoundedRectangle(cornerRadius: 14, style: .continuous))
    }

    private func scheduleWindowEditor(index: Int, window: SetupScheduleWindow) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack {
                Text("Window \(index + 1)")
                    .font(.headline)
                Spacer()
                Toggle("Enabled", isOn: scheduleEnabledBinding(index: index))
                    .labelsHidden()
                Button(role: .destructive) {
                    model.setup.removeScheduleWindow(id: window.id)
                } label: {
                    Image(systemName: "trash")
                }
                .buttonStyle(.plain)
            }

            HStack(spacing: 12) {
                TextField("Start HH:MM", text: scheduleStartBinding(index: index))
                    .textFieldStyle(.roundedBorder)
                TextField("End HH:MM", text: scheduleEndBinding(index: index))
                    .textFieldStyle(.roundedBorder)
            }

            HStack(spacing: 8) {
                ForEach(Array(weekdayOptions.enumerated()), id: \.offset) { day, label in
                    let selected = window.daysOfWeek.contains(day)
                    Button {
                        model.setup.toggleScheduleDay(windowID: window.id, day: day)
                    } label: {
                        Text(label)
                            .font(.caption.bold())
                            .padding(.horizontal, 8)
                            .padding(.vertical, 6)
                            .background(selected ? Color.heliosAccent.opacity(0.18) : Color.white.opacity(0.05), in: Capsule())
                            .foregroundStyle(selected ? Color.heliosAccent : .secondary)
                    }
                    .buttonStyle(.plain)
                }
            }
        }
        .padding(14)
        .background(.ultraThinMaterial, in: RoundedRectangle(cornerRadius: 14, style: .continuous))
    }

    private var footer: some View {
        HStack {
            Button("Back") {
                model.setup.goBack()
            }
            .buttonStyle(.glass)
            .disabled(!model.setup.canGoBack || model.setup.isSubmitting)

            Spacer()

            if model.setup.step == .review {
                Button {
                    Task { await model.completeNativeSetup() }
                } label: {
                    if model.setup.isSubmitting {
                        ProgressView()
                    } else {
                        Label("Finish Setup", systemImage: "checkmark.circle.fill")
                    }
                }
                .buttonStyle(.glassProminent)
                .tint(model.theme.accent.color)
                .disabled(!model.setup.configMutable || model.setup.isSubmitting)
            } else {
                Button {
                    Task {
                        if model.setup.step == .library {
                            await model.setup.previewDirectories(apiClient: model.connection.apiClient)
                        }
                        if let message = model.setup.validateCurrentStep() {
                            model.setup.lastError = .apiError(code: "setup_validation_failed", message: message)
                            return
                        }
                        model.setup.lastError = nil
                        model.setup.goNext()
                    }
                } label: {
                    Label("Next", systemImage: "arrow.right")
                }
                .buttonStyle(.glassProminent)
                .tint(model.theme.accent.color)
                .disabled(model.setup.isSubmitting)
            }
        }
    }

    private func labeledField<Content: View>(_ title: String, @ViewBuilder content: () -> Content) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(title)
                .font(.caption.bold())
                .foregroundStyle(.secondary)
            content()
        }
    }

    private func summaryRow(_ label: String, _ value: String) -> some View {
        HStack {
            Text(label)
                .foregroundStyle(.secondary)
            Spacer()
            Text(value)
                .font(.subheadline.monospacedDigit().bold())
        }
    }

    private var usernameBinding: Binding<String> {
        Binding(
            get: { model.setup.username },
            set: { model.setup.username = $0 }
        )
    }

    private var appearanceThemeBinding: Binding<String?> {
        Binding(
            get: { model.setup.settings.appearance.activeThemeID },
            set: { model.setup.settings.appearance.activeThemeID = $0 }
        )
    }

    private var notificationsEnabledBinding: Binding<Bool> {
        Binding(
            get: { model.setup.settings.notifications.enabled },
            set: { model.setup.settings.notifications.enabled = $0 }
        )
    }

    private func notificationNameBinding(index: Int) -> Binding<String> {
        Binding(
            get: { model.setup.settings.notifications.targets[index].name },
            set: { model.setup.settings.notifications.targets[index].name = $0 }
        )
    }

    private func notificationTypeBinding(index: Int) -> Binding<String> {
        Binding(
            get: { model.setup.settings.notifications.targets[index].targetType },
            set: { model.setup.settings.notifications.targets[index].targetType = $0 }
        )
    }

    private func notificationEndpointBinding(index: Int) -> Binding<String> {
        Binding(
            get: { model.setup.settings.notifications.targets[index].endpointURL },
            set: { model.setup.settings.notifications.targets[index].endpointURL = $0 }
        )
    }

    private func notificationAuthTokenBinding(index: Int) -> Binding<String> {
        Binding(
            get: { model.setup.settings.notifications.targets[index].authToken ?? "" },
            set: { model.setup.settings.notifications.targets[index].authToken = $0.isEmpty ? nil : $0 }
        )
    }

    private func notificationEnabledBinding(index: Int) -> Binding<Bool> {
        Binding(
            get: { model.setup.settings.notifications.targets[index].enabled },
            set: { model.setup.settings.notifications.targets[index].enabled = $0 }
        )
    }

    private func scheduleStartBinding(index: Int) -> Binding<String> {
        Binding(
            get: { model.setup.settings.schedule.windows[index].startTime },
            set: { model.setup.settings.schedule.windows[index].startTime = $0 }
        )
    }

    private func scheduleEndBinding(index: Int) -> Binding<String> {
        Binding(
            get: { model.setup.settings.schedule.windows[index].endTime },
            set: { model.setup.settings.schedule.windows[index].endTime = $0 }
        )
    }

    private func scheduleEnabledBinding(index: Int) -> Binding<Bool> {
        Binding(
            get: { model.setup.settings.schedule.windows[index].enabled },
            set: { model.setup.settings.schedule.windows[index].enabled = $0 }
        )
    }

    private var passwordBinding: Binding<String> {
        Binding(
            get: { model.setup.password },
            set: { model.setup.password = $0 }
        )
    }

    private var confirmPasswordBinding: Binding<String> {
        Binding(
            get: { model.setup.confirmPassword },
            set: { model.setup.confirmPassword = $0 }
        )
    }

    private var transcodeConcurrentJobsBinding: Binding<Int> {
        Binding(
            get: { model.setup.settings.transcode.concurrentJobs },
            set: { model.setup.settings.transcode.concurrentJobs = $0 }
        )
    }

    private var transcodeMinFileSizeBinding: Binding<Int> {
        Binding(
            get: { model.setup.settings.transcode.minFileSizeMB },
            set: { model.setup.settings.transcode.minFileSizeMB = $0 }
        )
    }

    private var transcodeReductionThresholdBinding: Binding<Double> {
        Binding(
            get: { model.setup.settings.transcode.sizeReductionThreshold },
            set: { model.setup.settings.transcode.sizeReductionThreshold = $0 }
        )
    }

    private var transcodeMinBppBinding: Binding<Double> {
        Binding(
            get: { model.setup.settings.transcode.minBppThreshold },
            set: { model.setup.settings.transcode.minBppThreshold = $0 }
        )
    }

    private var transcodeOutputCodecBinding: Binding<String> {
        Binding(
            get: { model.setup.settings.transcode.outputCodec },
            set: { model.setup.settings.transcode.outputCodec = $0 }
        )
    }

    private var transcodeQualityProfileBinding: Binding<String> {
        Binding(
            get: { model.setup.settings.transcode.qualityProfile },
            set: { model.setup.settings.transcode.qualityProfile = $0 }
        )
    }

    private var transcodeSubtitleModeBinding: Binding<String> {
        Binding(
            get: { model.setup.settings.transcode.subtitleMode },
            set: { model.setup.settings.transcode.subtitleMode = $0 }
        )
    }

    private var transcodeAllowFallbackBinding: Binding<Bool> {
        Binding(
            get: { model.setup.settings.transcode.allowFallback },
            set: { model.setup.settings.transcode.allowFallback = $0 }
        )
    }

    private var qualityEnableVMAFBinding: Binding<Bool> {
        Binding(
            get: { model.setup.settings.quality.enableVMAF },
            set: { model.setup.settings.quality.enableVMAF = $0 }
        )
    }

    private var qualityRevertBinding: Binding<Bool> {
        Binding(
            get: { model.setup.settings.quality.revertOnLowQuality },
            set: { model.setup.settings.quality.revertOnLowQuality = $0 }
        )
    }

    private var systemEnableTelemetryBinding: Binding<Bool> {
        Binding(
            get: { model.setup.settings.system.enableTelemetry },
            set: { model.setup.settings.system.enableTelemetry = $0 }
        )
    }

    private var hardwareAllowCPUEncodingBinding: Binding<Bool> {
        Binding(
            get: { model.setup.settings.hardware.allowCPUEncoding },
            set: { model.setup.settings.hardware.allowCPUEncoding = $0 }
        )
    }

    private var hardwareAllowCPUFallbackBinding: Binding<Bool> {
        Binding(
            get: { model.setup.settings.hardware.allowCPUFallback },
            set: { model.setup.settings.hardware.allowCPUFallback = $0 }
        )
    }

    private var hardwareCPUPresetBinding: Binding<String> {
        Binding(
            get: { model.setup.settings.hardware.cpuPreset },
            set: { model.setup.settings.hardware.cpuPreset = $0 }
        )
    }

    private var filesDeleteSourceBinding: Binding<Bool> {
        Binding(
            get: { model.setup.settings.files.deleteSource },
            set: { model.setup.settings.files.deleteSource = $0 }
        )
    }

    private var filesOutputSuffixBinding: Binding<String> {
        Binding(
            get: { model.setup.settings.files.outputSuffix },
            set: { model.setup.settings.files.outputSuffix = $0 }
        )
    }

    private var themeOptions: [(id: String, name: String)] {
        [
            ("helios-orange", "Helios Orange"),
            ("sunset", "Sunset"),
            ("midnight", "Midnight"),
            ("emerald", "Emerald"),
            ("deep-blue", "Deep Blue"),
            ("lavender", "Lavender"),
        ]
    }

    private var notificationEventOptions: [String] {
        ["completed", "failed", "queued"]
    }

    private var weekdayOptions: [String] {
        ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"]
    }
}

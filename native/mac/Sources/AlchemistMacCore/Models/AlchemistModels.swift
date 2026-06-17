import Foundation

public enum ConnectionMode: String, CaseIterable, Identifiable, Sendable {
    case bundled
    case remote

    public var id: String { rawValue }

    public var label: String {
        switch self {
        case .bundled: "Bundled daemon"
        case .remote: "Existing server"
        }
    }
}

public enum AppSection: String, CaseIterable, Identifiable, Sendable {
    case dashboard
    case queue
    case logs
    case statistics
    case intelligence
    case convert
    case system
    case settings

    public var id: String { rawValue }

    public var label: String {
        switch self {
        case .dashboard: "Dashboard"
        case .queue: "Jobs"
        case .logs: "Logs"
        case .statistics: "Statistics"
        case .intelligence: "Intelligence"
        case .convert: "Convert"
        case .system: "System"
        case .settings: "Settings"
        }
    }

    public var symbol: String {
        switch self {
        case .dashboard: "gauge.with.dots.needle.bottom.50percent"
        case .queue: "film.stack"
        case .logs: "terminal"
        case .statistics: "chart.bar.xaxis"
        case .intelligence: "sparkles"
        case .convert: "wand.and.sparkles"
        case .system: "waveform.path.ecg"
        case .settings: "gearshape"
        }
    }
}

public enum JobTab: String, CaseIterable, Identifiable, Codable, Sendable {
    case all
    case active
    case queued
    case completed
    case failed
    case skipped
    case archived

    public var id: String { rawValue }

    public var label: String {
        switch self {
        case .all: "All"
        case .active: "Active"
        case .queued: "Queued"
        case .completed: "Completed"
        case .failed: "Failed"
        case .skipped: "Skipped"
        case .archived: "Archived"
        }
    }

    public var statusFilter: [String] {
        switch self {
        case .all, .archived: []
        case .active: ["analyzing", "encoding", "remuxing", "resuming"]
        case .queued: ["queued"]
        case .completed: ["completed"]
        case .failed: ["failed", "cancelled"]
        case .skipped: ["skipped"]
        }
    }

    public var includesArchived: Bool {
        self == .archived
    }
}

public enum JobSortField: String, CaseIterable, Identifiable, Codable, Sendable {
    case updatedAt = "updated_at"
    case createdAt = "created_at"
    case inputPath = "input_path"
    case size

    public var id: String { rawValue }

    public var label: String {
        switch self {
        case .updatedAt: "Last Updated"
        case .createdAt: "Date Added"
        case .inputPath: "File Name"
        case .size: "File Size"
        }
    }
}

public struct SavedJobView: Codable, Identifiable, Equatable, Sendable {
    public let id: String
    public let label: String
    public let activeTab: JobTab
    public let sortBy: JobSortField
    public let sortDesc: Bool
    public let search: String?

    public init(
        id: String,
        label: String,
        activeTab: JobTab,
        sortBy: JobSortField,
        sortDesc: Bool,
        search: String?
    ) {
        self.id = id
        self.label = label
        self.activeTab = activeTab
        self.sortBy = sortBy
        self.sortDesc = sortDesc
        self.search = search
    }

    enum CodingKeys: String, CodingKey {
        case id
        case label
        case activeTab
        case sortBy
        case sortDesc
        case search
    }

    public static let builtIn: [SavedJobView] = [
        SavedJobView(id: "builtin-recent-failures", label: "Recent Failures", activeTab: .failed, sortBy: .updatedAt, sortDesc: true, search: nil),
        SavedJobView(id: "builtin-queued", label: "Queued", activeTab: .queued, sortBy: .createdAt, sortDesc: false, search: nil),
        SavedJobView(id: "builtin-recent-completions", label: "Recent Completions", activeTab: .completed, sortBy: .updatedAt, sortDesc: true, search: nil),
    ]
}

public struct APIErrorEnvelope: Decodable, Error, Sendable {
    public struct Payload: Decodable, Sendable {
        public let code: String
        public let message: String
    }

    public let error: Payload
}

public struct Job: Decodable, Identifiable, Hashable, Sendable {
    public let id: Int64
    public let inputPath: String
    public let outputPath: String?
    public let status: String
    public let priority: Int?
    public let progress: Double?
    public let createdAt: String?
    public let updatedAt: String?
    public let attemptCount: Int?
    public let encoder: String?
    public let vmafScore: Double?
    public let decisionReason: String?
    public let decisionExplanation: ExplanationPayload?

    enum CodingKeys: String, CodingKey {
        case id
        case inputPath = "input_path"
        case outputPath = "output_path"
        case status
        case priority
        case progress
        case createdAt = "created_at"
        case updatedAt = "updated_at"
        case attemptCount = "attempt_count"
        case encoder
        case vmafScore = "vmaf_score"
        case decisionReason = "decision_reason"
        case decisionExplanation = "decision_explanation"
    }

    public init(
        id: Int64,
        inputPath: String,
        outputPath: String?,
        status: String,
        priority: Int?,
        progress: Double?,
        createdAt: String?,
        updatedAt: String?,
        attemptCount: Int?,
        encoder: String?,
        vmafScore: Double? = nil,
        decisionReason: String? = nil,
        decisionExplanation: ExplanationPayload? = nil
    ) {
        self.id = id
        self.inputPath = inputPath
        self.outputPath = outputPath
        self.status = status
        self.priority = priority
        self.progress = progress
        self.createdAt = createdAt
        self.updatedAt = updatedAt
        self.attemptCount = attemptCount
        self.encoder = encoder
        self.vmafScore = vmafScore
        self.decisionReason = decisionReason
        self.decisionExplanation = decisionExplanation
    }

    public var fileName: String {
        URL(fileURLWithPath: inputPath).lastPathComponent
    }

    public var progressFraction: Double {
        max(0, min((progress ?? 0) / 100, 1))
    }

    public var isActive: Bool {
        ["analyzing", "encoding", "remuxing", "resuming"].contains(status)
    }

    public var isTerminal: Bool {
        ["completed", "skipped", "failed", "cancelled"].contains(status)
    }

    public func withProgress(_ progress: Double) -> Job {
        Job(
            id: id,
            inputPath: inputPath,
            outputPath: outputPath,
            status: status,
            priority: priority,
            progress: progress,
            createdAt: createdAt,
            updatedAt: updatedAt,
            attemptCount: attemptCount,
            encoder: encoder,
            vmafScore: vmafScore,
            decisionReason: decisionReason,
            decisionExplanation: decisionExplanation
        )
    }

    public func withStatus(_ status: String) -> Job {
        Job(
            id: id,
            inputPath: inputPath,
            outputPath: outputPath,
            status: status,
            priority: priority,
            progress: progress,
            createdAt: createdAt,
            updatedAt: updatedAt,
            attemptCount: attemptCount,
            encoder: encoder,
            vmafScore: vmafScore,
            decisionReason: decisionReason,
            decisionExplanation: decisionExplanation
        )
    }

    public func withPriority(_ priority: Int) -> Job {
        Job(
            id: id,
            inputPath: inputPath,
            outputPath: outputPath,
            status: status,
            priority: priority,
            progress: progress,
            createdAt: createdAt,
            updatedAt: updatedAt,
            attemptCount: attemptCount,
            encoder: encoder,
            vmafScore: vmafScore,
            decisionReason: decisionReason,
            decisionExplanation: decisionExplanation
        )
    }
}

public struct JobStats: Decodable, Equatable, Sendable {
    public let total: Int
    public let completed: Int
    public let active: Int
    public let failed: Int
    public let concurrentLimit: Int

    public static let empty = JobStats(total: 0, completed: 0, active: 0, failed: 0, concurrentLimit: 0)

    enum CodingKeys: String, CodingKey {
        case total
        case completed
        case active
        case failed
        case concurrentLimit = "concurrent_limit"
    }
}

public struct SavingsSummary: Decodable, Equatable, Sendable {
    public let totalInputBytes: Int64
    public let totalOutputBytes: Int64
    public let totalBytesSaved: Int64
    public let savingsPercent: Double
    public let jobCount: Int

    public static let empty = SavingsSummary(
        totalInputBytes: 0,
        totalOutputBytes: 0,
        totalBytesSaved: 0,
        savingsPercent: 0,
        jobCount: 0
    )

    enum CodingKeys: String, CodingKey {
        case totalInputBytes = "total_input_bytes"
        case totalOutputBytes = "total_output_bytes"
        case totalBytesSaved = "total_bytes_saved"
        case savingsPercent = "savings_percent"
        case jobCount = "job_count"
    }
}

public struct DailyStat: Decodable, Equatable, Sendable {
    public let date: String
    public let jobsCompleted: Int
    public let bytesSaved: Int64

    enum CodingKeys: String, CodingKey {
        case date
        case jobsCompleted = "jobs_completed"
        case bytesSaved = "bytes_saved"
    }
}

public struct EngineStatus: Decodable, Equatable, Sendable {
    public let status: String
    public let manualPaused: Bool
    public let schedulerPaused: Bool
    public let draining: Bool
    public let mode: String?
    public let concurrentLimit: Int?
    public let hardwarePending: Bool?

    public static let offline = EngineStatus(
        status: "offline",
        manualPaused: false,
        schedulerPaused: false,
        draining: false,
        mode: nil,
        concurrentLimit: nil,
        hardwarePending: nil
    )

    enum CodingKeys: String, CodingKey {
        case status
        case manualPaused = "manual_paused"
        case schedulerPaused = "scheduler_paused"
        case draining
        case mode
        case concurrentLimit = "concurrent_limit"
        case hardwarePending = "hardware_pending"
    }
}

public struct SystemInfo: Decodable, Equatable, Sendable {
    public let version: String
    public let osVersion: String
    public let isDocker: Bool
    public let telemetryEnabled: Bool
    public let ffmpegVersion: String

    enum CodingKeys: String, CodingKey {
        case version
        case osVersion = "os_version"
        case isDocker = "is_docker"
        case telemetryEnabled = "telemetry_enabled"
        case ffmpegVersion = "ffmpeg_version"
    }
}

public struct SystemResources: Decodable, Equatable, Sendable {
    public let cpuPercent: Float
    public let memoryUsedMb: UInt64
    public let memoryTotalMb: UInt64
    public let memoryPercent: Float
    public let uptimeSeconds: UInt64
    public let activeJobs: Int64
    public let concurrentLimit: Int
    public let cpuCount: Int
    public let gpuUtilization: Float?
    public let gpuMemoryPercent: Float?

    public static let empty = SystemResources(
        cpuPercent: 0,
        memoryUsedMb: 0,
        memoryTotalMb: 0,
        memoryPercent: 0,
        uptimeSeconds: 0,
        activeJobs: 0,
        concurrentLimit: 0,
        cpuCount: 0,
        gpuUtilization: nil,
        gpuMemoryPercent: nil
    )

    enum CodingKeys: String, CodingKey {
        case cpuPercent = "cpu_percent"
        case memoryUsedMb = "memory_used_mb"
        case memoryTotalMb = "memory_total_mb"
        case memoryPercent = "memory_percent"
        case uptimeSeconds = "uptime_seconds"
        case activeJobs = "active_jobs"
        case concurrentLimit = "concurrent_limit"
        case cpuCount = "cpu_count"
        case gpuUtilization = "gpu_utilization"
        case gpuMemoryPercent = "gpu_memory_percent"
    }

    public var uptimeDescription: String {
        let hours = uptimeSeconds / 3600
        let minutes = (uptimeSeconds % 3600) / 60
        if hours > 0 {
            return "\(hours)h \(minutes)m"
        } else {
            return "\(minutes)m"
        }
    }
}

public struct EnqueueJobResponse: Decodable, Equatable, Sendable {
    public let enqueued: Bool
    public let message: String
}

public struct WatchDirectory: Decodable, Identifiable, Hashable, Sendable {
    public let id: Int64
    public let path: String
    public let isRecursive: Bool?
    public let profileId: Int64?
    public let profileName: String?

    enum CodingKeys: String, CodingKey {
        case id
        case path
        case isRecursive = "is_recursive"
        case profileId = "profile_id"
        case profileName = "profile_name"
    }
}

public struct LibraryProfile: Decodable, Identifiable, Hashable, Sendable {
    public let id: Int64
    public let name: String
    public let preset: String
    public let codec: String
    public let qualityProfile: String
    public let hdrMode: String
    public let audioMode: String
    public let crfOverride: Int?
    public let notes: String?

    enum CodingKeys: String, CodingKey {
        case id
        case name
        case preset
        case codec
        case qualityProfile = "quality_profile"
        case hdrMode = "hdr_mode"
        case audioMode = "audio_mode"
        case crfOverride = "crf_override"
        case notes
    }
}

public struct ConversionUploadResponse: Decodable, Sendable {
    public let conversionJobID: Int64

    enum CodingKeys: String, CodingKey {
        case conversionJobID = "conversion_job_id"
    }
}

public struct ExplanationPayload: Codable, Equatable, Hashable, Sendable {
    public let category: String
    public let code: String
    public let summary: String
    public let detail: String
    public let operatorGuidance: String?
    public let measured: [String: JSONValue]
    public let legacyReason: String

    enum CodingKeys: String, CodingKey {
        case category
        case code
        case summary
        case detail
        case operatorGuidance = "operator_guidance"
        case measured
        case legacyReason = "legacy_reason"
    }
}

public enum JSONValue: Codable, Equatable, Hashable, Sendable {
    case string(String)
    case number(Double)
    case bool(Bool)
    case array([JSONValue])
    case object([String: JSONValue])
    case null

    public init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        if container.decodeNil() {
            self = .null
        } else if let bool = try? container.decode(Bool.self) {
            self = .bool(bool)
        } else if let number = try? container.decode(Double.self) {
            self = .number(number)
        } else if let array = try? container.decode([JSONValue].self) {
            self = .array(array)
        } else if let object = try? container.decode([String: JSONValue].self) {
            self = .object(object)
        } else {
            self = .string((try? container.decode(String.self)) ?? "")
        }
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        switch self {
        case .string(let value):
            try container.encode(value)
        case .number(let value):
            try container.encode(value)
        case .bool(let value):
            try container.encode(value)
        case .array(let value):
            try container.encode(value)
        case .object(let value):
            try container.encode(value)
        case .null:
            try container.encodeNil()
        }
    }

    public var displayString: String {
        switch self {
        case .string(let value):
            value
        case .number(let value):
            value.formatted(.number.precision(.fractionLength(0...2)))
        case .bool(let value):
            value ? "true" : "false"
        case .array(let value):
            "\(value.count) items"
        case .object:
            "Details"
        case .null:
            "-"
        }
    }
}

public struct SettingsBundleResponse: Decodable, Equatable, Sendable {
    public let settings: SetupSettings
    public let sourceOfTruth: String?
    public let projectionStatus: String?

    enum CodingKeys: String, CodingKey {
        case settings
        case sourceOfTruth = "source_of_truth"
        case projectionStatus = "projection_status"
    }
}

public struct SetupStatusResponse: Codable, Equatable, Sendable {
    public let setupRequired: Bool
    public let enableTelemetry: Bool?
    public let configMutable: Bool?

    enum CodingKeys: String, CodingKey {
        case setupRequired = "setup_required"
        case enableTelemetry = "enable_telemetry"
        case configMutable = "config_mutable"
    }
}

public struct SetupSettings: Codable, Equatable, Sendable {
    public var appearance: SetupAppearanceSettings
    public var scanner: SetupScannerSettings
    public var transcode: SetupTranscodeSettings
    public var hardware: SetupHardwareSettings
    public var files: SetupFileSettings
    public var quality: SetupQualitySettings
    public var notifications: SetupNotificationSettings
    public var schedule: SetupScheduleSettings
    public var system: SetupSystemSettings

    public static let defaultValue = SetupSettings(
        appearance: .init(activeThemeID: "helios-orange"),
        scanner: .init(directories: [], watchEnabled: true, extraWatchDirs: []),
        transcode: .init(
            concurrentJobs: 2,
            sizeReductionThreshold: 0.3,
            minBppThreshold: 0.1,
            minFileSizeMB: 100,
            outputCodec: "av1",
            qualityProfile: "balanced",
            allowFallback: true,
            subtitleMode: "copy"
        ),
        hardware: .init(
            allowCPUEncoding: true,
            allowCPUFallback: true,
            preferredVendor: nil,
            cpuPreset: "medium",
            devicePath: nil
        ),
        files: .init(
            deleteSource: false,
            outputExtension: "mkv",
            outputSuffix: "-alchemist",
            replaceStrategy: "keep",
            outputRoot: nil
        ),
        quality: .init(enableVMAF: false, minVMAFScore: 90, revertOnLowQuality: true),
        notifications: .init(enabled: false, targets: []),
        schedule: .init(windows: []),
        system: .init(
            enableTelemetry: false,
            monitoringPollInterval: 2,
            conversionUploadLimitGB: 8,
            conversionDownloadRetentionHours: 1
        )
    )

    public func mergedWithDefaults(enableTelemetryOverride: Bool?) -> SetupSettings {
        var merged = Self.defaultValue
        merged.appearance = appearance
        merged.scanner = scanner
        merged.transcode = transcode
        merged.hardware = hardware
        merged.files = files
        merged.quality = quality
        merged.notifications = notifications
        merged.schedule = schedule
        merged.system = system
        if let enableTelemetryOverride {
            merged.system.enableTelemetry = enableTelemetryOverride
        }
        return merged
    }
}

public struct SetupAppearanceSettings: Codable, Equatable, Sendable {
    public var activeThemeID: String?

    enum CodingKeys: String, CodingKey {
        case activeThemeID = "active_theme_id"
    }
}

public struct SetupWatchDir: Codable, Equatable, Sendable {
    public var path: String
    public var isRecursive: Bool

    enum CodingKeys: String, CodingKey {
        case path
        case isRecursive = "is_recursive"
    }
}

public struct SetupScannerSettings: Codable, Equatable, Sendable {
    public var directories: [String]
    public var watchEnabled: Bool
    public var extraWatchDirs: [SetupWatchDir]

    enum CodingKeys: String, CodingKey {
        case directories
        case watchEnabled = "watch_enabled"
        case extraWatchDirs = "extra_watch_dirs"
    }
}

public struct SetupTranscodeSettings: Codable, Equatable, Sendable {
    public var concurrentJobs: Int
    public var sizeReductionThreshold: Double
    public var minBppThreshold: Double
    public var minFileSizeMB: Int
    public var outputCodec: String
    public var qualityProfile: String
    public var allowFallback: Bool
    public var subtitleMode: String

    enum CodingKeys: String, CodingKey {
        case concurrentJobs = "concurrent_jobs"
        case sizeReductionThreshold = "size_reduction_threshold"
        case minBppThreshold = "min_bpp_threshold"
        case minFileSizeMB = "min_file_size_mb"
        case outputCodec = "output_codec"
        case qualityProfile = "quality_profile"
        case allowFallback = "allow_fallback"
        case subtitleMode = "subtitle_mode"
    }
}

public struct SetupHardwareSettings: Codable, Equatable, Sendable {
    public var allowCPUEncoding: Bool
    public var allowCPUFallback: Bool
    public var preferredVendor: String?
    public var cpuPreset: String
    public var devicePath: String?

    enum CodingKeys: String, CodingKey {
        case allowCPUEncoding = "allow_cpu_encoding"
        case allowCPUFallback = "allow_cpu_fallback"
        case preferredVendor = "preferred_vendor"
        case cpuPreset = "cpu_preset"
        case devicePath = "device_path"
    }
}

public struct SetupFileSettings: Codable, Equatable, Sendable {
    public var deleteSource: Bool
    public var outputExtension: String
    public var outputSuffix: String
    public var replaceStrategy: String
    public var outputRoot: String?

    enum CodingKeys: String, CodingKey {
        case deleteSource = "delete_source"
        case outputExtension = "output_extension"
        case outputSuffix = "output_suffix"
        case replaceStrategy = "replace_strategy"
        case outputRoot = "output_root"
    }
}

public struct SetupQualitySettings: Codable, Equatable, Sendable {
    public var enableVMAF: Bool
    public var minVMAFScore: Double
    public var revertOnLowQuality: Bool

    enum CodingKeys: String, CodingKey {
        case enableVMAF = "enable_vmaf"
        case minVMAFScore = "min_vmaf_score"
        case revertOnLowQuality = "revert_on_low_quality"
    }
}

public struct SetupNotificationTarget: Codable, Equatable, Sendable, Identifiable {
    public var localID = UUID()
    public var id: UUID { localID }
    public var name: String
    public var targetType: String
    public var configJSON: [String: JSONValue]
    public var endpointURL: String
    public var authToken: String?
    public var events: [String]
    public var enabled: Bool

    public init(
        name: String,
        targetType: String,
        configJSON: [String: JSONValue] = [:],
        endpointURL: String,
        authToken: String?,
        events: [String],
        enabled: Bool,
        localID: UUID = UUID()
    ) {
        self.localID = localID
        self.name = name
        self.targetType = targetType
        self.configJSON = configJSON
        self.endpointURL = endpointURL
        self.authToken = authToken
        self.events = events
        self.enabled = enabled
    }

    enum CodingKeys: String, CodingKey {
        case name
        case targetType = "target_type"
        case configJSON = "config_json"
        case endpointURL = "endpoint_url"
        case authToken = "auth_token"
        case events
        case enabled
    }
}

public struct SetupNotificationSettings: Codable, Equatable, Sendable {
    public var enabled: Bool
    public var targets: [SetupNotificationTarget]
}

public struct SetupScheduleWindow: Codable, Equatable, Sendable, Identifiable {
    public var localID = UUID()
    public var id: UUID { localID }
    public var startTime: String
    public var endTime: String
    public var daysOfWeek: [Int]
    public var enabled: Bool

    public init(
        startTime: String,
        endTime: String,
        daysOfWeek: [Int],
        enabled: Bool,
        localID: UUID = UUID()
    ) {
        self.localID = localID
        self.startTime = startTime
        self.endTime = endTime
        self.daysOfWeek = daysOfWeek
        self.enabled = enabled
    }

    enum CodingKeys: String, CodingKey {
        case startTime = "start_time"
        case endTime = "end_time"
        case daysOfWeek = "days_of_week"
        case enabled
    }
}

public struct SetupScheduleSettings: Codable, Equatable, Sendable {
    public var windows: [SetupScheduleWindow]
}

public struct SetupSystemSettings: Codable, Equatable, Sendable {
    public var enableTelemetry: Bool
    public var monitoringPollInterval: Double
    public var conversionUploadLimitGB: Int
    public var conversionDownloadRetentionHours: Int

    enum CodingKeys: String, CodingKey {
        case enableTelemetry = "enable_telemetry"
        case monitoringPollInterval = "monitoring_poll_interval"
        case conversionUploadLimitGB = "conversion_upload_limit_gb"
        case conversionDownloadRetentionHours = "conversion_download_retention_hours"
    }
}

public struct HardwareInfo: Codable, Equatable, Sendable {
    public let vendor: String
    public let devicePath: String?
    public let supportedCodecs: [String]

    enum CodingKeys: String, CodingKey {
        case vendor
        case devicePath = "device_path"
        case supportedCodecs = "supported_codecs"
    }
}

public struct FsRecommendation: Codable, Equatable, Sendable, Identifiable {
    public var id: String { path }
    public let path: String
    public let label: String
    public let reason: String
    public let mediaHint: String

    enum CodingKeys: String, CodingKey {
        case path
        case label
        case reason
        case mediaHint = "media_hint"
    }
}

public struct FsRecommendationsResponse: Codable, Equatable, Sendable {
    public let recommendations: [FsRecommendation]
}

public struct FsPreviewRequest: Codable, Equatable, Sendable {
    public let directories: [String]
}

public struct FsPreviewDirectory: Codable, Equatable, Sendable, Identifiable {
    public var id: String { path }
    public let path: String
    public let exists: Bool
    public let readable: Bool
    public let mediaFiles: Int
    public let sampleFiles: [String]
    public let mediaHint: String
    public let warnings: [String]

    enum CodingKeys: String, CodingKey {
        case path
        case exists
        case readable
        case mediaFiles = "media_files"
        case sampleFiles = "sample_files"
        case mediaHint = "media_hint"
        case warnings
    }
}

public struct FsPreviewResponse: Codable, Equatable, Sendable {
    public let directories: [FsPreviewDirectory]
    public let totalMediaFiles: Int
    public let warnings: [String]

    enum CodingKeys: String, CodingKey {
        case directories
        case totalMediaFiles = "total_media_files"
        case warnings
    }
}

public struct SetupCompletePayload: Codable, Sendable {
    public let username: String
    public let password: String
    public let settings: SetupSettings
}

public struct SetupCompleteResponse: Codable, Equatable, Sendable {
    public let status: String
    public let message: String
    public let concurrentJobs: Int

    enum CodingKeys: String, CodingKey {
        case status
        case message
        case concurrentJobs = "concurrent_jobs"
    }
}

public struct IntelligenceResponse: Decodable, Equatable, Sendable {
    public let duplicateGroups: [DuplicateGroup]
    public let totalDuplicates: Int
    public let recommendationCounts: RecommendationCounts
    public let recommendations: [IntelligenceRecommendation]

    enum CodingKeys: String, CodingKey {
        case duplicateGroups = "duplicate_groups"
        case totalDuplicates = "total_duplicates"
        case recommendationCounts = "recommendation_counts"
        case recommendations
    }
}

public struct DuplicateGroup: Decodable, Identifiable, Equatable, Sendable {
    public var id: String { stem }
    public let stem: String
    public let count: Int
    public let paths: [DuplicatePath]
}

public struct DuplicatePath: Decodable, Identifiable, Equatable, Sendable {
    public let id: Int64
    public let path: String
    public let status: String
}

public struct RecommendationCounts: Decodable, Equatable, Sendable {
    public let duplicates: Int
    public let remuxOnlyCandidate: Int
    public let wastefulAudioLayout: Int
    public let commentaryCleanupCandidate: Int

    enum CodingKeys: String, CodingKey {
        case duplicates
        case remuxOnlyCandidate = "remux_only_candidate"
        case wastefulAudioLayout = "wasteful_audio_layout"
        case commentaryCleanupCandidate = "commentary_cleanup_candidate"
    }
}

public struct IntelligenceRecommendation: Decodable, Identifiable, Equatable, Sendable {
    public var id: String { "\(type)-\(path)-\(title)" }
    public let type: String
    public let title: String
    public let summary: String
    public let path: String
    public let suggestedAction: String

    enum CodingKeys: String, CodingKey {
        case type
        case title
        case summary
        case path
        case suggestedAction = "suggested_action"
    }
}

public struct JobMetadata: Decodable, Equatable, Sendable {
    public let durationSecs: Double?
    public let codecName: String?
    public let width: Int?
    public let height: Int?
    public let bitDepth: Int?
    public let sizeBytes: Int64?
    public let videoBitrateBps: Int64?
    public let containerBitrateBps: Int64?
    public let fps: Double?
    public let container: String?
    public let audioCodec: String?
    public let audioChannels: Int?
    public let dynamicRange: String?

    enum CodingKeys: String, CodingKey {
        case durationSecs = "duration_secs"
        case codecName = "codec_name"
        case width
        case height
        case bitDepth = "bit_depth"
        case sizeBytes = "size_bytes"
        case videoBitrateBps = "video_bitrate_bps"
        case containerBitrateBps = "container_bitrate_bps"
        case fps
        case container
        case audioCodec = "audio_codec"
        case audioChannels = "audio_channels"
        case dynamicRange = "dynamic_range"
    }
}

public struct EncodeStats: Decodable, Equatable, Sendable {
    public let inputSizeBytes: Int64
    public let outputSizeBytes: Int64
    public let compressionRatio: Double
    public let encodeTimeSeconds: Double
    public let encodeSpeed: Double
    public let avgBitrateKbps: Double
    public let vmafScore: Double?

    enum CodingKeys: String, CodingKey {
        case inputSizeBytes = "input_size_bytes"
        case outputSizeBytes = "output_size_bytes"
        case compressionRatio = "compression_ratio"
        case encodeTimeSeconds = "encode_time_seconds"
        case encodeSpeed = "encode_speed"
        case avgBitrateKbps = "avg_bitrate_kbps"
        case vmafScore = "vmaf_score"
    }
}

public struct EncodeAttempt: Decodable, Identifiable, Equatable, Sendable {
    public let id: Int64
    public let attemptNumber: Int
    public let startedAt: String?
    public let finishedAt: String?
    public let outcome: String
    public let failureCode: String?
    public let failureSummary: String?
    public let inputSizeBytes: Int64?
    public let outputSizeBytes: Int64?
    public let encodeTimeSeconds: Double?

    enum CodingKeys: String, CodingKey {
        case id
        case attemptNumber = "attempt_number"
        case startedAt = "started_at"
        case finishedAt = "finished_at"
        case outcome
        case failureCode = "failure_code"
        case failureSummary = "failure_summary"
        case inputSizeBytes = "input_size_bytes"
        case outputSizeBytes = "output_size_bytes"
        case encodeTimeSeconds = "encode_time_seconds"
    }
}

public struct EncodeHistoryRun: Decodable, Identifiable, Equatable, Sendable {
    public var id: Int { runNumber }
    public let runNumber: Int
    public let current: Bool
    public let outcome: String
    public let startedAt: String?
    public let finishedAt: String
    public let failureSummary: String?
    public let inputSizeBytes: Int64?
    public let outputSizeBytes: Int64?
    public let encodeTimeSeconds: Double?
    public let attempts: [EncodeAttempt]

    enum CodingKeys: String, CodingKey {
        case runNumber = "run_number"
        case current
        case outcome
        case startedAt = "started_at"
        case finishedAt = "finished_at"
        case failureSummary = "failure_summary"
        case inputSizeBytes = "input_size_bytes"
        case outputSizeBytes = "output_size_bytes"
        case encodeTimeSeconds = "encode_time_seconds"
        case attempts
    }
}

public struct LogEntry: Decodable, Identifiable, Equatable, Sendable {
    public let id: Int64
    public let level: String
    public let jobID: Int64?
    public let message: String
    public let createdAt: String

    enum CodingKeys: String, CodingKey {
        case id
        case level
        case jobID = "job_id"
        case message
        case createdAt = "created_at"
    }
}

public struct JobDetail: Decodable, Equatable, Sendable {
    public let job: Job
    public let metadata: JobMetadata?
    public let encodeStats: EncodeStats?
    public let encodeAttempts: [EncodeAttempt]
    public let encodeHistoryRuns: [EncodeHistoryRun]
    public let jobLogs: [LogEntry]
    public let jobFailureSummary: String?
    public let decisionExplanation: ExplanationPayload?
    public let failureExplanation: ExplanationPayload?
    public let queuePosition: UInt32?

    enum CodingKeys: String, CodingKey {
        case job
        case metadata
        case encodeStats = "encode_stats"
        case encodeAttempts = "encode_attempts"
        case encodeHistoryRuns = "encode_history_runs"
        case jobLogs = "job_logs"
        case jobFailureSummary = "job_failure_summary"
        case decisionExplanation = "decision_explanation"
        case failureExplanation = "failure_explanation"
        case queuePosition = "queue_position"
    }

    public func withJob(_ job: Job, queuePosition: UInt32? = nil) -> JobDetail {
        JobDetail(
            job: job,
            metadata: metadata,
            encodeStats: encodeStats,
            encodeAttempts: encodeAttempts,
            encodeHistoryRuns: encodeHistoryRuns,
            jobLogs: jobLogs,
            jobFailureSummary: jobFailureSummary,
            decisionExplanation: decisionExplanation,
            failureExplanation: failureExplanation,
            queuePosition: queuePosition ?? self.queuePosition
        )
    }
}

public struct ProcessorStatus: Decodable, Equatable, Sendable {
    public let blockedReason: String?
    public let message: String
    public let manualPaused: Bool
    public let schedulerPaused: Bool
    public let draining: Bool
    public let activeJobs: Int64
    public let concurrentLimit: Int

    enum CodingKeys: String, CodingKey {
        case blockedReason = "blocked_reason"
        case message
        case manualPaused = "manual_paused"
        case schedulerPaused = "scheduler_paused"
        case draining
        case activeJobs = "active_jobs"
        case concurrentLimit = "concurrent_limit"
    }
}

public struct CountMessageResponse: Decodable, Equatable, Sendable {
    public let count: Int
    public let message: String?
}

public struct SelftestResponse: Decodable, Equatable, Sendable {
    public let success: Bool
    public let stages: [SelftestStage]
    public let error: String?
}

public struct SelftestStage: Decodable, Equatable, Sendable {
    public let name: String
    public let success: Bool
    public let durationMs: Int64
    public let message: String

    enum CodingKeys: String, CodingKey {
        case name, success, message
        case durationMs = "duration_ms"
    }
}

public struct PreferenceResponse: Decodable, Equatable, Sendable {
    public let key: String
    public let value: String
}

public enum AlchemistEvent: Decodable, Equatable, Sendable {
    /// Synthetic marker yielded by `streamEvents()` once the SSE HTTP response is
    /// established (2xx). It is never produced by `parse`/`init(from:)`; the
    /// connection state machine uses it to flip to `.connected` and reset backoff.
    case connected
    case progress(jobID: Int64, percentage: Double, time: Double?)
    case status(jobID: Int64, status: String)
    case decision(jobID: Int64, action: String, reason: String)
    case log(jobID: Int64, level: String, message: String)
    case configUpdated
    case watchFolderAdded(path: String)
    case watchFolderRemoved(path: String)
    case scanStarted
    case scanCompleted
    case engineIdle
    case engineStatusChanged
    case hardwareStateChanged
    case lagged(skipped: UInt64)
    case unknown(eventName: String)

    enum CodingKeys: String, CodingKey {
        case jobID = "job_id"
        case percentage
        case time
        case status
        case action
        case reason
        case level
        case message
        case path
        case skipped
    }

    public static func parse(eventName: String, data: String) -> AlchemistEvent {
        let payloadData = data.isEmpty ? Data("{}".utf8) : Data(data.utf8)
        let payload = try? JSONDecoder().decode(SSEPayload.self, from: payloadData)

        switch eventName {
        case "progress":
            if let jobID = payload?.jobID, let percentage = payload?.percentage {
                return .progress(jobID: jobID, percentage: percentage, time: payload?.time)
            }
        case "status":
            if let jobID = payload?.jobID, let status = payload?.status {
                return .status(jobID: jobID, status: status)
            }
        case "decision":
            if let jobID = payload?.jobID, let action = payload?.action {
                return .decision(jobID: jobID, action: action, reason: payload?.reason ?? "")
            }
        case "log":
            if let jobID = payload?.jobID, let level = payload?.level, let message = payload?.message {
                return .log(jobID: jobID, level: level, message: message)
            }
        case "config_updated":
            return .configUpdated
        case "watch_folder_added":
            if let path = payload?.path {
                return .watchFolderAdded(path: path)
            }
        case "watch_folder_removed":
            if let path = payload?.path {
                return .watchFolderRemoved(path: path)
            }
        case "scan_started":
            return .scanStarted
        case "scan_completed":
            return .scanCompleted
        case "engine_idle":
            return .engineIdle
        case "engine_status_changed":
            return .engineStatusChanged
        case "hardware_state_changed":
            return .hardwareStateChanged
        case "lagged":
            return .lagged(skipped: payload?.skipped ?? 0)
        default:
            break
        }

        return .unknown(eventName: eventName)
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)

        if let jobID = try? container.decode(Int64.self, forKey: .jobID) {
            if let percentage = try? container.decode(Double.self, forKey: .percentage) {
                self = .progress(jobID: jobID, percentage: percentage, time: try? container.decode(Double.self, forKey: .time))
            } else if let status = try? container.decode(String.self, forKey: .status) {
                self = .status(jobID: jobID, status: status)
            } else if let action = try? container.decode(String.self, forKey: .action) {
                self = .decision(jobID: jobID, action: action, reason: try container.decode(String.self, forKey: .reason))
            } else if let level = try? container.decode(String.self, forKey: .level) {
                self = .log(jobID: jobID, level: level, message: try container.decode(String.self, forKey: .message))
            } else {
                self = .unknown(eventName: "job")
            }
        } else {
            self = .unknown(eventName: "message")
        }
    }
}

public struct AlchemistSSEParser: Sendable {
    private var eventName: String?
    private var dataLines: [String] = []
    private var byteBuffer: [UInt8] = []

    public init() {}

    /// Feed a single raw byte from the SSE byte stream. Returns a completed event
    /// when a line terminator (`\n`) closes a frame. This is the production path:
    /// `URLSession.AsyncBytes.lines` silently drops the blank lines that delimit SSE
    /// frames, so framing must be done over raw bytes (see audit P1-12).
    public mutating func consume(byte: UInt8) -> AlchemistEvent? {
        if byte == UInt8(ascii: "\n") {
            var line = String(decoding: byteBuffer, as: UTF8.self)
            if line.hasSuffix("\r") { line.removeLast() }
            byteBuffer.removeAll(keepingCapacity: true)
            return parse(line: line)
        }
        byteBuffer.append(byte)
        return nil
    }

    /// Convenience for tests and buffered callers: feed a whole `Data` blob and
    /// collect every event it completes.
    public mutating func consume(data: Data) -> [AlchemistEvent] {
        var events: [AlchemistEvent] = []
        for byte in data {
            if let event = consume(byte: byte) {
                events.append(event)
            }
        }
        return events
    }

    public mutating func parse(line: String) -> AlchemistEvent? {
        if line.isEmpty {
            return finish()
        }

        if line.hasPrefix(":") {
            return nil
        }

        if line.hasPrefix("event:") {
            // Defensive: if a previous frame was never closed by a blank line, flush it
            // before recording the new event name so events can't bleed together.
            let flushed = (eventName != nil || !dataLines.isEmpty) ? finish() : nil
            eventName = Self.fieldValue(from: line, prefix: "event:")
            return flushed
        }

        if line.hasPrefix("data:") {
            dataLines.append(Self.fieldValue(from: line, prefix: "data:"))
            return nil
        }

        return nil
    }

    public mutating func finish() -> AlchemistEvent? {
        guard eventName != nil || !dataLines.isEmpty else {
            reset()
            return nil
        }

        let parsed = AlchemistEvent.parse(
            eventName: eventName ?? "message",
            data: dataLines.joined(separator: "\n")
        )
        reset()
        return parsed
    }

    private mutating func reset() {
        eventName = nil
        dataLines.removeAll(keepingCapacity: true)
    }

    private static func fieldValue(from line: String, prefix: String) -> String {
        var value = String(line.dropFirst(prefix.count))
        if value.first == " " {
            value.removeFirst()
        }
        return value
    }
}

private struct SSEPayload: Decodable {
    let jobID: Int64?
    let percentage: Double?
    let time: Double?
    let status: String?
    let action: String?
    let reason: String?
    let level: String?
    let message: String?
    let path: String?
    let skipped: UInt64?

    enum CodingKeys: String, CodingKey {
        case jobID = "job_id"
        case percentage
        case time
        case status
        case action
        case reason
        case level
        case message
        case path
        case skipped
    }
}

public enum AlchemistFormatter {
    public static func bytes(_ value: Int64) -> String {
        let formatter = ByteCountFormatter()
        formatter.allowedUnits = [.useGB, .useMB, .useKB]
        formatter.countStyle = .file
        return formatter.string(fromByteCount: value)
    }

    public static func duration(_ seconds: Double?) -> String {
        guard let seconds else { return "-" }
        let totalSeconds = max(0, Int(seconds.rounded()))
        let hours = totalSeconds / 3600
        let minutes = (totalSeconds % 3600) / 60
        let remainingSeconds = totalSeconds % 60

        if hours > 0 {
            return "\(hours)h \(minutes)m"
        }
        if minutes > 0 {
            return "\(minutes)m \(remainingSeconds)s"
        }
        return "\(remainingSeconds)s"
    }

    public static func shortDate(_ value: String?) -> String {
        guard let value, !value.isEmpty else { return "-" }
        let parserWithFractionalSeconds = ISO8601DateFormatter()
        parserWithFractionalSeconds.formatOptions = [.withInternetDateTime, .withFractionalSeconds]

        let parser = ISO8601DateFormatter()
        parser.formatOptions = [.withInternetDateTime]

        guard let date = parserWithFractionalSeconds.date(from: value) ?? parser.date(from: value) else {
            return value
        }
        return DateFormatter.localizedString(from: date, dateStyle: .short, timeStyle: .short)
    }
}

import Foundation
import SwiftUI

public enum AlchemistSupportPaths {
    public static var root: URL {
        FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent("Library", isDirectory: true)
            .appendingPathComponent("Application Support", isDirectory: true)
            .appendingPathComponent("Alchemist", isDirectory: true)
    }

    public static var config: URL {
        root.appendingPathComponent("config.toml")
    }

    public static var database: URL {
        root.appendingPathComponent("alchemist.db")
    }

    public static var temp: URL {
        root.appendingPathComponent("temp", isDirectory: true)
    }

    public static func ensureDirectories() throws {
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        try FileManager.default.createDirectory(at: temp, withIntermediateDirectories: true)
    }
}

@MainActor
public final class DaemonController: ObservableObject {
    @Published public private(set) var status = "Not started"
    @Published public private(set) var lastError: String?

    private var process: Process?

    public init() {}

    public func startBundledDaemon(port: Int = 3000) {
        if process?.isRunning == true {
            status = "Running on 127.0.0.1:\(port)"
            lastError = nil
            return
        }

        do {
            try AlchemistSupportPaths.ensureDirectories()
            let executable = try daemonExecutableURL()
            let process = Process()
            process.executableURL = executable
            process.environment = daemonEnvironment(port: port)
            process.standardOutput = Pipe()
            process.standardError = Pipe()
            try process.run()
            self.process = process
            status = "Running on 127.0.0.1:\(port)"
            lastError = nil
        } catch {
            status = "Unavailable"
            lastError = error.localizedDescription
        }
    }

    public func stopBundledDaemon() {
        process?.terminate()
        process = nil
        status = "Stopped"
    }

    private func daemonExecutableURL() throws -> URL {
        if let override = ProcessInfo.processInfo.environment["ALCHEMIST_DAEMON_PATH"], !override.isEmpty {
            return URL(fileURLWithPath: override)
        }

        if let bundled = Bundle.main.url(forResource: "alchemistd", withExtension: nil) {
            return bundled
        }

        throw CocoaError(.fileNoSuchFile, userInfo: [
            NSFilePathErrorKey: "Set ALCHEMIST_DAEMON_PATH or bundle Contents/MacOS/alchemistd."
        ])
    }

    private func daemonEnvironment(port: Int) -> [String: String] {
        var environment = ProcessInfo.processInfo.environment
        environment["ALCHEMIST_CONFIG_PATH"] = AlchemistSupportPaths.config.path
        environment["ALCHEMIST_DB_PATH"] = AlchemistSupportPaths.database.path
        environment["ALCHEMIST_TEMP_DIR"] = AlchemistSupportPaths.temp.path
        environment["ALCHEMIST_SERVER_PORT"] = String(port)
        environment["ALCHEMIST_CONFIG_MUTABLE"] = "true"
        environment["RUST_LOG"] = environment["RUST_LOG"] ?? "info,alchemist=info"
        return environment
    }
}

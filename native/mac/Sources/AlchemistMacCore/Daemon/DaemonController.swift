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

    public static var daemonLog: URL {
        root.appendingPathComponent("daemon.log")
    }

    public static func ensureDirectories() throws {
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        try FileManager.default.createDirectory(at: temp, withIntermediateDirectories: true)
    }
}

@MainActor
public final class DaemonController: ObservableObject {
    /// Bundled mode binds an uncommon fixed port rather than the collision-prone 3000,
    /// so the app cannot silently talk to (or hand admin credentials to) a foreign
    /// process that happens to own the port (audit P2-40).
    public static let bundledPort = 41737
    public static let bundledBaseURLString = "http://127.0.0.1:\(bundledPort)"

    @Published public private(set) var status = "Not started"
    @Published public private(set) var lastError: String?

    private var process: Process?
    private var logHandle: FileHandle?
    private var startTask: Task<Void, Never>?

    public init() {}

    public func startBundledDaemon(port: Int = bundledPort) {
        if process?.isRunning == true {
            status = "Running on 127.0.0.1:\(port)"
            lastError = nil
            return
        }
        startTask?.cancel()
        startTask = Task { [weak self] in
            await self?.superviseStart(port: port)
        }
    }

    private func superviseStart(port: Int) async {
        // If something is already serving on the port, adopt it instead of spawning a
        // doomed duplicate that fails to bind and dies silently (audit P2-36). The
        // uncommon port keeps the odds of adopting a foreign process negligible; a
        // strict version match would need an auth-free version endpoint we don't have.
        if await Self.isServing(port: port) {
            status = "Running on 127.0.0.1:\(port) (adopted)"
            lastError = nil
            return
        }

        do {
            try AlchemistSupportPaths.ensureDirectories()
            let executable = try daemonExecutableURL()

            // Stream stdout+stderr to a log file rather than an undrained Pipe(); an
            // unread pipe blocks the daemon forever once its 64 KB buffer fills (P1-13).
            let logURL = AlchemistSupportPaths.daemonLog
            FileManager.default.createFile(atPath: logURL.path, contents: nil)
            let handle = try FileHandle(forWritingTo: logURL)
            logHandle = handle

            let process = Process()
            process.executableURL = executable
            process.environment = daemonEnvironment(port: port)
            process.standardOutput = handle
            process.standardError = handle
            process.terminationHandler = { [weak self] proc in
                let code = proc.terminationStatus
                Task { @MainActor in
                    self?.handleTermination(proc, code: code)
                }
            }
            try process.run()
            self.process = process
            status = "Starting on 127.0.0.1:\(port)…"
            lastError = nil
        } catch {
            status = "Unavailable"
            lastError = error.localizedDescription
            logHandle?.closeFile()
            logHandle = nil
            return
        }

        // Confirm the daemon actually bound the port before claiming Running, so a bind
        // failure surfaces within seconds instead of a frozen "Running" (audit P2-40).
        let deadline = Date().addingTimeInterval(10)
        while Date() < deadline {
            if Task.isCancelled { return }
            if process?.isRunning != true {
                status = "Unavailable"
                lastError = "Daemon exited during startup.\n\(recentLogLines(8))"
                return
            }
            if await Self.isServing(port: port) {
                status = "Running on 127.0.0.1:\(port)"
                lastError = nil
                return
            }
            try? await Task.sleep(nanoseconds: 300_000_000)
        }
        status = "Unavailable"
        lastError = "Daemon did not become ready on port \(port).\n\(recentLogLines(8))"
    }

    private func handleTermination(_ proc: Process, code: Int32) {
        // Ignore terminations for a process we already replaced or stopped.
        guard proc === process else { return }
        process = nil
        logHandle?.closeFile()
        logHandle = nil
        status = "Stopped (exit \(code))"
        if code != 0 {
            lastError = "Daemon exited with code \(code).\n\(recentLogLines(8))"
        }
    }

    public func stopBundledDaemon(waitForExit: Bool = false) {
        startTask?.cancel()
        startTask = nil
        if let process, process.isRunning {
            // Drop the handler first so the synchronous stop path owns the status string.
            process.terminationHandler = nil
            process.terminate() // SIGTERM; the daemon shuts down cleanly on it.
            if waitForExit {
                let deadline = Date().addingTimeInterval(3)
                while process.isRunning && Date() < deadline {
                    Thread.sleep(forTimeInterval: 0.05)
                }
                if process.isRunning {
                    kill(process.processIdentifier, SIGKILL)
                }
            }
        }
        process = nil
        logHandle?.closeFile()
        logHandle = nil
        status = "Stopped"
    }

    /// Tail of the daemon log, surfaced so bind failures and crashes are visible.
    public func recentLogLines(_ n: Int = 40) -> String {
        guard let data = try? Data(contentsOf: AlchemistSupportPaths.daemonLog),
              let text = String(data: data, encoding: .utf8), !text.isEmpty else {
            return ""
        }
        let lines = text.split(separator: "\n", omittingEmptySubsequences: false)
        return lines.suffix(n).joined(separator: "\n")
    }

    /// Liveness probe against the unauthenticated `/api/ready` endpoint. Any HTTP
    /// response means a server is bound to the port.
    private static func isServing(port: Int) async -> Bool {
        guard let url = URL(string: "http://127.0.0.1:\(port)/api/ready") else { return false }
        var request = URLRequest(url: url)
        request.httpMethod = "GET"
        request.timeoutInterval = 2
        let config = URLSessionConfiguration.ephemeral
        config.timeoutIntervalForRequest = 2
        config.timeoutIntervalForResource = 2
        let session = URLSession(configuration: config)
        guard let (_, response) = try? await session.data(for: request) else { return false }
        return response is HTTPURLResponse
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

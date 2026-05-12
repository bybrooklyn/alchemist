import Foundation

@Observable
@MainActor
public final class TaskRegistry {
    private var tasks: [String: Task<Void, Never>] = [:]

    public init() {}

    public func run(_ key: String, _ work: @escaping @Sendable () async -> Void) {
        tasks[key]?.cancel()
        tasks[key] = Task { await work() }
    }

    public func cancel(_ key: String) {
        tasks[key]?.cancel()
        tasks[key] = nil
    }

    public func cancelAll() {
        for (_, task) in tasks {
            task.cancel()
        }
        tasks.removeAll()
    }

    deinit {
        // Tasks are cancelled via cancelAll() before deallocation
    }
}

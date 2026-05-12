import AlchemistMacCore
import SwiftUI

@main
struct AlchemistMac: App {
    @State private var appModel = AppModel()

    var body: some Scene {
        WindowGroup {
            RootView()
                .environment(appModel)
                .frame(minWidth: 1100, minHeight: 720)
        }
        .windowStyle(.titleBar)
        .commands {
            CommandMenu("Navigate") {
                Button("Dashboard") {
                    appModel.navigation.navigate(to: .dashboard)
                }
                .keyboardShortcut("1", modifiers: [.command])

                Button("Jobs") {
                    appModel.navigation.navigate(to: .queue)
                }
                .keyboardShortcut("2", modifiers: [.command])

                Button("Logs") {
                    appModel.navigation.navigate(to: .logs)
                }
                .keyboardShortcut("3", modifiers: [.command])

                Button("Statistics") {
                    appModel.navigation.navigate(to: .statistics)
                }
                .keyboardShortcut("4", modifiers: [.command])

                Button("Intelligence") {
                    appModel.navigation.navigate(to: .intelligence)
                }
                .keyboardShortcut("5", modifiers: [.command])

                Button("Convert") {
                    appModel.navigation.navigate(to: .convert)
                }
                .keyboardShortcut("6", modifiers: [.command])

                Button("System") {
                    appModel.navigation.navigate(to: .system)
                }
                .keyboardShortcut("7", modifiers: [.command])

                Button("Settings") {
                    appModel.navigation.navigate(to: .settings)
                }
                .keyboardShortcut("8", modifiers: [.command])
            }

            CommandMenu("Jobs") {
                Button("Refresh Jobs") {
                    Task { await appModel.jobs.refresh(apiClient: appModel.connection.apiClient) }
                }
                .keyboardShortcut("r", modifiers: [.command])

                Button("Retry Failed Jobs") {
                    Task { await appModel.jobs.restartFailed(apiClient: appModel.connection.apiClient) }
                }

                Button("Clear Completed Jobs") {
                    Task { await appModel.jobs.clearCompleted(apiClient: appModel.connection.apiClient) }
                }

                Divider()

                Button("Cancel Selected Jobs") {
                    Task { await appModel.jobs.performBatch(.cancel, apiClient: appModel.connection.apiClient) }
                }
                .disabled(appModel.jobs.selectedIDs.isEmpty)

                Button("Restart Selected Jobs") {
                    Task { await appModel.jobs.performBatch(.restart, apiClient: appModel.connection.apiClient) }
                }
                .disabled(appModel.jobs.selectedIDs.isEmpty || appModel.jobs.hasSelectedActiveJobs)

                Button("Delete Selected Jobs") {
                    Task { await appModel.jobs.performBatch(.delete, apiClient: appModel.connection.apiClient) }
                }
                .disabled(appModel.jobs.selectedIDs.isEmpty || appModel.jobs.hasSelectedActiveJobs)

                Divider()

                Button("Start Queue") {
                    Task { await appModel.resumeQueue() }
                }
                .keyboardShortcut(.space, modifiers: [.command])

                Button("Pause Queue") {
                    Task { await appModel.pauseQueue() }
                }
                .keyboardShortcut(.space, modifiers: [.command, .shift])
            }

            CommandMenu("View") {
                Button("Command Palette") {
                    appModel.navigation.toggleCommandPalette()
                }
                .keyboardShortcut("k", modifiers: [.command])

                Button(appModel.navigation.showingInspector ? "Hide Inspector" : "Show Inspector") {
                    appModel.navigation.toggleInspector()
                }
                .keyboardShortcut("i", modifiers: [.command, .option])
            }
        }

        MenuBarExtra("Alchemist", systemImage: "wand.and.sparkles") {
            MenuBarStatusView()
                .environment(appModel)
        }
        .menuBarExtraStyle(.window)

        Settings {
            SettingsView()
                .environment(appModel)
                .frame(width: 640, height: 520)
        }
    }
}

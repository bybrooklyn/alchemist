import SwiftUI

public struct SettingsView: View {
    @Environment(AppModel.self) private var model

    public init() {}

    public var body: some View {
        TabView {
            SettingsGeneralView()
                .tabItem { Label("General", systemImage: "gear") }

            SettingsAppearanceView()
                .tabItem { Label("Appearance", systemImage: "paintbrush") }

            SettingsEncodingView()
                .tabItem { Label("Encoding", systemImage: "film") }

            SettingsLibraryView()
                .tabItem { Label("Library", systemImage: "folder") }

            SettingsNotificationsView()
                .tabItem { Label("Notifications", systemImage: "bell") }

            SettingsNetworkView()
                .tabItem { Label("Network", systemImage: "network") }

            SettingsAdvancedView()
                .tabItem { Label("Advanced", systemImage: "wrench") }
        }
        .environment(model)
    }
}

import SwiftUI

public struct RootView: View {
    @Environment(AppModel.self) private var model

    public init() {}

    private var selectedSectionBinding: Binding<AppSection?> {
        Binding(
            get: { model.navigation.selectedSection },
            set: { model.navigation.selectedSection = $0 ?? .dashboard }
        )
    }

    private var showingLoginBinding: Binding<Bool> {
        Binding(
            get: { model.navigation.showingLogin },
            set: { value in
                if value {
                    model.navigation.presentLogin()
                } else {
                    model.navigation.dismissLogin()
                }
            }
        )
    }

    private var showingCommandPaletteBinding: Binding<Bool> {
        Binding(
            get: { model.navigation.showingCommandPalette },
            set: { model.navigation.showingCommandPalette = $0 }
        )
    }

    public var body: some View {
        NavigationSplitView {
            List(selection: selectedSectionBinding) {
                ForEach(AppSection.allCases) { section in
                    Label(section.label, systemImage: section.symbol)
                        .tag(section as AppSection?)
                }
            }
            .listStyle(.sidebar)
            .navigationTitle("Alchemist")
        } detail: {
            VStack(alignment: .leading, spacing: 12) {
                if model.connection.sseState != .connected {
                    ConnectionBanner(state: model.connection.sseState)
                        .padding(.horizontal, 16)
                        .padding(.top, 16)
                }

                Group {
                    if model.setupRequired {
                        SetupRequiredView()
                    } else {
                        switch model.navigation.selectedSection {
                        case .dashboard:
                            DashboardView()
                        case .queue:
                            JobsWorkspaceView()
                        case .logs:
                            LogsView()
                        case .statistics:
                            StatisticsView()
                        case .intelligence:
                            IntelligenceView()
                        case .convert:
                            ConvertView()
                        case .system:
                            SystemView()
                        }
                    }
                }
            }
            .animation(.easeInOut(duration: 0.15), value: model.navigation.selectedSection)
            .task {
                await model.refreshAll()
            }
            .onChange(of: model.connection.lastError) { _, error in
                if error == .authenticationRequired {
                    model.navigation.presentLogin()
                }
            }
            .sheet(isPresented: showingLoginBinding) {
                LoginView()
            }
            .sheet(isPresented: showingCommandPaletteBinding) {
                CommandPaletteView()
            }
        }
        .toolbar {
            ToolbarItemGroup(placement: .primaryAction) {
                queueControlButton

                Button {
                    model.navigation.toggleCommandPalette()
                } label: {
                    Label("Command Palette", systemImage: "command")
                }
                .buttonStyle(.glass)

                Button {
                    model.navigation.selectedSection = .convert
                } label: {
                    Label("Convert", systemImage: "wand.and.sparkles")
                }
                .buttonStyle(.glass)

                Button {
                    Task { await model.refreshAll() }
                } label: {
                    Label("Refresh", systemImage: "arrow.clockwise")
                }
                .buttonStyle(.glass)
                .disabled(model.isRefreshing)
            }
        }
        .preferredColorScheme(model.theme.theme.colorScheme)
        .accentColor(model.theme.accent.color)
        .toast(Binding(
            get: { model.toast },
            set: { model.toast = $0 }
        ))
    }

    @ViewBuilder
    private var queueControlButton: some View {
        if model.engine.isPaused {
            Button {
                Task { await model.resumeQueue() }
            } label: {
                Label("Start", systemImage: "play.fill")
            }
            .buttonStyle(.glassProminent)
            .tint(model.theme.accent.color)
        } else {
            Button {
                Task { await model.pauseQueue() }
            } label: {
                Label("Pause", systemImage: "pause.fill")
            }
            .buttonStyle(.glass)
        }
    }
}

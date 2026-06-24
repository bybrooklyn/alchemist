import SwiftUI

struct CommandPaletteItem: Identifiable {
    let id = UUID()
    let title: String
    let subtitle: String
    let symbol: String
    let action: () -> Void
}

struct CommandPaletteView: View {
    @Environment(AppModel.self) private var model
    @Environment(\.dismiss) private var dismiss
    @State private var query = ""

    private var items: [CommandPaletteItem] {
        [
            CommandPaletteItem(title: "Dashboard", subtitle: "Open dashboard overview", symbol: "gauge.with.dots.needle.bottom.50percent") {
                model.navigation.navigate(to: .dashboard)
            },
            CommandPaletteItem(title: "Jobs", subtitle: "Open queue and job controls", symbol: "film.stack") {
                model.navigation.navigate(to: .queue)
            },
            CommandPaletteItem(title: "Logs", subtitle: "Open runtime logs", symbol: "terminal") {
                model.navigation.navigate(to: .logs)
            },
            CommandPaletteItem(title: "Statistics", subtitle: "Open savings and throughput stats", symbol: "chart.bar.xaxis") {
                model.navigation.navigate(to: .statistics)
            },
            CommandPaletteItem(title: "Intelligence", subtitle: "Open recommendations", symbol: "sparkles") {
                model.navigation.navigate(to: .intelligence)
            },
            CommandPaletteItem(title: "Convert", subtitle: "Open conversion tools", symbol: "wand.and.sparkles") {
                model.navigation.navigate(to: .convert)
            },
            CommandPaletteItem(title: "System", subtitle: "Open daemon and resource diagnostics", symbol: "waveform.path.ecg") {
                model.navigation.navigate(to: .system)
            },
            CommandPaletteItem(title: "Settings", subtitle: "Open app settings", symbol: "gearshape") {
                NSApp.sendAction(Selector(("showSettingsWindow:")), to: nil, from: nil)
            },
            CommandPaletteItem(title: model.engine.isPaused ? "Start Queue" : "Pause Queue", subtitle: "Control queue execution", symbol: model.engine.isPaused ? "play.fill" : "pause.fill") {
                Task {
                    if model.engine.isPaused {
                        await model.resumeQueue()
                    } else {
                        await model.pauseQueue()
                    }
                }
            },
            CommandPaletteItem(title: "Refresh", subtitle: "Reload dashboard and queue data", symbol: "arrow.clockwise") {
                Task { await model.refreshAll() }
            },
            CommandPaletteItem(title: "Login", subtitle: "Authenticate with current Alchemist node", symbol: "person.crop.circle") {
                model.navigation.presentLogin()
            },
        ]
    }

    private var filteredItems: [CommandPaletteItem] {
        let needle = query.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        guard !needle.isEmpty else { return items }
        return items.filter {
            $0.title.lowercased().contains(needle)
                || $0.subtitle.lowercased().contains(needle)
        }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            TextField("Type a command", text: $query)
                .textFieldStyle(.roundedBorder)

            ScrollView {
                LazyVStack(alignment: .leading, spacing: 8) {
                    ForEach(filteredItems) { item in
                        Button {
                            dismiss()
                            item.action()
                        } label: {
                            HStack(spacing: 12) {
                                Image(systemName: item.symbol)
                                    .frame(width: 18)
                                    .foregroundStyle(.secondary)
                                VStack(alignment: .leading, spacing: 2) {
                                    Text(item.title)
                                        .font(.headline)
                                        .foregroundStyle(.primary)
                                    Text(item.subtitle)
                                        .font(.caption)
                                        .foregroundStyle(.secondary)
                                }
                                Spacer()
                            }
                            .padding(10)
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .background(.thinMaterial, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
                        }
                        .buttonStyle(.plain)
                    }
                }
            }
        }
        .padding(24)
        .frame(width: 560, height: 420)
    }
}

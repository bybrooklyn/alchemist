import SwiftUI

struct LogsView: View {
    @Environment(AppModel.self) private var model

    private var logQueryBinding: Binding<String> {
        Binding(get: { model.logs.query }, set: { model.logs.query = $0 })
    }
    private var logLevelBinding: Binding<String> {
        Binding(get: { model.logs.levelFilter }, set: { model.logs.levelFilter = $0 })
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            HStack {
                VStack(alignment: .leading, spacing: 4) {
                    Text("Logs")
                        .font(.system(size: 34, weight: .bold, design: .rounded))
                    Text("Server logs and live job events")
                        .font(.title3)
                        .foregroundStyle(.secondary)
                }
                Spacer()
                Button {
                    Task { await model.logs.load(apiClient: model.connection.apiClient) }
                } label: {
                    Label("Reload", systemImage: "arrow.clockwise")
                }
                .buttonStyle(.glass)
                Button(role: .destructive) {
                    Task { await model.logs.clear(apiClient: model.connection.apiClient) }
                } label: {
                    Label("Clear", systemImage: "trash")
                }
                .buttonStyle(.glass)
            }

            HStack(spacing: 12) {
                Label("Search", systemImage: "magnifyingglass")
                    .labelStyle(.iconOnly)
                    .foregroundStyle(.secondary)
                TextField("Filter by text or job id", text: logQueryBinding)
                    .textFieldStyle(.plain)
                Picker("Level", selection: logLevelBinding) {
                    Text("All levels").tag("all")
                    Text("Info").tag("info")
                    Text("Warnings").tag("warn")
                    Text("Errors").tag("error")
                }
                .frame(width: 150)
            }
            .padding(12)
            .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 12, style: .continuous))

            ScrollView {
                LazyVStack(alignment: .leading, spacing: 0) {
                    if model.logs.filteredLogs.isEmpty {
                        EmptyHeroCard(title: "No log entries", detail: "Logs appear here once the engine starts processing jobs.")
                            .padding(20)
                    } else {
                        ForEach(model.logs.filteredLogs) { log in
                            LogRow(log: log)
                        }
                    }
                }
            }
            .background(.thinMaterial, in: RoundedRectangle(cornerRadius: 12, style: .continuous))
        }
        .padding(28)
        .task { await model.logs.load(apiClient: model.connection.apiClient) }
    }
}

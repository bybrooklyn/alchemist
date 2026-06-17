import SwiftUI
import AppKit

struct SettingsLibraryView: View {
    @Environment(AppModel.self) private var model
    @State private var showingAddFolder = false

    var body: some View {
        Form {
            Section("Watch Folders") {
                if model.dashboard.watchDirectories.isEmpty {
                    Text("No watch folders configured.")
                        .foregroundStyle(.secondary)
                } else {
                    watchFolderList
                }

                Button {
                    showingAddFolder = true
                } label: {
                    Label("Add Folder", systemImage: "folder.badge.plus")
                }
            }

            Section("Scan Settings") {
                LabeledContent("Watch Enabled") {
                    Text(model.dashboard.settingsBundle?.settings.scanner.watchEnabled == true ? "Yes" : "No")
                }
            }
        }
        .formStyle(.grouped)
        .padding(20)
        .sheet(isPresented: $showingAddFolder) {
            AddWatchFolderSheet { url in
                Task {
                    do {
                        _ = try await model.connection.apiClient?.addWatchFolder(path: url.path)
                        model.showToast(.success, "Added \(url.lastPathComponent)")
                        await model.dashboard.refresh(apiClient: model.connection.apiClient)
                    } catch {
                        model.showToast(.error, error.localizedDescription)
                    }
                }
            }
        }
    }

    @ViewBuilder
    private var watchFolderList: some View {
        WatchFolderList(dirs: model.dashboard.watchDirectories) { dir in
            Task {
                do {
                    _ = try await model.connection.apiClient?.deleteWatchFolder(id: dir.id)
                    model.showToast(.success, "Removed \(URL(fileURLWithPath: dir.path).lastPathComponent)")
                    await model.dashboard.refresh(apiClient: model.connection.apiClient)
                } catch {
                    model.showToast(.error, error.localizedDescription)
                }
            }
        }
    }
}

struct AddWatchFolderSheet: View {
    let onSelect: (URL) -> Void
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        VStack(spacing: 16) {
            Text("Add Watch Folder")
                .font(.headline)
            Text("Select a folder to monitor for new media.")
                .foregroundStyle(.secondary)
            HStack {
                Button("Cancel") { dismiss() }
                    .keyboardShortcut(.cancelAction)
                Button("Select Folder") {
                    let panel = NSOpenPanel()
                    panel.canChooseFiles = false
                    panel.canChooseDirectories = true
                    panel.allowsMultipleSelection = false
                    if panel.runModal() == .OK, let url = panel.url {
                        onSelect(url)
                    }
                    dismiss()
                }
                .keyboardShortcut(.defaultAction)
            }
        }
        .padding(24)
        .frame(width: 360)
    }
}

private struct WatchFolderList: View {
    let dirs: [WatchDirectory]
    let onDelete: (WatchDirectory) -> Void

    var body: some View {
        ForEach(dirs, id: \.id) { dir in
            HStack {
                VStack(alignment: .leading, spacing: 2) {
                    Text(dir.path)
                        .font(.body)
                        .lineLimit(1)
                    HStack(spacing: 6) {
                        if dir.isRecursive == true {
                            Label("Recursive", systemImage: "arrow.triangle.branch")
                                .font(.caption2)
                                .foregroundStyle(.secondary)
                        }
                        if let profile = dir.profileName {
                            Label(profile, systemImage: "person.crop.rectangle.stack")
                                .font(.caption2)
                                .foregroundStyle(.secondary)
                        }
                    }
                }
                Spacer()
                Button(role: .destructive) {
                    onDelete(dir)
                } label: {
                    Image(systemName: "trash")
                }
                .buttonStyle(.plain)
                .foregroundStyle(.secondary)
            }
        }
    }
}

import AppKit
import SwiftUI

struct ConvertView: View {
    @Environment(AppModel.self) private var model
    @State private var isTargeted = false

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 28) {
                VStack(alignment: .leading, spacing: 4) {
                    Text("Convert")
                        .font(.system(size: 34, weight: .bold, design: .rounded))
                    Text("Stage a single file conversion or add media to the queue")
                        .font(.title3)
                        .foregroundStyle(.secondary)
                }

                LazyVGrid(columns: [GridItem(.flexible()), GridItem(.flexible()), GridItem(.flexible())], spacing: 16) {
                    ImportActionCard(title: "Convert File", detail: "Upload or stage one file for manual conversion.", symbol: "wand.and.sparkles") {
                        if let file = selectFiles(allowsMultiple: false).first {
                            Task { await model.uploadConversion(file) }
                        }
                    }
                    ImportActionCard(title: "Enqueue Files", detail: "Add files directly to the Jobs queue.", symbol: "film.stack") {
                        Task { await model.enqueueFiles(selectFiles()) }
                    }
                    ImportActionCard(title: "Watch Folder", detail: "Monitor a directory for new media automatically.", symbol: "folder.badge.plus") {
                        Task { await model.addWatchFolders(selectFolders()) }
                    }
                }

                DropZone(isTargeted: $isTargeted) { providers in
                    Task { await handleDrop(providers) }
                }

                if let error = model.jobs.lastError {
                    ErrorPanel(message: error.localizedDescription)
                }
            }
            .padding(28)
        }
    }

    private func handleDrop(_ providers: [NSItemProvider]) async {
        var fileURLs: [URL] = []
        var folderURLs: [URL] = []

        for provider in providers {
            if let data = try? await provider.loadItem(forTypeIdentifier: "public.file-url", options: nil) as? Data,
               let url = URL(dataRepresentation: data, relativeTo: nil) {
                var isDir: ObjCBool = false
                if FileManager.default.fileExists(atPath: url.path, isDirectory: &isDir) {
                    if isDir.boolValue {
                        folderURLs.append(url)
                    } else {
                        fileURLs.append(url)
                    }
                }
            }
        }

        if !fileURLs.isEmpty { await model.enqueueFiles(fileURLs) }
        if !folderURLs.isEmpty { await model.addWatchFolders(folderURLs) }
    }

    private func selectFiles(allowsMultiple: Bool = true) -> [URL] {
        let panel = NSOpenPanel()
        panel.canChooseFiles = true
        panel.canChooseDirectories = false
        panel.allowsMultipleSelection = allowsMultiple
        return panel.runModal() == .OK ? panel.urls : []
    }

    private func selectFolders() -> [URL] {
        let panel = NSOpenPanel()
        panel.canChooseFiles = false
        panel.canChooseDirectories = true
        panel.allowsMultipleSelection = true
        return panel.runModal() == .OK ? panel.urls : []
    }
}

struct DropZone: View {
    @Binding var isTargeted: Bool
    var onDrop: ([NSItemProvider]) -> Void

    var body: some View {
        VStack(spacing: 14) {
            Image(systemName: "square.and.arrow.down")
                .font(.system(size: 30, weight: .bold))
                .foregroundStyle(isTargeted ? Color.heliosAccent : .secondary)
            Text("Drop media here")
                .font(.headline)
            Text("Files are enqueued; folders are added as watched folders.")
                .font(.subheadline)
                .foregroundStyle(.secondary)
        }
        .frame(maxWidth: .infinity)
        .padding(.vertical, 40)
        .background(.ultraThinMaterial, in: RoundedRectangle(cornerRadius: 16, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 16).strokeBorder(isTargeted ? Color.heliosAccent : .white.opacity(0.1), style: StrokeStyle(lineWidth: 1.5, dash: isTargeted ? [] : [6, 4])))
        .onDrop(of: [.fileURL], isTargeted: $isTargeted) { providers in
            onDrop(providers)
            return true
        }
    }
}

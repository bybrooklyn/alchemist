import SwiftUI

struct JobNoticePanel: View {
    let message: String
    let isError: Bool

    var body: some View {
        HStack(spacing: 10) {
            Image(systemName: isError ? "exclamationmark.triangle.fill" : "checkmark.circle.fill")
            Text(message)
                .font(.subheadline)
            Spacer()
        }
        .foregroundStyle(isError ? Color.heliosError : Color.heliosSuccess)
        .padding(12)
        .background((isError ? Color.heliosError : Color.heliosSuccess).opacity(0.10), in: RoundedRectangle(cornerRadius: 12, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 12, style: .continuous).stroke((isError ? Color.heliosError : Color.heliosSuccess).opacity(0.28), lineWidth: 1))
    }
}

struct JobTabButton: View {
    @Environment(AppModel.self) private var model
    let tab: JobTab

    var body: some View {
        if model.jobs.activeTab == tab {
            Button {
                model.jobs.setTab(tab)
            } label: {
                Text(tab.label)
                    .frame(minWidth: 64)
            }
            .buttonStyle(.glassProminent)
            .tint(model.theme.accent.color)
        } else {
            Button {
                model.jobs.setTab(tab)
            } label: {
                Text(tab.label)
                    .frame(minWidth: 64)
            }
            .buttonStyle(.glass)
        }
    }
}

struct JobsTableHeader: View {
    @Environment(AppModel.self) private var model
    let allSelected: Bool

    var body: some View {
        HStack(spacing: 12) {
            Button {
                model.jobs.toggleAllVisible()
            } label: {
                Image(systemName: allSelected ? "checkmark.square.fill" : "square")
            }
            .buttonStyle(.plain)
            .frame(width: 26)

            Text("File")
                .frame(maxWidth: .infinity, alignment: .leading)
            Text("Status")
                .frame(width: 118, alignment: .leading)
            Text("Progress")
                .frame(width: 126, alignment: .leading)
            Text("Updated")
                .frame(width: 138, alignment: .leading)
            Text("")
                .frame(width: 34)
        }
        .font(.caption.weight(.semibold))
        .foregroundStyle(.secondary)
        .padding(.horizontal, 14)
        .padding(.vertical, 12)
        .background(.regularMaterial)
    }
}

struct SavedViewChip: View {
    let view: SavedJobView
    let active: Bool
    let action: () -> Void
    var onDelete: (() -> Void)?

    var body: some View {
        HStack(spacing: 4) {
            if active {
                Button(action: action) {
                    Text(view.label)
                        .font(.caption.bold())
                }
                .buttonStyle(.glassProminent)
            } else {
                Button(action: action) {
                    Text(view.label)
                        .font(.caption.bold())
                }
                .buttonStyle(.glass)
            }

            if let onDelete {
                Button(action: onDelete) {
                    Image(systemName: "trash")
                }
                .buttonStyle(.glass)
                .help("Delete view")
            }
        }
    }
}

struct SaveJobViewSheet: View {
    @Binding var name: String
    let onSave: () -> Void
    let onCancel: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            Text("Save Job View")
                .font(.title3.bold())
            TextField("View name", text: $name)
                .textFieldStyle(.roundedBorder)
            HStack {
                Spacer()
                Button("Cancel", action: onCancel)
                Button("Save", action: onSave)
                    .keyboardShortcut(.defaultAction)
                    .disabled(name.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
            }
        }
    }
}

struct EnqueuePathSheet: View {
    @Binding var path: String
    let onSubmit: () -> Void
    let onCancel: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            Text("Add File by Path")
                .font(.title3.bold())
            TextField("/absolute/path/to/media.mkv", text: $path)
                .textFieldStyle(.roundedBorder)
            HStack {
                Spacer()
                Button("Cancel", action: onCancel)
                Button("Add", action: onSubmit)
                    .keyboardShortcut(.defaultAction)
                    .disabled(path.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
            }
        }
    }
}

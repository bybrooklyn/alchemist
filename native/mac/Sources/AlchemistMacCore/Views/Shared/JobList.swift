import SwiftUI

struct JobList: View {
    let jobs: [Job]
    var onRestart: ((Job) -> Void)? = nil
    var onCancel: ((Job) -> Void)? = nil

    var body: some View {
        VStack(spacing: 8) {
            if jobs.isEmpty {
                EmptyHeroCard(title: "No jobs yet", detail: "Add media to populate the queue.")
            } else {
                ForEach(jobs) { job in
                    HStack(spacing: 16) {
                        Image(systemName: job.isActive ? "bolt.circle.fill" : "circle")
                            .font(.title3)
                            .foregroundStyle(job.isActive ? Color.heliosAccent : .secondary.opacity(0.5))

                        VStack(alignment: .leading, spacing: 4) {
                            Text(job.fileName)
                                .font(.headline)
                                .foregroundStyle(.primary)
                                .lineLimit(1)
                            Text(job.inputPath)
                                .font(.caption)
                                .foregroundStyle(.secondary)
                                .lineLimit(1)
                        }

                        Spacer()

                        VStack(alignment: .trailing, spacing: 6) {
                            StatusPill(status: job.status)
                            if job.isActive {
                                ProgressView(value: job.progressFraction)
                                    .tint(Color.heliosAccent)
                                    .frame(width: 100)
                                    .controlSize(.small)
                            }
                        }
                    }
                    .padding(.horizontal, 16)
                    .padding(.vertical, 14)
                    .background(Color(nsColor: .controlBackgroundColor).opacity(0.5), in: RoundedRectangle(cornerRadius: 14, style: .continuous))
                    .overlay(RoundedRectangle(cornerRadius: 14, style: .continuous).stroke(Color.white.opacity(0.05), lineWidth: 1))
                    .contextMenu {
                        if let onRestart {
                            Button("Restart Job") { onRestart(job) }
                        }
                        if let onCancel {
                            Button("Cancel Job", role: .destructive) { onCancel(job) }
                        }
                    }
                }
            }
        }
    }
}

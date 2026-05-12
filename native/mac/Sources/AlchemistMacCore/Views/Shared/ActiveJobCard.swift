import SwiftUI

struct ActiveJobCard: View {
    let job: Job
    var onCancel: (() -> Void)? = nil
    var onRestart: (() -> Void)? = nil

    var body: some View {
        VStack(alignment: .leading, spacing: 18) {
            HStack(alignment: .top) {
                VStack(alignment: .leading, spacing: 4) {
                    Text("ACTIVE ENCODE")
                        .font(.caption.weight(.heavy))
                        .foregroundStyle(Color.heliosAccent)
                        .tracking(1)
                    Text(job.fileName)
                        .font(.title3.bold())
                        .foregroundStyle(.primary)
                        .lineLimit(1)
                }
                Spacer()
                StatusPill(status: job.status)
            }

            VStack(spacing: 8) {
                ProgressView(value: job.progressFraction)
                    .tint(Color.heliosAccent)
                    .controlSize(.large)

                HStack {
                    Text("\((job.progress ?? 0).formatted(.number.precision(.fractionLength(1))))%")
                        .font(.caption.monospacedDigit().bold())
                        .foregroundStyle(.primary)
                    Spacer()
                    Text(job.encoder ?? "Encoder pending")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
            }

            Divider().background(.white.opacity(0.1))

            HStack {
                if let onRestart {
                    Button {
                        onRestart()
                    } label: {
                        Label("Restart", systemImage: "arrow.counterclockwise")
                    }
                    .buttonStyle(.glass)
                }
                if let onCancel {
                    Button(role: .destructive) {
                        onCancel()
                    } label: {
                        Label("Cancel", systemImage: "xmark.circle")
                    }
                    .buttonStyle(.glass)
                }
            }
        }
        .padding(24)
        .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 24, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 24, style: .continuous).stroke(Color.white.opacity(0.1), lineWidth: 1))
        .shadow(color: .black.opacity(0.2), radius: 20, y: 10)
    }
}

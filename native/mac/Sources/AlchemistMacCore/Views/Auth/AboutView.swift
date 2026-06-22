import SwiftUI

struct AboutView: View {
    var body: some View {
        VStack(spacing: 24) {
            Spacer()

            Image(systemName: "flask.fill")
                .font(.system(size: 80))
                .foregroundStyle(Color.heliosAccent)
                .shadow(color: Color.heliosAccent.opacity(0.3), radius: 10)

            VStack(spacing: 8) {
                Text("Alchemist")
                    .font(.system(size: 48, weight: .bold, design: .rounded))
                    .foregroundStyle(.primary)
                Text("v0.3.0-dev")
                    .font(.title3.weight(.semibold))
                    .foregroundStyle(.secondary)
            }

            Text("Brutally capable transcoding pipeline.")
                .font(.title3)
                .foregroundStyle(.secondary)

            VStack(spacing: 16) {
                Link(destination: URL(string: "https://github.com/bybrooklyn/alchemist")!) {
                    Label("GitHub Repository", systemImage: "link")
                }
                Link(destination: URL(string: "https://alchemist.sh")!) {
                    Label("Documentation", systemImage: "doc.text")
                }
            }
            .buttonStyle(.glass)
            .padding(.top, 10)

            Spacer()

            Text("(c) 2026 Brooklyn. Distributed under AGPLv3.")
                .font(.caption.weight(.medium))
                .foregroundStyle(.tertiary)
        }
        .padding(60)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

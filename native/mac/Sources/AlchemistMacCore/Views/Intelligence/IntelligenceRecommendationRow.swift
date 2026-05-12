import SwiftUI

struct IntelligenceRecommendationRow: View {
    let recommendation: IntelligenceRecommendation
    var onEnqueue: ((String) -> Void)? = nil

    var body: some View {
        HStack(alignment: .top, spacing: 12) {
            Image(systemName: "sparkles")
                .foregroundStyle(Color.heliosAccent)
                .frame(width: 24)
            VStack(alignment: .leading, spacing: 4) {
                Text(recommendation.title)
                    .font(.subheadline.bold())
                Text(recommendation.summary)
                    .font(.caption)
                    .foregroundStyle(.secondary)
                Text(recommendation.path)
                    .font(.caption2.monospaced())
                    .foregroundStyle(.tertiary)
                    .lineLimit(1)
            }
            Spacer()
            Button(recommendation.suggestedAction.capitalized) {
                onEnqueue?(recommendation.path)
            }
            .buttonStyle(.glass)
        }
        .padding(12)
        .background(.thinMaterial, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
    }
}

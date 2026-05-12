import SwiftUI

struct IntelligenceView: View {
    @Environment(AppModel.self) private var model

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 22) {
                HStack {
                    VStack(alignment: .leading, spacing: 4) {
                        Text("Intelligence")
                            .font(.system(size: 34, weight: .bold, design: .rounded))
                        Text("Library recommendations and duplicate detection")
                            .font(.title3)
                            .foregroundStyle(.secondary)
                    }
                    Spacer()
                    Button {
                        Task { await model.intelligence.load(apiClient: model.connection.apiClient) }
                    } label: {
                        Label("Refresh", systemImage: "arrow.clockwise")
                    }
                    .buttonStyle(.glass)
                }

                if let intelligence = model.intelligence.intelligence {
                    LazyVGrid(columns: Array(repeating: GridItem(.flexible(), spacing: 12), count: 4), spacing: 12) {
                        DashboardStatCard(title: "Duplicates", value: "\(intelligence.totalDuplicates)", symbol: "doc.on.doc.fill", color: Color.heliosWarning)
                        DashboardStatCard(title: "Remux", value: "\(intelligence.recommendationCounts.remuxOnlyCandidate)", symbol: "arrow.triangle.2.circlepath", color: Color.heliosAccent)
                        DashboardStatCard(title: "Audio Layout", value: "\(intelligence.recommendationCounts.wastefulAudioLayout)", symbol: "speaker.wave.3.fill", color: Color.heliosAccent)
                        DashboardStatCard(title: "Commentary", value: "\(intelligence.recommendationCounts.commentaryCleanupCandidate)", symbol: "waveform", color: Color.heliosAccent)
                    }

                    VStack(alignment: .leading, spacing: 12) {
                        Label("Recommendations", systemImage: "sparkles")
                            .font(.headline)
                        if intelligence.recommendations.isEmpty {
                            EmptyHeroCard(title: "No recommendations", detail: "Alchemist has not found optimization opportunities yet.")
                        } else {
                            ForEach(intelligence.recommendations.prefix(30)) { recommendation in
                                IntelligenceRecommendationRow(recommendation: recommendation) { path in
                                    Task { await model.intelligence.enqueuePath(path, apiClient: model.connection.apiClient) }
                                }
                            }
                        }
                    }
                } else {
                    EmptyHeroCard(title: "No intelligence loaded", detail: "Refresh to load duplicate groups and optimization recommendations.")
                }
            }
            .padding(28)
        }
        .task { await model.intelligence.load(apiClient: model.connection.apiClient) }
    }
}

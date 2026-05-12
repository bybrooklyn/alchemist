import SwiftUI

struct StatisticsView: View {
    @Environment(AppModel.self) private var model

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 22) {
                HStack {
                    VStack(alignment: .leading, spacing: 4) {
                        Text("Statistics")
                            .font(.system(size: 34, weight: .bold, design: .rounded))
                        Text("Savings, throughput, and recent completion history")
                            .font(.title3)
                            .foregroundStyle(.secondary)
                    }
                    Spacer()
                    Button {
                        Task { await model.refreshAll() }
                    } label: {
                        Label("Refresh", systemImage: "arrow.clockwise")
                    }
                    .buttonStyle(.glass)
                }

                LazyVGrid(columns: Array(repeating: GridItem(.flexible(), spacing: 12), count: 4), spacing: 12) {
                    DashboardStatCard(title: "Recovered", value: AlchemistFormatter.bytes(model.dashboard.savings.totalBytesSaved), symbol: "externaldrive.fill", color: Color.heliosSuccess)
                    DashboardStatCard(title: "Completed", value: "\(model.dashboard.savings.jobCount)", symbol: "checkmark.circle.fill", color: Color.heliosAccent)
                    DashboardStatCard(title: "Input", value: AlchemistFormatter.bytes(model.dashboard.savings.totalInputBytes), symbol: "arrow.down.doc.fill", color: Color.heliosAccent)
                    DashboardStatCard(title: "Output", value: AlchemistFormatter.bytes(model.dashboard.savings.totalOutputBytes), symbol: "arrow.up.doc.fill", color: Color.heliosAccent)
                }

                VStack(alignment: .leading, spacing: 12) {
                    Label("Daily History", systemImage: "chart.line.uptrend.xyaxis")
                        .font(.headline)
                    ForEach(model.dashboard.dailyStats.suffix(14), id: \.date) { stat in
                        HStack {
                            Text(stat.date)
                                .font(.subheadline.monospacedDigit())
                            Spacer()
                            Text("\(stat.jobsCompleted) jobs")
                                .foregroundStyle(.secondary)
                            Text(AlchemistFormatter.bytes(stat.bytesSaved))
                                .font(.subheadline.monospacedDigit().bold())
                                .foregroundStyle(Color.heliosSuccess)
                        }
                        .padding(12)
                        .background(.thinMaterial, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
                    }
                }
                .padding(18)
                .background(.ultraThinMaterial, in: RoundedRectangle(cornerRadius: 14, style: .continuous))
            }
            .padding(28)
        }
    }
}

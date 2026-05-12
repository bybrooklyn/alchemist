import SwiftUI

struct DashboardView: View {
    @Environment(AppModel.self) private var model

    private var weeklyBytesSaved: Int64 {
        model.dashboard.dailyStats.suffix(7).reduce(0) { $0 + $1.bytesSaved }
    }

    private var weeklyJobsCompleted: Int {
        model.dashboard.dailyStats.suffix(7).reduce(0) { $0 + $1.jobsCompleted }
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 20) {
                header
                engineBanner

                LazyVGrid(columns: Array(repeating: GridItem(.flexible(), spacing: 12), count: 4), spacing: 12) {
                    DashboardStatCard(title: "Active Jobs", value: "\(model.dashboard.stats.active)", symbol: "bolt.fill", color: Color.heliosAccent)
                    DashboardStatCard(title: "Completed", value: "\(model.dashboard.stats.completed)", symbol: "checkmark.circle.fill", color: Color.heliosSuccess)
                    DashboardStatCard(title: "Failed", value: "\(model.dashboard.stats.failed)", symbol: "exclamationmark.triangle.fill", color: Color.heliosError)
                    DashboardStatCard(title: "Total Processed", value: "\(model.dashboard.stats.total)", symbol: "server.rack", color: Color.heliosAccent)
                }

                HStack(alignment: .top, spacing: 20) {
                    recentActivity
                        .frame(maxWidth: .infinity)

                    VStack(alignment: .leading, spacing: 20) {
                        WeekPanel(weeklyBytesSaved: weeklyBytesSaved, weeklyJobsCompleted: weeklyJobsCompleted)
                        ConfigPanel(settingsBundle: model.dashboard.settingsBundle)
                    }
                    .frame(width: 320)
                }

                SystemHealthMonitor()
            }
            .padding(32)
        }
        .animation(.easeInOut(duration: 0.2), value: model.engine.status.status)
    }

    private var header: some View {
        HStack(alignment: .center) {
            VStack(alignment: .leading, spacing: 4) {
                Text("Dashboard")
                    .font(.system(size: 34, weight: .bold, design: .rounded))
                Text("Real-time system overview")
                    .font(.title3)
                    .foregroundStyle(.secondary)
            }

            Spacer()

            HStack(spacing: 12) {
                StatusPill(status: model.engine.status.status)

                if model.isAuthenticated {
                    Button("Logout", systemImage: "rectangle.portrait.and.arrow.right") {
                        Task { await model.logout() }
                    }
                    .buttonStyle(.glass)
                } else {
                    Button("Login", systemImage: "person.crop.circle") {
                        model.navigation.presentLogin()
                    }
                    .buttonStyle(.glassProminent)
                    .tint(model.theme.accent.color)
                }
            }
        }
    }

    @ViewBuilder
    private var engineBanner: some View {
        if model.engine.isPaused {
            HStack(spacing: 12) {
                Text("ENGINE PAUSED")
                    .font(.caption.bold())
                    .foregroundStyle(Color.heliosAccent)
                Text("Analysis runs automatically. Start the engine to begin encoding.")
                    .font(.subheadline)
                    .foregroundStyle(.primary)
                Spacer()
                Button {
                    Task { await model.resumeQueue() }
                } label: {
                    Label("Start", systemImage: "play.fill")
                }
                .buttonStyle(.glassProminent)
                .tint(model.theme.accent.color)
            }
            .padding(14)
            .background(Color.heliosAccent.opacity(0.10), in: RoundedRectangle(cornerRadius: 12, style: .continuous))
            .overlay(RoundedRectangle(cornerRadius: 12, style: .continuous).stroke(Color.heliosAccent.opacity(0.22), lineWidth: 1))
        }
    }

    private var recentActivity: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack {
                Label("Recent Activity", systemImage: "clock")
                    .font(.headline)
                Spacer()
                Button("View all") {
                    model.navigation.selectedSection = .queue
                }
                .buttonStyle(.glass)
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 14)
            .background(.regularMaterial)

            VStack(spacing: 0) {
                if model.jobs.jobs.isEmpty {
                    EmptyHeroCard(title: "No recent activity", detail: "Add a library folder or convert a file to populate the queue.")
                        .padding(18)
                } else {
                    ForEach(model.jobs.jobs.prefix(5)) { job in
                        DashboardActivityRow(job: job)
                    }
                }
            }
            .frame(minHeight: 260, alignment: .top)
        }
        .background(.thinMaterial, in: RoundedRectangle(cornerRadius: 12, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 12, style: .continuous).stroke(Color.white.opacity(0.08), lineWidth: 1))
    }
}

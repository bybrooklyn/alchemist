import SwiftUI

struct SystemHealthMonitor: View {
    @Environment(AppModel.self) private var model

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            Label("System Health", systemImage: "waveform.path.ecg")
                .font(.headline)
                .foregroundStyle(.secondary)

            LazyVGrid(columns: [GridItem(.adaptive(minimum: 200, maximum: 350), spacing: 16)], spacing: 16) {
                ResourceCard(
                    title: "CPU Usage",
                    value: "\(Int(model.dashboard.resources.cpuPercent))%",
                    detailLeft: "CPU Cores",
                    detailRight: "\(model.dashboard.resources.cpuCount) Logical",
                    symbol: "cpu",
                    progress: Double(model.dashboard.resources.cpuPercent / 100),
                    tint: Color.heliosAccent
                )

                ResourceCard(
                    title: "Memory",
                    value: "\(Int(model.dashboard.resources.memoryPercent))%",
                    detailLeft: "\(model.dashboard.resources.memoryUsedMb / 1024) GB used",
                    detailRight: "\(model.dashboard.resources.memoryTotalMb / 1024) GB total",
                    symbol: "memorychip",
                    progress: Double(model.dashboard.resources.memoryPercent / 100),
                    tint: .blue
                )

                ResourceCard(
                    title: "Active Jobs",
                    value: "\(model.dashboard.resources.activeJobs) / \(model.dashboard.resources.concurrentLimit)",
                    detailLeft: "Processor Load",
                    detailRight: "",
                    symbol: "layers",
                    progress: Double(model.dashboard.resources.activeJobs) / Double(max(1, model.dashboard.resources.concurrentLimit)),
                    tint: Color.heliosSuccess
                )

                ResourceCard(
                    title: "GPU",
                    value: model.dashboard.resources.gpuUtilization.map { "\(Int($0))%" } ?? "N/A",
                    detailLeft: "VRAM",
                    detailRight: model.dashboard.resources.gpuMemoryPercent.map { "\(Int($0))% used" } ?? "-",
                    symbol: "cpu.fill",
                    progress: model.dashboard.resources.gpuUtilization.map { Double($0 / 100) },
                    tint: .purple
                )

                ResourceCard(
                    title: "Uptime",
                    value: model.dashboard.resources.uptimeDescription,
                    symbol: "clock",
                    progress: nil,
                    tint: .primary,
                    isUptime: true
                )
            }
        }
    }
}

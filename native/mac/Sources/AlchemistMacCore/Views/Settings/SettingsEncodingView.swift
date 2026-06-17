import SwiftUI

struct SettingsEncodingView: View {
    @Environment(AppModel.self) private var model

    var body: some View {
        Form {
            Section("Output") {
                LabeledContent("Codec") {
                    Text(model.dashboard.settingsBundle?.settings.transcode.outputCodec.uppercased() ?? "AV1")
                        .monospaced()
                }
                LabeledContent("Quality Profile") {
                    Text(model.dashboard.settingsBundle?.settings.transcode.qualityProfile.capitalized ?? "Balanced")
                }
                LabeledContent("Subtitle Mode") {
                    Text(model.dashboard.settingsBundle?.settings.transcode.subtitleMode.capitalized ?? "Copy")
                }
            }

            Section("Limits") {
                LabeledContent("Concurrent Jobs") {
                    Text("\(model.dashboard.settingsBundle?.settings.transcode.concurrentJobs ?? 2)")
                        .monospaced()
                }
                LabeledContent("Min File Size") {
                    Text("\(model.dashboard.settingsBundle?.settings.transcode.minFileSizeMB ?? 100) MB")
                        .monospaced()
                }
                LabeledContent("Size Reduction Threshold") {
                    Text("\(Int((model.dashboard.settingsBundle?.settings.transcode.sizeReductionThreshold ?? 0.3) * 100))%")
                        .monospaced()
                }
            }

            Section("Hardware") {
                LabeledContent("CPU Encoding") {
                    Text(model.dashboard.settingsBundle?.settings.hardware.allowCPUEncoding == true ? "Enabled" : "Disabled")
                }
                LabeledContent("CPU Fallback") {
                    Text(model.dashboard.settingsBundle?.settings.hardware.allowCPUFallback == true ? "Enabled" : "Disabled")
                }
                if let vendor = model.dashboard.settingsBundle?.settings.hardware.preferredVendor {
                    LabeledContent("Preferred Vendor") {
                        Text(vendor)
                    }
                }
                LabeledContent("CPU Preset") {
                    Text(model.dashboard.settingsBundle?.settings.hardware.cpuPreset ?? "medium")
                        .monospaced()
                }
            }

            Section("Quality") {
                LabeledContent("VMAF") {
                    Text(model.dashboard.settingsBundle?.settings.quality.enableVMAF == true ? "Enabled" : "Disabled")
                }
                if model.dashboard.settingsBundle?.settings.quality.enableVMAF == true {
                    LabeledContent("Min VMAF Score") {
                        Text("\(Int(model.dashboard.settingsBundle?.settings.quality.minVMAFScore ?? 90))")
                            .monospaced()
                    }
                    LabeledContent("Revert on Low Quality") {
                        Text(model.dashboard.settingsBundle?.settings.quality.revertOnLowQuality == true ? "Yes" : "No")
                    }
                }
            }
        }
        .formStyle(.grouped)
        .padding(20)
    }
}

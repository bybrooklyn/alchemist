import SwiftUI

struct ResourceCard: View {
    let title: String
    let value: String
    var detailLeft: String? = nil
    var detailRight: String? = nil
    let symbol: String
    let progress: Double?
    let tint: Color
    var isUptime: Bool = false

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack {
                Label {
                    Text(title)
                        .font(.system(size: 13, weight: .medium))
                        .foregroundStyle(.primary)
                } icon: {
                    Image(systemName: symbol)
                        .font(.system(size: 14))
                        .foregroundStyle(.secondary)
                }

                Spacer()

                if !isUptime {
                    Text(value)
                        .font(.system(size: 11, weight: .bold))
                        .foregroundStyle(tint)
                        .padding(.horizontal, 8)
                        .padding(.vertical, 3)
                        .background(tint.opacity(0.15))
                        .clipShape(Capsule())
                } else {
                    Image(systemName: "waveform.path.ecg")
                        .font(.system(size: 14))
                        .foregroundStyle(Color.heliosSuccess)
                }
            }
            .padding(.bottom, 12)

            if !isUptime {
                if let progress {
                    GeometryReader { geo in
                        ZStack(alignment: .leading) {
                            Capsule()
                                .fill(Color.white.opacity(0.05))
                                .frame(height: 6)
                            Capsule()
                                .fill(tint)
                                .frame(width: max(0, min(geo.size.width, geo.size.width * CGFloat(progress))), height: 6)
                        }
                    }
                    .frame(height: 6)
                    .padding(.bottom, 8)
                } else {
                    Capsule()
                        .fill(Color.white.opacity(0.05))
                        .frame(height: 6)
                        .padding(.bottom, 8)
                }

                HStack {
                    if let detailLeft { Text(detailLeft) }
                    Spacer()
                    if let detailRight { Text(detailRight) }
                }
                .font(.system(size: 11))
                .foregroundStyle(.secondary)
            } else {
                Spacer()
                Text(value)
                    .font(.system(size: 26, weight: .bold))
                    .foregroundStyle(.primary)
                    .padding(.bottom, 4)
            }
        }
        .padding(14)
        .frame(maxWidth: .infinity, minHeight: 96, alignment: .topLeading)
        .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 12, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 12, style: .continuous).stroke(Color.white.opacity(0.1), lineWidth: 1))
    }
}

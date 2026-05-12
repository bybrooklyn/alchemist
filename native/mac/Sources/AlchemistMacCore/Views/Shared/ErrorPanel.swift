import SwiftUI

struct ErrorPanel: View {
    let message: String

    var body: some View {
        Text(message)
            .font(.callout)
            .foregroundStyle(Color.heliosError)
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(14)
            .background(Color.heliosError.opacity(0.1), in: RoundedRectangle(cornerRadius: 12, style: .continuous))
            .overlay(RoundedRectangle(cornerRadius: 12, style: .continuous).stroke(Color.heliosError.opacity(0.3), lineWidth: 1))
    }
}

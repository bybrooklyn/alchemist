import SwiftUI

struct SectionHeader: View {
    let title: String
    let actionTitle: String
    let action: () -> Void

    var body: some View {
        HStack {
            Text(title)
                .font(.title2.bold())
                .foregroundStyle(.primary)
            Spacer()
            Button(actionTitle, action: action)
                .buttonStyle(.glass)
        }
    }
}

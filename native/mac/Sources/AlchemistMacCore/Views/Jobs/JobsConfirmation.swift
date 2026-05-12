import Foundation
import SwiftUI

struct JobsConfirmation: Identifiable {
    let id = UUID()
    let title: String
    let message: String
    let confirmLabel: String
    let role: ButtonRole?
    let action: () -> Void

    init(
        title: String,
        message: String,
        confirmLabel: String,
        role: ButtonRole? = nil,
        action: @escaping () -> Void
    ) {
        self.title = title
        self.message = message
        self.confirmLabel = confirmLabel
        self.role = role
        self.action = action
    }
}

enum JobDetailEmptyState {
    static func forStatus(_ status: String) -> (title: String, detail: String) {
        switch status {
        case "queued":
            ("Waiting in queue", "This job is queued and waiting for an available worker slot.")
        case "analyzing":
            ("Analyzing media", "Alchemist is reading metadata and planning the next action.")
        case "encoding":
            ("Encoding in progress", "The transcode is running now. Detailed input metadata may appear after analysis persists.")
        case "remuxing":
            ("Remuxing in progress", "The job is copying compatible streams into the target container.")
        case "resuming":
            ("Resuming job", "The job is being prepared to continue processing.")
        case "failed":
            ("No metadata captured", "This job failed before full media metadata was persisted.")
        case "skipped":
            ("No metadata captured", "This file was skipped before full media metadata was stored.")
        default:
            ("No encode data available", "Detailed metadata is not available for this job yet.")
        }
    }
}

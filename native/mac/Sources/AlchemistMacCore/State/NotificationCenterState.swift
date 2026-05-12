import Foundation
import UserNotifications

@Observable
@MainActor
public final class NotificationCenterState {
    public var authorizationStatus: UNAuthorizationStatus = .notDetermined

    public init() {}

    private var notificationsAvailable: Bool {
        let bundleType = Bundle.main.object(forInfoDictionaryKey: "CFBundlePackageType") as? String
        return bundleType == "APPL" && Bundle.main.bundleURL.pathExtension.lowercased() == "app"
    }

    public func requestAuthorizationIfNeeded() async {
        guard notificationsAvailable else {
            authorizationStatus = .denied
            return
        }

        let center = UNUserNotificationCenter.current()
        let settings = await center.notificationSettings()
        authorizationStatus = settings.authorizationStatus
        guard settings.authorizationStatus == .notDetermined else {
            return
        }

        do {
            let granted = try await center.requestAuthorization(options: [.alert, .sound, .badge])
            authorizationStatus = granted ? .authorized : .denied
        } catch {
            authorizationStatus = .denied
        }
    }

    public func postJobNotification(title: String, body: String) {
        guard notificationsAvailable else {
            return
        }
        guard authorizationStatus == .authorized || authorizationStatus == .provisional else {
            return
        }

        let content = UNMutableNotificationContent()
        content.title = title
        content.body = body
        content.sound = .default

        let request = UNNotificationRequest(
            identifier: UUID().uuidString,
            content: content,
            trigger: nil
        )
        UNUserNotificationCenter.current().add(request)
    }
}

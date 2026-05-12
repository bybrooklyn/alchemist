import Foundation
import SwiftUI

@Observable
@MainActor
public final class NavigationRouter {
    public var selectedSection: AppSection = .dashboard
    public var showingInspector = true
    public var showingLogin = false
    public var showingCommandPalette = false

    public init() {}

    public func navigate(to section: AppSection) {
        selectedSection = section
    }

    public func toggleInspector() {
        showingInspector.toggle()
    }

    public func presentLogin() {
        showingLogin = true
    }

    public func dismissLogin() {
        showingLogin = false
    }

    public func toggleCommandPalette() {
        showingCommandPalette.toggle()
    }
}

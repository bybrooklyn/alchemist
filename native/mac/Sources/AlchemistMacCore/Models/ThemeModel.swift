import AppKit
import SwiftUI

public enum AppTheme: String, CaseIterable, Identifiable, Sendable {
    case system
    case light
    case dark

    public var id: String { rawValue }

    public var label: String {
        switch self {
        case .system: "System"
        case .light: "Light"
        case .dark: "Dark"
        }
    }

    public var colorScheme: ColorScheme? {
        switch self {
        case .system: nil
        case .light: .light
        case .dark: .dark
        }
    }
}

public enum AppAccent: String, CaseIterable, Identifiable, Sendable {
    case heliosOrange = "helios-orange"
    case alchemistGreen = "alchemist-green"
    case system
    case graphite

    public var id: String { rawValue }

    public var label: String {
        switch self {
        case .heliosOrange: "Helios Orange"
        case .alchemistGreen: "Alchemist Green"
        case .system: "System"
        case .graphite: "Graphite"
        }
    }

    public var color: Color {
        switch self {
        case .heliosOrange: Color.heliosAccent
        case .alchemistGreen: .green
        case .system: Color(nsColor: .controlAccentColor)
        case .graphite: .gray
        }
    }
}

public enum MaterialIntensity: String, CaseIterable, Identifiable, Sendable {
    case adaptive
    case reduced
    case solid

    public var id: String { rawValue }

    public var label: String {
        switch self {
        case .adaptive: "Adaptive"
        case .reduced: "Reduced"
        case .solid: "Solid"
        }
    }
}

public enum AppDensity: String, CaseIterable, Identifiable, Sendable {
    case compact
    case comfortable
    case spacious

    public var id: String { rawValue }

    public var label: String {
        switch self {
        case .compact: "Compact"
        case .comfortable: "Comfortable"
        case .spacious: "Spacious"
        }
    }
}

public struct ThemeModel: Equatable, Sendable {
    public var theme: AppTheme
    public var accent: AppAccent
    public var material: MaterialIntensity
    public var density: AppDensity

    public init(
        theme: AppTheme = .system,
        accent: AppAccent = .heliosOrange,
        material: MaterialIntensity = .adaptive,
        density: AppDensity = .comfortable
    ) {
        self.theme = theme
        self.accent = accent
        self.material = material
        self.density = density
    }

    public static let prototypeDefault = ThemeModel()

    // Prototype-only view state. Canonical appearance settings should move to
    // Alchemist config once the backend exposes those fields.
}

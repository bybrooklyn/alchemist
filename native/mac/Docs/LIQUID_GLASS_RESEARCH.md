# Liquid Glass Research Notes

Research date: 2026-04-30

Sources:

- Apple Developer Documentation: Adopting Liquid Glass
  <https://developer.apple.com/documentation/TechnologyOverviews/adopting-liquid-glass>
- Apple Human Interface Guidelines: Materials
  <https://developer.apple.com/design/Human-Interface-Guidelines/materials>
- Apple Developer Documentation: SwiftUI updates
  <https://developer.apple.com/documentation/updates/swiftui>
- Apple Developer Documentation: What is new in SwiftUI, WWDC25
  <https://developer.apple.com/videos/play/wwdc2025/256/>
- Apple Developer Documentation: MenuBarExtra
  <https://developer.apple.com/documentation/SwiftUI/MenuBarExtra>

## Conclusions

- Rebuild with the newest SDK and prefer standard SwiftUI/AppKit components.
  `NavigationSplitView`, `toolbar`, sheets, popovers, menus, controls, and
  sidebars adopt the new system look with minimal custom code.
- Liquid Glass belongs in the functional/navigation layer: sidebars, toolbars,
  floating command groups, sheets, popovers, and transient controls.
- Dense content should stay solid or use standard materials. Do not apply
  Liquid Glass to job tables, logs, error details, file path lists, or long
  settings forms.
- Use custom `glassEffect(_:in:)` sparingly. When multiple custom glass elements
  sit near each other, wrap them in `GlassEffectContainer` so the system can
  render and morph them efficiently.
- Prefer SwiftUI button styles such as `.glass` and `.glassProminent` over
  handcrafted glass backgrounds.
- Group toolbar items by task and use `ToolbarSpacer` to separate groups.
- Add accessibility labels to icon-only controls and never make color the only
  status signal.
- Use `.searchable` on the split/navigation surface for queue/library search
  rather than building custom search chrome.
- Use `MenuBarExtra` later for always-available queue controls; it is a native
  SwiftUI scene for persistent menu bar access.
- Design the app icon as layered shapes and let Icon Composer/system effects
  handle depth, reflection, blur, and appearance variants.

## Alchemist Application Rules

- Dashboard may use a restrained glass command island because it is a primary
  functional surface.
- Queue rows, logs, diagnostics, and settings forms stay on solid or standard
  material surfaces for readability.
- Main toolbar actions are icon-first with labels/accessibility names.
- Any future custom liquid-glass component must have a fallback for Reduce
  Transparency, Increase Contrast, and Reduce Motion.
- Do not add broad custom backgrounds to the sidebar or toolbar; let system
  navigation containers own those surfaces.

# Alchemist Backlog

Future improvements and features to consider for the project.

## High Priority

### E2E Test Coverage
- Expand Playwright tests for more UI flows
- Test job queue management scenarios
- Test error states and recovery flows

### AMD AV1 Validation
- Validate and tune the existing AMD AV1 paths on real hardware
- Cover Linux VAAPI and Windows AMF separately
- Verify encoder selection, fallback behavior, and quality/performance defaults
- Do not treat this as support-from-scratch: encoder wiring and hardware detection already exist

## Medium Priority

### Performance Optimizations
- Profile scanner/analyzer hot paths before changing behavior
- Only tune connection pooling after measuring database contention under load
- Consider caching repeated FFprobe calls on identical files if profiling shows probe churn is material

### Monitoring & Observability
- Add Prometheus metrics endpoint
- Track encode times, queue depths, error rates
- Add structured logging with correlation IDs

### UI Improvements
- Improve mobile responsiveness
- Add keyboard shortcuts for common actions

### Notification Improvements
- Add email notification support
- Add Telegram integration
- Per-job notification rules (only notify on failure, etc.)

## Low Priority

### Features from DESIGN_PHILOSOPHY.md
- Consider WebSocket alternative to SSE for bidirectional communication
- Add batch job templates

### Code Quality
- Increase test coverage for edge cases
- Add property-based testing for codec parameter generation
- Add fuzzing for FFprobe output parsing

### Documentation
- Add architecture diagrams
- Add contributor guide with development setup
- Video tutorials for common workflows
- API client examples in multiple languages

### Distribution
- Add Homebrew formula
- Add AUR package
- Add Flatpak/Snap packages
- Improve Windows installer (WiX) with auto-updates

## Completed (Recent)

- [x] Split server.rs into modules
- [x] Add API versioning (/api/v1/)
- [x] Add typed broadcast channels
- [x] Add security headers middleware
- [x] Add database query timeouts
- [x] Add config file permission check
- [x] Handle SSE lagged events in frontend
- [x] Create FFmpeg integration tests
- [x] Expand documentation site
- [x] Create OpenAPI spec
- [x] Pin MSRV in Cargo.toml
- [x] Add schema versioning for migrations
- [x] Enable SQLite WAL mode
- [x] Add theme persistence and selection
- [x] Add job history filtering and search
- [x] Add subtitle extraction sidecars

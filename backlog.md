# Alchemist Backlog

Future improvements and features to consider for the project.

## High Priority

### AMD AV1 Support
- Add AV1 encoding support for AMD GPUs (RDNA3+)
- Requires hardware for testing
- Update `src/media/ffmpeg/amd.rs` with AV1 encoder parameters
- Add detection in `src/system/hardware.rs`

### E2E Test Coverage
- Expand Playwright tests for more UI flows
- Test job queue management scenarios
- Test error states and recovery flows
- Add visual regression tests

### Database Migrations
- Add migration versioning system
- Consider SQLite WAL mode for better concurrency
- Add migration rollback support

## Medium Priority

### Performance Optimizations
- Profile and optimize hot paths in scanner/analyzer
- Consider connection pooling tuning for high-load scenarios
- Add caching for repeated FFprobe calls on same files

### Monitoring & Observability
- Add Prometheus metrics endpoint
- Track encode times, queue depths, error rates
- Add structured logging with correlation IDs

### UI Improvements
- Add dark/light theme toggle
- Improve mobile responsiveness
- Add keyboard shortcuts for common actions
- Job history filtering and search

### Notification Improvements
- Add email notification support
- Add Telegram integration
- Per-job notification rules (only notify on failure, etc.)

## Low Priority

### Features from DESIGN_PHILOSOPHY.md
- Consider WebSocket alternative to SSE for bidirectional communication
- Add support for subtitle extraction/conversion
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

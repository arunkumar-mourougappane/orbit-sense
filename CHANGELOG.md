# Changelog

All notable changes to the `orbit-sense` project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.1] - 2026-03-02

### Added

- Comprehensive rustdoc comments across core data structures (`Location`, `Observation`, `AppSettings`, `OrbitSenseApp`, `MapStyle`, `AppMessage`).
- The application now compiles natively without `#![warn(missing_docs)]` or `clippy` lints/warnings.
- `README.md` technical architecture notes outlining how SGP4 overhead prediction and map rendering are handled computationally.

### Changed

- Shifted the UI map navigation controls (zoom, auto-track) upward by 30 pixels from the bottom-right corner to prevent overlapping with the `egui` bottom status bar.
- Refactored `predict_next_pass` to dynamically stride ahead by larger minute increments when the satellite is far outside the observation radius, massively reducing the CPU overhead of processing the `sgp4` positional math.

### Fixed

- Reverted experimental infinite panning logic that previously cloned duplicating phantom maps and satellites, removing ghost artifacts from the `walkers` coordinate field.
- Addressed geometric `has_antimeridian_crossing` edge cases where the swath footprint footprint would detach from the satellite icon or render incorrectly when bridging the `-180`/`+180` longitude borders.
- Removed outdated or redundant caching logic that caused the footprint polygon to visually "lag" behind the spacecraft's high velocity. The geometry ring is now calculated safely alongside the marker inside the per-frame loop.

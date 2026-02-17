# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2026-02-14

### Added

- Camera animation system with queued moves and easing functions
- `ZoomToFit` event for framing entities with `Aabb` components (animated via `duration_ms` or instant with `0.0`)
- `StartAnimation` event for queued camera movement sequences
- `SetFitTarget` event for fit target visualization
- Extension trait `PanOrbitCameraExt` for camera manipulation
- `calculate_fit_radius` public function for computing fit radius at arbitrary orientations
- Focus centering algorithm with tolerance-based convergence
- Fit target visualization with `FitTargetVisualizationPlugin`

[0.1.0]: https://github.com/YOUR_USERNAME/bevy_panorbit_camera_ext/releases/tag/v0.1.0

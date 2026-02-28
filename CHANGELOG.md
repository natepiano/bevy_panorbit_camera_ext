# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2026-02-14

### Added

- `PanOrbitCameraExtPlugin` with all functionality registered automatically
- `ZoomToFit` event for framing entities (instant or animated via `Duration`)
- `AnimateToFit` event for combined orientation change and zoom-to-fit
- `PlayAnimation` event for queued `CameraMove` sequences (`ToPosition`/`ToOrbit`)
- `SetFitTarget` event for setting the visualization target without zooming
- `ToggleFitVisualization` event for enabling/disabling debug visualization
- `InputInterruptBehavior` component (`Cancel`/`Complete`) for controlling animation interruption
- `AnimationSource` enum for distinguishing `PlayAnimation` vs `AnimateToFit` origins
- `CurrentFitTarget` component persisted after fit for visualization continuity
- Builder pattern on `ZoomToFit` and `AnimateToFit` (`.margin()`, `.duration()`, `.easing()`)
- Lifecycle events: `ZoomBegin`/`ZoomEnd`/`ZoomCancelled`, `AnimationBegin`/`AnimationEnd`/`AnimationCancelled`, `CameraMoveBegin`/`CameraMoveEnd`, `FitVisualizationBegin`/`FitVisualizationEnd`
- Perspective and orthographic projection support
- Automatic camera smoothness stashing/restoration during animations
- `visualization` feature flag (default-enabled) gating `FitTargetVisualizationConfig` and gizmo overlays

[0.1.0]: https://github.com/natepiano/bevy_panorbit_camera_ext/releases/tag/v0.1.0

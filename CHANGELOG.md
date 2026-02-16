# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2026-02-14

### Added

- Camera animation system with queued moves and easing functions
- Zoom-to-fit for bounding boxes and meshes with configurable margins
- Snap-to-fit for instant camera positioning
- Extension trait `PanOrbitCameraExt` for camera manipulation
- Entity events: `SnapToFit`, `ZoomToFit`, `ZoomToFitMesh`, `StartAnimation`
- Automatic smoothness preservation via `SmoothnessStash` component
- `CameraExtPlugin` for Bevy integration

[0.1.0]: https://github.com/YOUR_USERNAME/bevy_panorbit_camera_ext/releases/tag/v0.1.0

# bevy_panorbit_camera_ext

Extension library for [`bevy_panorbit_camera`](https://github.com/Plonq/bevy_panorbit_camera) providing camera animation, zoom-to-fit, and helper utilities.

## Features

- Camera animation system with queued moves and easing
- Zoom-to-fit for bounding boxes and meshes
- Snap-to-fit for instant positioning
- Extension traits for camera manipulation
- Automatic smoothness preservation during operations

## Installation

```toml
[dependencies]
bevy = "0.18.0"
bevy_panorbit_camera = "0.34.0"
bevy_panorbit_camera_ext = "0.1.0"
```

## Usage

```rust
use bevy::prelude::*;
use bevy_panorbit_camera::PanOrbitCamera;
use bevy_panorbit_camera_ext::prelude::*;

App::new()
    .add_plugins(CameraExtPlugin)
    .run();

// Zoom to frame a mesh
commands.trigger_targets(ZoomToFitMesh { target }, camera);

// Animate camera movement
commands.trigger_targets(
    StartAnimation {
        moves: vec![
            CameraMove::new(Vec3::new(10.0, 5.0, 10.0), 2.0),
        ],
    },
    camera,
);
```

## Events

- `SnapToFit` - Instantly position camera
- `ZoomToFit` - Smooth zoom to bounding box
- `ZoomToFitMesh` - Smooth zoom to mesh
- `StartAnimation` - Queue camera movements

## Compatibility

- Bevy 0.18.0
- bevy_panorbit_camera 0.34.0

# bevy_panorbit_camera_ext

Extension library for [`bevy_panorbit_camera`](https://github.com/Plonq/bevy_panorbit_camera) providing camera animation, zoom-to-fit, and helper utilities.

## Features

- Camera animation system with queued moves and easing
- Zoom-to-fit with animated or instant positioning
- Extension traits for camera manipulation
- Automatic smoothness preservation during operations
- Fit target visualization for debugging

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
use bevy_panorbit_camera_ext::prelude::*;

App::new()
    .add_plugins(CameraExtPlugin)
    .run();
```

## Events

### `ZoomToFit` vs `AnimateToFit`

Both events fit a target entity in the camera view, but they differ in how they handle orientation:

- **`ZoomToFit`** - Fits the target from the camera's **current** orientation. The camera stays at its current yaw and pitch, only adjusting radius and focus to frame the target. Use this when the user is looking at something from a particular angle and wants to zoom in on it.

- **`AnimateToFit`** - Fits the target from a **specified** orientation. The camera animates to the given yaw and pitch while simultaneously fitting the target. Use this when you need to move the camera to a specific viewing angle, such as returning to a home position.

```rust
// Zoom to fit at current orientation (e.g., user presses "Z" to frame selection)
commands.trigger(ZoomToFit::new(camera_entity, target_entity, DEFAULT_MARGIN, 500.0));

// Animate to a specific orientation and fit (e.g., "home" button returns to front view)
commands.trigger(AnimateToFit::new(
    camera_entity,
    target_entity,
    0.0,   // yaw
    0.0,   // pitch
    DEFAULT_MARGIN,
    1200.0,
    EaseFunction::QuadraticOut,
));
```

### `StartAnimation`

Queue one or more camera moves for sequential playback with easing functions. Useful for cinematic sequences or splash screen animations.

```rust
let moves = VecDeque::from([
    CameraMove {
        target_translation: Vec3::new(0.0, 5.0, 20.0),
        target_focus:       Vec3::ZERO,
        duration_ms:        2000.0,
        easing:             EaseFunction::QuadraticInOut,
    },
]);
commands.trigger(StartAnimation::new(camera_entity, moves));
```

## Compatibility

- Bevy 0.18.0
- bevy_panorbit_camera 0.34.0

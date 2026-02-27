# bevy_panorbit_camera_ext

Extension library for [`bevy_panorbit_camera`](https://github.com/Plonq/bevy_panorbit_camera) providing camera animation, zoom-to-fit, and helper utilities.

## Features

- Simple camera animation system with queued moves and easing
- Zoom-to-fit with animated or instant positioning
- Zoom target debug visualization

## Quick Start

Add the plugin to your app alongside `PanOrbitCameraPlugin`:

```rust
use bevy::prelude::*;
use bevy_panorbit_camera::PanOrbitCameraPlugin;
use bevy_panorbit_camera_ext::PanorbitCameraExtPlugin;

App::new()
    .add_plugins(DefaultPlugins)
    .add_plugins(PanOrbitCameraPlugin)
    .add_plugins(PanOrbitCameraExtPlugin)
    .run();
```

Check out the [demo example](examples/demo.rs) to see everything in action, or run it with:

```sh
cargo run --example demo
```

## Events

### `ZoomToFit` vs `AnimateToFit`

Both events fit a target entity in the camera view, but they differ in how they handle orientation:

- **`ZoomToFit`** - Fits the target from the camera's **current** orientation. The camera stays at its current yaw and pitch, only adjusting radius and focus to frame the target. Use this when the user is looking at something from a particular angle and wants to zoom in on it.

- **`AnimateToFit`** - Fits the target from a **specified** orientation. The camera animates to the given yaw and pitch while simultaneously fitting the target. Use this when you need to move the camera to a specific viewing angle, such as returning to a home position.

```rust
use std::time::Duration;

use bevy::math::curve::easing::EaseFunction;

// Zoom to fit at current orientation (e.g., user presses "Z" to frame selection)
commands.trigger(
    ZoomToFit::new(camera_entity, target_entity)
        .margin(DEFAULT_MARGIN)
        .duration(Duration::from_millis(500)),
);

// Animate to a specific orientation and fit (e.g., "home" button returns to front view)
commands.trigger(
    AnimateToFit::new(camera_entity, target_entity)
        .yaw(0.0)
        .pitch(0.0)
        .margin(DEFAULT_MARGIN)
        .duration(Duration::from_millis(1200))
        .easing(EaseFunction::QuadraticOut),
);
```

### `PlayAnimation`

Queue one or more camera moves for sequential playback with easing functions. Useful for cinematic sequences or splash screen animations.

```rust
use std::time::Duration;

let moves = VecDeque::from([
    CameraMove::ToPosition {
        translation: Vec3::new(0.0, 5.0, 20.0),
        focus:       Vec3::ZERO,
        duration:    Duration::from_secs(2),
        easing:      EaseFunction::QuadraticInOut,
    },
]);
commands.trigger(PlayAnimation::new(camera_entity, moves));
```

`CameraMove` has two variants:
- `ToPosition` — world-space translation + focus (cinematic sequences)
- `ToOrbit` — orbital parameters around a focus (inspection, zoom-to-fit)

### `SetFitTarget`

Sets the debug visualization target entity on a camera without triggering any zoom or animation. This lets you inspect the debug gizmos (bounding box, margins, screen-space bounds) for an entity before deciding to invoke one of the zoom/animation behaviors.

```rust
use std::time::Duration;

// Preview what the debug visualization looks like for this entity
commands.trigger(SetFitTarget::new(camera_entity, target_entity));

// Later, when ready, trigger the actual zoom
commands.trigger(
    ZoomToFit::new(camera_entity, target_entity)
        .margin(DEFAULT_MARGIN)
        .duration(Duration::from_millis(500)),
);
```

### Lifecycle Events

Every animation and zoom operation fires begin/end events that consumers can observe:

| Level | Begin | End |
|-------|-------|-----|
| Zoom operation | `ZoomBegin` | `ZoomEnd` |
| Animation queue | `AnimationBegin` | `AnimationEnd` |
| Individual move | `CameraMoveBegin` | `CameraMoveEnd` |

`CameraMoveBegin` includes the full `CameraMove` via its `camera_move` field.

```rust
// React when a zoom-to-fit ends on a specific camera
commands.entity(camera_entity).observe(|_: On<ZoomEnd>| {
    info!("Zoom finished!");
});

// React to each individual move in an animation queue
commands.entity(camera_entity).observe(|event: On<CameraMoveBegin>| {
    info!("Move to {:?} started", event.camera_move.focus());
});
```

## Version Compatibility

| bevy_panorbit_camera_ext | bevy_panorbit_camera | Bevy |
|--------------------------|----------------------|------|
| 0.1                      | 0.34                 | 0.18 |

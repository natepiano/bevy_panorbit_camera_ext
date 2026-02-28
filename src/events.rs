//! Events for camera animations and zoom operations.
//!
//! Events are organized by feature. Each group starts with the **trigger** event
//! (fire with `commands.trigger(...)`) followed by the **fired** events it produces
//! (observe with `.add_observer(...)`).
//!
//! # Common patterns
//!
//! **Duration** — several events accept a `duration` field. When set to
//! `Duration::ZERO` the operation completes instantly and both the `Begin` and `End`
//! events fire in the same frame. When `duration > Duration::ZERO` the operation
//! animates over time and the `End` event fires on the frame the animation finishes.
//!
//! **Easing** — events that animate also accept an `easing` field
//! ([`EaseFunction`]) that controls the interpolation curve. This only has an effect
//! when `duration > Duration::ZERO`.
//!
//! # Emitted event data
//! Reference of data carried by events - for comparison purposes.
//!
//! | Event                    | `camera_entity` | `target_entity` | `margin` | `duration` | `easing` | `source` | `camera_move` |
//! |--------------------------|-----------------|-----------------|----------|------------|----------|----------|---------------|
//! | [`ZoomBegin`]            | yes             | yes             | yes      | yes        | yes      | —        | —             |
//! | [`ZoomEnd`]              | yes             | yes             | yes      | yes        | yes      | —        | —             |
//! | [`ZoomCancelled`]        | yes             | yes             | yes      | yes        | yes      | —        | —             |
//! | [`AnimationBegin`]       | yes             | —               | —        | —          | —        | yes      | —             |
//! | [`AnimationEnd`]         | yes             | —               | —        | —          | —        | yes      | —             |
//! | [`AnimationCancelled`]   | yes             | —               | —        | —          | —        | yes      | yes           |
//! | [`CameraMoveBegin`]      | yes             | —               | —        | —          | —        | —        | yes           |
//! | [`CameraMoveEnd`]        | yes             | —               | —        | —          | —        | —        | yes           |

use std::collections::VecDeque;
use std::time::Duration;

use bevy::math::curve::easing::EaseFunction;
use bevy::prelude::*;

use crate::animation::CameraMove;

/// Identifies which event triggered an animation lifecycle.
///
/// Carried by [`AnimationBegin`], [`AnimationEnd`], and [`AnimationCancelled`] so
/// observers know whether the animation originated from [`PlayAnimation`] or
/// [`AnimateToFit`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Reflect)]
pub enum AnimationSource {
    /// Animation was triggered by [`PlayAnimation`].
    PlayAnimation,
    /// Animation was triggered by [`AnimateToFit`].
    AnimateToFit,
}

/// `ZoomToFit` — frames a target entity in the camera view without changing the camera angle.
///
/// - `camera_entity` — the entity with a `PanOrbitCamera` component.
/// - `target` — the entity to frame; must have an `Aabb` (added automatically to meshes).
/// - `margin` — total fraction of the screen to leave as space between the target's screen-space
///   bounding box and the screen edge, split equally across both sides of the constraining
///   dimension (e.g. `0.25` → ~12.5% each side).
/// - `duration` — see module-level docs on **Duration**.
/// - `easing` — see module-level docs on **Easing**.
///
/// Fires [`ZoomBegin`] → [`ZoomEnd`] on success, or [`ZoomCancelled`] if the user
/// interrupts with camera input during an animated zoom.
///
/// Trigger [`ToggleFitVisualization`] to see a debug visualization of the chosen
/// `ZoomToFit` target. The last chosen target is preserved on the camera so you can
/// continue to see the visualization after the zoom ends.
///
/// Trigger [`SetFitTarget`] to control where the visualization shows before a
/// `ZoomToFit` has been triggered.
#[derive(EntityEvent, Reflect)]
#[reflect(Event, FromReflect)]
pub struct ZoomToFit {
    #[event_target]
    pub camera_entity: Entity,
    pub target:        Entity,
    pub margin:        f32,
    pub duration:      Duration,
    pub easing:        EaseFunction,
}

impl ZoomToFit {
    pub const fn new(camera_entity: Entity, target: Entity) -> Self {
        Self {
            camera_entity,
            target,
            margin: 0.1,
            duration: Duration::ZERO,
            easing: EaseFunction::CubicOut,
        }
    }

    pub const fn margin(mut self, margin: f32) -> Self {
        self.margin = margin;
        self
    }

    pub const fn duration(mut self, duration: Duration) -> Self {
        self.duration = duration;
        self
    }

    pub const fn easing(mut self, easing: EaseFunction) -> Self {
        self.easing = easing;
        self
    }
}

/// `ZoomBegin` — emitted when a [`ZoomToFit`] operation begins.
///
/// - `camera_entity` — the camera that is zooming.
/// - `target_entity` — the entity being framed.
/// - `margin` — the margin value from the triggering [`ZoomToFit`].
/// - `duration` — the duration from the triggering [`ZoomToFit`].
/// - `easing` — the easing curve from the triggering [`ZoomToFit`].
#[derive(EntityEvent, Reflect)]
#[reflect(Event, FromReflect)]
pub struct ZoomBegin {
    #[event_target]
    pub camera_entity: Entity,
    pub target_entity: Entity,
    pub margin:        f32,
    pub duration:      Duration,
    pub easing:        EaseFunction,
}

/// `ZoomEnd` — emitted when a [`ZoomToFit`] operation completes (both animated and instant).
///
/// - `camera_entity` — the camera that finished zooming.
/// - `target_entity` — the entity that was framed.
/// - `margin` — the margin value from the triggering [`ZoomToFit`].
/// - `duration` — the duration from the triggering [`ZoomToFit`].
/// - `easing` — the easing curve from the triggering [`ZoomToFit`].
#[derive(EntityEvent, Reflect)]
#[reflect(Event, FromReflect)]
pub struct ZoomEnd {
    #[event_target]
    pub camera_entity: Entity,
    pub target_entity: Entity,
    pub margin:        f32,
    pub duration:      Duration,
    pub easing:        EaseFunction,
}

/// `ZoomCancelled` — emitted when a [`ZoomToFit`] animation is cancelled by external
/// camera input. The camera stays at its current position — no snap to final.
///
/// - `camera_entity` — the camera whose zoom was cancelled.
/// - `target_entity` — the entity that was being framed.
/// - `margin` — the margin value from the triggering [`ZoomToFit`].
/// - `duration` — the duration from the triggering [`ZoomToFit`].
/// - `easing` — the easing curve from the triggering [`ZoomToFit`].
#[derive(EntityEvent, Reflect)]
#[reflect(Event, FromReflect)]
pub struct ZoomCancelled {
    #[event_target]
    pub camera_entity: Entity,
    pub target_entity: Entity,
    pub margin:        f32,
    pub duration:      Duration,
    pub easing:        EaseFunction,
}

/// `PlayAnimation` — plays a queued sequence of [`CameraMove`] steps.
///
/// - `camera_entity` — the entity with a `PanOrbitCamera` component.
/// - `camera_moves` — a [`VecDeque`] of [`CameraMove`] steps to play in order. Each [`CameraMove`]
///   is either a `ToPosition` (world-space translation + focus) or a `ToOrbit` (orbital parameters
///   around a focus point), each with its own duration and easing.
///
/// Fires [`AnimationBegin`] at queue start, then [`CameraMoveBegin`] →
/// [`CameraMoveEnd`] for each move, and finally [`AnimationEnd`] when the queue is
/// drained. [`AnimationCancelled`] fires if the user interrupts with camera input.
#[derive(EntityEvent, Reflect)]
#[reflect(Event, FromReflect)]
pub struct PlayAnimation {
    #[event_target]
    pub camera_entity: Entity,
    pub camera_moves:  VecDeque<CameraMove>,
}

impl PlayAnimation {
    pub const fn new(camera_entity: Entity, camera_moves: VecDeque<CameraMove>) -> Self {
        Self {
            camera_entity,
            camera_moves,
        }
    }
}

/// `AnimationBegin` — emitted when a `CameraMoveList` begins processing.
///
/// - `camera_entity` — the camera being animated.
/// - `source` — whether this animation originated from [`PlayAnimation`] or [`AnimateToFit`].
#[derive(EntityEvent, Reflect)]
#[reflect(Event, FromReflect)]
pub struct AnimationBegin {
    #[event_target]
    pub camera_entity: Entity,
    pub source:        AnimationSource,
}

/// `AnimationEnd` — emitted when a `CameraMoveList` finishes all its queued moves.
///
/// - `camera_entity` — the camera that finished animating.
/// - `source` — whether this animation originated from [`PlayAnimation`] or [`AnimateToFit`].
#[derive(EntityEvent, Reflect)]
#[reflect(Event, FromReflect)]
pub struct AnimationEnd {
    #[event_target]
    pub camera_entity: Entity,
    pub source:        AnimationSource,
}

/// `AnimationCancelled` — emitted when a [`PlayAnimation`] or [`AnimateToFit`] is
/// cancelled by external camera input. The camera stays at its current position — no
/// snap to final.
///
/// - `camera_entity` — the camera whose animation was cancelled.
/// - `source` — whether this animation originated from [`PlayAnimation`] or [`AnimateToFit`].
/// - `camera_move` — the [`CameraMove`] that was in progress when cancelled.
#[derive(EntityEvent, Reflect)]
#[reflect(Event, FromReflect)]
pub struct AnimationCancelled {
    #[event_target]
    pub camera_entity: Entity,
    pub source:        AnimationSource,
    pub camera_move:   CameraMove,
}

/// `CameraMoveBegin` — emitted when an individual [`CameraMove`] begins.
///
/// - `camera_entity` — the camera being animated.
/// - `camera_move` — the [`CameraMove`] step that is starting.
#[derive(EntityEvent, Reflect)]
#[reflect(Event, FromReflect)]
pub struct CameraMoveBegin {
    #[event_target]
    pub camera_entity: Entity,
    pub camera_move:   CameraMove,
}

/// `CameraMoveEnd` — emitted when an individual [`CameraMove`] completes.
///
/// - `camera_entity` — the camera that finished this move step.
/// - `camera_move` — the [`CameraMove`] step that completed.
#[derive(EntityEvent, Reflect)]
#[reflect(Event, FromReflect)]
pub struct CameraMoveEnd {
    #[event_target]
    pub camera_entity: Entity,
    pub camera_move:   CameraMove,
}

/// `AnimateToFit` — animates the camera to a specific orientation while framing a target
/// entity in view.
///
/// - `camera_entity` — the entity with a `PanOrbitCamera` component.
/// - `target` — the entity to frame; must have an `Aabb` (added automatically to meshes).
/// - `yaw` — final yaw in radians; updates `PanOrbitCamera::target_yaw`.
/// - `pitch` — final pitch in radians; updates `PanOrbitCamera::target_pitch`.
/// - `margin` — see [`ZoomToFit`] for details on how margin is applied.
/// - `duration` — see module-level docs on **Duration**.
/// - `easing` — see module-level docs on **Easing**.
///
/// Combines orientation change with zoom-to-fit in a single smooth animation.
/// Unlike [`ZoomToFit`], this fires [`AnimationBegin`]/[`AnimationEnd`] rather than
/// [`ZoomBegin`]/[`ZoomEnd`]. `ZoomToFit` is a pure framing operation (preserves the
/// current camera angle), while `AnimateToFit` is a cinematic move that changes yaw and
/// pitch — so it uses the general animation lifecycle instead.
#[derive(EntityEvent, Reflect)]
#[reflect(Event, FromReflect)]
pub struct AnimateToFit {
    #[event_target]
    pub camera_entity: Entity,
    pub target:        Entity,
    pub yaw:           f32,
    pub pitch:         f32,
    pub margin:        f32,
    pub duration:      Duration,
    pub easing:        EaseFunction,
}

impl AnimateToFit {
    pub const fn new(camera_entity: Entity, target: Entity) -> Self {
        Self {
            camera_entity,
            target,
            yaw: 0.0,
            pitch: 0.0,
            margin: 0.1,
            duration: Duration::ZERO,
            easing: EaseFunction::CubicOut,
        }
    }

    pub const fn yaw(mut self, yaw: f32) -> Self {
        self.yaw = yaw;
        self
    }

    pub const fn pitch(mut self, pitch: f32) -> Self {
        self.pitch = pitch;
        self
    }

    pub const fn margin(mut self, margin: f32) -> Self {
        self.margin = margin;
        self
    }

    pub const fn duration(mut self, duration: Duration) -> Self {
        self.duration = duration;
        self
    }

    pub const fn easing(mut self, easing: EaseFunction) -> Self {
        self.easing = easing;
        self
    }
}

/// `SetFitTarget` — sets the visualization target without triggering a zoom. Allows you
/// to inspect bounds before triggering [`ZoomToFit`].
///
/// - `camera_entity` — the entity with a `PanOrbitCamera` component.
/// - `target` — the entity whose bounds to visualize; must have an `Aabb`.
#[derive(EntityEvent, Reflect)]
#[reflect(Event, FromReflect)]
pub struct SetFitTarget {
    #[event_target]
    pub camera_entity: Entity,
    pub target:        Entity,
}

impl SetFitTarget {
    pub const fn new(camera_entity: Entity, target: Entity) -> Self {
        Self {
            camera_entity,
            target,
        }
    }
}

/// `ToggleFitVisualization` — toggles the fit target debug visualization on or off.
/// This is a global toggle (not entity-targeted) — fire it with
/// `commands.trigger(ToggleFitVisualization)`.
///
/// Fires [`FitVisualizationBegin`] when enabling, [`FitVisualizationEnd`] when
/// disabling.
#[derive(Event, Reflect)]
#[reflect(Event, FromReflect)]
pub struct ToggleFitVisualization;

/// `FitVisualizationBegin` — emitted when fit target visualization is enabled.
#[derive(Event, Reflect)]
#[reflect(Event, FromReflect)]
pub struct FitVisualizationBegin;

/// `FitVisualizationEnd` — emitted when fit target visualization is disabled.
#[derive(Event, Reflect)]
#[reflect(Event, FromReflect)]
pub struct FitVisualizationEnd;

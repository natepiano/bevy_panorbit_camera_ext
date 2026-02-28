//! Events for camera animations and zoom operations.
//!
//! Events are organized by feature. Each group starts with the **trigger** event
//! (fire with `commands.trigger(...)`) followed by the **fired** events it produces
//! (observe with `.add_observer(...)`).
//!
//! # Common patterns
//!
//! **Duration** — several events accept a `duration` field. When set to
//! `Duration::ZERO` the operation completes instantly — the camera snaps to its
//! final position and only the **operation-level** begin/end events fire (see
//! [instant paths](#instant-operations) below). When `duration > Duration::ZERO`
//! the operation animates over time through [`PlayAnimation`], so the full nested
//! event sequence fires.
//!
//! **Easing** — events that animate also accept an `easing` field
//! ([`EaseFunction`]) that controls the interpolation curve. This only has an effect
//! when `duration > Duration::ZERO`.
//!
//! # Event ordering
//!
//! Events nest from outermost (operation-level) to innermost (move-level). Every
//! animated path goes through [`PlayAnimation`], so [`AnimationBegin`]/[`AnimationEnd`]
//! and [`CameraMoveBegin`]/[`CameraMoveEnd`] fire for **all** animated operations —
//! including [`ZoomToFit`] and [`AnimateToFit`].
//!
//! ## `PlayAnimation` — normal completion
//!
//! ```text
//! AnimationBegin → CameraMoveBegin → CameraMoveEnd → … → AnimationEnd
//! ```
//!
//! ## `ZoomToFit` (animated) — normal completion
//!
//! `Zoom*` events wrap the animation lifecycle:
//!
//! ```text
//! ZoomBegin → AnimationBegin → CameraMoveBegin → CameraMoveEnd → AnimationEnd → ZoomEnd
//! ```
//!
//! ## `AnimateToFit` (animated) — normal completion
//!
//! No extra wrapping events — uses `source: AnimationSource::AnimateToFit` to
//! distinguish from a plain [`PlayAnimation`]:
//!
//! ```text
//! AnimationBegin → CameraMoveBegin → CameraMoveEnd → AnimationEnd
//! ```
//!
//! ## Instant operations
//!
//! When `duration` is `Duration::ZERO`, the animation system is bypassed entirely.
//! Only the operation-level events fire — no [`AnimationBegin`]/[`AnimationEnd`] or
//! [`CameraMoveBegin`]/[`CameraMoveEnd`].
//!
//! ### `ZoomToFit` (instant)
//!
//! ```text
//! ZoomBegin → ZoomEnd
//! ```
//!
//! ### `AnimateToFit` (instant)
//!
//! Fires animation-level events (to notify observers) but no camera-move-level events:
//!
//! ```text
//! AnimationBegin → AnimationEnd
//! ```
//!
//! ## User input interruption ([`InputInterruptBehavior`](crate::InputInterruptBehavior))
//!
//! When the user physically moves the camera during an animation:
//!
//! - **`Cancel`** (default) — stops where it is:
//!
//!   ```text
//!   … → AnimationCancelled → ZoomCancelled (if zoom)
//!   ```
//!
//! - **`Complete`** — jumps to the final position:
//!
//!   ```text
//!   … → AnimationEnd → ZoomEnd (if zoom)
//!   ```
//!
//! ## Animation conflict ([`AnimationConflictPolicy`](crate::AnimationConflictPolicy))
//!
//! When a new animation request arrives while one is already in-flight:
//!
//! - **`LastWins`** (default) — cancels the in-flight animation, then starts the new one.
//!   `AnimationCancelled` always fires; `ZoomCancelled` additionally fires if the in-flight
//!   operation is a zoom:
//!
//!   ```text
//!   AnimationCancelled → ZoomCancelled (if zoom) → AnimationBegin (new) → …
//!   ```
//!
//! - **`FirstWins`** — rejects the incoming request. No zoom lifecycle events fire — the rejection
//!   is detected before `ZoomBegin`:
//!
//!   ```text
//!   AnimationRejected
//!   ```
//!
//!   The [`AnimationRejected::source`] field identifies what was rejected
//!   ([`AnimationSource::PlayAnimation`], [`AnimationSource::ZoomToFit`], or
//!   [`AnimationSource::AnimateToFit`]).
//!
//! # Emitted event data
//!
//! Reference of data carried by events — for comparison purposes.
//!
//! | Event                    | `camera_entity` | `target_entity` | `margin` | `duration` | `easing` | `source` | `camera_move` |
//! |--------------------------|-----------------|-----------------|----------|------------|----------|----------|---------------|
//! | [`ZoomBegin`]            | yes             | yes             | yes      | yes        | yes      | —        | —             |
//! | [`ZoomEnd`]              | yes             | yes             | yes      | yes        | yes      | —        | —             |
//! | [`ZoomCancelled`]        | yes             | yes             | yes      | yes        | yes      | —        | —             |
//! | [`AnimationBegin`]       | yes             | —               | —        | —          | —        | yes      | —             |
//! | [`AnimationEnd`]         | yes             | —               | —        | —          | —        | yes      | —             |
//! | [`AnimationCancelled`]   | yes             | —               | —        | —          | —        | yes      | yes           |
//! | [`AnimationRejected`]    | yes             | —               | —        | —          | —        | yes      | —             |
//! | [`CameraMoveBegin`]      | yes             | —               | —        | —          | —        | —        | yes           |
//! | [`CameraMoveEnd`]        | yes             | —               | —        | —          | —        | —        | yes           |

use std::collections::VecDeque;
use std::time::Duration;

use bevy::math::curve::easing::EaseFunction;
use bevy::prelude::*;

use crate::animation::CameraMove;

/// Context for a zoom-to-fit operation, passed through [`PlayAnimation`] so
/// that `on_play_animation` can fire [`ZoomBegin`] and insert
/// [`ZoomAnimationMarker`](crate::components::ZoomAnimationMarker) at the
/// single point where conflict resolution has already completed.
#[derive(Clone, Reflect)]
pub struct ZoomContext {
    pub target_entity: Entity,
    pub margin:        f32,
    pub duration:      Duration,
    pub easing:        EaseFunction,
}

/// Identifies which event triggered an animation lifecycle.
///
/// Carried by [`AnimationBegin`], [`AnimationEnd`], [`AnimationCancelled`], and
/// [`AnimationRejected`] so observers know whether the animation originated from
/// [`PlayAnimation`], [`ZoomToFit`], or [`AnimateToFit`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Reflect)]
pub enum AnimationSource {
    /// Animation was triggered by [`PlayAnimation`].
    PlayAnimation,
    /// Animation was triggered by [`ZoomToFit`].
    ZoomToFit,
    /// Animation was triggered by [`AnimateToFit`].
    AnimateToFit,
}

/// `ZoomToFit` — frames a target entity in the camera view without changing the camera angle.
///
/// - `camera_entity` — the entity with a `PanOrbitCamera` component.
/// - `target` — the entity to frame; must have a `Mesh3d` (direct or on descendants).
/// - `margin` — total fraction of the screen to leave as space between the target's screen-space
///   bounding box and the screen edge, split equally across both sides of the constraining
///   dimension (e.g. `0.25` → ~12.5% each side).
/// - `duration` — see module-level docs on **Duration**.
/// - `easing` — see module-level docs on **Easing**.
///
/// Animated zooms route through [`PlayAnimation`], so the full event sequence is
/// `ZoomBegin` → `AnimationBegin` → `CameraMoveBegin` → `CameraMoveEnd` →
/// `AnimationEnd` → `ZoomEnd`. See the [module-level event ordering](self#event-ordering)
/// docs for interruption and conflict scenarios.
///
/// Insert the [`FitVisualization`](crate::FitVisualization) component on the camera entity
/// to see a debug visualization of the chosen `ZoomToFit` target. Remove it to disable.
/// The last chosen target is preserved on the camera so you can continue to see the
/// visualization after the zoom ends.
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

/// `ZoomCancelled` — emitted when a [`ZoomToFit`] animation is cancelled before
/// completion. The camera stays at its current position — no snap to final.
///
/// Cancellation happens in two scenarios:
/// - **User input** — the user physically moves the camera while
///   [`InputInterruptBehavior::Cancel`](crate::InputInterruptBehavior::Cancel) is active.
/// - **Animation conflict** — a new animation request arrives while
///   [`AnimationConflictPolicy::LastWins`](crate::AnimationConflictPolicy::LastWins) is active,
///   cancelling the in-flight zoom.
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
/// - `camera_moves` — accepts any `impl IntoIterator<Item = CameraMove>` (arrays, [`Vec`],
///   [`VecDeque`], etc.). Each [`CameraMove`] is either a `ToPosition` (world-space translation +
///   focus) or a `ToOrbit` (orbital parameters around a focus point), each with its own duration
///   and easing.
/// - `source` — the [`AnimationSource`] identifying the origin of this animation. Defaults to
///   [`AnimationSource::PlayAnimation`]; set to [`AnimationSource::AnimateToFit`] via the
///   `.source()` builder when the animation originates from [`AnimateToFit`].
/// - `zoom_context` — when `Some`, the animation originated from [`ZoomToFit`]. The
///   `on_play_animation` observer uses this to fire [`ZoomBegin`] and insert
///   [`ZoomAnimationMarker`](crate::components::ZoomAnimationMarker) after conflict resolution
///   passes. Source is implicitly [`AnimationSource::ZoomToFit`] when set.
///
/// ```rust,ignore
/// commands.trigger(PlayAnimation::new(camera, [move1, move2, move3]));
/// ```
///
/// Fires `AnimationBegin` → (`CameraMoveBegin` → `CameraMoveEnd`) × N → `AnimationEnd`.
/// See the [module-level event ordering](self#event-ordering) docs for interruption and
/// conflict scenarios.
#[derive(EntityEvent, Reflect)]
#[reflect(Event, FromReflect)]
pub struct PlayAnimation {
    #[event_target]
    pub camera_entity: Entity,
    pub camera_moves:  VecDeque<CameraMove>,
    pub source:        AnimationSource,
    pub zoom_context:  Option<ZoomContext>,
}

impl PlayAnimation {
    pub fn new(camera_entity: Entity, camera_moves: impl IntoIterator<Item = CameraMove>) -> Self {
        Self {
            camera_entity,
            camera_moves: camera_moves.into_iter().collect(),
            source: AnimationSource::PlayAnimation,
            zoom_context: None,
        }
    }

    pub fn source(mut self, source: AnimationSource) -> Self {
        self.source = source;
        self
    }

    pub fn zoom_context(mut self, ctx: ZoomContext) -> Self {
        self.zoom_context = Some(ctx);
        self.source = AnimationSource::ZoomToFit;
        self
    }
}

/// `AnimationBegin` — emitted when a `CameraMoveList` begins processing.
///
/// - `camera_entity` — the camera being animated.
/// - `source` — whether this animation originated from [`PlayAnimation`], [`ZoomToFit`], or
///   [`AnimateToFit`].
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
/// - `source` — whether this animation originated from [`PlayAnimation`], [`ZoomToFit`], or
///   [`AnimateToFit`].
#[derive(EntityEvent, Reflect)]
#[reflect(Event, FromReflect)]
pub struct AnimationEnd {
    #[event_target]
    pub camera_entity: Entity,
    pub source:        AnimationSource,
}

/// `AnimationCancelled` — emitted when a [`PlayAnimation`], [`ZoomToFit`], or [`AnimateToFit`] is
/// cancelled before completion. The camera stays at its current position — no snap to
/// final.
///
/// Cancellation happens in two scenarios:
/// - **User input** — the user physically moves the camera while
///   [`InputInterruptBehavior::Cancel`](crate::InputInterruptBehavior::Cancel) is active.
/// - **Animation conflict** — a new animation request arrives while
///   [`AnimationConflictPolicy::LastWins`](crate::AnimationConflictPolicy::LastWins) is active,
///   cancelling the in-flight (non-zoom) animation.
///
/// - `camera_entity` — the camera whose animation was cancelled.
/// - `source` — whether this animation originated from [`PlayAnimation`], [`ZoomToFit`], or
///   [`AnimateToFit`].
/// - `camera_move` — the [`CameraMove`] that was in progress when cancelled.
#[derive(EntityEvent, Reflect)]
#[reflect(Event, FromReflect)]
pub struct AnimationCancelled {
    #[event_target]
    pub camera_entity: Entity,
    pub source:        AnimationSource,
    pub camera_move:   CameraMove,
}

/// `AnimationRejected` — emitted when an incoming animation request is rejected because
/// [`AnimationConflictPolicy::FirstWins`](crate::AnimationConflictPolicy::FirstWins) is
/// active and an animation is already in-flight.
///
/// The in-flight animation continues uninterrupted.
///
/// - `camera_entity` — the camera that rejected the animation.
/// - `source` — the [`AnimationSource`] of the rejected request.
#[derive(EntityEvent, Reflect)]
#[reflect(Event, FromReflect)]
pub struct AnimationRejected {
    #[event_target]
    pub camera_entity: Entity,
    pub source:        AnimationSource,
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
/// - `target` — the entity to frame; must have a `Mesh3d` (direct or on descendants).
/// - `yaw` — final yaw in radians; updates `PanOrbitCamera::target_yaw`.
/// - `pitch` — final pitch in radians; updates `PanOrbitCamera::target_pitch`.
/// - `margin` — see [`ZoomToFit`] for details on how margin is applied.
/// - `duration` — see module-level docs on **Duration**.
/// - `easing` — see module-level docs on **Easing**.
///
/// Combines orientation change with zoom-to-fit in a single smooth animation.
/// Unlike [`ZoomToFit`], this does not fire [`ZoomBegin`]/[`ZoomEnd`] — only the
/// standard animation events with `source: AnimationSource::AnimateToFit`.
/// See the [module-level event ordering](self#event-ordering) docs for the full
/// sequence and interruption/conflict scenarios.
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
/// - `target` — the entity whose bounds to visualize; must have a `Mesh3d` (direct or on
///   descendants).
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

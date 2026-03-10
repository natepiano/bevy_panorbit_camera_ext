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
//! ## User input interruption ([`CameraInputInterruptBehavior`](crate::CameraInputInterruptBehavior))
//!
//! When the user physically moves the camera during an animation:
//!
//! - **`Ignore`** (default) — temporarily disables camera input and continues animating:
//!
//!   ```text
//!   … (no interrupt lifecycle event)
//!   ```
//!
//! - **`Cancel`** — stops where it is:
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
//! | Event                    | `camera` | `target` | `margin` | `duration` | `easing` | `source` | `camera_move` |
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
    pub target:   Entity,
    pub margin:   f32,
    pub duration: Duration,
    pub easing:   EaseFunction,
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
    /// Animation was triggered by [`LookAt`].
    LookAt,
    /// Animation was triggered by [`LookAtAndZoomToFit`].
    LookAtAndZoomToFit,
}

/// `ZoomToFit` — frames a target entity in the camera view without changing the
/// camera's viewing angle.
///
/// The camera's yaw and pitch stay fixed. Only the focus and radius change so
/// that the target fills the viewport with the requested margin. Because the
/// viewing angle is preserved, the camera *translates* to a new position rather
/// than rotating — if the target is off to the side, the view slides over to it.
///
/// # See also
///
/// - [`LookAt`] — keeps the camera in place and *rotates* to face the target (no framing / radius
///   adjustment).
/// - [`LookAtAndZoomToFit`] — *rotates* to face the target and adjusts radius to frame it. Use this
///   when you want the camera to turn toward the target instead of sliding.
/// - [`AnimateToFit`] — frames the target from a caller-specified viewing angle.
///
/// # Fields
///
/// - `camera` — the entity with a `PanOrbitCamera` component.
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
    pub camera:   Entity,
    pub target:   Entity,
    pub margin:   f32,
    pub duration: Duration,
    pub easing:   EaseFunction,
}

impl ZoomToFit {
    pub const fn new(camera: Entity, target: Entity) -> Self {
        Self {
            camera,
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
/// - `camera` — the camera that is zooming.
/// - `target` — the entity being framed.
/// - `margin` — the margin value from the triggering [`ZoomToFit`].
/// - `duration` — the duration from the triggering [`ZoomToFit`].
/// - `easing` — the easing curve from the triggering [`ZoomToFit`].
#[derive(EntityEvent, Reflect)]
#[reflect(Event, FromReflect)]
pub struct ZoomBegin {
    #[event_target]
    pub camera:   Entity,
    pub target:   Entity,
    pub margin:   f32,
    pub duration: Duration,
    pub easing:   EaseFunction,
}

/// `ZoomEnd` — emitted when a [`ZoomToFit`] operation completes (both animated and instant).
///
/// - `camera` — the camera that finished zooming.
/// - `target` — the entity that was framed.
/// - `margin` — the margin value from the triggering [`ZoomToFit`].
/// - `duration` — the duration from the triggering [`ZoomToFit`].
/// - `easing` — the easing curve from the triggering [`ZoomToFit`].
#[derive(EntityEvent, Reflect)]
#[reflect(Event, FromReflect)]
pub struct ZoomEnd {
    #[event_target]
    pub camera:   Entity,
    pub target:   Entity,
    pub margin:   f32,
    pub duration: Duration,
    pub easing:   EaseFunction,
}

/// `ZoomCancelled` — emitted when a [`ZoomToFit`] animation is cancelled before
/// completion. The camera stays at its current position — no snap to final.
///
/// Cancellation happens in two scenarios:
/// - **User input** — the user physically moves the camera while
///   [`CameraInputInterruptBehavior::Cancel`](crate::CameraInputInterruptBehavior::Cancel) is
///   active.
/// - **Animation conflict** — a new animation request arrives while
///   [`AnimationConflictPolicy::LastWins`](crate::AnimationConflictPolicy::LastWins) is active,
///   cancelling the in-flight zoom.
///
/// When user input behavior is `Ignore` or `Complete`, user input does not emit
/// `ZoomCancelled`.
///
/// - `camera` — the camera whose zoom was cancelled.
/// - `target` — the entity that was being framed.
/// - `margin` — the margin value from the triggering [`ZoomToFit`].
/// - `duration` — the duration from the triggering [`ZoomToFit`].
/// - `easing` — the easing curve from the triggering [`ZoomToFit`].
#[derive(EntityEvent, Reflect)]
#[reflect(Event, FromReflect)]
pub struct ZoomCancelled {
    #[event_target]
    pub camera:   Entity,
    pub target:   Entity,
    pub margin:   f32,
    pub duration: Duration,
    pub easing:   EaseFunction,
}

/// `PlayAnimation` — plays a queued sequence of [`CameraMove`] steps.
///
/// - `camera` — the entity with a `PanOrbitCamera` component.
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
    pub camera:       Entity,
    pub camera_moves: VecDeque<CameraMove>,
    pub source:       AnimationSource,
    pub zoom_context: Option<ZoomContext>,
}

impl PlayAnimation {
    pub fn new(camera: Entity, camera_moves: impl IntoIterator<Item = CameraMove>) -> Self {
        Self {
            camera,
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
/// - `camera` — the camera being animated.
/// - `source` — whether this animation originated from [`PlayAnimation`], [`ZoomToFit`], or
///   [`AnimateToFit`].
#[derive(EntityEvent, Reflect)]
#[reflect(Event, FromReflect)]
pub struct AnimationBegin {
    #[event_target]
    pub camera: Entity,
    pub source: AnimationSource,
}

/// `AnimationEnd` — emitted when a `CameraMoveList` finishes all its queued moves.
///
/// - `camera` — the camera that finished animating.
/// - `source` — whether this animation originated from [`PlayAnimation`], [`ZoomToFit`], or
///   [`AnimateToFit`].
#[derive(EntityEvent, Reflect)]
#[reflect(Event, FromReflect)]
pub struct AnimationEnd {
    #[event_target]
    pub camera: Entity,
    pub source: AnimationSource,
}

/// `AnimationCancelled` — emitted when a [`PlayAnimation`], [`ZoomToFit`], or [`AnimateToFit`] is
/// cancelled before completion. The camera stays at its current position — no snap to
/// final.
///
/// Cancellation happens in two scenarios:
/// - **User input** — the user physically moves the camera while
///   [`CameraInputInterruptBehavior::Cancel`](crate::CameraInputInterruptBehavior::Cancel) is
///   active.
/// - **Animation conflict** — a new animation request arrives while
///   [`AnimationConflictPolicy::LastWins`](crate::AnimationConflictPolicy::LastWins) is active,
///   cancelling the in-flight (non-zoom) animation.
///
/// When user input behavior is `Ignore` or `Complete`, user input does not emit
/// `AnimationCancelled`.
///
/// - `camera` — the camera whose animation was cancelled.
/// - `source` — whether this animation originated from [`PlayAnimation`], [`ZoomToFit`], or
///   [`AnimateToFit`].
/// - `camera_move` — the [`CameraMove`] that was in progress when cancelled.
#[derive(EntityEvent, Reflect)]
#[reflect(Event, FromReflect)]
pub struct AnimationCancelled {
    #[event_target]
    pub camera:      Entity,
    pub source:      AnimationSource,
    pub camera_move: CameraMove,
}

/// `AnimationRejected` — emitted when an incoming animation request is rejected because
/// [`AnimationConflictPolicy::FirstWins`](crate::AnimationConflictPolicy::FirstWins) is
/// active and an animation is already in-flight.
///
/// The in-flight animation continues uninterrupted.
///
/// - `camera` — the camera that rejected the animation.
/// - `source` — the [`AnimationSource`] of the rejected request.
#[derive(EntityEvent, Reflect)]
#[reflect(Event, FromReflect)]
pub struct AnimationRejected {
    #[event_target]
    pub camera: Entity,
    pub source: AnimationSource,
}

/// `CameraMoveBegin` — emitted when an individual [`CameraMove`] begins.
///
/// - `camera` — the camera being animated.
/// - `camera_move` — the [`CameraMove`] step that is starting.
#[derive(EntityEvent, Reflect)]
#[reflect(Event, FromReflect)]
pub struct CameraMoveBegin {
    #[event_target]
    pub camera:      Entity,
    pub camera_move: CameraMove,
}

/// `CameraMoveEnd` — emitted when an individual [`CameraMove`] completes.
///
/// - `camera` — the camera that finished this move step.
/// - `camera_move` — the [`CameraMove`] step that completed.
#[derive(EntityEvent, Reflect)]
#[reflect(Event, FromReflect)]
pub struct CameraMoveEnd {
    #[event_target]
    pub camera:      Entity,
    pub camera_move: CameraMove,
}

/// `AnimateToFit` — animates the camera to a caller-specified orientation while
/// framing a target entity in view.
///
/// You specify the exact yaw and pitch the camera should end up at, and the
/// system computes the radius needed to frame the target from that angle.
///
/// # See also
///
/// - [`LookAtAndZoomToFit`] — like `AnimateToFit` but the yaw/pitch are automatically back-solved
///   from the camera's current position, so you don't specify them. Use this for a "turn and frame"
///   operation.
/// - [`ZoomToFit`] — keeps the current viewing angle, only adjusts focus and radius.
/// - [`LookAt`] — rotates to face the target without framing.
///
/// # Fields
///
/// - `camera` — the entity with a `PanOrbitCamera` component.
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
    pub camera:   Entity,
    pub target:   Entity,
    pub yaw:      f32,
    pub pitch:    f32,
    pub margin:   f32,
    pub duration: Duration,
    pub easing:   EaseFunction,
}

impl AnimateToFit {
    pub const fn new(camera: Entity, target: Entity) -> Self {
        Self {
            camera,
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

/// `LookAt` — rotates the camera in place to face a target entity.
///
/// The camera stays at its current world position and turns to look at the target,
/// like a person turning their head. The orbit pivot re-anchors to the target
/// entity's [`GlobalTransform`] translation, and yaw/pitch/radius are back-solved
/// so the camera does not move — only its orientation changes.
///
/// This differs from [`ZoomToFit`], which keeps the camera's viewing angle fixed
/// and slides the focus to the target (the camera *moves* but doesn't *rotate*).
/// `LookAt` does the opposite: the camera *rotates* but doesn't *move*.
///
/// Only requires the target to have a [`GlobalTransform`] — no mesh needed.
///
/// # See also
///
/// - [`LookAtAndZoomToFit`] — same rotation, but also adjusts radius to frame the target in view.
/// - [`ZoomToFit`] — keeps the viewing angle, moves the camera to frame the target.
/// - [`AnimateToFit`] — like [`LookAtAndZoomToFit`] but with caller-specified yaw/pitch instead of
///   back-solving from the current position.
///
/// # Fields
///
/// - `camera` — the entity with a `PanOrbitCamera` component.
/// - `target` — the entity to look at; must have a [`GlobalTransform`].
/// - `duration` — see module-level docs on **Duration**.
/// - `easing` — see module-level docs on **Easing**.
///
/// Animated paths route through [`PlayAnimation`] using [`CameraMove::ToPosition`],
/// so the full event sequence is `AnimationBegin` → `CameraMoveBegin` →
/// `CameraMoveEnd` → `AnimationEnd` with `source: AnimationSource::LookAt`.
#[derive(EntityEvent, Reflect)]
#[reflect(Event, FromReflect)]
pub struct LookAt {
    #[event_target]
    pub camera:   Entity,
    pub target:   Entity,
    pub duration: Duration,
    pub easing:   EaseFunction,
}

impl LookAt {
    pub const fn new(camera: Entity, target: Entity) -> Self {
        Self {
            camera,
            target,
            duration: Duration::ZERO,
            easing: EaseFunction::CubicOut,
        }
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

/// `LookAtAndZoomToFit` — rotates the camera to face a target entity and adjusts
/// the radius to frame it in view, all in one fluid motion.
///
/// Combines [`LookAt`] (turn in place) with [`ZoomToFit`] (frame the target).
/// The yaw and pitch are back-solved from the camera's current world position
/// relative to the target's bounds center — you don't specify them.
///
/// # How it differs from [`ZoomToFit`]
///
/// [`ZoomToFit`] preserves the camera's current viewing angle (yaw/pitch) and
/// slides the orbit focus to the target. If the target is off to the side, the
/// camera *translates* to keep the same angle — it doesn't turn to face it.
///
/// `LookAtAndZoomToFit` instead *rotates* the camera toward the target from its
/// current position, then adjusts the radius to frame it. The difference is most
/// visible when the target is near the edge of the viewport: `ZoomToFit` slides
/// the view sideways while `LookAtAndZoomToFit` turns to face it head-on.
///
/// # How it differs from [`AnimateToFit`]
///
/// [`AnimateToFit`] requires you to specify the final yaw and pitch explicitly.
/// `LookAtAndZoomToFit` computes them automatically from the camera's current
/// world position, making it a "turn and frame" operation with no angle to specify.
///
/// # See also
///
/// - [`LookAt`] — same rotation without the zoom-to-fit radius adjustment.
/// - [`ZoomToFit`] — keeps the viewing angle, moves the camera to frame the target.
/// - [`AnimateToFit`] — frames the target from a caller-specified viewing angle.
///
/// # Fields
///
/// - `camera` — the entity with a `PanOrbitCamera` component.
/// - `target` — the entity to frame; must have a `Mesh3d` (direct or on descendants).
/// - `margin` — see [`ZoomToFit`] for details on how margin is applied.
/// - `duration` — see module-level docs on **Duration**.
/// - `easing` — see module-level docs on **Easing**.
///
/// Animated paths route through [`PlayAnimation`] using [`CameraMove::ToOrbit`],
/// so the full event sequence is `AnimationBegin` → `CameraMoveBegin` →
/// `CameraMoveEnd` → `AnimationEnd` with `source: AnimationSource::LookAtAndZoomToFit`.
#[derive(EntityEvent, Reflect)]
#[reflect(Event, FromReflect)]
pub struct LookAtAndZoomToFit {
    #[event_target]
    pub camera:   Entity,
    pub target:   Entity,
    pub margin:   f32,
    pub duration: Duration,
    pub easing:   EaseFunction,
}

impl LookAtAndZoomToFit {
    pub const fn new(camera: Entity, target: Entity) -> Self {
        Self {
            camera,
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

/// `SetFitTarget` — sets the visualization target without triggering a zoom. Allows you
/// to inspect bounds before triggering [`ZoomToFit`].
///
/// - `camera` — the entity with a `PanOrbitCamera` component.
/// - `target` — the entity whose bounds to visualize; must have a `Mesh3d` (direct or on
///   descendants).
#[derive(EntityEvent, Reflect)]
#[reflect(Event, FromReflect)]
pub struct SetFitTarget {
    #[event_target]
    pub camera: Entity,
    pub target: Entity,
}

impl SetFitTarget {
    pub const fn new(camera: Entity, target: Entity) -> Self { Self { camera, target } }
}

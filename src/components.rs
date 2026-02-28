//! Components used by the camera extension system.

use bevy::prelude::*;

use crate::events::AnimationSource;
use crate::events::ZoomContext;

/// Controls what happens when **user input** (orbit, pan, zoom) occurs during an
/// in-flight animation.
///
/// This is a required component on [`CameraMoveList`](crate::CameraMoveList) — if not
/// explicitly inserted, it defaults to [`Cancel`](InputInterruptBehavior::Cancel).
///
/// This component is orthogonal to [`AnimationConflictPolicy`] — `InputInterruptBehavior`
/// handles physical camera input during an animation, while `AnimationConflictPolicy`
/// handles programmatic animation requests that arrive while one is already playing.
///
/// - [`Cancel`](InputInterruptBehavior::Cancel) — stop the camera where it is and fire `*Cancelled`
///   events
/// - [`Complete`](InputInterruptBehavior::Complete) — jump to the final position of the entire
///   queue and fire normal `*End` events
#[derive(Component, Reflect, Default, Clone, Copy, Debug, PartialEq, Eq)]
#[reflect(Component, Default)]
pub enum InputInterruptBehavior {
    /// Stop the camera at its current position. Fires `AnimationCancelled` or `ZoomCancelled`.
    #[default]
    Cancel,
    /// Jump to the final queued position. Fires `AnimationEnd` or `ZoomEnd`.
    Complete,
}

/// Controls what happens when a **new animation request** arrives while one is already
/// in-flight.
///
/// Insert this component on a camera entity to configure conflict resolution. If not
/// present, defaults to [`LastWins`](AnimationConflictPolicy::LastWins).
///
/// This component is orthogonal to [`InputInterruptBehavior`] — `AnimationConflictPolicy`
/// handles programmatic animation requests (e.g. [`ZoomToFit`](crate::ZoomToFit),
/// [`PlayAnimation`](crate::PlayAnimation)) that conflict with an active animation, while
/// `InputInterruptBehavior` handles physical user input interrupting an animation.
///
/// - [`LastWins`](AnimationConflictPolicy::LastWins) — cancel the current animation and start the
///   new one. Fires appropriate `*Cancelled` events for the interrupted operation.
/// - [`FirstWins`](AnimationConflictPolicy::FirstWins) — reject the incoming request. Fires
///   [`AnimationRejected`](crate::AnimationRejected).
#[derive(Component, Reflect, Default, Clone, Copy, Debug, PartialEq, Eq)]
#[reflect(Component, Default)]
pub enum AnimationConflictPolicy {
    #[default]
    LastWins,
    FirstWins,
}

/// Marks the entity that the camera is currently fitted to.
/// Persists after fit completes to enable persistent visualization.
#[derive(Component, Reflect, Debug)]
#[reflect(Component)]
pub struct CurrentFitTarget(pub Entity);

/// Marker component that tracks a zoom-to-fit operation routed through the animation system.
/// When `AnimationEnd` fires on an entity with this marker, `ZoomEnd` is triggered and the
/// marker is removed. Wraps the [`ZoomContext`] that originated the zoom.
#[derive(Component, Clone)]
pub struct ZoomAnimationMarker(pub ZoomContext);

/// Marker component that tracks whether an animation was triggered by
/// [`PlayAnimation`](crate::PlayAnimation), [`ZoomToFit`](crate::ZoomToFit), or
/// [`AnimateToFit`](crate::AnimateToFit). Inserted alongside
/// [`CameraMoveList`](crate::CameraMoveList) and removed when the animation ends or is cancelled.
#[derive(Component)]
pub struct AnimationSourceMarker(pub AnimationSource);

/// Component that stores camera smoothness values during animations.
///
/// When camera animations are active (via `CameraMoveList`), the smoothness values are
/// temporarily set to 0.0 for instant movement, and the original values are stored here.
/// When the animation completes and the component is removed, the smoothness is
/// automatically restored via an observer.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct SmoothnessStash {
    pub zoom:  f32,
    pub pan:   f32,
    pub orbit: f32,
}

/// Enables fit target debug visualization on a camera entity.
///
/// Insert this component to enable visualization, remove it to disable.
/// The presence or absence of the component is the toggle — no boolean field needed.
#[cfg(feature = "visualization")]
#[derive(Component, Reflect, Default)]
#[reflect(Component, Default)]
pub struct FitVisualization;

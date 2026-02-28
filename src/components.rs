//! Components used by the camera extension system.

use std::time::Duration;

use bevy::math::curve::easing::EaseFunction;
use bevy::prelude::*;

use crate::events::AnimationSource;

/// Configures what happens when external camera input interrupts an animation.
///
/// This is a required component on [`CameraMoveList`](crate::CameraMoveList) — if not
/// explicitly inserted, it defaults to [`Cancel`](InterruptBehavior::Cancel).
///
/// - [`Cancel`](InterruptBehavior::Cancel) — stop the camera where it is and fire `*Cancelled`
///   events
/// - [`Complete`](InterruptBehavior::Complete) — jump to the final position of the entire queue and
///   fire normal `*End` events
#[derive(Component, Reflect, Default, Clone, Copy, Debug, PartialEq, Eq)]
#[reflect(Component, Default)]
pub enum InterruptBehavior {
    /// Stop the camera at its current position. Fires `AnimationCancelled` or `ZoomCancelled`.
    #[default]
    Cancel,
    /// Jump to the final queued position. Fires `AnimationEnd` or `ZoomEnd`.
    Complete,
}

/// Marks the entity that the camera is currently fitted to.
/// Persists after fit completes to enable persistent visualization.
#[derive(Component, Reflect, Debug)]
#[reflect(Component)]
pub struct CurrentFitTarget(pub Entity);

/// Marker component that tracks a zoom-to-fit operation routed through the animation system.
/// When `AnimationEnd` fires on an entity with this marker, `ZoomEnd` is triggered and the
/// marker is removed.
#[derive(Component)]
pub struct ZoomAnimationMarker {
    pub target_entity: Entity,
    pub margin:        f32,
    pub duration:      Duration,
    pub easing:        EaseFunction,
}

/// Marker component that tracks whether an animation was triggered by
/// [`PlayAnimation`](crate::PlayAnimation) or [`AnimateToFit`](crate::AnimateToFit).
/// Inserted alongside [`CameraMoveList`](crate::CameraMoveList) and removed when the
/// animation ends or is cancelled.
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

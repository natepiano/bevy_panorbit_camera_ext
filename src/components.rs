//! Components used by the camera extension system.

use bevy::math::curve::easing::EaseFunction;
use bevy::prelude::*;

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
    pub duration_ms:   f32,
    pub easing:        EaseFunction,
}

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

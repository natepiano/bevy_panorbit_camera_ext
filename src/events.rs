//! Lifecycle events for camera animations and zoom operations.

use bevy::math::curve::easing::EaseFunction;
use bevy::prelude::*;

// ============================================================================
// Animation lifecycle (queue-level)
// ============================================================================

/// Fired when a `CameraMoveList` begins processing.
#[derive(EntityEvent, Reflect)]
#[reflect(Event, FromReflect)]
pub struct AnimationStart {
    #[event_target]
    pub camera_entity: Entity,
}

/// Fired when a `CameraMoveList` finishes all its queued moves.
#[derive(EntityEvent, Reflect)]
#[reflect(Event, FromReflect)]
pub struct AnimationComplete {
    #[event_target]
    pub camera_entity: Entity,
}

// ============================================================================
// Camera move lifecycle (per-move)
// ============================================================================

/// Fired when an individual `CameraMove` begins.
#[derive(EntityEvent, Reflect)]
#[reflect(Event, FromReflect)]
pub struct CameraMoveStart {
    #[event_target]
    pub camera_entity:      Entity,
    pub target_translation: Vec3,
    pub target_focus:       Vec3,
    pub duration_ms:        f32,
    pub easing:             EaseFunction,
}

/// Fired when an individual `CameraMove` completes.
#[derive(EntityEvent, Reflect)]
#[reflect(Event, FromReflect)]
pub struct CameraMoveComplete {
    #[event_target]
    pub camera_entity:      Entity,
    pub target_translation: Vec3,
    pub target_focus:       Vec3,
    pub duration_ms:        f32,
    pub easing:             EaseFunction,
}

// ============================================================================
// Zoom lifecycle
// ============================================================================

/// Fired when a `ZoomToFit` operation begins.
#[derive(EntityEvent, Reflect)]
#[reflect(Event, FromReflect)]
pub struct ZoomStart {
    #[event_target]
    pub camera_entity: Entity,
}

/// Fired when a `ZoomToFit` operation completes (both animated and instant).
#[derive(EntityEvent, Reflect)]
#[reflect(Event, FromReflect)]
pub struct ZoomComplete {
    #[event_target]
    pub camera_entity: Entity,
}

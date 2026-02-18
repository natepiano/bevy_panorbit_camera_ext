//! Events for camera animations and zoom operations.

use std::collections::VecDeque;

use bevy::math::curve::easing::EaseFunction;
use bevy::prelude::*;

use crate::animation::CameraMove;

// ============================================================================
// Animation lifecycle (queue-level)
// ============================================================================

/// Fired when a `CameraMoveList` begins processing.
#[derive(EntityEvent, Reflect)]
#[reflect(Event, FromReflect)]
pub struct AnimationBegin {
    #[event_target]
    pub camera_entity: Entity,
}

/// Fired when a `CameraMoveList` finishes all its queued moves.
#[derive(EntityEvent, Reflect)]
#[reflect(Event, FromReflect)]
pub struct AnimationEnd {
    #[event_target]
    pub camera_entity: Entity,
}

// ============================================================================
// Camera move lifecycle (per-move)
// ============================================================================

/// Fired when an individual `CameraMove` begins.
#[derive(EntityEvent, Reflect)]
#[reflect(Event, FromReflect)]
pub struct CameraMoveBegin {
    #[event_target]
    pub camera_entity: Entity,
    pub camera_move:   CameraMove,
}

/// Fired when an individual `CameraMove` completes.
#[derive(EntityEvent, Reflect)]
#[reflect(Event, FromReflect)]
pub struct CameraMoveEnd {
    #[event_target]
    pub camera_entity: Entity,
    pub camera_move:   CameraMove,
}

// ============================================================================
// Zoom lifecycle
// ============================================================================

/// Fired when a `ZoomToFit` operation begins.
#[derive(EntityEvent, Reflect)]
#[reflect(Event, FromReflect)]
pub struct ZoomBegin {
    #[event_target]
    pub camera_entity: Entity,
    pub target_entity: Entity,
    pub margin:        f32,
    pub duration_ms:   f32,
    pub easing:        EaseFunction,
}

/// Fired when a `ZoomToFit` operation completes (both animated and instant).
#[derive(EntityEvent, Reflect)]
#[reflect(Event, FromReflect)]
pub struct ZoomEnd {
    #[event_target]
    pub camera_entity: Entity,
    pub target_entity: Entity,
    pub margin:        f32,
    pub duration_ms:   f32,
    pub easing:        EaseFunction,
}

// ============================================================================
// Command events
// ============================================================================

/// Event to frame a target entity in the camera view.
/// Use `duration_ms > 0.0` for a smooth animated zoom, or `0.0` for an instant snap.
///
/// The `margin` is the **total** fraction of screen reserved for padding — it is split
/// equally across both sides of the constraining dimension. For example, a margin of
/// `0.25` leaves ~12.5% padding on each side (25% total). The non-constraining
/// dimension will have additional padding to preserve the target's aspect ratio.
#[derive(EntityEvent, Reflect)]
#[reflect(Event, FromReflect)]
pub struct ZoomToFit {
    #[event_target]
    pub camera_entity: Entity,
    pub target:        Entity,
    pub margin:        f32,
    pub duration_ms:   f32,
    pub easing:        EaseFunction,
}

impl ZoomToFit {
    pub const fn new(
        camera_entity: Entity,
        target: Entity,
        margin: f32,
        duration_ms: f32,
        easing: EaseFunction,
    ) -> Self {
        Self {
            camera_entity,
            target,
            margin,
            duration_ms,
            easing,
        }
    }
}

/// Event to play a queued camera animation
#[derive(EntityEvent, Reflect)]
#[reflect(Event, FromReflect)]
pub struct PlayAnimation {
    #[event_target]
    pub camera_entity: Entity,
    pub moves:         VecDeque<CameraMove>,
}

impl PlayAnimation {
    pub const fn new(camera_entity: Entity, moves: VecDeque<CameraMove>) -> Self {
        Self {
            camera_entity,
            moves,
        }
    }
}

/// Event to set the target entity for fit visualization debugging
/// Allows you to set the target, and then turn on visualization debugging before
/// invoking a ZoomToFit event.
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

/// Event to animate the camera to a specific orientation and fit a target entity in view.
/// Combines orientation change with zoom-to-fit in a single smooth animation.
///
/// See [`ZoomToFit`] for details on how `margin` is applied.
#[derive(EntityEvent, Reflect)]
#[reflect(Event, FromReflect)]
pub struct AnimateToFit {
    #[event_target]
    pub camera_entity: Entity,
    pub target:        Entity,
    pub yaw:           f32,
    pub pitch:         f32,
    pub margin:        f32,
    pub duration_ms:   f32,
    pub easing:        EaseFunction,
}

impl AnimateToFit {
    pub const fn new(
        camera_entity: Entity,
        target: Entity,
        yaw: f32,
        pitch: f32,
        margin: f32,
        duration_ms: f32,
        easing: EaseFunction,
    ) -> Self {
        Self {
            camera_entity,
            target,
            yaw,
            pitch,
            margin,
            duration_ms,
            easing,
        }
    }
}

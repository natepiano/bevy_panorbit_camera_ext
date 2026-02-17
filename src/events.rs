//! Completion events for camera animations and zoom operations.

use bevy::prelude::*;

/// Fired when a `CameraMoveList` finishes all its queued moves.
#[derive(EntityEvent, Reflect)]
#[reflect(Event, FromReflect)]
pub struct AnimationComplete {
    /// The camera entity whose animation completed.
    #[event_target]
    pub(crate) camera_entity: Entity,
}

/// Fired when a `ZoomToFit` operation completes (both animated and instant).
#[derive(EntityEvent, Reflect)]
#[reflect(Event, FromReflect)]
pub struct ZoomComplete {
    /// The camera entity whose zoom completed.
    #[event_target]
    pub(crate) camera_entity: Entity,
}

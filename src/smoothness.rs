use bevy::prelude::*;
use bevy_panorbit_camera::PanOrbitCamera;

use crate::animation::CameraMoveList;
use crate::zoom::ZoomToFitComponent;

/// Component that stores camera smoothness values during animations.
///
/// When camera animations are active (via `CameraMoveList` or `ZoomToFitComponent`),
/// the smoothness values are temporarily set to 0.0 for instant movement, and the original
/// values are stored here. When the animation completes and the component is removed,
/// the smoothness is automatically restored via an observer.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct SmoothnessStash {
    pub zoom:  f32,
    pub pan:   f32,
    pub orbit: f32,
}

/// Observer that restores smoothness when CameraMoveList is removed
pub fn restore_smoothness_on_move_complete(
    remove: On<Remove, CameraMoveList>,
    mut commands: Commands,
    mut query: Query<(&SmoothnessStash, &mut PanOrbitCamera)>,
) {
    let entity = remove.entity;

    let Ok((stash, mut camera)) = query.get_mut(entity) else {
        return;
    };

    camera.zoom_smoothness = stash.zoom;
    camera.pan_smoothness = stash.pan;
    camera.orbit_smoothness = stash.orbit;

    commands.entity(entity).remove::<SmoothnessStash>();
}

/// Observer that restores smoothness when ZoomToFitComponent is removed
pub fn restore_smoothness_on_zoom_complete(
    remove: On<Remove, ZoomToFitComponent>,
    mut commands: Commands,
    mut query: Query<(&SmoothnessStash, &mut PanOrbitCamera)>,
) {
    let entity = remove.entity;

    let Ok((stash, mut camera)) = query.get_mut(entity) else {
        return;
    };

    camera.zoom_smoothness = stash.zoom;
    camera.pan_smoothness = stash.pan;
    camera.orbit_smoothness = stash.orbit;

    commands.entity(entity).remove::<SmoothnessStash>();
}

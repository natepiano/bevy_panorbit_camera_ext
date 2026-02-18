//! Observers that wire events to camera behavior.

use std::collections::VecDeque;

use bevy::prelude::*;
use bevy_panorbit_camera::PanOrbitCamera;

use crate::animation::CameraMove;
use crate::animation::CameraMoveList;
use crate::components::CurrentFitTarget;
use crate::components::SmoothnessStash;
use crate::components::ZoomAnimationMarker;
use crate::events::AnimateToFit;
use crate::events::AnimationBegin;
use crate::events::PlayAnimation;
use crate::events::SetFitTarget;
use crate::events::ZoomBegin;
use crate::events::ZoomEnd;
use crate::events::ZoomToFit;
use crate::fit::calculate_fit;
use crate::support::extract_mesh_vertices;

/// Observer for `ZoomToFit` event - frames a target entity in the camera view.
/// When `duration_ms > 0.0`, animates smoothly over that duration.
/// When `duration_ms <= 0.0`, snaps instantly.
/// Requires target entity to have a `Mesh3d` (direct or on descendants).
pub fn on_zoom_to_fit(
    zoom: On<ZoomToFit>,
    mut commands: Commands,
    mut camera_query: Query<(&mut PanOrbitCamera, &Projection, &Camera)>,
    mesh_query: Query<&Mesh3d>,
    children_query: Query<&Children>,
    global_transform_query: Query<&GlobalTransform>,
    meshes: Res<Assets<Mesh>>,
) {
    let camera_entity = zoom.camera_entity;
    let target_entity = zoom.target;
    let margin = zoom.margin;
    let duration_ms = zoom.duration_ms;
    let easing = zoom.easing;

    let Ok((mut camera, projection, cam)) = camera_query.get_mut(camera_entity) else {
        return;
    };

    commands.trigger(ZoomBegin {
        camera_entity,
        target_entity,
        margin,
        duration_ms,
        easing,
    });

    info!(
        "ZoomToFit: yaw={:.3} pitch={:.3} current_focus={:.1?} current_radius={:.1} duration_ms={duration_ms:.0}",
        camera.target_yaw, camera.target_pitch, camera.target_focus, camera.target_radius
    );

    let Some((vertices, geometric_center)) = extract_mesh_vertices(
        target_entity,
        &children_query,
        &mesh_query,
        &global_transform_query,
        &meshes,
    ) else {
        warn!("ZoomToFit: Failed to extract mesh vertices for entity {target_entity:?}");
        return;
    };

    let Some((target_radius, target_focus)) = calculate_fit(
        &vertices,
        geometric_center,
        camera.target_yaw,
        camera.target_pitch,
        margin,
        projection,
        cam,
    ) else {
        warn!("ZoomToFit: Failed to calculate target radius for entity {target_entity:?}");
        return;
    };

    if duration_ms > 0.0 {
        // Animated path: use `ToOrbit` to pass orbital params directly, avoiding
        // gimbal lock from atan2 decomposition at extreme pitch angles.
        let moves = VecDeque::from([CameraMove::ToOrbit {
            focus: target_focus,
            yaw: camera.target_yaw,
            pitch: camera.target_pitch,
            radius: target_radius,
            duration_ms,
            easing,
        }]);

        // Mark this as a zoom operation so AnimationEnd fires ZoomEnd
        commands.entity(camera_entity).insert(ZoomAnimationMarker {
            target_entity,
            margin,
            duration_ms,
            easing,
        });

        commands.trigger(PlayAnimation::new(camera_entity, moves));
    } else {
        // Instant path: snap directly to target
        camera.target_focus = target_focus;
        camera.target_radius = target_radius;
        camera.force_update = true;
        commands.trigger(ZoomEnd {
            camera_entity,
            target_entity,
            margin,
            duration_ms,
            easing,
        });
    }

    // Mark current fit target for visualization
    commands
        .entity(camera_entity)
        .insert(CurrentFitTarget(target_entity));
}

/// Observer for `PlayAnimation` event - initiates camera animation sequence
pub fn on_play_animation(
    start: On<PlayAnimation>,
    mut commands: Commands,
    mut camera_query: Query<&mut PanOrbitCamera>,
    marker_query: Query<(), With<ZoomAnimationMarker>>,
) {
    let entity = start.camera_entity;

    let Ok(mut camera) = camera_query.get_mut(entity) else {
        return;
    };

    // Only fire `AnimationBegin` for user-initiated animations, not internal zoom animations
    if marker_query.get(entity).is_err() {
        commands.trigger(AnimationBegin {
            camera_entity: entity,
        });
    }

    // Stash and disable smoothness for precise animation control
    let stash = SmoothnessStash {
        zoom:  camera.zoom_smoothness,
        pan:   camera.pan_smoothness,
        orbit: camera.orbit_smoothness,
    };
    camera.zoom_smoothness = 0.0;
    camera.pan_smoothness = 0.0;
    camera.orbit_smoothness = 0.0;
    commands.entity(entity).insert(stash);

    // Add the animation component
    commands
        .entity(entity)
        .insert(CameraMoveList::new(start.moves.clone()));
}

/// Observer for `SetFitTarget` event - sets the target entity for fit visualization
pub fn on_set_fit_target(set_target: On<SetFitTarget>, mut commands: Commands) {
    commands
        .entity(set_target.camera_entity)
        .insert(CurrentFitTarget(set_target.target));
}

/// Observer for `AnimateToFit` event - animates the camera to a specific orientation
/// while fitting a target entity in view.
pub fn on_animate_to_fit(
    event: On<AnimateToFit>,
    mut commands: Commands,
    camera_query: Query<(&PanOrbitCamera, &Projection, &Camera)>,
    mesh_query: Query<&Mesh3d>,
    children_query: Query<&Children>,
    global_transform_query: Query<&GlobalTransform>,
    meshes: Res<Assets<Mesh>>,
) {
    let camera_entity = event.camera_entity;
    let target_entity = event.target;
    let yaw = event.yaw;
    let pitch = event.pitch;
    let margin = event.margin;
    let duration_ms = event.duration_ms;
    let easing = event.easing;

    let Ok((_, projection, cam)) = camera_query.get(camera_entity) else {
        return;
    };

    let Some((vertices, geometric_center)) = extract_mesh_vertices(
        target_entity,
        &children_query,
        &mesh_query,
        &global_transform_query,
        &meshes,
    ) else {
        warn!("AnimateToFit: Failed to extract mesh vertices for entity {target_entity:?}");
        return;
    };

    let Some((target_radius, target_focus)) = calculate_fit(
        &vertices,
        geometric_center,
        yaw,
        pitch,
        margin,
        projection,
        cam,
    ) else {
        warn!("AnimateToFit: Failed to calculate fit for entity {target_entity:?}");
        return;
    };

    let moves = VecDeque::from([CameraMove::ToOrbit {
        focus: target_focus,
        yaw,
        pitch,
        radius: target_radius,
        duration_ms,
        easing,
    }]);

    commands.trigger(PlayAnimation::new(camera_entity, moves));
    commands
        .entity(camera_entity)
        .insert(CurrentFitTarget(target_entity));
}

/// Observer that restores smoothness when `CameraMoveList` is removed
pub fn restore_smoothness_on_move_end(
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

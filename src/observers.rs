//! Observers that wire events to camera behavior.

use std::collections::VecDeque;
use std::time::Duration;

use bevy::math::curve::easing::EaseFunction;
use bevy::prelude::*;
use bevy_panorbit_camera::PanOrbitCamera;

use crate::animation::CameraMove;
use crate::animation::CameraMoveList;
use crate::components::AnimationConflictPolicy;
use crate::components::AnimationSourceMarker;
use crate::components::CurrentFitTarget;
use crate::components::SmoothnessStash;
use crate::components::ZoomAnimationMarker;
use crate::events::AnimateToFit;
use crate::events::AnimationBegin;
use crate::events::AnimationCancelled;
use crate::events::AnimationEnd;
use crate::events::AnimationRejected;
use crate::events::AnimationSource;
use crate::events::PlayAnimation;
use crate::events::SetFitTarget;
use crate::events::ZoomBegin;
use crate::events::ZoomCancelled;
use crate::events::ZoomEnd;
use crate::events::ZoomToFit;
use crate::fit::FitSolution;
use crate::fit::calculate_fit;
use crate::support::extract_mesh_vertices;

/// Ensures camera smoothness is stashed once and disabled while animations are active.
fn ensure_animation_smoothness(
    commands: &mut Commands,
    entity: Entity,
    camera: &mut PanOrbitCamera,
    has_existing_stash: bool,
) {
    if !has_existing_stash {
        let stash = SmoothnessStash {
            zoom:  camera.zoom_smoothness,
            pan:   camera.pan_smoothness,
            orbit: camera.orbit_smoothness,
        };
        commands.entity(entity).insert(stash);
    }

    camera.zoom_smoothness = 0.0;
    camera.pan_smoothness = 0.0;
    camera.orbit_smoothness = 0.0;
}

/// Shared fit preparation used by both ZoomToFit and AnimateToFit observers.
/// Extracts target mesh vertices and computes the fit solution for the requested
/// camera orientation.
#[allow(clippy::too_many_arguments)]
fn prepare_fit_for_target(
    context: &str,
    target_entity: Entity,
    yaw: f32,
    pitch: f32,
    margin: f32,
    projection: &Projection,
    camera: &Camera,
    mesh_query: &Query<&Mesh3d>,
    children_query: &Query<&Children>,
    global_transform_query: &Query<&GlobalTransform>,
    meshes: &Assets<Mesh>,
) -> Option<FitSolution> {
    let Some((vertices, geometric_center)) = extract_mesh_vertices(
        target_entity,
        children_query,
        mesh_query,
        global_transform_query,
        meshes,
    ) else {
        warn!("{context}: Failed to extract mesh vertices for entity {target_entity:?}");
        return None;
    };

    let fit = match calculate_fit(
        &vertices,
        geometric_center,
        yaw,
        pitch,
        margin,
        projection,
        camera,
    ) {
        Ok(fit) => fit,
        Err(error) => {
            warn!("{context}: Failed to calculate fit for entity {target_entity:?}: {error}");
            return None;
        },
    };

    Some(fit)
}

/// Observer for `ZoomToFit` event - frames a target entity in the camera view.
/// When duration is `Duration::ZERO`, snaps instantly.
/// When duration is greater than zero, animates smoothly.
/// Requires target entity to have a `Mesh3d` (direct or on descendants).
#[allow(clippy::too_many_arguments)]
pub fn on_zoom_to_fit(
    zoom: On<ZoomToFit>,
    mut commands: Commands,
    mut camera_query: Query<(
        &mut PanOrbitCamera,
        &Projection,
        &Camera,
        Option<&AnimationConflictPolicy>,
    )>,
    mesh_query: Query<&Mesh3d>,
    children_query: Query<&Children>,
    global_transform_query: Query<&GlobalTransform>,
    move_list_query: Query<&CameraMoveList>,
    meshes: Res<Assets<Mesh>>,
) {
    let camera_entity = zoom.camera_entity;
    let target_entity = zoom.target;
    let margin = zoom.margin;
    let duration = zoom.duration;
    let easing = zoom.easing;

    let Ok((mut camera, projection, cam, conflict_policy)) = camera_query.get_mut(camera_entity)
    else {
        return;
    };

    // Reject early when `FirstWins` is active and an animation is in-flight.
    // This prevents `ZoomBegin` and `ZoomAnimationMarker` from leaking when the
    // underlying `PlayAnimation` would be rejected.
    if duration > Duration::ZERO {
        let policy = conflict_policy.copied().unwrap_or_default();
        let has_in_flight = move_list_query.get(camera_entity).is_ok();
        if has_in_flight && policy == AnimationConflictPolicy::FirstWins {
            commands.trigger(AnimationRejected {
                camera_entity,
                source: AnimationSource::ZoomToFit,
            });
            return;
        }
    }

    debug!(
        "ZoomToFit: yaw={:.3} pitch={:.3} current_focus={:.1?} current_radius={:.1} duration_ms={:.0}",
        camera.target_yaw,
        camera.target_pitch,
        camera.target_focus,
        camera.target_radius,
        duration.as_secs_f32() * 1000.0,
    );

    let Some(fit) = prepare_fit_for_target(
        "ZoomToFit",
        target_entity,
        camera.target_yaw,
        camera.target_pitch,
        margin,
        projection,
        cam,
        &mesh_query,
        &children_query,
        &global_transform_query,
        &meshes,
    ) else {
        return;
    };

    commands.trigger(ZoomBegin {
        camera_entity,
        target_entity,
        margin,
        duration,
        easing,
    });

    if duration > Duration::ZERO {
        // Animated path: use `ToOrbit` to pass orbital params directly, avoiding
        // gimbal lock from atan2 decomposition at extreme pitch angles.
        let camera_moves = VecDeque::from([CameraMove::ToOrbit {
            focus: fit.focus,
            yaw: camera.target_yaw,
            pitch: camera.target_pitch,
            radius: fit.radius,
            duration,
            easing,
        }]);

        // Trigger `PlayAnimation` BEFORE inserting `ZoomAnimationMarker` so that
        // `on_play_animation` sees the old world state during conflict resolution.
        // If inserted first, LastWins would cancel the NEW zoom's own marker.
        commands.trigger(
            PlayAnimation::new(camera_entity, camera_moves).source(AnimationSource::ZoomToFit),
        );

        // Mark this as a zoom operation so `AnimationEnd` fires `ZoomEnd`
        commands.entity(camera_entity).insert(ZoomAnimationMarker {
            target_entity,
            margin,
            duration,
            easing,
        });
    } else {
        // Instant path: snap directly to target
        camera.focus = fit.focus;
        camera.radius = Some(fit.radius);
        camera.target_focus = fit.focus;
        camera.target_radius = fit.radius;
        camera.force_update = true;
        commands.trigger(ZoomEnd {
            camera_entity,
            target_entity,
            margin,
            duration: Duration::ZERO,
            easing,
        });
    }

    // Route fit target updates through a single lifecycle owner.
    commands.trigger(SetFitTarget::new(camera_entity, target_entity));
}

/// Observer for `PlayAnimation` event - initiates camera animation sequence
pub fn on_play_animation(
    start: On<PlayAnimation>,
    mut commands: Commands,
    mut camera_query: Query<(
        &mut PanOrbitCamera,
        Option<&SmoothnessStash>,
        Option<&AnimationConflictPolicy>,
    )>,
    move_list_query: Query<&CameraMoveList>,
    marker_query: Query<&ZoomAnimationMarker>,
    source_marker_query: Query<&AnimationSourceMarker>,
) {
    let entity = start.camera_entity;
    let source = start.source;

    let Ok((mut camera, existing_stash, conflict_policy)) = camera_query.get_mut(entity) else {
        return;
    };

    let policy = conflict_policy.copied().unwrap_or_default();
    let has_in_flight = move_list_query.get(entity).is_ok();

    if has_in_flight {
        match policy {
            AnimationConflictPolicy::FirstWins => {
                commands.trigger(AnimationRejected {
                    camera_entity: entity,
                    source,
                });
                return;
            },
            AnimationConflictPolicy::LastWins => {
                // Cancel in-flight animation — read source from existing marker
                let in_flight_source = source_marker_query
                    .get(entity)
                    .map_or(AnimationSource::PlayAnimation, |m| m.0);
                if let Ok(queue) = move_list_query.get(entity) {
                    let camera_move =
                        queue
                            .camera_moves
                            .front()
                            .cloned()
                            .unwrap_or(CameraMove::ToOrbit {
                                focus:    Vec3::ZERO,
                                yaw:      0.0,
                                pitch:    0.0,
                                radius:   1.0,
                                duration: Duration::ZERO,
                                easing:   EaseFunction::Linear,
                            });
                    commands.trigger(AnimationCancelled {
                        camera_entity: entity,
                        source: in_flight_source,
                        camera_move,
                    });
                }
                // Cancel in-flight zoom if present
                if let Ok(marker) = marker_query.get(entity) {
                    commands.entity(entity).remove::<ZoomAnimationMarker>();
                    commands.trigger(ZoomCancelled {
                        camera_entity: entity,
                        target_entity: marker.target_entity,
                        margin:        marker.margin,
                        duration:      marker.duration,
                        easing:        marker.easing,
                    });
                }
            },
        }
    }

    commands.trigger(AnimationBegin {
        camera_entity: entity,
        source,
    });

    ensure_animation_smoothness(&mut commands, entity, &mut camera, existing_stash.is_some());

    commands
        .entity(entity)
        .insert(CameraMoveList::new(start.camera_moves.clone()));
    commands
        .entity(entity)
        .insert(AnimationSourceMarker(source));
}

/// Observer for direct `CameraMoveList` insertion (bypassing `PlayAnimation`).
/// Reuses the same smoothness behavior as the event-driven path.
pub fn on_camera_move_list_added(
    add: On<Add, CameraMoveList>,
    mut commands: Commands,
    mut camera_query: Query<(&mut PanOrbitCamera, Option<&SmoothnessStash>)>,
) {
    let entity = add.entity;
    let Ok((mut camera, existing_stash)) = camera_query.get_mut(entity) else {
        return;
    };

    ensure_animation_smoothness(&mut commands, entity, &mut camera, existing_stash.is_some());
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
    mut camera_query: Query<(&mut PanOrbitCamera, &Projection, &Camera)>,
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
    let duration = event.duration;
    let easing = event.easing;

    let Ok((mut camera, projection, cam)) = camera_query.get_mut(camera_entity) else {
        return;
    };

    let Some(fit) = prepare_fit_for_target(
        "AnimateToFit",
        target_entity,
        yaw,
        pitch,
        margin,
        projection,
        cam,
        &mesh_query,
        &children_query,
        &global_transform_query,
        &meshes,
    ) else {
        return;
    };

    if duration > Duration::ZERO {
        let camera_moves = VecDeque::from([CameraMove::ToOrbit {
            focus: fit.focus,
            yaw,
            pitch,
            radius: fit.radius,
            duration,
            easing,
        }]);
        commands.trigger(
            PlayAnimation::new(camera_entity, camera_moves).source(AnimationSource::AnimateToFit),
        );
    } else {
        camera.focus = fit.focus;
        camera.yaw = Some(yaw);
        camera.pitch = Some(pitch);
        camera.radius = Some(fit.radius);
        camera.target_focus = fit.focus;
        camera.target_radius = fit.radius;
        camera.target_yaw = yaw;
        camera.target_pitch = pitch;
        camera.force_update = true;
        let source = AnimationSource::AnimateToFit;
        commands.trigger(AnimationBegin {
            camera_entity,
            source,
        });
        commands.trigger(AnimationEnd {
            camera_entity,
            source,
        });
    }
    // Route fit target updates through a single lifecycle owner.
    commands.trigger(SetFitTarget::new(camera_entity, target_entity));
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

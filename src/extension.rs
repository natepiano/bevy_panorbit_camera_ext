//! Extension trait and events for PanOrbitCamera manipulation

use std::collections::VecDeque;

use bevy::camera::primitives::Aabb;
use bevy::math::curve::easing::EaseFunction;
use bevy::prelude::*;
use bevy_panorbit_camera::PanOrbitCamera;

use crate::animation::CameraMove;
use crate::animation::CameraMoveList;
use crate::events::AnimationBegin;
use crate::events::AnimationEnd;
use crate::events::ZoomBegin;
use crate::events::ZoomEnd;
use crate::smoothness::SmoothnessStash;

/// Marks the entity that the camera is currently fitted to.
/// Persists after fit completes to enable persistent visualization.
#[derive(Component, Reflect, Debug)]
#[reflect(Component)]
pub struct CurrentFitTarget(pub Entity);

/// Extension trait for `PanOrbitCamera` providing convenience methods.
pub trait PanOrbitCameraExt {
    /// Disables interpolation for precise control during animations.
    fn disable_interpolation(&mut self);

    /// Enables interpolation for smooth transitions.
    fn enable_interpolation(&mut self, zoom: f32, pan: f32, orbit: f32);

    /// Stashes current smoothness values and disables smoothness.
    /// Returns a `SmoothnessStash` that can be inserted as a component.
    fn stash_and_disable_smoothness(&mut self) -> SmoothnessStash;

    /// Calculates the optimal radius to fit a target entity in the camera view.
    /// Uses the current camera orientation (`target_yaw`, `target_pitch`, `target_radius`).
    ///
    /// # Parameters
    /// - `target_entity`: Entity with `Aabb` to fit in view
    /// - `margin`: Margin as fraction of screen (0.1 = 10% margin on each side)
    /// - `projection`: Camera projection
    /// - `camera`: Camera component
    /// - Query references for `Aabb`, `Children`, and `GlobalTransform`
    ///
    /// Returns `Some(radius)` if successful, `None` if target has no `Aabb` or calculation fails.
    #[allow(clippy::too_many_arguments)]
    fn calculate_fit_radius(
        &self,
        target_entity: Entity,
        margin: f32,
        projection: &Projection,
        camera: &Camera,
        aabb_query: &Query<&Aabb>,
        children_query: &Query<&Children>,
        global_transform_query: &Query<&GlobalTransform>,
    ) -> Option<f32>;
}

impl PanOrbitCameraExt for PanOrbitCamera {
    fn disable_interpolation(&mut self) {
        self.zoom_smoothness = 0.0;
        self.pan_smoothness = 0.0;
        self.orbit_smoothness = 0.0;
    }

    fn enable_interpolation(&mut self, zoom: f32, pan: f32, orbit: f32) {
        self.zoom_smoothness = zoom;
        self.pan_smoothness = pan;
        self.orbit_smoothness = orbit;
    }

    fn stash_and_disable_smoothness(&mut self) -> SmoothnessStash {
        let stash = SmoothnessStash {
            zoom:  self.zoom_smoothness,
            pan:   self.pan_smoothness,
            orbit: self.orbit_smoothness,
        };

        self.zoom_smoothness = 0.0;
        self.pan_smoothness = 0.0;
        self.orbit_smoothness = 0.0;

        stash
    }

    fn calculate_fit_radius(
        &self,
        target_entity: Entity,
        margin: f32,
        projection: &Projection,
        camera: &Camera,
        aabb_query: &Query<&Aabb>,
        children_query: &Query<&Children>,
        global_transform_query: &Query<&GlobalTransform>,
    ) -> Option<f32> {
        calculate_fit_radius(
            target_entity,
            self.target_yaw,
            self.target_pitch,
            margin,
            projection,
            camera,
            aabb_query,
            children_query,
            global_transform_query,
        )
    }
}

// ============================================================================
// Entity Events
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
    camera_entity: Entity,
    target:        Entity,
    margin:        f32,
    duration_ms:   f32,
    easing:        EaseFunction,
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

/// Event to play a queued camera animation
#[derive(EntityEvent, Reflect)]
#[reflect(Event, FromReflect)]
pub struct PlayAnimation {
    #[event_target]
    camera_entity: Entity,
    moves:         VecDeque<CameraMove>,
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
#[derive(EntityEvent, Reflect)]
#[reflect(Event, FromReflect)]
pub struct SetFitTarget {
    #[event_target]
    camera_entity: Entity,
    target:        Entity,
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
    camera_entity: Entity,
    target:        Entity,
    yaw:           f32,
    pitch:         f32,
    margin:        f32,
    duration_ms:   f32,
    easing:        EaseFunction,
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

// ============================================================================
// Observers
// ============================================================================

/// Calculates the optimal radius to fit a target entity in the camera view.
/// Uses the convergence algorithm from the given camera orientation (`yaw`, `pitch`).
///
/// Returns `Some(radius)` if successful, `None` if the target has no `Aabb` or the
/// calculation fails.
#[allow(clippy::too_many_arguments)]
pub fn calculate_fit_radius(
    target_entity: Entity,
    yaw: f32,
    pitch: f32,
    margin: f32,
    projection: &Projection,
    camera: &Camera,
    aabb_query: &Query<&Aabb>,
    children_query: &Query<&Children>,
    global_transform_query: &Query<&GlobalTransform>,
) -> Option<f32> {
    calculate_fit(
        target_entity,
        yaw,
        pitch,
        margin,
        projection,
        camera,
        aabb_query,
        children_query,
        global_transform_query,
    )
    .map(|(radius, _)| radius)
}

/// Calculates the optimal radius and centered focus to fit a target entity in the camera view.
/// The focus is adjusted so the projected box is centered in the viewport.
#[allow(clippy::too_many_arguments)]
fn calculate_fit(
    target_entity: Entity,
    yaw: f32,
    pitch: f32,
    margin: f32,
    projection: &Projection,
    camera: &Camera,
    aabb_query: &Query<&Aabb>,
    children_query: &Query<&Children>,
    global_transform_query: &Query<&GlobalTransform>,
) -> Option<(f32, Vec3)> {
    let Projection::Perspective(perspective) = projection else {
        return None;
    };

    let (aabb_entity, aabb) = find_descendant_aabb(target_entity, children_query, aabb_query)?;
    let global_transform = global_transform_query.get(aabb_entity).ok()?;
    let corners = aabb_to_world_corners(aabb, global_transform);
    let geometric_center = global_transform.translation();

    calculate_convergence_radius(
        &corners,
        geometric_center,
        yaw,
        pitch,
        margin,
        perspective,
        camera.logical_viewport_size(),
    )
}

/// Calculates the radius and centered focus needed to fit corners in view.
///
/// For each candidate radius, computes the focus that centers the projected box in the viewport
/// (since the geometric center doesn't project to screen center from off-axis angles), then
/// evaluates margins at that centered position. Returns the `(radius, focus)` pair where the
/// constraining margin equals the target and the box is centered.
///
/// Note: A lateral camera shift doesn't change corner depths, so the centering is geometrically
/// exact for the constraining margin check.
#[allow(clippy::too_many_arguments)]
fn calculate_convergence_radius(
    corners: &[Vec3; 8],
    geometric_center: Vec3,
    yaw: f32,
    pitch: f32,
    margin: f32,
    perspective: &PerspectiveProjection,
    viewport_size: Option<Vec2>,
) -> Option<(f32, Vec3)> {
    use crate::zoom::ScreenSpaceBounds;
    use crate::zoom::zoom_margin_multiplier;

    let aspect_ratio = viewport_size.map_or(perspective.aspect_ratio, |s| s.x / s.y);
    let zoom_multiplier = zoom_margin_multiplier(margin);

    let rot = Quat::from_euler(EulerRot::YXZ, yaw, -pitch, 0.0);
    let cam_right = rot * Vec3::X;
    let cam_up = rot * Vec3::Y;

    // Compute the object's bounding sphere radius from corners for sensible search bounds.
    // The search range is based purely on object size to ensure deterministic results
    // regardless of the camera's current radius.
    let object_radius = corners
        .iter()
        .map(|c| (*c - geometric_center).length())
        .fold(0.0_f32, f32::max);

    // Binary search for the correct radius
    let mut min_radius = object_radius * 0.1;
    let mut max_radius = object_radius * 100.0;
    let mut best_radius = object_radius * 2.0;
    let mut best_focus = geometric_center;
    let mut best_error = f32::INFINITY;

    info!(
        "Binary search starting: range [{:.1}, {:.1}]",
        min_radius, max_radius
    );

    for iteration in 0..crate::zoom::MAX_ITERATIONS {
        let test_radius = (min_radius + max_radius) * 0.5;

        // Step 1: find the centered focus using accurate depth-based centering
        let centered_focus = refine_focus_centering(
            corners,
            geometric_center,
            test_radius,
            rot,
            cam_right,
            cam_up,
            perspective,
            aspect_ratio,
        );

        // Step 2: evaluate margins at the centered focus position
        let cam_pos = centered_focus + rot * Vec3::new(0.0, 0.0, test_radius);
        let cam_global =
            GlobalTransform::from(Transform::from_translation(cam_pos).with_rotation(rot));

        let Some(bounds) = ScreenSpaceBounds::from_corners(
            corners,
            &cam_global,
            perspective,
            aspect_ratio,
            zoom_multiplier,
        ) else {
            info!(
                "Iteration {iteration}: Corners behind camera at radius {test_radius:.1}, searching higher"
            );
            min_radius = test_radius;
            continue;
        };

        // Find constraining dimension (minimum margin)
        let h_min = bounds.left_margin.min(bounds.right_margin);
        let v_min = bounds.top_margin.min(bounds.bottom_margin);

        let (current_margin, target_margin, dimension) = if h_min < v_min {
            (h_min, bounds.target_margin_x, "H")
        } else {
            (v_min, bounds.target_margin_y, "V")
        };

        info!(
            "Iteration {iteration}: radius={test_radius:.1} | {dimension} margin={current_margin:.3} \
             target={target_margin:.3} | L={:.3} R={:.3} T={:.3} B={:.3} | range=[{min_radius:.1}, {max_radius:.1}]",
            bounds.left_margin, bounds.right_margin, bounds.top_margin, bounds.bottom_margin
        );

        // Track the closest match to target margin
        let margin_error = (current_margin - target_margin).abs();
        if margin_error < best_error {
            best_error = margin_error;
            best_radius = test_radius;
            best_focus = centered_focus;
        }

        if current_margin > target_margin {
            max_radius = test_radius;
        } else {
            min_radius = test_radius;
        }

        if (max_radius - min_radius) < 0.001 {
            info!(
                "Iteration {iteration}: Converged to best radius {best_radius:.3} error={best_error:.5}"
            );
            return Some((best_radius, best_focus));
        }
    }

    info!(
        "Binary search did not converge in {} iterations. Using best radius {best_radius:.1}",
        crate::zoom::MAX_ITERATIONS
    );

    Some((best_radius, best_focus))
}

/// Shifts the camera focus so the projected bounding box is centered on screen.
///
/// A single correction step uses `avg_depth` as the depth estimate, but corners sit at
/// varying depths (near vs far side of the box). Each iteration reduces the centering
/// error by roughly 70-80% (the residual is proportional to depth variance across
/// corners). With `CENTERING_MAX_ITERATIONS` = 10 the residual is ~0.3^10 ≈ 0.000006,
/// well past the `CENTERING_TOLERANCE` of 0.0001. In practice convergence takes 3-5
/// iterations.
#[allow(clippy::too_many_arguments)]
fn refine_focus_centering(
    corners: &[Vec3; 8],
    initial_focus: Vec3,
    radius: f32,
    rot: Quat,
    cam_right: Vec3,
    cam_up: Vec3,
    perspective: &PerspectiveProjection,
    aspect_ratio: f32,
) -> Vec3 {
    use crate::zoom::CENTERING_MAX_ITERATIONS;
    use crate::zoom::CENTERING_TOLERANCE;
    use crate::zoom::ScreenSpaceBounds;

    let mut focus = initial_focus;
    for _ in 0..CENTERING_MAX_ITERATIONS {
        let cam_pos = focus + rot * Vec3::new(0.0, 0.0, radius);
        let cam_global =
            GlobalTransform::from(Transform::from_translation(cam_pos).with_rotation(rot));
        let Some(bounds) =
            ScreenSpaceBounds::from_corners(corners, &cam_global, perspective, aspect_ratio, 1.0)
        else {
            break;
        };
        let (cx, cy) = bounds.center();
        if cx.abs() < CENTERING_TOLERANCE && cy.abs() < CENTERING_TOLERANCE {
            break;
        }
        focus += cam_right * cx * bounds.avg_depth + cam_up * cy * bounds.avg_depth;
    }
    focus
}

/// Observer for `ZoomToFit` event - frames a target entity in the camera view.
/// When `duration_ms > 0.0`, animates smoothly over that duration.
/// When `duration_ms <= 0.0`, snaps instantly.
/// Requires target entity to have an `Aabb` (direct or on descendants).
pub fn on_zoom_to_fit(
    zoom: On<ZoomToFit>,
    mut commands: Commands,
    mut camera_query: Query<(&mut PanOrbitCamera, &Projection, &Camera)>,
    aabb_query: Query<&Aabb>,
    children_query: Query<&Children>,
    global_transform_query: Query<&GlobalTransform>,
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

    let Some((target_radius, target_focus)) = calculate_fit(
        target_entity,
        camera.target_yaw,
        camera.target_pitch,
        margin,
        projection,
        cam,
        &aabb_query,
        &children_query,
        &global_transform_query,
    ) else {
        warn!("ZoomToFit: Failed to calculate target radius for entity {target_entity:?}");
        return;
    };

    if duration_ms > 0.0 {
        // Animated path: convert to a single CameraMove routed through PlayAnimation
        let yaw_rot = Quat::from_axis_angle(Vec3::Y, camera.target_yaw);
        let pitch_rot = Quat::from_axis_angle(Vec3::X, -camera.target_pitch);
        let rotation = yaw_rot * pitch_rot;
        let target_translation = target_focus + rotation * Vec3::new(0.0, 0.0, target_radius);

        let moves = VecDeque::from([CameraMove {
            target_translation,
            target_focus,
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

/// Observer that fires `ZoomEnd` when an animation completes on an entity with a
/// `ZoomAnimationMarker`, bridging the animation lifecycle to the zoom lifecycle.
pub fn on_zoom_animation_end(
    event: On<AnimationEnd>,
    mut commands: Commands,
    marker_query: Query<&ZoomAnimationMarker>,
) {
    let camera_entity = event.camera_entity;

    let Ok(marker) = marker_query.get(camera_entity) else {
        return;
    };

    commands.trigger(ZoomEnd {
        camera_entity,
        target_entity: marker.target_entity,
        margin: marker.margin,
        duration_ms: marker.duration_ms,
        easing: marker.easing,
    });

    commands
        .entity(camera_entity)
        .remove::<ZoomAnimationMarker>();
}

/// Recursively searches for an `Aabb` component on an entity or its descendants
pub fn find_descendant_aabb<'a>(
    entity: Entity,
    children_query: &Query<&Children>,
    aabb_query: &'a Query<&Aabb>,
) -> Option<(Entity, &'a Aabb)> {
    // Check if this entity has an Aabb
    if let Ok(aabb) = aabb_query.get(entity) {
        return Some((entity, aabb));
    }

    // Recursively check children
    if let Ok(children) = children_query.get(entity) {
        for child in children.iter() {
            if let Some(result) = find_descendant_aabb(child, children_query, aabb_query) {
                return Some(result);
            }
        }
    }

    None
}

/// Converts an `Aabb` to 8 corners in world space
pub fn aabb_to_world_corners(aabb: &Aabb, global_transform: &GlobalTransform) -> [Vec3; 8] {
    let center = Vec3::from(aabb.center);
    let half_extents = Vec3::from(aabb.half_extents);

    // Create 8 corners of the box
    let corners = [
        center + Vec3::new(-half_extents.x, -half_extents.y, -half_extents.z),
        center + Vec3::new(half_extents.x, -half_extents.y, -half_extents.z),
        center + Vec3::new(-half_extents.x, half_extents.y, -half_extents.z),
        center + Vec3::new(half_extents.x, half_extents.y, -half_extents.z),
        center + Vec3::new(-half_extents.x, -half_extents.y, half_extents.z),
        center + Vec3::new(half_extents.x, -half_extents.y, half_extents.z),
        center + Vec3::new(-half_extents.x, half_extents.y, half_extents.z),
        center + Vec3::new(half_extents.x, half_extents.y, half_extents.z),
    ];

    // Transform to world space
    corners.map(|corner| global_transform.transform_point(corner))
}

/// Observer for PlayAnimation event - initiates camera animation sequence
pub fn on_play_animation(
    start: On<PlayAnimation>,
    mut commands: Commands,
    mut camera_query: Query<&mut PanOrbitCamera>,
) {
    let entity = start.camera_entity;

    let Ok(mut camera) = camera_query.get_mut(entity) else {
        return;
    };

    commands.trigger(AnimationBegin {
        camera_entity: entity,
    });

    // Stash and disable smoothness for precise animation control
    let stash = camera.stash_and_disable_smoothness();
    commands.entity(entity).insert(stash);

    // Add the animation component
    commands
        .entity(entity)
        .insert(CameraMoveList::new(start.moves.clone()));
}

/// Observer for SetFitTarget event - sets the target entity for fit visualization
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
    aabb_query: Query<&Aabb>,
    children_query: Query<&Children>,
    global_transform_query: Query<&GlobalTransform>,
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

    let Some((target_radius, target_focus)) = calculate_fit(
        target_entity,
        yaw,
        pitch,
        margin,
        projection,
        cam,
        &aabb_query,
        &children_query,
        &global_transform_query,
    ) else {
        warn!("AnimateToFit: Failed to calculate fit for entity {target_entity:?}");
        return;
    };

    // Convert spherical (yaw, pitch, radius) to cartesian position relative to focus
    // Must match `PanOrbitCamera`'s `update_orbit_transform`: yaw around Y, then pitch around X
    let yaw_rot = Quat::from_axis_angle(Vec3::Y, yaw);
    let pitch_rot = Quat::from_axis_angle(Vec3::X, -pitch);
    let rotation = yaw_rot * pitch_rot;
    let target_translation = target_focus + rotation * Vec3::new(0.0, 0.0, target_radius);

    let moves = VecDeque::from([CameraMove {
        target_translation,
        target_focus,
        duration_ms,
        easing,
    }]);

    commands.trigger(PlayAnimation::new(camera_entity, moves));
    commands
        .entity(camera_entity)
        .insert(CurrentFitTarget(target_entity));
}

//! Extension trait and events for PanOrbitCamera manipulation

use std::collections::VecDeque;

use bevy::camera::primitives::Aabb;
use bevy::prelude::*;
use bevy_panorbit_camera::PanOrbitCamera;

use crate::animation::CameraMove;
use crate::animation::CameraMoveList;
use crate::smoothness::SmoothnessStash;

/// Configuration for zoom-to-fit behavior
#[derive(Component, Debug, Clone)]
pub struct ZoomToFitConfig {
    /// Padding factor for zoom-to-fit (1.0 = no padding, 1.2 = 20% padding)
    pub padding: f32,
}

/// Marks the entity that the camera is currently fitted to.
/// Persists after fit completes to enable persistent visualization.
#[derive(Component, Reflect, Debug)]
#[reflect(Component)]
pub struct CurrentFitTarget(pub Entity);

impl Default for ZoomToFitConfig {
    fn default() -> Self { Self { padding: 1.2 } }
}

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
            self.target_radius,
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

/// Event to instantly snap camera to frame a target entity
#[derive(EntityEvent, Reflect)]
#[reflect(Event)]
pub struct SnapToFit {
    pub entity: Entity,
    pub target: Entity,
    pub margin: f32,
}

impl SnapToFit {
    pub const fn new(entity: Entity, target: Entity, margin: f32) -> Self {
        Self {
            entity,
            target,
            margin,
        }
    }
}

/// Event to smoothly animate camera to frame a target transform
#[derive(EntityEvent, Reflect)]
#[reflect(Event)]
pub struct ZoomToFit {
    pub entity: Entity,
    pub target: Entity,
    pub margin: f32,
}

impl ZoomToFit {
    pub const fn new(entity: Entity, target: Entity, margin: f32) -> Self {
        Self {
            entity,
            target,
            margin,
        }
    }
}

/// Event to start a queued camera animation
#[derive(EntityEvent, Reflect)]
#[reflect(Event)]
pub struct StartAnimation {
    pub entity: Entity,
    pub moves:  VecDeque<CameraMove>,
}

impl StartAnimation {
    pub const fn new(entity: Entity, moves: VecDeque<CameraMove>) -> Self { Self { entity, moves } }
}

/// Event to set the target entity for fit visualization debugging
#[derive(EntityEvent, Reflect)]
#[reflect(Event)]
pub struct SetFitTarget {
    pub entity: Entity,
    pub target: Entity,
}

impl SetFitTarget {
    pub const fn new(entity: Entity, target: Entity) -> Self { Self { entity, target } }
}

// ============================================================================
// Observers
// ============================================================================

/// Observer that automatically adds `ZoomToFitConfig` to cameras
pub fn auto_add_zoom_config(
    add: On<Add, PanOrbitCamera>,
    mut commands: Commands,
    config_query: Query<&ZoomToFitConfig>,
) {
    let entity = add.entity;

    // Only add if not already present
    if config_query.get(entity).is_err() {
        commands.entity(entity).insert(ZoomToFitConfig::default());
    }
}

/// Calculates the optimal radius to fit a target entity in the camera view.
/// Uses the convergence algorithm from the given camera orientation (`yaw`, `pitch`).
///
/// Returns `Some(radius)` if successful, `None` if the target has no `Aabb` or the
/// calculation fails.
pub fn calculate_fit_radius(
    target_entity: Entity,
    current_radius: f32,
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
        current_radius,
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
fn calculate_fit(
    target_entity: Entity,
    current_radius: f32,
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
        current_radius,
        geometric_center,
        yaw,
        pitch,
        margin,
        perspective,
        camera.logical_viewport_size(),
    )
}

/// Observer for SnapToFit event - instantly positions camera to frame the target
/// Requires target entity to have an Aabb (direct or on descendants)
pub fn on_snap_to_fit(
    snap: On<SnapToFit>,
    mut commands: Commands,
    mut camera_query: Query<(&mut PanOrbitCamera, &Projection, &Camera)>,
    aabb_query: Query<&Aabb>,
    children_query: Query<&Children>,
    global_transform_query: Query<&GlobalTransform>,
) {
    let camera_entity = snap.entity;
    let target_entity = snap.target;
    let margin = snap.margin;

    let Ok((mut camera, projection, cam)) = camera_query.get_mut(camera_entity) else {
        return;
    };

    let Some((radius, focus)) = calculate_fit(
        target_entity,
        camera.target_radius,
        camera.target_yaw,
        camera.target_pitch,
        margin,
        projection,
        cam,
        &aabb_query,
        &children_query,
        &global_transform_query,
    ) else {
        warn!("SnapToFit: Failed to calculate radius for entity {target_entity:?}");
        return;
    };

    camera.target_focus = focus;
    camera.target_radius = radius;
    camera.force_update = true;

    // Mark current fit target for visualization
    commands
        .entity(camera_entity)
        .insert(CurrentFitTarget(target_entity));
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
fn calculate_convergence_radius(
    corners: &[Vec3; 8],
    initial_radius: f32,
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

    // Binary search for the correct radius
    let mut min_radius = initial_radius * 0.1;
    let mut max_radius = initial_radius * 10.0;
    let mut best_radius = initial_radius;
    let mut best_focus = geometric_center;
    let mut best_error = f32::INFINITY;

    info!(
        "Binary search starting: range [{:.1}, {:.1}]",
        min_radius, max_radius
    );

    for iteration in 0..crate::zoom::MAX_ITERATIONS {
        let test_radius = (min_radius + max_radius) * 0.5;

        // Step 1: find the centered focus by iterating focus adjustments.
        // Corners have varying depths so one step isn't exact; 3 iterations converge quickly.
        let mut centered_focus = geometric_center;
        for _ in 0..3 {
            let cam_pos_iter = centered_focus + rot * Vec3::new(0.0, 0.0, test_radius);
            let cam_global_iter =
                GlobalTransform::from(Transform::from_translation(cam_pos_iter).with_rotation(rot));
            if let Some(iter_bounds) = ScreenSpaceBounds::from_corners(
                corners,
                &cam_global_iter,
                perspective,
                aspect_ratio,
                1.0, // no zoom multiplier - we only need the screen-space center
            ) {
                let (cx, cy) = iter_bounds.center();
                centered_focus += cam_right * cx * test_radius + cam_up * cy * test_radius;
            }
        }

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

/// Observer for ZoomToFit event - initiates smooth zoom-to-fit animation
/// Requires target entity to have an Aabb (direct or on descendants)
pub fn on_zoom_to_fit(
    zoom: On<ZoomToFit>,
    mut commands: Commands,
    mut camera_query: Query<(&mut PanOrbitCamera, &Projection, &Camera)>,
    aabb_query: Query<&Aabb>,
    children_query: Query<&Children>,
    global_transform_query: Query<&GlobalTransform>,
) {
    let camera_entity = zoom.entity;
    let target_entity = zoom.target;
    let margin = zoom.margin;

    let Ok((mut camera, projection, cam)) = camera_query.get_mut(camera_entity) else {
        return;
    };

    info!(
        "ZoomToFit: yaw={:.3} pitch={:.3} current_focus={:.1?} current_radius={:.1}",
        camera.target_yaw, camera.target_pitch, camera.target_focus, camera.target_radius
    );

    let Some((target_radius, target_focus)) = calculate_fit(
        target_entity,
        camera.target_radius,
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

    // Store current values as animation start
    let start_focus = camera.target_focus;
    let start_radius = camera.target_radius;

    // Create animation component
    use crate::zoom::ZoomToFitAnimation;
    commands.entity(camera_entity).insert(ZoomToFitAnimation {
        start_focus,
        target_focus,
        start_radius,
        target_radius,
        duration_ms: 500.0,
        elapsed_ms: 0.0,
    });

    // Disable smoothness during animation
    let stash = camera.stash_and_disable_smoothness();
    commands.entity(camera_entity).insert(stash);

    // Mark current fit target for visualization
    commands
        .entity(camera_entity)
        .insert(CurrentFitTarget(target_entity));
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

/// Observer for StartAnimation event - initiates camera animation sequence
pub fn on_start_animation(
    start: On<StartAnimation>,
    mut commands: Commands,
    mut camera_query: Query<&mut PanOrbitCamera>,
) {
    let entity = start.entity;

    let Ok(mut camera) = camera_query.get_mut(entity) else {
        return;
    };

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
        .entity(set_target.entity)
        .insert(CurrentFitTarget(set_target.target));
}

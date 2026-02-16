//! Extension trait and events for PanOrbitCamera manipulation

use std::collections::VecDeque;

use bevy::camera::primitives::Aabb;
use bevy::prelude::*;
use bevy_panorbit_camera::PanOrbitCamera;

use crate::animation::CameraMove;
use crate::animation::CameraMoveList;
use crate::smoothness::SmoothnessStash;
use crate::zoom::ZoomConfig;
use crate::zoom::ZoomToFitComponent;

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
}

impl SnapToFit {
    pub const fn new(entity: Entity, target: Entity) -> Self { Self { entity, target } }
}

/// Event to smoothly animate camera to frame a target transform
#[derive(EntityEvent, Reflect)]
#[reflect(Event)]
pub struct ZoomToFit {
    pub entity: Entity,
    pub target: Entity,
}

impl ZoomToFit {
    pub const fn new(entity: Entity, target: Entity) -> Self { Self { entity, target } }
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
/// Uses the convergence algorithm from a canonical camera position (yaw=0, pitch=0).
/// This is useful for pre-calculating camera positions (e.g., for animations that should
/// end at the same position as SnapToFit or ZoomToFit).
///
/// Returns `Some(radius)` if the calculation succeeds, `None` if the target has no Aabb
/// or if the calculation fails.
pub fn calculate_fit_radius(
    target_entity: Entity,
    current_radius: f32,
    projection: &Projection,
    camera: &Camera,
    aabb_query: &Query<&Aabb>,
    children_query: &Query<&Children>,
    global_transform_query: &Query<&GlobalTransform>,
    zoom_config: &ZoomConfig,
) -> Option<f32> {
    let Projection::Perspective(perspective) = projection else {
        return None;
    };

    // Find Aabb (direct or descendants)
    let (aabb_entity, aabb) = find_descendant_aabb(target_entity, children_query, aabb_query)?;

    let global_transform = global_transform_query.get(aabb_entity).ok()?;

    let corners = aabb_to_world_corners(aabb, global_transform);
    let focus = global_transform.translation();

    // Calculate radius using convergence from canonical position
    calculate_convergence_radius(
        &corners,
        current_radius,
        focus,
        perspective,
        camera.logical_viewport_size(),
        zoom_config,
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
    zoom_config: Res<ZoomConfig>,
) {
    let camera_entity = snap.entity;
    let target_entity = snap.target;

    let Ok((mut camera, projection, cam)) = camera_query.get_mut(camera_entity) else {
        return;
    };

    // Use the public calculation function
    let Some(radius) = calculate_fit_radius(
        target_entity,
        camera.target_radius,
        projection,
        cam,
        &aabb_query,
        &children_query,
        &global_transform_query,
        &zoom_config,
    ) else {
        warn!("SnapToFit: Failed to calculate radius for entity {target_entity:?}");
        return;
    };

    // Get the focus point
    let Some((aabb_entity, _)) = find_descendant_aabb(target_entity, &children_query, &aabb_query)
    else {
        return;
    };

    let Ok(global_transform) = global_transform_query.get(aabb_entity) else {
        return;
    };

    let focus = global_transform.translation();

    // Set camera to look at target center with calculated radius
    camera.target_focus = focus;
    camera.target_yaw = 0.0;
    camera.target_pitch = 0.0;
    camera.target_radius = radius;
    camera.force_update = true;

    // Mark current fit target for visualization
    commands
        .entity(camera_entity)
        .insert(CurrentFitTarget(target_entity));
}

/// Calculates the radius needed to fit corners in view using the convergence algorithm.
/// Runs iterations from a canonical camera position (looking at focus with yaw=0, pitch=0).
fn calculate_convergence_radius(
    corners: &[Vec3; 8],
    initial_radius: f32,
    focus: Vec3,
    perspective: &PerspectiveProjection,
    viewport_size: Option<Vec2>,
    zoom_config: &ZoomConfig,
) -> Option<f32> {
    use crate::zoom::ScreenSpaceBounds;

    let aspect_ratio = if let Some(size) = viewport_size {
        size.x / size.y
    } else {
        perspective.aspect_ratio
    };

    let mut current_radius = initial_radius;

    // Run convergence iterations from canonical position (yaw=0, pitch=0)
    for _ in 0..10 {
        // Construct camera transform at current radius estimate
        // Camera at (focus.x, focus.y, focus.z + radius) looking at focus
        let cam_pos = focus + Vec3::new(0.0, 0.0, current_radius);
        let cam_global = GlobalTransform::from_translation(cam_pos);

        let Some(bounds) = ScreenSpaceBounds::from_corners(
            corners,
            &cam_global,
            perspective,
            aspect_ratio,
            zoom_config.zoom_margin_multiplier(),
        ) else {
            // Corners behind camera, move back
            current_radius *= 1.5;
            continue;
        };

        let (span_x, span_y) = bounds.span();
        let target_span_x = 2.0 * bounds.half_tan_hfov / zoom_config.zoom_margin_multiplier();
        let target_span_y = 2.0 * bounds.half_tan_vfov / zoom_config.zoom_margin_multiplier();

        let ratio_x = span_x / target_span_x;
        let ratio_y = span_y / target_span_y;
        let ratio = ratio_x.max(ratio_y);

        let target_radius = current_radius * ratio;

        // Check if converged (within tolerance)
        if (target_radius - current_radius).abs() < 0.01 {
            return Some(target_radius);
        }

        current_radius = target_radius;
    }

    Some(current_radius)
}

/// Observer for ZoomToFit event - initiates smooth zoom-to-fit
/// Requires target entity to have an Aabb (direct or on descendants)
pub fn on_zoom_to_fit(
    zoom: On<ZoomToFit>,
    mut commands: Commands,
    mut camera_query: Query<&mut PanOrbitCamera>,
    aabb_query: Query<&Aabb>,
    children_query: Query<&Children>,
    global_transform_query: Query<&GlobalTransform>,
) {
    let camera_entity = zoom.entity;
    let target_entity = zoom.target;

    // Find Aabb (direct or descendants)
    let Some((aabb_entity, aabb)) =
        find_descendant_aabb(target_entity, &children_query, &aabb_query)
    else {
        warn!("ZoomToFit: No Aabb found on entity {target_entity:?} or its descendants");
        return;
    };

    let Ok(global_transform) = global_transform_query.get(aabb_entity) else {
        warn!("No GlobalTransform found for Aabb entity");
        return;
    };

    let corners = aabb_to_world_corners(aabb, global_transform);

    // Common setup
    let Ok(mut camera) = camera_query.get_mut(camera_entity) else {
        return;
    };

    // Stash and disable smoothness for precise convergence
    let stash = camera.stash_and_disable_smoothness();
    commands.entity(camera_entity).insert(stash);

    // Add ZoomToFitComponent (will be processed by zoom_to_fit_convergence_system)
    commands.entity(camera_entity).insert(ZoomToFitComponent {
        target_corners:     corners,
        iteration_count:    0,
        final_target_focus: None,
    });

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

//! Zoom-to-fit convergence system for framing objects in the camera view

use bevy::prelude::*;
use bevy_panorbit_camera::PanOrbitCamera;

/// Configuration for zoom-to-fit behavior
#[derive(Resource, Reflect, Debug, Clone)]
#[reflect(Resource)]
pub struct ZoomConfig {
    /// Maximum iterations before giving up
    pub max_iterations: usize,
    /// Margin as fraction of screen (0.1 = 10% margin on each side)
    pub margin: f32,
    /// Margin tolerance for convergence detection
    pub margin_tolerance: f32,
    /// Convergence rate for adjustments (0.30 = 30% per frame)
    pub convergence_rate: f32,
}

impl Default for ZoomConfig {
    fn default() -> Self {
        Self {
            max_iterations: 200,
            margin: 0.1,
            margin_tolerance: 0.00001,
            convergence_rate: 0.30,
        }
    }
}

impl ZoomConfig {
    /// Returns the zoom margin multiplier (1.0 / (1.0 - margin))
    /// For example, a margin of 0.08 returns 1.087 (8% margin)
    pub const fn zoom_margin_multiplier(&self) -> f32 {
        1.0 / (1.0 - self.margin)
    }
}

/// Component that marks a camera as actively performing zoom-to-fit convergence
#[derive(Component, Debug)]
pub struct ZoomToFitComponent {
    pub target_corners: [Vec3; 8],
    pub iteration_count: usize,
    /// Final target focus calculated once at convergence start
    pub final_target_focus: Option<Vec3>,
}

/// Screen edge identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Edge {
    Left,
    Right,
    Top,
    Bottom,
}

/// Screen-space bounds information with margin calculations
#[derive(Debug, Clone)]
pub struct ScreenSpaceBounds {
    /// Distance from left edge (positive = inside, negative = outside)
    pub left_margin: f32,
    /// Distance from right edge (positive = inside, negative = outside)
    pub right_margin: f32,
    /// Distance from top edge (positive = inside, negative = outside)
    pub top_margin: f32,
    /// Distance from bottom edge (positive = inside, negative = outside)
    pub bottom_margin: f32,
    /// Target margin for horizontal (in screen-space units)
    pub target_margin_x: f32,
    /// Target margin for vertical (in screen-space units)
    pub target_margin_y: f32,
    /// Minimum normalized x coordinate in screen space
    pub min_norm_x: f32,
    /// Maximum normalized x coordinate in screen space
    pub max_norm_x: f32,
    /// Minimum normalized y coordinate in screen space
    pub min_norm_y: f32,
    /// Maximum normalized y coordinate in screen space
    pub max_norm_y: f32,
    /// Average depth of boundary corners from camera
    pub avg_depth: f32,
    /// Half tangent of vertical field of view
    pub half_tan_vfov: f32,
    /// Half tangent of horizontal field of view (vfov * aspect_ratio)
    pub half_tan_hfov: f32,
}

impl ScreenSpaceBounds {
    /// Creates screen space bounds from a camera's view of a set of corners.
    /// Returns `None` if any corner is behind the camera.
    #[allow(clippy::too_many_arguments)]
    pub fn from_corners(
        corners: &[Vec3; 8],
        cam_global: &GlobalTransform,
        perspective: &PerspectiveProjection,
        viewport_aspect: f32,
        zoom_multiplier: f32,
    ) -> Option<Self> {
        let half_tan_vfov = (perspective.fov * 0.5).tan();
        let half_tan_hfov = half_tan_vfov * viewport_aspect;

        // Get camera basis vectors from global transform
        let cam_pos = cam_global.translation();
        let cam_rot = cam_global.rotation();
        let cam_forward = cam_rot * Vec3::NEG_Z;
        let cam_right = cam_rot * Vec3::X;
        let cam_up = cam_rot * Vec3::Y;

        // Project corners to screen space
        let mut min_norm_x = f32::INFINITY;
        let mut max_norm_x = f32::NEG_INFINITY;
        let mut min_norm_y = f32::INFINITY;
        let mut max_norm_y = f32::NEG_INFINITY;
        let mut avg_depth = 0.0;

        for corner in corners {
            let relative = *corner - cam_pos;
            let depth = relative.dot(cam_forward);

            // Check if corner is behind camera
            if depth <= 0.1 {
                return None;
            }

            let x = relative.dot(cam_right);
            let y = relative.dot(cam_up);

            let norm_x = x / depth;
            let norm_y = y / depth;

            min_norm_x = min_norm_x.min(norm_x);
            max_norm_x = max_norm_x.max(norm_x);
            min_norm_y = min_norm_y.min(norm_y);
            max_norm_y = max_norm_y.max(norm_y);
            avg_depth += depth;
        }
        avg_depth /= 8.0;

        // Screen edges are at ±half_tan_hfov and ±half_tan_vfov
        // Target edges (with margin) are at ±(half_tan_hfov / zoom_multiplier)
        let target_edge_x = half_tan_hfov / zoom_multiplier;
        let target_edge_y = half_tan_vfov / zoom_multiplier;

        // Calculate margins as distance from bounds to screen edges
        let left_margin = min_norm_x - (-half_tan_hfov);
        let right_margin = half_tan_hfov - max_norm_x;
        let bottom_margin = min_norm_y - (-half_tan_vfov);
        let top_margin = half_tan_vfov - max_norm_y;

        // Target margins are the difference between screen edge and target edge
        let target_margin_x = half_tan_hfov - target_edge_x;
        let target_margin_y = half_tan_vfov - target_edge_y;

        Some(Self {
            left_margin,
            right_margin,
            top_margin,
            bottom_margin,
            target_margin_x,
            target_margin_y,
            min_norm_x,
            max_norm_x,
            min_norm_y,
            max_norm_y,
            avg_depth,
            half_tan_vfov,
            half_tan_hfov,
        })
    }

    /// Returns true if the margins are balanced (opposite sides are equal)
    pub fn is_balanced(&self, tolerance: f32) -> bool {
        let horizontal_balanced = (self.left_margin - self.right_margin).abs() < tolerance;
        let vertical_balanced = (self.top_margin - self.bottom_margin).abs() < tolerance;
        horizontal_balanced && vertical_balanced
    }

    /// Returns true if the constraining dimension has reached its target margin
    pub fn is_fitted(&self, at_target_tolerance: f32) -> bool {
        let h_min = self.left_margin.min(self.right_margin);
        let v_min = self.top_margin.min(self.bottom_margin);

        // The constraining dimension is the one with smaller margin
        let (constraining_margin, target_margin) = if h_min < v_min {
            (h_min, self.target_margin_x)
        } else {
            (v_min, self.target_margin_y)
        };

        // Check if constraining dimension is at target
        (constraining_margin - target_margin).abs() < at_target_tolerance
    }

    /// Returns the center of the bounds in normalized screen space
    pub const fn center(&self) -> (f32, f32) {
        let center_x = (self.min_norm_x + self.max_norm_x) * 0.5;
        let center_y = (self.min_norm_y + self.max_norm_y) * 0.5;
        (center_x, center_y)
    }

    /// Returns the span (width, height) of the bounds in normalized screen space
    pub const fn span(&self) -> (f32, f32) {
        let span_x = self.max_norm_x - self.min_norm_x;
        let span_y = self.max_norm_y - self.min_norm_y;
        (span_x, span_y)
    }
}

/// Computes the 8 corners of a bounding box from a transform.
/// For a transform centered at the origin with only scale, returns corners in local space.
/// For a transform with translation/rotation, returns corners in world space.
pub fn compute_bounding_corners(transform: &Transform) -> [Vec3; 8] {
    // Create unit cube corners (before scaling)
    let unit_corners = [
        Vec3::new(-0.5, -0.5, -0.5),
        Vec3::new(0.5, -0.5, -0.5),
        Vec3::new(-0.5, 0.5, -0.5),
        Vec3::new(0.5, 0.5, -0.5),
        Vec3::new(-0.5, -0.5, 0.5),
        Vec3::new(0.5, -0.5, 0.5),
        Vec3::new(-0.5, 0.5, 0.5),
        Vec3::new(0.5, 0.5, 0.5),
    ];

    // Transform unit corners to world space (applies translation, rotation, AND scale)
    unit_corners.map(|corner| transform.transform_point(corner))
}

/// System that performs iterative zoom-to-fit convergence
pub fn zoom_to_fit_convergence_system(
    mut commands: Commands,
    zoom_config: Res<ZoomConfig>,
    mut camera_query: Query<(
        Entity,
        &GlobalTransform,
        &mut PanOrbitCamera,
        &Projection,
        &Camera,
        &mut ZoomToFitComponent,
    )>,
) {
    for (entity, cam_global, mut pan_orbit, projection, camera, mut zoom_state) in &mut camera_query
    {
        let Projection::Perspective(perspective) = projection else {
            continue;
        };

        // Get actual viewport aspect ratio
        let aspect_ratio = if let Some(viewport_size) = camera.logical_viewport_size() {
            viewport_size.x / viewport_size.y
        } else {
            perspective.aspect_ratio
        };

        // Calculate screen-space bounds and margins
        let Some(bounds) = ScreenSpaceBounds::from_corners(
            &zoom_state.target_corners,
            cam_global,
            perspective,
            aspect_ratio,
            zoom_config.zoom_margin_multiplier(),
        ) else {
            // Corners behind camera, move camera back
            let corners_center = zoom_state.target_corners.iter().sum::<Vec3>() / 8.0;
            pan_orbit.target_focus = corners_center;
            pan_orbit.target_radius *= 1.5;
            pan_orbit.force_update = true;
            zoom_state.iteration_count += 1;
            continue;
        };

        // Calculate final target focus once on first iteration
        if zoom_state.final_target_focus.is_none() {
            let corners_center = zoom_state.target_corners.iter().sum::<Vec3>() / 8.0;
            zoom_state.final_target_focus = Some(corners_center);
        }

        let final_target_focus = zoom_state.final_target_focus.unwrap();

        // Calculate target radius (this changes based on current view)
        let current_radius = pan_orbit.target_radius;
        let (span_x, span_y) = bounds.span();
        let target_radius =
            calculate_target_radius(current_radius, span_x, span_y, &bounds, &zoom_config);

        // Calculate deltas - focus moves toward final target, radius toward calculated target
        let focus_delta = final_target_focus - pan_orbit.target_focus;
        let radius_delta = target_radius - current_radius;

        // Adaptive convergence rate: faster for small adjustments, slower for large changes
        let base_rate = zoom_config.convergence_rate;
        let radius_change_ratio = radius_delta.abs() / current_radius.max(0.1);
        let focus_change_ratio = focus_delta.length() / current_radius.max(0.1);

        // Speed up convergence when changes are small (less than 5% of current radius)
        let adaptive_rate = if radius_change_ratio < 0.05 && focus_change_ratio < 0.05 {
            0.8 // Fast convergence for fine adjustments
        } else {
            base_rate // Use configured rate for large changes
        };

        pan_orbit.target_focus += focus_delta * adaptive_rate;
        pan_orbit.target_radius = current_radius + radius_delta * adaptive_rate;
        pan_orbit.force_update = true;

        // Check convergence
        let balanced = bounds.is_balanced(zoom_config.margin_tolerance);
        let fitted = bounds.is_fitted(zoom_config.margin_tolerance);

        if balanced && fitted {
            // Synchronize current values with target values to prevent interpolation
            // artifacts when smoothness is restored after convergence
            pan_orbit.focus = pan_orbit.target_focus;
            pan_orbit.radius = Some(pan_orbit.target_radius);
            pan_orbit.yaw = Some(pan_orbit.target_yaw);
            pan_orbit.pitch = Some(pan_orbit.target_pitch);

            commands.entity(entity).remove::<ZoomToFitComponent>();
            continue;
        }

        zoom_state.iteration_count += 1;

        // Stop if we hit max iterations
        if zoom_state.iteration_count >= zoom_config.max_iterations {
            commands.entity(entity).remove::<ZoomToFitComponent>();
        }
    }
}

/// Calculate target radius using span ratios
fn calculate_target_radius(
    current_radius: f32,
    span_x: f32,
    span_y: f32,
    bounds: &ScreenSpaceBounds,
    zoom_config: &ZoomConfig,
) -> f32 {
    // Target spans with proper margins
    let target_span_x = 2.0 * bounds.half_tan_hfov / zoom_config.zoom_margin_multiplier();
    let target_span_y = 2.0 * bounds.half_tan_vfov / zoom_config.zoom_margin_multiplier();

    // Calculate ratios for each dimension
    let ratio_x = span_x / target_span_x;
    let ratio_y = span_y / target_span_y;

    // Use the larger ratio (constraining dimension) to ensure both fit
    let ratio = ratio_x.max(ratio_y);

    // Calculate target radius from current radius and span ratio
    current_radius * ratio
}

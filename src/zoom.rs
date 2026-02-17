//! Zoom-to-fit convergence system for framing objects in the camera view

use bevy::prelude::*;
use bevy_panorbit_camera::PanOrbitCamera;

// Algorithm constants (internal implementation details)
pub const MAX_ITERATIONS: usize = 200;
pub const TOLERANCE: f32 = 0.001; // 0.1% tolerance for convergence
pub const CENTERING_MAX_ITERATIONS: usize = 10;
pub const CENTERING_TOLERANCE: f32 = 0.0001; // normalized screen-space center offset

/// Default margin for zoom-to-fit operations (0.1 = 10% margin on each side)
pub const DEFAULT_MARGIN: f32 = 0.1;

/// Returns the zoom margin multiplier (1.0 / (1.0 - margin))
/// For example, a margin of 0.08 returns 1.087 (8% margin)
pub const fn zoom_margin_multiplier(margin: f32) -> f32 { 1.0 / (1.0 - margin) }

/// Component that marks a camera as actively animating zoom-to-fit
#[derive(Component, Debug)]
pub struct ZoomToFitAnimation {
    pub start_focus:   Vec3,
    pub target_focus:  Vec3,
    pub start_radius:  f32,
    pub target_radius: f32,
    pub duration_ms:   f32,
    pub elapsed_ms:    f32,
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
    pub left_margin:     f32,
    /// Distance from right edge (positive = inside, negative = outside)
    pub right_margin:    f32,
    /// Distance from top edge (positive = inside, negative = outside)
    pub top_margin:      f32,
    /// Distance from bottom edge (positive = inside, negative = outside)
    pub bottom_margin:   f32,
    /// Target margin for horizontal (in screen-space units)
    pub target_margin_x: f32,
    /// Target margin for vertical (in screen-space units)
    pub target_margin_y: f32,
    /// Minimum normalized x coordinate in screen space
    pub min_norm_x:      f32,
    /// Maximum normalized x coordinate in screen space
    pub max_norm_x:      f32,
    /// Minimum normalized y coordinate in screen space
    pub min_norm_y:      f32,
    /// Maximum normalized y coordinate in screen space
    pub max_norm_y:      f32,
    /// Average depth of boundary corners from camera
    pub avg_depth:       f32,
    /// Half tangent of vertical field of view
    pub half_tan_vfov:   f32,
    /// Half tangent of horizontal field of view (vfov * aspect_ratio)
    pub half_tan_hfov:   f32,
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

        info!(
            "Screen space: aspect={:.3} vfov={:.1}° half_tan_v={:.3} half_tan_h={:.3}",
            viewport_aspect,
            perspective.fov.to_degrees(),
            half_tan_vfov,
            half_tan_hfov
        );

        // Get camera basis vectors from global transform
        let cam_pos = cam_global.translation();
        let cam_rot = cam_global.rotation();
        let cam_forward = cam_rot * Vec3::NEG_Z;
        let cam_right = cam_rot * Vec3::X;
        let cam_up = cam_rot * Vec3::Y;

        info!(
            "Camera basis: right={:.3?} up={:.3?} forward={:.3?}",
            cam_right, cam_up, cam_forward
        );

        // Project corners to screen space
        let mut min_norm_x = f32::INFINITY;
        let mut max_norm_x = f32::NEG_INFINITY;
        let mut min_norm_y = f32::INFINITY;
        let mut max_norm_y = f32::NEG_INFINITY;
        let mut avg_depth = 0.0;

        for (i, corner) in corners.iter().enumerate() {
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

            // Log ALL corners for the first call only
            if i == 0 {
                info!("=== ALL 8 CORNERS PROJECTION ===");
            }
            if i < 8 {
                info!(
                    "Corner[{}]: world=({:.0},{:.0},{:.0}) → screen_x={:.1} screen_y={:.1} depth={:.1} → norm=({:.3},{:.3})",
                    i, corner.x, corner.y, corner.z, x, y, depth, norm_x, norm_y
                );
            }

            min_norm_x = min_norm_x.min(norm_x);
            max_norm_x = max_norm_x.max(norm_x);
            min_norm_y = min_norm_y.min(norm_y);
            max_norm_y = max_norm_y.max(norm_y);
            avg_depth += depth;
        }
        avg_depth /= 8.0;

        // Determine which dimension SHOULD constrain based on aspect ratios
        let boundary_aspect = (max_norm_x - min_norm_x) / (max_norm_y - min_norm_y);
        let screen_aspect = half_tan_hfov / half_tan_vfov;

        // If boundary is wider (relative to height) than screen, width constrains
        // If boundary is taller (relative to width) than screen, height constrains
        let width_constrains = boundary_aspect > screen_aspect;

        info!(
            "Aspect ratios: boundary={:.3} screen={:.3} → {} constrains",
            boundary_aspect,
            screen_aspect,
            if width_constrains {
                "WIDTH (horizontal)"
            } else {
                "HEIGHT (vertical)"
            }
        );

        // Calculate target edge for the constraining dimension only
        let (target_edge_x, target_edge_y) = if width_constrains {
            // Width constrains - set horizontal target, vertical gets extra space
            let target_x = half_tan_hfov / zoom_multiplier;
            // Vertical target is at the boundary's aspect ratio from horizontal
            let target_y = target_x / boundary_aspect;
            (target_x, target_y)
        } else {
            // Height constrains - set vertical target, horizontal gets extra space
            let target_y = half_tan_vfov / zoom_multiplier;
            // Horizontal target is at the boundary's aspect ratio from vertical
            let target_x = target_y * boundary_aspect;
            (target_x, target_y)
        };

        // Calculate margins as distance from bounds to screen edges
        let left_margin = min_norm_x - (-half_tan_hfov);
        let right_margin = half_tan_hfov - max_norm_x;
        let bottom_margin = min_norm_y - (-half_tan_vfov);
        let top_margin = half_tan_vfov - max_norm_y;

        // Target margins are the difference between screen edge and target edge
        let target_margin_x = half_tan_hfov - target_edge_x;
        let target_margin_y = half_tan_vfov - target_edge_y;

        // Calculate which dimension constrains
        let h_min = left_margin.min(right_margin);
        let v_min = top_margin.min(bottom_margin);
        let (constraining_dim, constraining_margin, target_for_constraining) = if h_min < v_min {
            ("HORIZONTAL", h_min, target_margin_x)
        } else {
            ("VERTICAL", v_min, target_margin_y)
        };

        info!(
            "Box extents: norm_x=[{:.3}, {:.3}] norm_y=[{:.3}, {:.3}]",
            min_norm_x, max_norm_x, min_norm_y, max_norm_y
        );
        info!(
            "Margins: L={:.3} R={:.3} T={:.3} B={:.3}",
            left_margin, right_margin, top_margin, bottom_margin
        );
        info!(
            "Targets: horiz={:.3} vert={:.3}",
            target_margin_x, target_margin_y
        );
        info!(
            "CONSTRAINING DIMENSION: {} (margin={:.3} target={:.3})",
            constraining_dim, constraining_margin, target_for_constraining
        );

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

/// System that animates zoom-to-fit by lerping to pre-calculated target
pub fn zoom_to_fit_animation_system(
    mut commands: Commands,
    time: Res<Time>,
    mut camera_query: Query<(Entity, &mut PanOrbitCamera, &mut ZoomToFitAnimation)>,
) {
    for (entity, mut camera, mut anim) in &mut camera_query {
        // Update elapsed time (convert seconds to milliseconds)
        anim.elapsed_ms += time.delta_secs() * 1000.0;

        // Calculate lerp fraction (0.0 to 1.0)
        let t = (anim.elapsed_ms / anim.duration_ms).min(1.0);

        // Lerp both focus and radius toward target
        camera.target_focus = anim.start_focus.lerp(anim.target_focus, t);
        camera.target_radius = anim.start_radius.lerp(anim.target_radius, t);
        camera.force_update = true;

        // Animation complete?
        if t >= 1.0 {
            info!("Zoom-to-fit animation complete");

            // Synchronize current values with target values
            camera.focus = camera.target_focus;
            camera.radius = Some(camera.target_radius);
            camera.yaw = Some(camera.target_yaw);
            camera.pitch = Some(camera.target_pitch);

            // Remove animation component (triggers smoothness restore)
            commands.entity(entity).remove::<ZoomToFitAnimation>();
        }
    }
}

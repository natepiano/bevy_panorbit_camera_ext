//! Fit algorithm for framing objects in the camera view.
//!
//! Provides screen-space projection, margin calculation, and a binary search convergence
//! loop that finds the optimal camera radius and focus to frame a set of mesh vertices
//! with a specified margin.

use bevy::prelude::*;

use crate::support::ProjectionParams;
use crate::support::project_point;
use crate::support::projection_aspect_ratio;

// ============================================================================
// Constants
// ============================================================================

pub const MAX_ITERATIONS: usize = 200;
pub const TOLERANCE: f32 = 0.001; // 0.1% tolerance for convergence
pub const CENTERING_MAX_ITERATIONS: usize = 10;
pub const CENTERING_TOLERANCE: f32 = 0.0001; // normalized screen-space center offset
/// Returns the zoom margin multiplier (1.0 / (1.0 - margin))
/// For example, a margin of 0.08 returns 1.087 (8% margin)
pub const fn zoom_margin_multiplier(margin: f32) -> f32 { 1.0 / (1.0 - margin) }

// ============================================================================
// Types
// ============================================================================

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
    pub left_margin:       f32,
    /// Distance from right edge (positive = inside, negative = outside)
    pub right_margin:      f32,
    /// Distance from top edge (positive = inside, negative = outside)
    pub top_margin:        f32,
    /// Distance from bottom edge (positive = inside, negative = outside)
    pub bottom_margin:     f32,
    /// Target margin for horizontal (in screen-space units)
    pub target_margin_x:   f32,
    /// Target margin for vertical (in screen-space units)
    pub target_margin_y:   f32,
    /// Minimum normalized x coordinate in screen space
    pub min_norm_x:        f32,
    /// Maximum normalized x coordinate in screen space
    pub max_norm_x:        f32,
    /// Minimum normalized y coordinate in screen space
    pub min_norm_y:        f32,
    /// Maximum normalized y coordinate in screen space
    pub max_norm_y:        f32,
    /// Harmonic mean depth of the two corners defining horizontal extremes
    /// (1.0 for orthographic since projection is depth-independent)
    pub centering_depth_x: f32,
    /// Harmonic mean depth of the two corners defining vertical extremes
    /// (1.0 for orthographic since projection is depth-independent)
    pub centering_depth_y: f32,
    /// Half visible extent in x (perspective: half_tan_hfov, ortho: area.width()/2)
    pub half_extent_x:     f32,
    /// Half visible extent in y (perspective: half_tan_vfov, ortho: area.height()/2)
    pub half_extent_y:     f32,
}

impl ScreenSpaceBounds {
    /// Creates screen space bounds from a camera's view of a set of points.
    /// Returns `None` if any point is behind the camera (perspective only).
    ///
    /// Works with both perspective and orthographic projections:
    /// - **Perspective**: normalizes by depth (`x/depth`), uses harmonic mean for centering
    /// - **Orthographic**: uses raw camera-space coordinates (depth-independent), centering depth =
    ///   1.0
    pub fn from_points(
        points: &[Vec3],
        cam_global: &GlobalTransform,
        projection: &Projection,
        viewport_aspect: f32,
        zoom_multiplier: f32,
    ) -> Option<Self> {
        let ProjectionParams {
            half_extent_x,
            half_extent_y,
            is_ortho,
        } = ProjectionParams::from_projection(projection, viewport_aspect)?;

        // Get camera basis vectors from global transform
        let cam_pos = cam_global.translation();
        let cam_rot = cam_global.rotation();
        let cam_forward = cam_rot * Vec3::NEG_Z;
        let cam_right = cam_rot * Vec3::X;
        let cam_up = cam_rot * Vec3::Y;

        // Project points to normalized screen space
        let mut min_norm_x = f32::INFINITY;
        let mut max_norm_x = f32::NEG_INFINITY;
        let mut min_norm_y = f32::INFINITY;
        let mut max_norm_y = f32::NEG_INFINITY;
        let mut min_x_depth = 0.0_f32;
        let mut max_x_depth = 0.0_f32;
        let mut min_y_depth = 0.0_f32;
        let mut max_y_depth = 0.0_f32;

        for point in points {
            let (norm_x, norm_y, depth) =
                project_point(*point, cam_pos, cam_right, cam_up, cam_forward, is_ortho)?;

            if norm_x < min_norm_x {
                min_norm_x = norm_x;
                min_x_depth = depth;
            }
            if norm_x > max_norm_x {
                max_norm_x = norm_x;
                max_x_depth = depth;
            }
            if norm_y < min_norm_y {
                min_norm_y = norm_y;
                min_y_depth = depth;
            }
            if norm_y > max_norm_y {
                max_norm_y = norm_y;
                max_y_depth = depth;
            }
        }

        // Centering depths: perspective uses harmonic mean for perspective-correct
        // centering. Ortho uses 1.0 since projection is depth-independent.
        let (centering_depth_x, centering_depth_y) = if is_ortho {
            (1.0, 1.0)
        } else {
            (
                2.0 * min_x_depth * max_x_depth / (min_x_depth + max_x_depth),
                2.0 * min_y_depth * max_y_depth / (min_y_depth + max_y_depth),
            )
        };

        // Determine which dimension SHOULD constrain based on aspect ratios
        let boundary_aspect = (max_norm_x - min_norm_x) / (max_norm_y - min_norm_y);
        let screen_aspect = half_extent_x / half_extent_y;

        // If boundary is wider (relative to height) than screen, width constrains
        // If boundary is taller (relative to width) than screen, height constrains
        let width_constrains = boundary_aspect > screen_aspect;

        // Calculate target edge for the constraining dimension only
        let (target_edge_x, target_edge_y) = if width_constrains {
            // Width constrains - set horizontal target, vertical gets extra space
            let target_x = half_extent_x / zoom_multiplier;
            // Vertical target is at the boundary's aspect ratio from horizontal
            let target_y = target_x / boundary_aspect;
            (target_x, target_y)
        } else {
            // Height constrains - set vertical target, horizontal gets extra space
            let target_y = half_extent_y / zoom_multiplier;
            // Horizontal target is at the boundary's aspect ratio from vertical
            let target_x = target_y * boundary_aspect;
            (target_x, target_y)
        };

        // Calculate margins as distance from bounds to screen edges
        let left_margin = min_norm_x - (-half_extent_x);
        let right_margin = half_extent_x - max_norm_x;
        let bottom_margin = min_norm_y - (-half_extent_y);
        let top_margin = half_extent_y - max_norm_y;

        // Target margins are the difference between screen edge and target edge
        let target_margin_x = half_extent_x - target_edge_x;
        let target_margin_y = half_extent_y - target_edge_y;

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
            centering_depth_x,
            centering_depth_y,
            half_extent_x,
            half_extent_y,
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

// ============================================================================
// Convergence algorithm
// ============================================================================

/// Calculates the optimal radius and centered focus to fit pre-extracted vertices in the camera
/// view. The focus is adjusted so the projected mesh silhouette is centered in the viewport.
///
/// For each candidate radius, computes the focus that centers the projected silhouette in the
/// viewport (since the geometric center doesn't project to screen center from off-axis angles),
/// then evaluates margins at that centered position. Returns the `(radius, focus)` pair where
/// the constraining margin equals the target and the silhouette is centered.
///
/// Note: A lateral camera shift doesn't change point depths, so the centering is geometrically
/// exact for the constraining margin check.
pub fn calculate_fit(
    points: &[Vec3],
    geometric_center: Vec3,
    yaw: f32,
    pitch: f32,
    margin: f32,
    projection: &Projection,
    camera: &Camera,
) -> Option<(f32, Vec3)> {
    let aspect_ratio = projection_aspect_ratio(projection, camera.logical_viewport_size())?;

    // For ortho, the camera is always at a fixed distance from focus.
    // PanOrbitCamera sets this to `(near + far) / 2.0`.
    let ortho_fixed_distance = match projection {
        Projection::Orthographic(o) => Some((o.near + o.far) * 0.5),
        _ => None,
    };

    let zoom_multiplier = zoom_margin_multiplier(margin);

    let rot = Quat::from_euler(EulerRot::YXZ, yaw, -pitch, 0.0);

    // Compute the object's bounding sphere radius from points for sensible search bounds.
    // The search range is based purely on object size to ensure deterministic results
    // regardless of the camera's current radius.
    let object_radius = points
        .iter()
        .map(|c| (*c - geometric_center).length())
        .fold(0.0_f32, f32::max);

    // Binary search for the correct radius.
    // For perspective: radius = camera distance (changes apparent size).
    // For ortho: PanOrbitCamera maps radius → `OrthographicProjection::scale`,
    //   so searching over radius effectively searches over scale.
    let mut min_radius = object_radius * 0.1;
    let mut max_radius = object_radius * 100.0;
    let mut best_radius = object_radius * 2.0;
    let mut best_focus = geometric_center;
    let mut best_error = f32::INFINITY;

    debug!("Binary search starting: range [{min_radius:.1}, {max_radius:.1}]");

    for iteration in 0..MAX_ITERATIONS {
        let test_radius = (min_radius + max_radius) * 0.5;

        // Build the projection to use for this iteration.
        // For ortho, we need to compute what `area` would be at this test scale.
        let test_projection = build_test_projection(projection, test_radius);

        // Step 1: find the centered focus using accurate depth-based centering
        let centered_focus = refine_focus_centering(
            points,
            geometric_center,
            test_radius,
            rot,
            &test_projection,
            aspect_ratio,
            ortho_fixed_distance,
        );

        // Step 2: evaluate margins at the centered focus position.
        // For ortho, the camera distance is fixed regardless of test_radius.
        let cam_distance = ortho_fixed_distance.unwrap_or(test_radius);
        let cam_pos = centered_focus + rot * Vec3::new(0.0, 0.0, cam_distance);
        let cam_global =
            GlobalTransform::from(Transform::from_translation(cam_pos).with_rotation(rot));

        let Some(bounds) = ScreenSpaceBounds::from_points(
            points,
            &cam_global,
            &test_projection,
            aspect_ratio,
            zoom_multiplier,
        ) else {
            warn!(
                "Iteration {iteration}: Points behind camera at radius {test_radius:.1}, searching higher"
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

        debug!(
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
            debug!(
                "Iteration {iteration}: Converged to best radius {best_radius:.3} error={best_error:.5}"
            );
            return Some((best_radius, best_focus));
        }
    }

    warn!(
        "Binary search did not converge in {MAX_ITERATIONS} iterations. Using best radius {best_radius:.1}"
    );

    Some((best_radius, best_focus))
}

/// Builds a test projection with the given radius/scale for binary search iterations.
///
/// For perspective, returns the original projection unchanged.
/// For orthographic, creates a modified projection with `area` recomputed for the test scale,
/// since `PanOrbitCamera` maps `radius` → `OrthographicProjection::scale`.
fn build_test_projection(projection: &Projection, test_radius: f32) -> Projection {
    match projection {
        Projection::Perspective(_) => projection.clone(),
        Projection::Orthographic(ortho) => {
            // Compute what the area would be at this scale.
            // The current area is `base_size * current_scale`, so base_size = area / scale.
            // At test scale: new_area = base_size * test_radius.
            let current_scale = ortho.scale;
            let scale_ratio = if current_scale.abs() > f32::EPSILON {
                test_radius / current_scale
            } else {
                1.0
            };
            let new_area = Rect::new(
                ortho.area.min.x * scale_ratio,
                ortho.area.min.y * scale_ratio,
                ortho.area.max.x * scale_ratio,
                ortho.area.max.y * scale_ratio,
            );
            Projection::Orthographic(OrthographicProjection {
                scale: test_radius,
                area: new_area,
                ..*ortho
            })
        },
        _ => projection.clone(),
    }
}

/// Shifts the camera focus so the projected bounding box is centered on screen.
///
/// For perspective, each correction step uses the harmonic mean of the depths of the two
/// extreme points per dimension. This is the exact inverse of perspective projection: when
/// the camera shifts laterally by `delta`, a point at depth `d` shifts by `-delta/d` in
/// normalized screen space, so the screen-space center of two points at depths `d1` and `d2`
/// shifts by `-delta * (1/d1 + 1/d2) / 2`. The harmonic mean `2*d1*d2/(d1+d2)` inverts this
/// exactly. Convergence typically takes 1-2 iterations.
///
/// For orthographic, centering is depth-independent (centering_depth = 1.0), so the shift
/// is a direct 1:1 world-unit correction.
fn refine_focus_centering(
    points: &[Vec3],
    initial_focus: Vec3,
    radius: f32,
    rot: Quat,
    projection: &Projection,
    aspect_ratio: f32,
    ortho_fixed_distance: Option<f32>,
) -> Vec3 {
    let cam_right = rot * Vec3::X;
    let cam_up = rot * Vec3::Y;

    let cam_distance = ortho_fixed_distance.unwrap_or(radius);

    let mut focus = initial_focus;
    for _ in 0..CENTERING_MAX_ITERATIONS {
        let cam_pos = focus + rot * Vec3::new(0.0, 0.0, cam_distance);
        let cam_global =
            GlobalTransform::from(Transform::from_translation(cam_pos).with_rotation(rot));
        let Some(bounds) =
            ScreenSpaceBounds::from_points(points, &cam_global, projection, aspect_ratio, 1.0)
        else {
            break;
        };
        let (cx, cy) = bounds.center();
        if cx.abs() < CENTERING_TOLERANCE && cy.abs() < CENTERING_TOLERANCE {
            break;
        }
        focus += cam_right * cx * bounds.centering_depth_x + cam_up * cy * bounds.centering_depth_y;
    }
    focus
}

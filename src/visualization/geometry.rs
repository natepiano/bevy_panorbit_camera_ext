use bevy::prelude::*;

use super::types::FitTargetGizmo;
use super::types::FitTargetVisualizationConfig;
use crate::fit::Edge;
use crate::support::project_point;
use crate::support::CameraBasis;
use crate::support::ScreenSpaceBounds;

/// Returns true if horizontal margins are balanced.
pub fn is_horizontally_balanced(bounds: &ScreenSpaceBounds, tolerance: f32) -> bool {
    (bounds.left_margin - bounds.right_margin).abs() < tolerance
}

/// Returns true if vertical margins are balanced.
pub fn is_vertically_balanced(bounds: &ScreenSpaceBounds, tolerance: f32) -> bool {
    (bounds.top_margin - bounds.bottom_margin).abs() < tolerance
}

/// Returns the screen edges in normalized space: (left, right, top, bottom).
fn screen_edges_normalized(bounds: &ScreenSpaceBounds) -> (f32, f32, f32, f32) {
    (
        -bounds.half_extent_x,
        bounds.half_extent_x,
        bounds.half_extent_y,
        -bounds.half_extent_y,
    )
}

/// Returns the center of a boundary edge in normalized space.
pub fn boundary_edge_center(bounds: &ScreenSpaceBounds, edge: Edge) -> Option<(f32, f32)> {
    let (left_edge, right_edge, top_edge, bottom_edge) = screen_edges_normalized(bounds);

    match edge {
        Edge::Left if bounds.min_norm_x > left_edge => {
            let y = (bounds.min_norm_y.max(bottom_edge) + bounds.max_norm_y.min(top_edge)) * 0.5;
            Some((bounds.min_norm_x, y))
        },
        Edge::Right if bounds.max_norm_x < right_edge => {
            let y = (bounds.min_norm_y.max(bottom_edge) + bounds.max_norm_y.min(top_edge)) * 0.5;
            Some((bounds.max_norm_x, y))
        },
        Edge::Top if bounds.max_norm_y < top_edge => {
            let x = (bounds.min_norm_x.max(left_edge) + bounds.max_norm_x.min(right_edge)) * 0.5;
            Some((x, bounds.max_norm_y))
        },
        Edge::Bottom if bounds.min_norm_y > bottom_edge => {
            let x = (bounds.min_norm_x.max(left_edge) + bounds.max_norm_x.min(right_edge)) * 0.5;
            Some((x, bounds.min_norm_y))
        },
        _ => None,
    }
}

/// Returns the center of a screen edge in normalized space.
pub fn screen_edge_center(bounds: &ScreenSpaceBounds, edge: Edge) -> (f32, f32) {
    let (left_edge, right_edge, top_edge, bottom_edge) = screen_edges_normalized(bounds);

    match edge {
        Edge::Left => {
            let y = (bounds.min_norm_y.max(bottom_edge) + bounds.max_norm_y.min(top_edge)) * 0.5;
            (left_edge, y)
        },
        Edge::Right => {
            let y = (bounds.min_norm_y.max(bottom_edge) + bounds.max_norm_y.min(top_edge)) * 0.5;
            (right_edge, y)
        },
        Edge::Top => {
            let x = (bounds.min_norm_x.max(left_edge) + bounds.max_norm_x.min(right_edge)) * 0.5;
            (x, top_edge)
        },
        Edge::Bottom => {
            let x = (bounds.min_norm_x.max(left_edge) + bounds.max_norm_x.min(right_edge)) * 0.5;
            (x, bottom_edge)
        },
    }
}

/// Converts normalized screen-space coordinates to world space.
///
/// For perspective, reverses the perspective divide by multiplying by `avg_depth`.
/// For orthographic, coordinates are already in world units — `avg_depth` is only
/// used for the forward component to position the gizmo plane.
pub fn normalized_to_world(
    norm_x: f32,
    norm_y: f32,
    cam: &CameraBasis,
    avg_depth: f32,
    is_ortho: bool,
) -> Vec3 {
    let (world_x, world_y) = if is_ortho {
        (norm_x, norm_y)
    } else {
        (norm_x * avg_depth, norm_y * avg_depth)
    };
    cam.pos + cam.right * world_x + cam.up * world_y + cam.forward * avg_depth
}

/// Returns the margin percentage for a given edge.
/// Percentage represents how much of the screen width/height is margin.
pub fn margin_percentage(bounds: &ScreenSpaceBounds, edge: Edge) -> f32 {
    let screen_width = 2.0 * bounds.half_extent_x;
    let screen_height = 2.0 * bounds.half_extent_y;

    match edge {
        Edge::Left => (bounds.left_margin / screen_width) * 100.0,
        Edge::Right => (bounds.right_margin / screen_width) * 100.0,
        Edge::Top => (bounds.top_margin / screen_height) * 100.0,
        Edge::Bottom => (bounds.bottom_margin / screen_height) * 100.0,
    }
}

/// 2D cross product for three points (for convex hull turn detection).
fn cross_2d(o: (f32, f32), a: (f32, f32), b: (f32, f32)) -> f32 {
    (a.0 - o.0) * (b.1 - o.1) - (a.1 - o.1) * (b.0 - o.0)
}

/// Andrew's monotone chain algorithm for 2D convex hull.
/// Returns hull vertices in counter-clockwise order.
pub fn convex_hull_2d(points: &[(f32, f32)]) -> Vec<(f32, f32)> {
    let mut sorted: Vec<(f32, f32)> = points.to_vec();
    sorted.sort_by(|a, b| {
        a.0.partial_cmp(&b.0)
            .unwrap()
            .then(a.1.partial_cmp(&b.1).unwrap())
    });
    sorted.dedup();

    if sorted.len() <= 1 {
        return sorted;
    }

    let mut lower: Vec<(f32, f32)> = Vec::new();
    for &p in &sorted {
        while lower.len() >= 2 && cross_2d(lower[lower.len() - 2], lower[lower.len() - 1], p) <= 0.0
        {
            lower.pop();
        }
        lower.push(p);
    }

    let mut upper: Vec<(f32, f32)> = Vec::new();
    for &p in sorted.iter().rev() {
        while upper.len() >= 2 && cross_2d(upper[upper.len() - 2], upper[upper.len() - 1], p) <= 0.0
        {
            upper.pop();
        }
        upper.push(p);
    }

    lower.pop();
    upper.pop();

    lower.extend(upper);
    lower
}

/// Projects world-space vertices to 2D normalized screen space.
///
/// For perspective, divides by depth. For orthographic, uses raw camera-space coordinates.
pub fn project_vertices_to_2d(
    vertices: &[Vec3],
    cam: &CameraBasis,
    is_ortho: bool,
) -> Vec<(f32, f32)> {
    vertices
        .iter()
        .filter_map(|v| {
            let (norm_x, norm_y, _) = project_point(*v, cam, is_ortho)?;
            Some((norm_x, norm_y))
        })
        .collect()
}

/// Draws the silhouette polygon (convex hull of projected vertices) using gizmo lines.
pub fn draw_silhouette_polygon(
    gizmos: &mut Gizmos<FitTargetGizmo>,
    hull_points: &[(f32, f32)],
    cam: &CameraBasis,
    avg_depth: f32,
    is_ortho: bool,
    color: Color,
) {
    if hull_points.len() < 2 {
        return;
    }

    for i in 0..hull_points.len() {
        let next = (i + 1) % hull_points.len();
        let start =
            normalized_to_world(hull_points[i].0, hull_points[i].1, cam, avg_depth, is_ortho);
        let end = normalized_to_world(
            hull_points[next].0,
            hull_points[next].1,
            cam,
            avg_depth,
            is_ortho,
        );
        gizmos.line(start, end, color);
    }
}

/// Calculates the color for an edge based on balance state.
pub const fn calculate_edge_color(
    edge: Edge,
    h_balanced: bool,
    v_balanced: bool,
    config: &FitTargetVisualizationConfig,
) -> Color {
    match edge {
        Edge::Left | Edge::Right => {
            if h_balanced {
                config.balanced_color
            } else {
                config.unbalanced_color
            }
        },
        Edge::Top | Edge::Bottom => {
            if v_balanced {
                config.balanced_color
            } else {
                config.unbalanced_color
            }
        },
    }
}

/// Creates the 4 corners of the screen-aligned boundary rectangle in world space.
pub fn create_screen_corners(
    bounds: &ScreenSpaceBounds,
    cam: &CameraBasis,
    avg_depth: f32,
    is_ortho: bool,
) -> [Vec3; 4] {
    [
        normalized_to_world(
            bounds.min_norm_x,
            bounds.min_norm_y,
            cam,
            avg_depth,
            is_ortho,
        ),
        normalized_to_world(
            bounds.max_norm_x,
            bounds.min_norm_y,
            cam,
            avg_depth,
            is_ortho,
        ),
        normalized_to_world(
            bounds.max_norm_x,
            bounds.max_norm_y,
            cam,
            avg_depth,
            is_ortho,
        ),
        normalized_to_world(
            bounds.min_norm_x,
            bounds.max_norm_y,
            cam,
            avg_depth,
            is_ortho,
        ),
    ]
}

/// Draws the boundary rectangle outline.
pub fn draw_rectangle(
    gizmos: &mut Gizmos<FitTargetGizmo>,
    corners: &[Vec3; 4],
    config: &FitTargetVisualizationConfig,
) {
    for i in 0..4 {
        let next = (i + 1) % 4;
        gizmos.line(corners[i], corners[next], config.rectangle_color);
    }
}

/// Converts a normalized screen-space coordinate to viewport pixels.
pub fn norm_to_viewport(
    norm_x: f32,
    norm_y: f32,
    half_extent_x: f32,
    half_extent_y: f32,
    viewport_size: Vec2,
) -> Vec2 {
    Vec2::new(
        (norm_x / half_extent_x + 1.0) * 0.5 * viewport_size.x,
        (1.0 - norm_y / half_extent_y) * 0.5 * viewport_size.y,
    )
}

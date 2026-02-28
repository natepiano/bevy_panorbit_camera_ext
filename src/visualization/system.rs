use bevy::prelude::*;
use bevy_panorbit_camera::PanOrbitCamera;

use super::convex_hull::convex_hull_2d;
use super::convex_hull::project_vertices_to_2d;
use super::labels::BoundsLabel;
use super::labels::MarginLabel;
use super::labels::bounds_label_position;
use super::labels::calculate_label_pixel_position;
use super::labels::update_or_create_bounds_label;
use super::labels::update_or_create_margin_label;
use super::screen_space::boundary_edge_center;
use super::screen_space::is_horizontally_balanced;
use super::screen_space::is_vertically_balanced;
use super::screen_space::margin_percentage;
use super::screen_space::norm_to_viewport;
use super::screen_space::normalized_to_world;
use super::screen_space::screen_edge_center;
use super::types::FitTargetGizmo;
use super::types::FitTargetViewportMargins;
use super::types::FitTargetVisualizationConfig;
use crate::components::CurrentFitTarget;
use crate::fit::Edge;
use crate::support::CameraBasis;
use crate::support::ScreenSpaceBounds;
use crate::support::extract_mesh_vertices;
use crate::support::projection_aspect_ratio;

/// Calculates the color for an edge based on balance state.
const fn calculate_edge_color(
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
fn create_screen_corners(
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
fn draw_rectangle(
    gizmos: &mut Gizmos<FitTargetGizmo>,
    corners: &[Vec3; 4],
    config: &FitTargetVisualizationConfig,
) {
    for i in 0..4 {
        let next = (i + 1) % 4;
        gizmos.line(corners[i], corners[next], config.rectangle_color);
    }
}

/// Draws the silhouette polygon (convex hull of projected vertices) using gizmo lines.
fn draw_silhouette(
    gizmos: &mut Gizmos<FitTargetGizmo>,
    vertices: &[Vec3],
    cam: &CameraBasis,
    avg_depth: f32,
    is_ortho: bool,
    color: Color,
) {
    let projected = project_vertices_to_2d(vertices, cam, is_ortho);
    let hull = convex_hull_2d(&projected);

    if hull.len() < 2 {
        return;
    }

    for i in 0..hull.len() {
        let next = (i + 1) % hull.len();
        let start = normalized_to_world(hull[i].0, hull[i].1, cam, avg_depth, is_ortho);
        let end = normalized_to_world(hull[next].0, hull[next].1, cam, avg_depth, is_ortho);
        gizmos.line(start, end, color);
    }
}

/// Draws margin lines from boundary edges to screen edges and updates margin labels.
/// Returns the set of edges that had visible margins.
#[allow(clippy::too_many_arguments)]
fn draw_margin_lines_and_labels(
    commands: &mut Commands,
    gizmos: &mut Gizmos<FitTargetGizmo>,
    label_query: &mut Query<(Entity, &MarginLabel, &mut Text, &mut Node, &mut TextColor)>,
    bounds: &ScreenSpaceBounds,
    cam_basis: &CameraBasis,
    avg_depth: f32,
    is_ortho: bool,
    config: &FitTargetVisualizationConfig,
    visualization_enabled: bool,
    viewport_size: Option<Vec2>,
) -> Vec<Edge> {
    let h_balanced = is_horizontally_balanced(bounds, crate::fit::TOLERANCE);
    let v_balanced = is_vertically_balanced(bounds, crate::fit::TOLERANCE);

    let mut visible_edges: Vec<Edge> = Vec::new();

    for edge in [Edge::Left, Edge::Right, Edge::Top, Edge::Bottom] {
        let Some((boundary_x, boundary_y)) = boundary_edge_center(bounds, edge) else {
            continue;
        };
        visible_edges.push(edge);

        let (screen_x, screen_y) = screen_edge_center(bounds, edge);
        let boundary_pos =
            normalized_to_world(boundary_x, boundary_y, cam_basis, avg_depth, is_ortho);
        let screen_pos = normalized_to_world(screen_x, screen_y, cam_basis, avg_depth, is_ortho);

        let color = calculate_edge_color(edge, h_balanced, v_balanced, config);
        gizmos.line(boundary_pos, screen_pos, color);

        if visualization_enabled {
            let Some(vp) = viewport_size else {
                continue;
            };
            let percentage = margin_percentage(bounds, edge);
            let text = format!("margin: {percentage:.3}%");
            let label_screen_pos = calculate_label_pixel_position(edge, bounds, vp);

            update_or_create_margin_label(
                commands,
                label_query,
                edge,
                text,
                color,
                label_screen_pos,
                vp,
            );
        }
    }

    visible_edges
}

/// Removes labels for edges no longer visible or when visualization is disabled.
#[allow(clippy::type_complexity)]
fn cleanup_stale_labels(
    commands: &mut Commands,
    label_query: &Query<(Entity, &MarginLabel, &mut Text, &mut Node, &mut TextColor)>,
    bounds_label_query: &Query<(Entity, &mut Node), (With<BoundsLabel>, Without<MarginLabel>)>,
    visible_edges: &[Edge],
    visualization_enabled: bool,
) {
    for (entity, label, _, _, _) in label_query {
        if !visualization_enabled || !visible_edges.contains(&label.edge) {
            commands.entity(entity).despawn();
        }
    }

    if !visualization_enabled {
        for (entity, _) in bounds_label_query {
            commands.entity(entity).despawn();
        }
    }
}

/// Draws screen-aligned bounds for the current fit target.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn draw_fit_target_bounds(
    mut commands: Commands,
    mut gizmos: Gizmos<FitTargetGizmo>,
    config: Res<FitTargetVisualizationConfig>,
    config_store: Res<GizmoConfigStore>,
    camera_query: Query<
        (
            Entity,
            &Camera,
            &GlobalTransform,
            &Projection,
            &CurrentFitTarget,
        ),
        With<PanOrbitCamera>,
    >,
    mesh_query: Query<&Mesh3d>,
    children_query: Query<&Children>,
    global_transform_query: Query<&GlobalTransform>,
    meshes: Res<Assets<Mesh>>,
    mut label_query: Query<(Entity, &MarginLabel, &mut Text, &mut Node, &mut TextColor)>,
    mut bounds_label_query: Query<(Entity, &mut Node), (With<BoundsLabel>, Without<MarginLabel>)>,
) {
    let Ok((camera_entity, cam, cam_global, projection, current_target)) = camera_query.single()
    else {
        return;
    };

    let (gizmo_config, _) = config_store.config::<FitTargetGizmo>();
    let visualization_enabled = gizmo_config.enabled;

    let Some((vertices, _)) = extract_mesh_vertices(
        current_target.0,
        &children_query,
        &mesh_query,
        &global_transform_query,
        &meshes,
    ) else {
        return;
    };

    let cam_basis = CameraBasis::from_global_transform(cam_global);

    let Some(aspect_ratio) = projection_aspect_ratio(projection, cam.logical_viewport_size())
    else {
        return;
    };

    let Some((bounds, depths)) =
        ScreenSpaceBounds::from_points(&vertices, cam_global, projection, aspect_ratio)
    else {
        return;
    };

    let avg_depth = depths.avg_depth();
    let is_ortho = matches!(projection, Projection::Orthographic(_));
    let viewport_size = cam.logical_viewport_size();

    // Update margin percentages on camera entity for BRP inspection
    commands
        .entity(camera_entity)
        .insert(FitTargetViewportMargins::from_bounds(&bounds));

    // Bounding rectangle
    let corners = create_screen_corners(&bounds, &cam_basis, avg_depth, is_ortho);
    draw_rectangle(&mut gizmos, &corners, &config);

    // Silhouette convex hull
    if visualization_enabled {
        draw_silhouette(
            &mut gizmos,
            &vertices,
            &cam_basis,
            avg_depth,
            is_ortho,
            config.silhouette_color,
        );
    }

    // "Screen space bounds" label
    if visualization_enabled && let Some(vp) = viewport_size {
        let upper_left = norm_to_viewport(
            bounds.min_norm_x,
            bounds.max_norm_y,
            bounds.half_extent_x,
            bounds.half_extent_y,
            vp,
        );
        update_or_create_bounds_label(
            &mut commands,
            &mut bounds_label_query,
            bounds_label_position(upper_left),
        );
    }

    // Margin lines + labels
    let visible_edges = draw_margin_lines_and_labels(
        &mut commands,
        &mut gizmos,
        &mut label_query,
        &bounds,
        &cam_basis,
        avg_depth,
        is_ortho,
        &config,
        visualization_enabled,
        viewport_size,
    );

    // Remove stale labels
    cleanup_stale_labels(
        &mut commands,
        &label_query,
        &bounds_label_query,
        &visible_edges,
        visualization_enabled,
    );
}

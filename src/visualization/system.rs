use bevy::prelude::*;
use bevy_panorbit_camera::PanOrbitCamera;

use super::edge_layout::bounds_label_position;
use super::edge_layout::calculate_label_pixel_position;
use super::geometry::boundary_edge_center;
use super::geometry::calculate_edge_color;
use super::geometry::convex_hull_2d;
use super::geometry::create_screen_corners;
use super::geometry::draw_rectangle;
use super::geometry::draw_silhouette_polygon;
use super::geometry::is_horizontally_balanced;
use super::geometry::is_vertically_balanced;
use super::geometry::margin_percentage;
use super::geometry::norm_to_viewport;
use super::geometry::normalized_to_world;
use super::geometry::project_vertices_to_2d;
use super::geometry::screen_edge_center;
use super::labels::BoundsLabel;
use super::labels::MarginLabel;
use super::labels::update_or_create_bounds_label;
use super::labels::update_or_create_margin_label;
use super::types::FitTargetGizmo;
use super::types::FitTargetMargins;
use super::types::FitTargetVisualizationConfig;
use crate::components::CurrentFitTarget;
use crate::fit::Edge;
use crate::support::CameraBasis;
use crate::support::ScreenSpaceBounds;
use crate::support::extract_mesh_vertices;
use crate::support::projection_aspect_ratio;

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

    commands.entity(camera_entity).insert(FitTargetMargins {
        left_pct:   margin_percentage(&bounds, Edge::Left),
        right_pct:  margin_percentage(&bounds, Edge::Right),
        top_pct:    margin_percentage(&bounds, Edge::Top),
        bottom_pct: margin_percentage(&bounds, Edge::Bottom),
    });

    let rect_corners_world = create_screen_corners(&bounds, &cam_basis, avg_depth, is_ortho);
    draw_rectangle(&mut gizmos, &rect_corners_world, &config);

    if visualization_enabled {
        let projected = project_vertices_to_2d(&vertices, &cam_basis, is_ortho);
        let hull = convex_hull_2d(&projected);
        draw_silhouette_polygon(
            &mut gizmos,
            &hull,
            &cam_basis,
            avg_depth,
            is_ortho,
            config.silhouette_color,
        );
    }

    if visualization_enabled && let Some(viewport_size) = cam.logical_viewport_size() {
        let upper_left = norm_to_viewport(
            bounds.min_norm_x,
            bounds.max_norm_y,
            bounds.half_extent_x,
            bounds.half_extent_y,
            viewport_size,
        );
        update_or_create_bounds_label(
            &mut commands,
            &mut bounds_label_query,
            bounds_label_position(upper_left),
        );
    }

    let h_balanced = is_horizontally_balanced(&bounds, crate::fit::TOLERANCE);
    let v_balanced = is_vertically_balanced(&bounds, crate::fit::TOLERANCE);

    let mut visible_edges: Vec<Edge> = Vec::new();

    for edge in [Edge::Left, Edge::Right, Edge::Top, Edge::Bottom] {
        if let Some((boundary_x, boundary_y)) = boundary_edge_center(&bounds, edge) {
            visible_edges.push(edge);

            let (screen_x, screen_y) = screen_edge_center(&bounds, edge);

            let boundary_pos =
                normalized_to_world(boundary_x, boundary_y, &cam_basis, avg_depth, is_ortho);
            let screen_pos =
                normalized_to_world(screen_x, screen_y, &cam_basis, avg_depth, is_ortho);

            let color = calculate_edge_color(edge, h_balanced, v_balanced, &config);
            gizmos.line(boundary_pos, screen_pos, color);

            if visualization_enabled {
                let percentage = margin_percentage(&bounds, edge);
                let text = format!("{percentage:.3}%");

                let Some(viewport_size) = cam.logical_viewport_size() else {
                    continue;
                };
                let label_screen_pos = calculate_label_pixel_position(edge, &bounds, viewport_size);

                update_or_create_margin_label(
                    &mut commands,
                    &mut label_query,
                    edge,
                    text,
                    color,
                    label_screen_pos,
                    viewport_size,
                );
            }
        }
    }

    for (entity, label, _, _, _) in &label_query {
        if !visualization_enabled || !visible_edges.contains(&label.edge) {
            commands.entity(entity).despawn();
        }
    }

    if !visualization_enabled {
        for (entity, _) in &bounds_label_query {
            commands.entity(entity).despawn();
        }
    }
}

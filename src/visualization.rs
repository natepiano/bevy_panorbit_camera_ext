//! Visualization system for fit target debugging
//!
//! Provides screen-aligned boundary box visualization for the current camera fit target.
//! Uses Bevy's GizmoConfigGroup pattern (similar to Avian3D's PhysicsGizmos).

use bevy::camera::primitives::Aabb;
use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use bevy_panorbit_camera::PanOrbitCamera;

use crate::extension::CurrentFitTarget;
use crate::extension::aabb_to_world_corners;
use crate::extension::find_descendant_aabb;

/// Gizmo config group for fit target visualization.
/// Toggle via `GizmoConfigStore::config_mut::<FitTargetGizmo>().enabled`
#[derive(Default, Reflect, GizmoConfigGroup)]
pub struct FitTargetGizmo {}

/// Current screen-space margin percentages for the fit target.
/// Updated every frame by the visualization system.
/// Removed when fit target visualization is disabled.
#[derive(Component, Reflect, Debug, Default, Clone)]
#[reflect(Component)]
pub struct FitTargetMargins {
    pub left_pct:   f32,
    pub right_pct:  f32,
    pub top_pct:    f32,
    pub bottom_pct: f32,
}

/// Component marking margin percentage labels
#[derive(Component, Reflect)]
#[reflect(Component)]
struct MarginLabel {
    edge: Edge,
}

// Constants for label positioning
const LABEL_FONT_SIZE: f32 = 14.0;
const LABEL_TEXT_OFFSET: f32 = 0.02; // Offset in normalized screen space
const LABEL_WORLD_POS_OFFSET: f32 = 0.5; // Offset toward camera to prevent occlusion

/// Configuration for fit target visualization colors and appearance
#[derive(Resource, Reflect, Debug, Clone)]
#[reflect(Resource)]
pub struct FitTargetVisualizationConfig {
    pub rectangle_color:  Color,
    pub balanced_color:   Color,
    pub unbalanced_color: Color,
    pub line_width:       f32,
}

impl Default for FitTargetVisualizationConfig {
    fn default() -> Self {
        Self {
            rectangle_color:  Color::srgb(1.0, 1.0, 0.0), // Yellow
            balanced_color:   Color::srgb(0.0, 1.0, 0.0), // Green
            unbalanced_color: Color::srgb(1.0, 0.0, 0.0), // Red
            line_width:       2.0,
        }
    }
}

/// Plugin that adds fit target visualization functionality
pub struct FitTargetVisualizationPlugin;

impl Plugin for FitTargetVisualizationPlugin {
    fn build(&self, app: &mut App) {
        app.init_gizmo_group::<FitTargetGizmo>()
            .init_resource::<FitTargetVisualizationConfig>()
            .add_systems(Startup, init_fit_target_gizmo)
            .add_systems(
                Update,
                (sync_gizmo_render_layers, draw_fit_target_bounds).chain(),
            )
            .add_systems(Update, cleanup_labels_when_disabled);
    }
}

/// System that cleans up labels when gizmo is disabled
fn cleanup_labels_when_disabled(
    commands: Commands,
    config_store: Res<GizmoConfigStore>,
    label_query: Query<Entity, With<MarginLabel>>,
) {
    let (config, _) = config_store.config::<FitTargetGizmo>();
    if !config.enabled && !label_query.is_empty() {
        cleanup_margin_labels(commands, label_query);
    }
}

/// Initialize the fit target gizmo config (disabled by default)
fn init_fit_target_gizmo(
    mut config_store: ResMut<GizmoConfigStore>,
    viz_config: Res<FitTargetVisualizationConfig>,
) {
    let (config, _) = config_store.config_mut::<FitTargetGizmo>();
    config.enabled = false;
    config.line.width = viz_config.line_width;
    config.depth_bias = -1.0;
}

/// Syncs the gizmo render layers and line width with camera and visualization config
fn sync_gizmo_render_layers(
    mut config_store: ResMut<GizmoConfigStore>,
    viz_config: Res<FitTargetVisualizationConfig>,
    camera_query: Query<Option<&RenderLayers>, With<PanOrbitCamera>>,
) {
    let Ok(render_layers) = camera_query.single() else {
        return;
    };

    let (gizmo_config, _) = config_store.config_mut::<FitTargetGizmo>();
    if let Some(layers) = render_layers {
        gizmo_config.render_layers = layers.clone();
    }
    gizmo_config.line.width = viz_config.line_width;
}

/// Boundary box edges
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect)]
enum Edge {
    Left,
    Right,
    Top,
    Bottom,
}

/// Screen-space margin information for a boundary
struct ScreenSpaceBoundary {
    /// Distance from left edge (positive = inside, negative = outside)
    left_margin:   f32,
    /// Distance from right edge (positive = inside, negative = outside)
    right_margin:  f32,
    /// Distance from top edge (positive = inside, negative = outside)
    top_margin:    f32,
    /// Distance from bottom edge (positive = inside, negative = outside)
    bottom_margin: f32,
    /// Minimum normalized x coordinate in screen space
    min_norm_x:    f32,
    /// Maximum normalized x coordinate in screen space
    max_norm_x:    f32,
    /// Minimum normalized y coordinate in screen space
    min_norm_y:    f32,
    /// Maximum normalized y coordinate in screen space
    max_norm_y:    f32,
    /// Average depth of boundary corners from camera
    avg_depth:     f32,
    /// Half tangent of vertical field of view
    half_tan_vfov: f32,
    /// Half tangent of horizontal field of view (vfov * `aspect_ratio`)
    half_tan_hfov: f32,
}

impl ScreenSpaceBoundary {
    /// Creates screen space margins from a camera's view of target corners.
    /// Returns `None` if any corner is behind the camera.
    #[allow(clippy::similar_names)] // half_tan_hfov vs half_tan_vfov are standard FOV terms
    fn from_corners(
        corners: &[Vec3; 8],
        cam_global: &GlobalTransform,
        perspective: &PerspectiveProjection,
        viewport_aspect: f32,
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

        // Calculate margins as distance from bounds to screen edges
        let left_margin = min_norm_x - (-half_tan_hfov);
        let right_margin = half_tan_hfov - max_norm_x;
        let bottom_margin = min_norm_y - (-half_tan_vfov);
        let top_margin = half_tan_vfov - max_norm_y;

        Some(Self {
            left_margin,
            right_margin,
            top_margin,
            bottom_margin,
            min_norm_x,
            max_norm_x,
            min_norm_y,
            max_norm_y,
            avg_depth,
            half_tan_vfov,
            half_tan_hfov,
        })
    }

    /// Returns true if horizontal margins are balanced
    fn is_horizontally_balanced(&self, tolerance: f32) -> bool {
        (self.left_margin - self.right_margin).abs() < tolerance
    }

    /// Returns true if vertical margins are balanced
    fn is_vertically_balanced(&self, tolerance: f32) -> bool {
        (self.top_margin - self.bottom_margin).abs() < tolerance
    }

    /// Returns the center of a boundary edge in normalized space
    fn boundary_edge_center(&self, edge: Edge) -> Option<(f32, f32)> {
        let (left_edge, right_edge, top_edge, bottom_edge) = self.screen_edges_normalized();

        match edge {
            Edge::Left if self.min_norm_x > left_edge => {
                let y = (self.min_norm_y.max(bottom_edge) + self.max_norm_y.min(top_edge)) * 0.5;
                Some((self.min_norm_x, y))
            },
            Edge::Right if self.max_norm_x < right_edge => {
                let y = (self.min_norm_y.max(bottom_edge) + self.max_norm_y.min(top_edge)) * 0.5;
                Some((self.max_norm_x, y))
            },
            Edge::Top if self.max_norm_y < top_edge => {
                let x = (self.min_norm_x.max(left_edge) + self.max_norm_x.min(right_edge)) * 0.5;
                Some((x, self.max_norm_y))
            },
            Edge::Bottom if self.min_norm_y > bottom_edge => {
                let x = (self.min_norm_x.max(left_edge) + self.max_norm_x.min(right_edge)) * 0.5;
                Some((x, self.min_norm_y))
            },
            _ => None,
        }
    }

    /// Returns the center of a screen edge in normalized space
    fn screen_edge_center(&self, edge: Edge) -> (f32, f32) {
        let (left_edge, right_edge, top_edge, bottom_edge) = self.screen_edges_normalized();

        match edge {
            Edge::Left => {
                let y = (self.min_norm_y.max(bottom_edge) + self.max_norm_y.min(top_edge)) * 0.5;
                (left_edge, y)
            },
            Edge::Right => {
                let y = (self.min_norm_y.max(bottom_edge) + self.max_norm_y.min(top_edge)) * 0.5;
                (right_edge, y)
            },
            Edge::Top => {
                let x = (self.min_norm_x.max(left_edge) + self.max_norm_x.min(right_edge)) * 0.5;
                (x, top_edge)
            },
            Edge::Bottom => {
                let x = (self.min_norm_x.max(left_edge) + self.max_norm_x.min(right_edge)) * 0.5;
                (x, bottom_edge)
            },
        }
    }

    /// Returns the screen edges in normalized space
    fn screen_edges_normalized(&self) -> (f32, f32, f32, f32) {
        (
            -self.half_tan_hfov,
            self.half_tan_hfov,
            self.half_tan_vfov,
            -self.half_tan_vfov,
        )
    }

    /// Converts normalized screen-space coordinates to world space
    fn normalized_to_world(
        &self,
        norm_x: f32,
        norm_y: f32,
        cam_pos: Vec3,
        cam_right: Vec3,
        cam_up: Vec3,
        cam_forward: Vec3,
    ) -> Vec3 {
        let world_x = norm_x * self.avg_depth;
        let world_y = norm_y * self.avg_depth;
        cam_pos + cam_right * world_x + cam_up * world_y + cam_forward * self.avg_depth
    }

    /// Returns the margin percentage for a given edge.
    /// Percentage represents how much of the screen width/height is margin.
    fn margin_percentage(&self, edge: Edge) -> f32 {
        let screen_width = 2.0 * self.half_tan_hfov;
        let screen_height = 2.0 * self.half_tan_vfov;

        match edge {
            Edge::Left => (self.left_margin / screen_width) * 100.0,
            Edge::Right => (self.right_margin / screen_width) * 100.0,
            Edge::Top => (self.top_margin / screen_height) * 100.0,
            Edge::Bottom => (self.bottom_margin / screen_height) * 100.0,
        }
    }
}

/// Calculates the color for an edge based on balance state
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

/// Creates the 4 corners of the screen-aligned boundary rectangle in world space
fn create_screen_corners(
    margins: &ScreenSpaceBoundary,
    cam_pos: Vec3,
    cam_right: Vec3,
    cam_up: Vec3,
    cam_forward: Vec3,
) -> [Vec3; 4] {
    [
        margins.normalized_to_world(
            margins.min_norm_x,
            margins.min_norm_y,
            cam_pos,
            cam_right,
            cam_up,
            cam_forward,
        ),
        margins.normalized_to_world(
            margins.max_norm_x,
            margins.min_norm_y,
            cam_pos,
            cam_right,
            cam_up,
            cam_forward,
        ),
        margins.normalized_to_world(
            margins.max_norm_x,
            margins.max_norm_y,
            cam_pos,
            cam_right,
            cam_up,
            cam_forward,
        ),
        margins.normalized_to_world(
            margins.min_norm_x,
            margins.max_norm_y,
            cam_pos,
            cam_right,
            cam_up,
            cam_forward,
        ),
    ]
}

/// Draws the boundary rectangle outline
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

/// Calculates the normalized screen-space position for a label based on edge type
fn calculate_label_position(edge: Edge, margins: &ScreenSpaceBoundary) -> (f32, f32) {
    match edge {
        Edge::Left => {
            let (_, screen_y) = margins.screen_edge_center(edge);
            (
                -margins.half_tan_hfov + LABEL_TEXT_OFFSET,
                LABEL_TEXT_OFFSET.mul_add(2.0, screen_y),
            )
        },
        Edge::Right => {
            let (_, screen_y) = margins.screen_edge_center(edge);
            (
                margins.half_tan_hfov - LABEL_TEXT_OFFSET,
                LABEL_TEXT_OFFSET.mul_add(2.0, screen_y),
            )
        },
        Edge::Top => {
            let (screen_x, _) = margins.screen_edge_center(edge);
            (
                screen_x + LABEL_TEXT_OFFSET,
                margins.half_tan_vfov - LABEL_TEXT_OFFSET,
            )
        },
        Edge::Bottom => {
            let (screen_x, _) = margins.screen_edge_center(edge);
            (
                screen_x + LABEL_TEXT_OFFSET,
                -margins.half_tan_vfov + LABEL_TEXT_OFFSET,
            )
        },
    }
}

/// Updates an existing margin label or creates a new one
fn update_or_create_margin_label(
    commands: &mut Commands,
    label_query: &mut Query<(Entity, &MarginLabel, &mut Text, &mut Node, &mut TextColor)>,
    edge: Edge,
    text: String,
    color: Color,
    screen_pos: Vec2,
    viewport_size: Vec2,
) {
    // Find or update existing label for this edge
    let mut found = false;
    for (_, label, mut label_text, mut node, mut text_color) in label_query {
        if label.edge == edge {
            **label_text = text.clone();
            text_color.0 = color;
            match edge {
                Edge::Left | Edge::Top => {
                    node.left = Val::Px(screen_pos.x);
                    node.top = Val::Px(screen_pos.y);
                    node.right = Val::Auto;
                    node.bottom = Val::Auto;
                },
                Edge::Right => {
                    node.right = Val::Px(viewport_size.x - screen_pos.x);
                    node.top = Val::Px(screen_pos.y);
                    node.left = Val::Auto;
                    node.bottom = Val::Auto;
                },
                Edge::Bottom => {
                    node.left = Val::Px(screen_pos.x);
                    node.bottom = Val::Px(viewport_size.y - screen_pos.y);
                    node.right = Val::Auto;
                    node.top = Val::Auto;
                },
            }
            found = true;
            break;
        }
    }

    if !found {
        // Create new label
        let node = match edge {
            Edge::Left | Edge::Top => Node {
                position_type: PositionType::Absolute,
                left: Val::Px(screen_pos.x),
                top: Val::Px(screen_pos.y),
                ..default()
            },
            Edge::Right => Node {
                position_type: PositionType::Absolute,
                right: Val::Px(viewport_size.x - screen_pos.x),
                top: Val::Px(screen_pos.y),
                ..default()
            },
            Edge::Bottom => Node {
                position_type: PositionType::Absolute,
                left: Val::Px(screen_pos.x),
                bottom: Val::Px(viewport_size.y - screen_pos.y),
                ..default()
            },
        };

        commands.spawn((
            Text::new(text),
            TextFont {
                font_size: LABEL_FONT_SIZE,
                ..default()
            },
            TextColor(color),
            node,
            MarginLabel { edge },
        ));
    }
}

/// System to cleanup margin labels when visualization is disabled
fn cleanup_margin_labels(mut commands: Commands, label_query: Query<Entity, With<MarginLabel>>) {
    for entity in &label_query {
        commands.entity(entity).despawn();
    }
}

/// Draws screen-aligned bounds for the current fit target
fn draw_fit_target_bounds(
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
    aabb_query: Query<&Aabb>,
    children_query: Query<&Children>,
    global_transform_query: Query<&GlobalTransform>,
    mut label_query: Query<(Entity, &MarginLabel, &mut Text, &mut Node, &mut TextColor)>,
) {
    let Ok((camera_entity, cam, cam_global, projection, current_target)) = camera_query.single()
    else {
        return;
    };

    // Check if visualization is enabled
    let (gizmo_config, _) = config_store.config::<FitTargetGizmo>();
    let visualization_enabled = gizmo_config.enabled;

    let Projection::Perspective(perspective) = projection else {
        return;
    };

    // Find target's Aabb (same logic as zoom observers)
    let Some((aabb_entity, aabb)) =
        find_descendant_aabb(current_target.0, &children_query, &aabb_query)
    else {
        return; // No Aabb found
    };

    let Ok(target_transform) = global_transform_query.get(aabb_entity) else {
        return;
    };

    // Convert Aabb to world corners
    let corners = aabb_to_world_corners(aabb, target_transform);

    // Get actual viewport aspect ratio
    let aspect_ratio = if let Some(viewport_size) = cam.logical_viewport_size() {
        viewport_size.x / viewport_size.y
    } else {
        perspective.aspect_ratio
    };

    // Calculate screen-space bounds
    let Some(margins) =
        ScreenSpaceBoundary::from_corners(&corners, cam_global, perspective, aspect_ratio)
    else {
        return; // Target behind camera
    };

    // Update margin component on camera entity for BRP inspection
    commands.entity(camera_entity).insert(FitTargetMargins {
        left_pct:   margins.margin_percentage(Edge::Left),
        right_pct:  margins.margin_percentage(Edge::Right),
        top_pct:    margins.margin_percentage(Edge::Top),
        bottom_pct: margins.margin_percentage(Edge::Bottom),
    });

    // Get camera basis vectors
    let cam_pos = cam_global.translation();
    let cam_rot = cam_global.rotation();
    let cam_forward = cam_rot * Vec3::NEG_Z;
    let cam_right = cam_rot * Vec3::X;
    let cam_up = cam_rot * Vec3::Y;

    // Draw the screen-aligned rectangle
    let rect_corners_world =
        create_screen_corners(&margins, cam_pos, cam_right, cam_up, cam_forward);
    draw_rectangle(&mut gizmos, &rect_corners_world, &config);

    // Draw lines from visible boundary edges to screen edges and create margin labels
    let h_balanced = margins.is_horizontally_balanced(crate::zoom::TOLERANCE);
    let v_balanced = margins.is_vertically_balanced(crate::zoom::TOLERANCE);

    // Track which edges are currently visible for label cleanup
    let mut visible_edges: Vec<Edge> = Vec::new();

    for edge in [Edge::Left, Edge::Right, Edge::Top, Edge::Bottom] {
        if let Some((boundary_x, boundary_y)) = margins.boundary_edge_center(edge) {
            visible_edges.push(edge);

            let (screen_x, screen_y) = margins.screen_edge_center(edge);

            let boundary_pos = margins.normalized_to_world(
                boundary_x,
                boundary_y,
                cam_pos,
                cam_right,
                cam_up,
                cam_forward,
            );
            let screen_pos = margins.normalized_to_world(
                screen_x,
                screen_y,
                cam_pos,
                cam_right,
                cam_up,
                cam_forward,
            );

            let color = calculate_edge_color(edge, h_balanced, v_balanced, &config);
            gizmos.line(boundary_pos, screen_pos, color);

            // Only create labels when visualization is explicitly enabled
            if visualization_enabled {
                // Add text label showing margin percentage
                let percentage = margins.margin_percentage(edge);
                let text = format!("{percentage:.3}%");

                let (label_x, label_y) = calculate_label_position(edge, &margins);

                let mut world_pos = margins.normalized_to_world(
                    label_x,
                    label_y,
                    cam_pos,
                    cam_right,
                    cam_up,
                    cam_forward,
                );
                // Offset label toward camera to prevent occlusion
                world_pos -= cam_forward * LABEL_WORLD_POS_OFFSET;

                // Project to screen space
                if let Ok(label_screen_pos) = cam.world_to_viewport(cam_global, world_pos) {
                    // Extract viewport size - must exist if world_to_viewport succeeded
                    let Some(viewport_size) = cam.logical_viewport_size() else {
                        continue;
                    };

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
    }

    // Remove labels for edges that are no longer visible, or if visualization is disabled
    for (entity, label, _, _, _) in &label_query {
        if !visualization_enabled || !visible_edges.contains(&label.edge) {
            commands.entity(entity).despawn();
        }
    }
}

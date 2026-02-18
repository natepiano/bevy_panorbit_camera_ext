//! Visualization system for fit target debugging
//!
//! Provides screen-aligned boundary box and silhouette polygon visualization for the current
//! camera fit target. Uses Bevy's GizmoConfigGroup pattern (similar to Avian3D's PhysicsGizmos).

use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use bevy_panorbit_camera::PanOrbitCamera;

use crate::components::CurrentFitTarget;
use crate::support::ProjectionParams;
use crate::support::extract_mesh_vertices;
use crate::support::project_point;
use crate::support::projection_aspect_ratio;

/// Gizmo config group for fit target visualization (screen-aligned overlay).
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

/// Component marking the "screen space bounds" label
#[derive(Component, Reflect)]
#[reflect(Component)]
struct BoundsLabel;

// Constants for label positioning
const LABEL_FONT_SIZE: f32 = 11.0;
const LABEL_PIXEL_OFFSET: f32 = 8.0; // Fixed pixel offset from margin lines

/// Configuration for fit target visualization colors and appearance
#[derive(Resource, Reflect, Debug, Clone)]
#[reflect(Resource)]
pub struct FitTargetVisualizationConfig {
    pub rectangle_color:  Color,
    pub silhouette_color: Color,
    pub balanced_color:   Color,
    pub unbalanced_color: Color,
    pub line_width:       f32,
}

impl Default for FitTargetVisualizationConfig {
    fn default() -> Self {
        Self {
            rectangle_color:  Color::srgb(1.0, 1.0, 0.0), // Yellow
            silhouette_color: Color::srgb(1.0, 0.5, 0.0), // Orange
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

/// System that cleans up all visualization labels when gizmo is disabled
fn cleanup_labels_when_disabled(
    mut commands: Commands,
    config_store: Res<GizmoConfigStore>,
    label_query: Query<Entity, With<MarginLabel>>,
    bounds_label_query: Query<Entity, With<BoundsLabel>>,
) {
    let (config, _) = config_store.config::<FitTargetGizmo>();
    if !config.enabled {
        if !label_query.is_empty() {
            cleanup_margin_labels(commands.reborrow(), label_query);
        }
        for entity in &bounds_label_query {
            commands.entity(entity).despawn();
        }
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
    /// Average depth of boundary points from camera
    avg_depth:     f32,
    /// Half visible extent in x (perspective: half_extent_x, ortho: area.width()/2)
    half_extent_x: f32,
    /// Half visible extent in y (perspective: half_extent_y, ortho: area.height()/2)
    half_extent_y: f32,
    /// Whether this boundary uses orthographic projection
    is_ortho:      bool,
}

/// Camera basis vectors extracted from a `GlobalTransform`.
/// Bundles the position and orientation vectors that are frequently passed together.
struct CameraBasis {
    pos:     Vec3,
    right:   Vec3,
    up:      Vec3,
    forward: Vec3,
}

impl CameraBasis {
    fn from_global_transform(global: &GlobalTransform) -> Self {
        let rot = global.rotation();
        Self {
            pos:     global.translation(),
            right:   rot * Vec3::X,
            up:      rot * Vec3::Y,
            forward: rot * Vec3::NEG_Z,
        }
    }
}

impl ScreenSpaceBoundary {
    /// Creates screen space margins from a camera's view of target points.
    /// Returns `None` if any point is behind the camera (perspective only).
    fn from_points(
        points: &[Vec3],
        cam_global: &GlobalTransform,
        projection: &Projection,
        viewport_aspect: f32,
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

        // Project points to screen space
        let mut min_norm_x = f32::INFINITY;
        let mut max_norm_x = f32::NEG_INFINITY;
        let mut min_norm_y = f32::INFINITY;
        let mut max_norm_y = f32::NEG_INFINITY;
        let mut avg_depth = 0.0;

        for point in points {
            let (norm_x, norm_y, depth) =
                project_point(*point, cam_pos, cam_right, cam_up, cam_forward, is_ortho)?;

            min_norm_x = min_norm_x.min(norm_x);
            max_norm_x = max_norm_x.max(norm_x);
            min_norm_y = min_norm_y.min(norm_y);
            max_norm_y = max_norm_y.max(norm_y);
            avg_depth += depth;
        }
        avg_depth /= points.len() as f32;

        // Calculate margins as distance from bounds to screen edges
        let left_margin = min_norm_x - (-half_extent_x);
        let right_margin = half_extent_x - max_norm_x;
        let bottom_margin = min_norm_y - (-half_extent_y);
        let top_margin = half_extent_y - max_norm_y;

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
            half_extent_x,
            half_extent_y,
            is_ortho,
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
            -self.half_extent_x,
            self.half_extent_x,
            self.half_extent_y,
            -self.half_extent_y,
        )
    }

    /// Converts normalized screen-space coordinates to world space.
    ///
    /// For perspective, reverses the perspective divide by multiplying by `avg_depth`.
    /// For orthographic, coordinates are already in world units — `avg_depth` is only
    /// used for the forward component to position the gizmo plane.
    fn normalized_to_world(&self, norm_x: f32, norm_y: f32, cam: &CameraBasis) -> Vec3 {
        let (world_x, world_y) = if self.is_ortho {
            (norm_x, norm_y)
        } else {
            (norm_x * self.avg_depth, norm_y * self.avg_depth)
        };
        cam.pos + cam.right * world_x + cam.up * world_y + cam.forward * self.avg_depth
    }

    /// Returns the margin percentage for a given edge.
    /// Percentage represents how much of the screen width/height is margin.
    fn margin_percentage(&self, edge: Edge) -> f32 {
        let screen_width = 2.0 * self.half_extent_x;
        let screen_height = 2.0 * self.half_extent_y;

        match edge {
            Edge::Left => (self.left_margin / screen_width) * 100.0,
            Edge::Right => (self.right_margin / screen_width) * 100.0,
            Edge::Top => (self.top_margin / screen_height) * 100.0,
            Edge::Bottom => (self.bottom_margin / screen_height) * 100.0,
        }
    }
}

// ============================================================================
// Convex Hull
// ============================================================================

/// 2D cross product for three points (for convex hull turn detection)
fn cross_2d(o: (f32, f32), a: (f32, f32), b: (f32, f32)) -> f32 {
    (a.0 - o.0) * (b.1 - o.1) - (a.1 - o.1) * (b.0 - o.0)
}

/// Andrew's monotone chain algorithm for 2D convex hull.
/// Returns hull vertices in counter-clockwise order.
fn convex_hull_2d(points: &[(f32, f32)]) -> Vec<(f32, f32)> {
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

    // Build lower hull
    let mut lower: Vec<(f32, f32)> = Vec::new();
    for &p in &sorted {
        while lower.len() >= 2 && cross_2d(lower[lower.len() - 2], lower[lower.len() - 1], p) <= 0.0
        {
            lower.pop();
        }
        lower.push(p);
    }

    // Build upper hull
    let mut upper: Vec<(f32, f32)> = Vec::new();
    for &p in sorted.iter().rev() {
        while upper.len() >= 2 && cross_2d(upper[upper.len() - 2], upper[upper.len() - 1], p) <= 0.0
        {
            upper.pop();
        }
        upper.push(p);
    }

    // Remove last point of each half (it's the first point of the other half)
    lower.pop();
    upper.pop();

    lower.extend(upper);
    lower
}

/// Projects world-space vertices to 2D normalized screen space.
///
/// For perspective, divides by depth. For orthographic, uses raw camera-space coordinates.
fn project_vertices_to_2d(vertices: &[Vec3], cam: &CameraBasis, is_ortho: bool) -> Vec<(f32, f32)> {
    vertices
        .iter()
        .filter_map(|v| {
            let (norm_x, norm_y, _) =
                project_point(*v, cam.pos, cam.right, cam.up, cam.forward, is_ortho)?;
            Some((norm_x, norm_y))
        })
        .collect()
}

/// Draws the silhouette polygon (convex hull of projected vertices) using gizmo lines.
fn draw_silhouette_polygon(
    gizmos: &mut Gizmos<FitTargetGizmo>,
    hull_points: &[(f32, f32)],
    boundary: &ScreenSpaceBoundary,
    cam: &CameraBasis,
    color: Color,
) {
    if hull_points.len() < 2 {
        return;
    }

    for i in 0..hull_points.len() {
        let next = (i + 1) % hull_points.len();
        let start = boundary.normalized_to_world(hull_points[i].0, hull_points[i].1, cam);
        let end = boundary.normalized_to_world(hull_points[next].0, hull_points[next].1, cam);
        gizmos.line(start, end, color);
    }
}

// ============================================================================
// Drawing helpers
// ============================================================================

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
fn create_screen_corners(margins: &ScreenSpaceBoundary, cam: &CameraBasis) -> [Vec3; 4] {
    [
        margins.normalized_to_world(margins.min_norm_x, margins.min_norm_y, cam),
        margins.normalized_to_world(margins.max_norm_x, margins.min_norm_y, cam),
        margins.normalized_to_world(margins.max_norm_x, margins.max_norm_y, cam),
        margins.normalized_to_world(margins.min_norm_x, margins.max_norm_y, cam),
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

/// Converts a normalized screen-space coordinate to viewport pixels.
fn norm_to_viewport(
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

/// Calculates the viewport pixel position for a margin label, offset by a fixed
/// number of pixels from the screen-edge endpoint of the margin line.
fn calculate_label_pixel_position(
    edge: Edge,
    margins: &ScreenSpaceBoundary,
    viewport_size: Vec2,
) -> Vec2 {
    let (screen_x, screen_y) = margins.screen_edge_center(edge);
    let px = norm_to_viewport(
        screen_x,
        screen_y,
        margins.half_extent_x,
        margins.half_extent_y,
        viewport_size,
    );

    // Left/Right labels sit above the horizontal line;
    // Top/Bottom labels sit beside the vertical line with pixel offsets.
    let above_line = px.y - LABEL_FONT_SIZE - LABEL_PIXEL_OFFSET;

    match edge {
        Edge::Left => Vec2::new(LABEL_PIXEL_OFFSET, above_line),
        Edge::Right => Vec2::new(viewport_size.x - LABEL_PIXEL_OFFSET, above_line),
        Edge::Top => Vec2::new(px.x + LABEL_PIXEL_OFFSET, LABEL_PIXEL_OFFSET),
        Edge::Bottom => Vec2::new(
            px.x + LABEL_PIXEL_OFFSET,
            viewport_size.y - LABEL_PIXEL_OFFSET,
        ),
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

#[allow(clippy::type_complexity)]
/// Updates an existing bounds label position or creates a new one
fn update_or_create_bounds_label(
    commands: &mut Commands,
    bounds_query: &mut Query<(Entity, &mut Node), (With<BoundsLabel>, Without<MarginLabel>)>,
    screen_pos: Vec2,
) {
    if let Ok((_, mut node)) = bounds_query.single_mut() {
        node.left = Val::Px(screen_pos.x);
        node.top = Val::Px(screen_pos.y);
    } else {
        commands.spawn((
            Text::new("screen space bounds"),
            TextFont {
                font_size: LABEL_FONT_SIZE,
                ..default()
            },
            TextColor(Color::srgb(1.0, 1.0, 0.0)),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(screen_pos.x),
                top: Val::Px(screen_pos.y),
                ..default()
            },
            BoundsLabel,
        ));
    }
}

/// System to cleanup margin labels when visualization is disabled
fn cleanup_margin_labels(mut commands: Commands, label_query: Query<Entity, With<MarginLabel>>) {
    for entity in &label_query {
        commands.entity(entity).despawn();
    }
}

// ============================================================================
// Main visualization system
// ============================================================================

/// Draws screen-aligned bounds for the current fit target
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
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

    // Check if visualization is enabled
    let (gizmo_config, _) = config_store.config::<FitTargetGizmo>();
    let visualization_enabled = gizmo_config.enabled;

    // Extract mesh vertices (same logic as zoom observers)
    let Some((vertices, _)) = extract_mesh_vertices(
        current_target.0,
        &children_query,
        &mesh_query,
        &global_transform_query,
        &meshes,
    ) else {
        return; // No mesh found
    };

    // Get camera basis vectors
    let cam_basis = CameraBasis::from_global_transform(cam_global);

    // Get actual viewport aspect ratio
    let Some(aspect_ratio) = projection_aspect_ratio(projection, cam.logical_viewport_size())
    else {
        return;
    };

    // Calculate screen-space bounds from mesh vertices
    let Some(margins) =
        ScreenSpaceBoundary::from_points(&vertices, cam_global, projection, aspect_ratio)
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

    // Draw the screen-aligned bounding rectangle
    let rect_corners_world = create_screen_corners(&margins, &cam_basis);
    draw_rectangle(&mut gizmos, &rect_corners_world, &config);

    // Draw silhouette polygon (convex hull) when visualization is enabled
    if visualization_enabled {
        let projected = project_vertices_to_2d(&vertices, &cam_basis, margins.is_ortho);
        let hull = convex_hull_2d(&projected);
        draw_silhouette_polygon(
            &mut gizmos,
            &hull,
            &margins,
            &cam_basis,
            config.silhouette_color,
        );
    }

    // Place "screen space bounds" label inside the upper-left of the rectangle
    if visualization_enabled && let Some(viewport_size) = cam.logical_viewport_size() {
        let upper_left = norm_to_viewport(
            margins.min_norm_x,
            margins.max_norm_y,
            margins.half_extent_x,
            margins.half_extent_y,
            viewport_size,
        );
        let pos = Vec2::new(
            upper_left.x + LABEL_PIXEL_OFFSET,
            upper_left.y - LABEL_FONT_SIZE - LABEL_PIXEL_OFFSET,
        );
        update_or_create_bounds_label(&mut commands, &mut bounds_label_query, pos);
    }

    // Draw lines from visible boundary edges to screen edges and create margin labels
    let h_balanced = margins.is_horizontally_balanced(crate::fit::TOLERANCE);
    let v_balanced = margins.is_vertically_balanced(crate::fit::TOLERANCE);

    // Track which edges are currently visible for label cleanup
    let mut visible_edges: Vec<Edge> = Vec::new();

    for edge in [Edge::Left, Edge::Right, Edge::Top, Edge::Bottom] {
        if let Some((boundary_x, boundary_y)) = margins.boundary_edge_center(edge) {
            visible_edges.push(edge);

            let (screen_x, screen_y) = margins.screen_edge_center(edge);

            let boundary_pos = margins.normalized_to_world(boundary_x, boundary_y, &cam_basis);
            let screen_pos = margins.normalized_to_world(screen_x, screen_y, &cam_basis);

            let color = calculate_edge_color(edge, h_balanced, v_balanced, &config);
            gizmos.line(boundary_pos, screen_pos, color);

            // Only create labels when visualization is explicitly enabled
            if visualization_enabled {
                // Add text label showing margin percentage
                let percentage = margins.margin_percentage(edge);
                let text = format!("{percentage:.3}%");

                let Some(viewport_size) = cam.logical_viewport_size() else {
                    continue;
                };
                let label_screen_pos =
                    calculate_label_pixel_position(edge, &margins, viewport_size);

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

    // Remove labels for edges that are no longer visible, or if visualization is disabled
    for (entity, label, _, _, _) in &label_query {
        if !visualization_enabled || !visible_edges.contains(&label.edge) {
            commands.entity(entity).despawn();
        }
    }

    // Remove bounds label when visualization is disabled
    if !visualization_enabled {
        for (entity, _) in &bounds_label_query {
            commands.entity(entity).despawn();
        }
    }
}

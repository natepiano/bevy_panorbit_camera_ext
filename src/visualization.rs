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

/// Gizmo config group for fit target visualization (screen-aligned overlay).
/// Toggle via `GizmoConfigStore::config_mut::<FitTargetGizmo>().enabled`
#[derive(Default, Reflect, GizmoConfigGroup)]
pub struct FitTargetGizmo {}

/// Gizmo config group for the 3D AABB cuboid (depth-tested, occluded by geometry).
#[derive(Default, Reflect, GizmoConfigGroup)]
struct AabbGizmo {}

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

/// Component marking the "screen aligned bounding box" label
#[derive(Component, Reflect)]
#[reflect(Component)]
struct BoundsLabel;

// Constants for label positioning
const LABEL_FONT_SIZE: f32 = 11.0;
const LABEL_PIXEL_OFFSET: f32 = 8.0; // Fixed pixel offset from margin lines

// Constants for 3D AABB label
const AABB_LABEL_SCREEN_PX: f32 = 10.0; // Fixed screen-space font size in pixels
const AABB_LABEL_GAP_PX: f32 = 8.0; // Fixed screen-space gap in pixels between edge and label

/// Configuration for fit target visualization colors and appearance
#[derive(Resource, Reflect, Debug, Clone)]
#[reflect(Resource)]
pub struct FitTargetVisualizationConfig {
    pub rectangle_color:  Color,
    pub aabb_color:       Color,
    pub balanced_color:   Color,
    pub unbalanced_color: Color,
    pub line_width:       f32,
}

impl Default for FitTargetVisualizationConfig {
    fn default() -> Self {
        Self {
            rectangle_color:  Color::srgb(1.0, 1.0, 0.0), // Yellow
            aabb_color:       Color::srgb(1.0, 0.5, 0.0), // Orange
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
            .init_gizmo_group::<AabbGizmo>()
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

    // AABB gizmo: depth-tested so it's occluded by geometry
    let (aabb_config, _) = config_store.config_mut::<AabbGizmo>();
    aabb_config.enabled = false;
    aabb_config.line.width = viz_config.line_width;
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

    let layers = render_layers.cloned();
    let fit_enabled = config_store.config::<FitTargetGizmo>().0.enabled;

    let (gizmo_config, _) = config_store.config_mut::<FitTargetGizmo>();
    if let Some(ref layers) = layers {
        gizmo_config.render_layers = layers.clone();
    }
    gizmo_config.line.width = viz_config.line_width;

    let (aabb_config, _) = config_store.config_mut::<AabbGizmo>();
    if let Some(layers) = layers {
        aabb_config.render_layers = layers;
    }
    aabb_config.enabled = fit_enabled;
    aabb_config.line.width = viz_config.line_width;
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

/// Draws the 3D AABB as a wireframe cuboid (12 edges connecting 8 corners).
/// Uses `AabbGizmo` group so lines are depth-tested and occluded by geometry.
fn draw_aabb_cuboid(
    gizmos: &mut Gizmos<AabbGizmo>,
    corners: &[Vec3; 8],
    config: &FitTargetVisualizationConfig,
) {
    // Corner indices form a box:
    //   0(-x,-y,-z) 1(+x,-y,-z) 2(-x,+y,-z) 3(+x,+y,-z)
    //   4(-x,-y,+z) 5(+x,-y,+z) 6(-x,+y,+z) 7(+x,+y,+z)
    const EDGES: [(usize, usize); 12] = [
        // Bottom face
        (0, 1),
        (1, 3),
        (3, 2),
        (2, 0),
        // Top face
        (4, 5),
        (5, 7),
        (7, 6),
        (6, 4),
        // Vertical edges
        (0, 4),
        (1, 5),
        (2, 6),
        (3, 7),
    ];

    for (a, b) in EDGES {
        gizmos.line(corners[a], corners[b], config.aabb_color);
    }
}

/// Draws "AABB" as 3D stroke text using gizmo line segments.
/// `center` is the midpoint of the text. `right` and `up` are unit directions
/// defining the text plane. `font_size` scales the cap height (1.0 in glyph space).
fn draw_stroke_aabb(
    gizmos: &mut Gizmos<AabbGizmo>,
    center: Vec3,
    right: Vec3,
    up: Vec3,
    font_size: f32,
    color: Color,
) {
    // "AABB" layout: A(0.0), A(0.78), B(1.56), B(2.29)
    // Glyph A width=0.5, B width=0.45, inter-letter gap=0.28
    const TOTAL_ADVANCE: f32 = 2.74;
    let r = right * font_size;
    let u = up * font_size;
    let origin = center - r * (TOTAL_ADVANCE * 0.5);

    let w = |x: f32, y: f32| -> Vec3 { origin + r * x + u * y };

    // Glyph A: two diagonals + crossbar
    for x_off in [0.0, 0.78] {
        gizmos.line(w(x_off, 0.0), w(x_off + 0.25, 1.0), color);
        gizmos.line(w(x_off + 0.25, 1.0), w(x_off + 0.5, 0.0), color);
        gizmos.line(w(x_off + 0.1, 0.35), w(x_off + 0.4, 0.35), color);
    }

    // Glyph B: single polyline path through all stroke points
    for x_off in [1.56, 2.29] {
        let pts: [(f32, f32); 12] = [
            (0.0, 0.0),
            (0.0, 1.0),
            (0.35, 1.0),
            (0.45, 0.85),
            (0.45, 0.6),
            (0.3, 0.5),
            (0.0, 0.5),
            (0.3, 0.5),
            (0.45, 0.4),
            (0.45, 0.15),
            (0.35, 0.0),
            (0.0, 0.0),
        ];
        for pair in pts.windows(2) {
            gizmos.line(
                w(x_off + pair[0].0, pair[0].1),
                w(x_off + pair[1].0, pair[1].1),
                color,
            );
        }
    }
}

/// Draws a rotated "AABB" stroke-font label on the most camera-facing face of the cuboid,
/// just below the top edge of that face. Oriented flat on the face surface.
fn draw_aabb_label(
    gizmos: &mut Gizmos<AabbGizmo>,
    corners: &[Vec3; 8],
    cam_pos: Vec3,
    cam_rot: Quat,
    half_tan_vfov: f32,
    viewport_height: f32,
    config: &FitTargetVisualizationConfig,
) {
    let cam_up = cam_rot * Vec3::Y;
    let cam_right = cam_rot * Vec3::X;

    // The 6 faces of the AABB, each defined by 4 corner indices.
    // Corner layout: 0(-x,-y,-z) 1(+x,-y,-z) 2(-x,+y,-z) 3(+x,+y,-z)
    //                4(-x,-y,+z) 5(+x,-y,+z) 6(-x,+y,+z) 7(+x,+y,+z)
    let faces: [[usize; 4]; 6] = [
        [2, 3, 7, 6], // +Y (top)
        [0, 1, 5, 4], // -Y (bottom)
        [4, 5, 7, 6], // +Z (front)
        [0, 1, 3, 2], // -Z (back)
        [1, 5, 7, 3], // +X (right)
        [0, 4, 6, 2], // -X (left)
    ];

    // Find the face whose world-space normal most faces the camera.
    // Normals are computed from world-space corners via cross product, which yields
    // area-weighted normals that correctly handle rotation and non-uniform scale.
    let aabb_center = corners.iter().copied().sum::<Vec3>() / 8.0;
    let to_cam = (cam_pos - aabb_center).normalize();

    let mut best_face_idx = 0;
    let mut best_dot = f32::NEG_INFINITY;
    let mut best_normal = Vec3::ZERO;

    for (i, face) in faces.iter().enumerate() {
        let edge1 = corners[face[1]] - corners[face[0]];
        let edge2 = corners[face[3]] - corners[face[0]];
        let mut normal = edge1.cross(edge2);

        // Ensure outward-pointing: normal should point away from AABB center
        let face_center =
            (corners[face[0]] + corners[face[1]] + corners[face[2]] + corners[face[3]]) * 0.25;
        if normal.dot(face_center - aabb_center) < 0.0 {
            normal = -normal;
        }

        let dot = normal.dot(to_cam);
        if dot > best_dot {
            best_dot = dot;
            best_face_idx = i;
            best_normal = normal;
        }
    }

    let face_corners = faces[best_face_idx];
    let face_normal = best_normal.normalize();

    // Find the "top" edge of this face: the edge whose midpoint is highest on screen
    // (most positive dot with camera up). Each face has 4 edges.
    let face_edges: [(usize, usize); 4] = [
        (face_corners[0], face_corners[1]),
        (face_corners[1], face_corners[2]),
        (face_corners[2], face_corners[3]),
        (face_corners[3], face_corners[0]),
    ];

    let mut top_edge_idx = 0;
    let mut highest_screen_y = f32::NEG_INFINITY;
    for (i, &(a, b)) in face_edges.iter().enumerate() {
        let mid = (corners[a] + corners[b]) * 0.5;
        let screen_y = mid.dot(cam_up);
        if screen_y > highest_screen_y {
            highest_screen_y = screen_y;
            top_edge_idx = i;
        }
    }

    let (a, b) = face_edges[top_edge_idx];
    let start = corners[a];
    let end = corners[b];

    // Edge direction, flipped so text reads left-to-right
    let mut edge_dir = (end - start).normalize();
    if edge_dir.dot(cam_right) < 0.0 {
        edge_dir = -edge_dir;
    }

    let midpoint = (start + end) * 0.5;

    // "Into face" direction from the top edge: perpendicular to edge and face normal,
    // pointing toward the AABB center (into the face interior = downward on the face).
    let into_face = face_normal.cross(edge_dir).normalize();
    let to_center = (aabb_center - midpoint).normalize();
    let into_face = if into_face.dot(to_center) < 0.0 {
        -into_face
    } else {
        into_face
    };

    // Text "up" points away from the face interior (upward on the face)
    let text_up = -into_face;

    // Convert fixed screen-space pixel sizes to world-space at the label's depth.
    // world_size = (screen_px / viewport_height) * 2 * half_tan_vfov * depth
    let cam_forward = cam_rot * Vec3::NEG_Z;
    let depth = (midpoint - cam_pos).dot(cam_forward).max(0.1);
    let px_to_world = (2.0 * half_tan_vfov * depth) / viewport_height;
    let font_size = AABB_LABEL_SCREEN_PX * px_to_world;
    let gap = AABB_LABEL_GAP_PX * px_to_world;

    // Offset into the face so text sits just below the top edge, slightly outward
    let label_center = midpoint + into_face * (font_size + gap) + face_normal * gap;

    draw_stroke_aabb(
        gizmos,
        label_center,
        edge_dir,
        text_up,
        font_size,
        config.aabb_color,
    );
}

/// Converts a normalized screen-space coordinate to viewport pixels.
fn norm_to_viewport(
    norm_x: f32,
    norm_y: f32,
    half_tan_hfov: f32,
    half_tan_vfov: f32,
    viewport_size: Vec2,
) -> Vec2 {
    Vec2::new(
        (norm_x / half_tan_hfov + 1.0) * 0.5 * viewport_size.x,
        (1.0 - norm_y / half_tan_vfov) * 0.5 * viewport_size.y,
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
        margins.half_tan_hfov,
        margins.half_tan_vfov,
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
            Text::new("screen space AABB"),
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

/// Draws screen-aligned bounds for the current fit target
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn draw_fit_target_bounds(
    mut commands: Commands,
    mut gizmos: Gizmos<FitTargetGizmo>,
    mut aabb_gizmos: Gizmos<AabbGizmo>,
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
    mut bounds_label_query: Query<(Entity, &mut Node), (With<BoundsLabel>, Without<MarginLabel>)>,
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

    // Get camera basis vectors (needed for both AABB label and screen-space drawing)
    let cam_pos = cam_global.translation();
    let cam_rot = cam_global.rotation();
    let cam_forward = cam_rot * Vec3::NEG_Z;
    let cam_right = cam_rot * Vec3::X;
    let cam_up = cam_rot * Vec3::Y;

    // Draw the 3D AABB cuboid and label when visualization is enabled
    if visualization_enabled {
        draw_aabb_cuboid(&mut aabb_gizmos, &corners, &config);
        let half_tan_vfov = (perspective.fov * 0.5).tan();
        let viewport_height = cam.logical_viewport_size().map_or(720.0, |s| s.y);
        draw_aabb_label(
            &mut aabb_gizmos,
            &corners,
            cam_pos,
            cam_rot,
            half_tan_vfov,
            viewport_height,
            &config,
        );
    }

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

    // Draw the screen-aligned rectangle
    let rect_corners_world =
        create_screen_corners(&margins, cam_pos, cam_right, cam_up, cam_forward);
    draw_rectangle(&mut gizmos, &rect_corners_world, &config);

    // Place "screen aligned bounding box" label inside the upper-left of the rectangle
    if visualization_enabled && let Some(viewport_size) = cam.logical_viewport_size() {
        let upper_left = norm_to_viewport(
            margins.min_norm_x,
            margins.max_norm_y,
            margins.half_tan_hfov,
            margins.half_tan_vfov,
            viewport_size,
        );
        let pos = Vec2::new(
            upper_left.x + LABEL_PIXEL_OFFSET,
            upper_left.y - LABEL_FONT_SIZE - LABEL_PIXEL_OFFSET,
        );
        update_or_create_bounds_label(&mut commands, &mut bounds_label_query, pos);
    }

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

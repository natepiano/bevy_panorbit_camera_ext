//! Shared utility functions used across multiple modules.

use bevy::prelude::*;

// ============================================================================
// Projection utilities
// ============================================================================

/// Minimum depth for a point to be considered in front of the camera.
/// Points at or below this depth are treated as behind the camera in perspective projection.
pub const MIN_VISIBLE_DEPTH: f32 = 0.1;

/// Projection-derived parameters for screen-space normalization.
/// Consolidates the extraction of half extents and projection type from a `Projection`.
pub struct ProjectionParams {
    /// Half visible extent in x (perspective: half_tan_hfov, ortho: area.width()/2)
    pub half_extent_x: f32,
    /// Half visible extent in y (perspective: half_tan_vfov, ortho: area.height()/2)
    pub half_extent_y: f32,
    /// Whether this uses orthographic projection
    pub is_ortho:      bool,
}

impl ProjectionParams {
    /// Extracts projection parameters from a `Projection` and viewport aspect ratio.
    /// Returns `None` for unsupported projection variants.
    pub fn from_projection(projection: &Projection, viewport_aspect: f32) -> Option<Self> {
        let is_ortho = matches!(projection, Projection::Orthographic(_));
        let (half_extent_x, half_extent_y) = match projection {
            Projection::Perspective(p) => {
                let half_tan_vfov = (p.fov * 0.5).tan();
                (half_tan_vfov * viewport_aspect, half_tan_vfov)
            },
            Projection::Orthographic(o) => (o.area.width() * 0.5, o.area.height() * 0.5),
            _ => return None,
        };
        Some(Self {
            half_extent_x,
            half_extent_y,
            is_ortho,
        })
    }
}

/// Projects a world-space point to normalized screen coordinates.
///
/// Returns `(norm_x, norm_y, depth)` or `None` if the point is behind the camera
/// (perspective only — orthographic points are always valid).
pub fn project_point(
    point: Vec3,
    cam_pos: Vec3,
    cam_right: Vec3,
    cam_up: Vec3,
    cam_forward: Vec3,
    is_ortho: bool,
) -> Option<(f32, f32, f32)> {
    let relative = point - cam_pos;
    let depth = relative.dot(cam_forward);
    if !is_ortho && depth <= MIN_VISIBLE_DEPTH {
        return None;
    }
    let x = relative.dot(cam_right);
    let y = relative.dot(cam_up);
    let (norm_x, norm_y) = if is_ortho {
        (x, y)
    } else {
        (x / depth, y / depth)
    };
    Some((norm_x, norm_y, depth))
}

/// Extracts the aspect ratio from a `Projection`, using `viewport_size` for
/// perspective when available, falling back to `PerspectiveProjection::aspect_ratio`.
///
/// Returns `None` for orthographic projections with zero-height area or unknown
/// projection variants.
pub fn projection_aspect_ratio(
    projection: &Projection,
    viewport_size: Option<Vec2>,
) -> Option<f32> {
    match projection {
        Projection::Perspective(p) => Some(viewport_size.map_or(p.aspect_ratio, |s| s.x / s.y)),
        Projection::Orthographic(o) => {
            let area = o.area;
            if area.height().abs() < f32::EPSILON {
                return None;
            }
            Some(area.width() / area.height())
        },
        _ => None,
    }
}

// ============================================================================
// Mesh utilities
// ============================================================================

/// Extracts world-space vertex positions from all meshes on an entity and its descendants.
/// Returns `(vertices, geometric_center)` where `geometric_center` is the root entity's
/// `GlobalTransform` translation.
pub fn extract_mesh_vertices(
    entity: Entity,
    children_query: &Query<&Children>,
    mesh_query: &Query<&Mesh3d>,
    global_transform_query: &Query<&GlobalTransform>,
    meshes: &Assets<Mesh>,
) -> Option<(Vec<Vec3>, Vec3)> {
    let mesh_entities: Vec<Entity> = std::iter::once(entity)
        .chain(children_query.iter_descendants(entity))
        .filter(|e| mesh_query.get(*e).is_ok())
        .collect();

    if mesh_entities.is_empty() {
        return None;
    }

    let mut all_vertices = Vec::new();

    for mesh_entity in &mesh_entities {
        let Ok(mesh3d) = mesh_query.get(*mesh_entity) else {
            continue;
        };
        let Some(mesh) = meshes.get(&mesh3d.0) else {
            continue;
        };
        let Ok(global_transform) = global_transform_query.get(*mesh_entity) else {
            continue;
        };
        let Some(positions) = mesh
            .attribute(Mesh::ATTRIBUTE_POSITION)
            .and_then(|a| a.as_float3())
        else {
            continue;
        };

        all_vertices.extend(
            positions
                .iter()
                .map(|pos| global_transform.transform_point(Vec3::from_array(*pos))),
        );
    }

    if all_vertices.is_empty() {
        return None;
    }

    let geometric_center = global_transform_query
        .get(entity)
        .map_or(Vec3::ZERO, |gt| gt.translation());

    Some((all_vertices, geometric_center))
}

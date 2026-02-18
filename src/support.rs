//! Support utilities for mesh and hierarchy operations.

use bevy::prelude::*;

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

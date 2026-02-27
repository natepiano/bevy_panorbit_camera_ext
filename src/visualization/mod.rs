//! Visualization system for fit target debugging.
//!
//! Provides screen-aligned boundary box and silhouette polygon visualization for the current
//! camera fit target. Uses Bevy's GizmoConfigGroup pattern (similar to Avian3D's PhysicsGizmos).

mod convex_hull;
mod labels;
mod screen_space;
mod system;
mod types;

use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use bevy_panorbit_camera::PanOrbitCamera;
use labels::BoundsLabel;
use labels::MarginLabel;
use labels::cleanup_margin_labels;
use system::draw_fit_target_bounds;
pub use types::FitTargetGizmo;
pub use types::FitTargetMargins;
pub use types::FitTargetVisualizationConfig;

/// Plugin that adds fit target visualization functionality.
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

/// System that cleans up all visualization labels when gizmo is disabled.
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

/// Initialize the fit target gizmo config (disabled by default).
fn init_fit_target_gizmo(
    mut config_store: ResMut<GizmoConfigStore>,
    viz_config: Res<FitTargetVisualizationConfig>,
) {
    let (config, _) = config_store.config_mut::<FitTargetGizmo>();
    config.enabled = false;
    config.line.width = viz_config.line_width;
    config.depth_bias = -1.0;
}

/// Syncs the gizmo render layers and line width with camera and visualization config.
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

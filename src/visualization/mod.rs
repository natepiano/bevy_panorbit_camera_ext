//! Visualization system for fit target debugging.
//!
//! Provides screen-aligned boundary box and silhouette polygon visualization for the current
//! camera fit target. Uses Bevy's GizmoConfigGroup pattern (similar to Avian3D's PhysicsGizmos).

mod convex_hull;
mod labels;
mod screen_space;
mod systems;
mod types;

use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use bevy_panorbit_camera::PanOrbitCamera;
use labels::BoundsLabel;
use labels::MarginLabel;
pub use types::FitTargetGizmo;
use types::FitTargetViewportMargins;
pub use types::FitTargetVisualizationConfig;

use crate::components::FitVisualization;

/// Returns true when the gizmo and asset subsystems are available (absent in headless/test apps).
fn has_gizmo_subsystem(config_store: Option<Res<GizmoConfigStore>>) -> bool {
    config_store.is_some()
}

pub struct VisualizationPlugin;

impl Plugin for VisualizationPlugin {
    fn build(&self, app: &mut App) {
        // Only initialize gizmo infrastructure when the gizmo plugin is present
        // (absent in headless/test environments that skip `DefaultPlugins`).
        if app.is_plugin_added::<bevy::gizmos::GizmoPlugin>() {
            app.init_gizmo_group::<FitTargetGizmo>();
        }

        app.init_resource::<FitTargetVisualizationConfig>()
            .add_observer(on_add_fit_visualization)
            .add_observer(on_remove_fit_visualization)
            .add_systems(Startup, init_fit_target_gizmo.run_if(has_gizmo_subsystem))
            .add_systems(
                Update,
                (sync_gizmo_render_layers, systems::draw_fit_target_bounds)
                    .chain()
                    .run_if(any_with_component::<FitVisualization>.and(has_gizmo_subsystem)),
            );
    }
}

/// Observer that enables visualization when `FitVisualization` is added to a camera entity.
fn on_add_fit_visualization(
    _trigger: On<Add, FitVisualization>,
    config_store: Option<ResMut<GizmoConfigStore>>,
) {
    let Some(mut config_store) = config_store else {
        warn!(
            "`FitVisualization` added but `GizmoPlugin` is not present — \
             gizmo visualization requires `GizmoPlugin` to be added to the app"
        );
        return;
    };

    let (config, _) = config_store.config_mut::<FitTargetGizmo>();
    config.enabled = true;
}

/// Observer that disables visualization when `FitVisualization` is removed from a camera entity.
fn on_remove_fit_visualization(
    trigger: On<Remove, FitVisualization>,
    mut commands: Commands,
    config_store: Option<ResMut<GizmoConfigStore>>,
    label_query: Query<Entity, With<MarginLabel>>,
    bounds_label_query: Query<Entity, With<BoundsLabel>>,
) {
    let entity = trigger.entity;

    if let Some(mut config_store) = config_store {
        let (config, _) = config_store.config_mut::<FitTargetGizmo>();
        config.enabled = false;
    }

    // Clean up viewport margins from the camera entity
    commands.entity(entity).remove::<FitTargetViewportMargins>();

    // Clean up all visualization labels since the system will no longer run
    if !label_query.is_empty() {
        labels::cleanup_margin_labels(commands.reborrow(), label_query);
    }
    for label_entity in &bounds_label_query {
        commands.entity(label_entity).despawn();
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

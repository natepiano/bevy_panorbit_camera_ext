// bevy_panorbit_camera_ext
// Extension library for bevy_panorbit_camera providing:
// - Camera animation system with queued moves
// - Zoom-to-fit functionality
// - Extension traits for camera manipulation

use bevy::prelude::*;

mod animation;
mod extension;
pub mod prelude;
mod smoothness;
mod zoom;

pub use animation::{CameraMove, CameraMoveList, process_camera_move_list};
pub use extension::{
    PanOrbitCameraExt, SnapToFit, StartAnimation, ZoomToFit, ZoomToFitConfig, auto_add_zoom_config,
    on_snap_to_fit, on_start_animation, on_zoom_to_fit,
};
pub use smoothness::{
    SmoothnessStash, restore_smoothness_on_move_complete, restore_smoothness_on_zoom_complete,
};
pub use zoom::{
    Edge, ScreenSpaceBounds, ZoomConfig, ZoomToFitComponent, compute_bounding_corners,
    zoom_to_fit_convergence_system,
};

/// Plugin that adds all camera extension functionality
pub struct CameraExtPlugin;

impl Plugin for CameraExtPlugin {
    fn build(&self, app: &mut App) {
        app
            // Register observers for component lifecycle events
            .add_observer(restore_smoothness_on_move_complete)
            .add_observer(restore_smoothness_on_zoom_complete)
            .add_observer(auto_add_zoom_config)
            // Register observers for custom events
            .add_observer(on_snap_to_fit)
            .add_observer(on_zoom_to_fit)
            .add_observer(on_start_animation)
            // Add systems
            .add_systems(
                Update,
                (process_camera_move_list, zoom_to_fit_convergence_system),
            )
            // Initialize resources
            .init_resource::<ZoomConfig>();
    }
}

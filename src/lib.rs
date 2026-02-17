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
mod visualization;
mod zoom;

// Public API - Events
// Public API - Animation types (used by prelude and external code)
pub use animation::CameraMove;
pub use animation::CameraMoveList;
// Internal - used by plugin, not for external use
use animation::process_camera_move_list;
// Public API - Components (for querying)
pub use extension::CurrentFitTarget;
// Public API - Traits
pub use extension::PanOrbitCameraExt;
pub use extension::SetFitTarget;
pub use extension::SnapToFit;
pub use extension::StartAnimation;
pub use extension::ZoomToFit;
// Public API - Configuration components (used by prelude)
pub use extension::ZoomToFitConfig;
use extension::auto_add_zoom_config;
// Public API - Utility functions
pub use extension::calculate_fit_radius;
use extension::on_set_fit_target;
use extension::on_snap_to_fit;
use extension::on_start_animation;
use extension::on_zoom_to_fit;
pub use smoothness::SmoothnessStash;
use smoothness::restore_smoothness_on_move_complete;
use smoothness::restore_smoothness_on_zoom_complete;
// Public API - Gizmo groups (for enabling/disabling)
pub use visualization::FitTargetGizmo;
pub use visualization::FitTargetMargins;
// Public API - Configuration resources
pub use visualization::FitTargetVisualizationConfig;
// Public API - Plugins
pub use visualization::FitTargetVisualizationPlugin;
// Public API - Zoom types (used by prelude)
pub use zoom::DEFAULT_MARGIN;
pub use zoom::Edge;
pub use zoom::ScreenSpaceBounds;
use zoom::zoom_to_fit_animation_system;

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
            .add_observer(on_set_fit_target)
            // Add systems
            .add_systems(
                Update,
                (process_camera_move_list, zoom_to_fit_animation_system),
            );
    }
}

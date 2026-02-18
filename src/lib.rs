// bevy_panorbit_camera_ext
// Extension library for bevy_panorbit_camera providing:
// - Camera animation system with queued moves
// - Zoom-to-fit functionality
// - Extension traits for camera manipulation

use bevy::prelude::*;

mod animation;
mod components;
mod events;
mod fit;
mod observers;
mod support;
mod visualization;

// Public API - Animation types
pub use animation::CameraMove;
pub use animation::CameraMoveList;
use animation::process_camera_move_list;
// Public API - Components
pub use components::CurrentFitTarget;
pub use components::SmoothnessStash;
pub use components::ZoomAnimationMarker;
// Public API - Events
pub use events::AnimateToFit;
pub use events::AnimationBegin;
pub use events::AnimationEnd;
pub use events::CameraMoveBegin;
pub use events::CameraMoveEnd;
pub use events::PlayAnimation;
pub use events::SetFitTarget;
pub use events::ZoomBegin;
pub use events::ZoomEnd;
pub use events::ZoomToFit;
// Public API - Fit types
pub use fit::Edge;
pub use fit::ScreenSpaceBounds;
use observers::on_animate_to_fit;
use observers::on_play_animation;
use observers::on_set_fit_target;
use observers::on_zoom_to_fit;
use observers::restore_smoothness_on_move_end;
// Public API - Gizmo groups (for enabling/disabling)
pub use visualization::FitTargetGizmo;
pub use visualization::FitTargetMargins;
// Public API - Configuration resources
pub use visualization::FitTargetVisualizationConfig;
// Public API - Plugins
pub use visualization::FitTargetVisualizationPlugin;

/// Plugin that adds all camera extension functionality
pub struct PanOrbitCameraExtPlugin;

impl Plugin for PanOrbitCameraExtPlugin {
    fn build(&self, app: &mut App) {
        app
            // Register observers for component lifecycle events
            .add_observer(restore_smoothness_on_move_end)
            // Register observers for custom events
            .add_observer(on_zoom_to_fit)
            .add_observer(on_play_animation)
            .add_observer(on_set_fit_target)
            .add_observer(on_animate_to_fit)
            // Add systems
            .add_systems(Update, process_camera_move_list);
    }
}

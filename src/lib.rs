//! Extensions for [`bevy_panorbit_camera`] that add zoom-to-fit, queued camera
//! animations, and fit-target debug visualization.
//!
//! Add [`PanOrbitCameraExtPlugin`] to your app — it registers all observers,
//! systems, and visualization infrastructure automatically. Control the camera
//! by triggering events (e.g. [`ZoomToFit`], [`AnimateToFit`], [`PlayAnimation`])
//! and observe lifecycle events (e.g. [`ZoomBegin`], [`AnimationEnd`]) to react
//! when operations start, complete, or get cancelled.
//!
//! Usage instructions in events.rs

use bevy::prelude::*;

mod animation;
mod components;
mod events;
mod fit;
mod observers;
mod support;
#[cfg(feature = "visualization")]
mod visualization;

// Animation types
pub use animation::CameraMove;
pub use animation::CameraMoveList;
use animation::process_camera_move_list;
// Components
pub use components::AnimationConflictPolicy;
pub use components::CameraInputInterruptBehavior;
pub use components::CurrentFitTarget;
#[cfg(feature = "visualization")]
pub use components::FitVisualization;
// Events
pub use events::AnimateToFit;
pub use events::AnimationBegin;
pub use events::AnimationCancelled;
pub use events::AnimationEnd;
pub use events::AnimationRejected;
pub use events::AnimationSource;
pub use events::CameraMoveBegin;
pub use events::CameraMoveEnd;
pub use events::LookAt;
pub use events::LookAtAndZoomToFit;
pub use events::PlayAnimation;
pub use events::SetFitTarget;
pub use events::ZoomBegin;
pub use events::ZoomCancelled;
pub use events::ZoomContext;
pub use events::ZoomEnd;
pub use events::ZoomToFit;
use observers::on_animate_to_fit;
use observers::on_camera_move_list_added;
use observers::on_look_at;
use observers::on_look_at_and_zoom_to_fit;
use observers::on_play_animation;
use observers::on_set_fit_target;
use observers::on_zoom_to_fit;
use observers::restore_camera_state;
// Visualization
#[cfg(feature = "visualization")]
pub use visualization::FitTargetVisualizationConfig;

/// Plugin that adds all camera extension functionality
pub struct PanOrbitCameraExtPlugin;

impl Plugin for PanOrbitCameraExtPlugin {
    fn build(&self, app: &mut App) {
        app
            // Register observers for component lifecycle events
            .add_observer(on_camera_move_list_added)
            .add_observer(restore_camera_state)
            // Register observers for custom events
            .add_observer(on_zoom_to_fit)
            .add_observer(on_play_animation)
            .add_observer(on_set_fit_target)
            .add_observer(on_animate_to_fit)
            .add_observer(on_look_at)
            .add_observer(on_look_at_and_zoom_to_fit)
            // Add systems
            .add_systems(Update, process_camera_move_list);

        #[cfg(feature = "visualization")]
        app.add_plugins(visualization::VisualizationPlugin);
    }
}

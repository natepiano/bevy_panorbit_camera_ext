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
mod visualization;

// Animation types
pub use animation::CameraMove;
pub use animation::CameraMoveList;
use animation::process_camera_move_list;
pub use components::AnimationSourceMarker;
// Components
pub use components::CurrentFitTarget;
pub use components::InterruptBehavior;
pub use components::SmoothnessStash;
pub use components::ZoomAnimationMarker;
use observers::on_animate_to_fit;
use observers::on_camera_move_list_added;
use observers::on_play_animation;
use observers::on_set_fit_target;
use observers::on_zoom_to_fit;
use observers::restore_smoothness_on_move_end;
// Visualization
pub use visualization::FitTargetVisualizationConfig;

// Events — grouped by feature, each trigger followed by its fired events.
#[rustfmt::skip]
mod _events {
    // ZoomToFit
    pub use super::events::ZoomToFit;
    pub use super::events::ZoomBegin;
    pub use super::events::ZoomEnd;
    pub use super::events::ZoomCancelled;

    // PlayAnimation
    pub use super::events::AnimationSource;
    pub use super::events::PlayAnimation;
    pub use super::events::AnimationBegin;
    pub use super::events::AnimationEnd;
    pub use super::events::AnimationCancelled;
    pub use super::events::CameraMoveBegin;
    pub use super::events::CameraMoveEnd;

    // AnimateToFit (shares PlayAnimation lifecycle events)
    pub use super::events::AnimateToFit;

    // SetFitTarget
    pub use super::events::SetFitTarget;

    // ToggleFitVisualization
    pub use super::events::ToggleFitVisualization;
    pub use super::events::FitVisualizationBegin;
    pub use super::events::FitVisualizationEnd;
}
pub use _events::*;

/// Plugin that adds all camera extension functionality
pub struct PanOrbitCameraExtPlugin;

impl Plugin for PanOrbitCameraExtPlugin {
    fn build(&self, app: &mut App) {
        app
            // Register observers for component lifecycle events
            .add_observer(on_camera_move_list_added)
            .add_observer(restore_smoothness_on_move_end)
            // Register observers for custom events
            .add_observer(on_zoom_to_fit)
            .add_observer(on_play_animation)
            .add_observer(on_set_fit_target)
            .add_observer(on_animate_to_fit)
            // Add systems
            .add_systems(Update, process_camera_move_list);

        // Register visualization systems and resources
        visualization::register(app);
    }
}

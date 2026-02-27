use bevy::prelude::*;

/// Gizmo config group for fit target visualization (screen-aligned overlay).
/// Toggle via `GizmoConfigStore::config_mut::<FitTargetGizmo>().enabled`
#[derive(Default, Reflect, GizmoConfigGroup)]
pub struct FitTargetGizmo;

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

/// Configuration for fit target visualization colors and appearance.
#[derive(Resource, Reflect, Debug, Clone)]
#[reflect(Resource)]
pub struct FitTargetVisualizationConfig {
    pub rectangle_color:  Color,
    pub silhouette_color: Color,
    pub balanced_color:   Color,
    pub unbalanced_color: Color,
    pub line_width:       f32,
}

impl Default for FitTargetVisualizationConfig {
    fn default() -> Self {
        Self {
            rectangle_color:  Color::srgb(1.0, 1.0, 0.0), // Yellow
            silhouette_color: Color::srgb(1.0, 0.5, 0.0), // Orange
            balanced_color:   Color::srgb(0.0, 1.0, 0.0), // Green
            unbalanced_color: Color::srgb(1.0, 0.0, 0.0), // Red
            line_width:       2.0,
        }
    }
}

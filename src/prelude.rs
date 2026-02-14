//! Convenient re-exports for common types and traits

pub use crate::CameraExtPlugin;
pub use crate::animation::{CameraMove, CameraMoveList};
pub use crate::extension::{
    PanOrbitCameraExt, SnapToFit, StartAnimation, ZoomToFit, ZoomToFitConfig,
};
pub use crate::smoothness::SmoothnessStash;
pub use crate::zoom::{Edge, ScreenSpaceBounds, ZoomConfig};

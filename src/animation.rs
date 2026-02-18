//! Camera movement queue and animation system
//! Allows for simple animation of camera movements with easing functions.

use std::collections::VecDeque;

use bevy::math::curve::Curve;
use bevy::math::curve::easing::EaseFunction;
use bevy::prelude::*;
use bevy_panorbit_camera::PanOrbitCamera;

use crate::components::ZoomAnimationMarker;
use crate::events::AnimationEnd;
use crate::events::CameraMoveBegin;
use crate::events::CameraMoveEnd;
use crate::events::ZoomEnd;

/// Individual camera movement with target position and duration.
///
/// Two variants allow different ways to specify the target:
/// - `ToPosition` — world-space translation + focus (for cinematic sequences)
/// - `ToOrbit` — orbital parameters around a focus (for zoom-to-fit, avoids gimbal lock)
#[derive(Clone, Reflect)]
pub enum CameraMove {
    /// Animate to a world-space position looking at a focus point.
    /// The animation system decomposes this into orbital parameters internally.
    ToPosition {
        translation: Vec3,
        focus:       Vec3,
        duration_ms: f32,
        easing:      EaseFunction,
    },
    /// Animate to orbital parameters around a focus point.
    /// Avoids gimbal lock at extreme pitch angles (±PI/2) where world-space
    /// decomposition via `atan2` loses yaw information.
    ToOrbit {
        focus:       Vec3,
        yaw:         f32,
        pitch:       f32,
        radius:      f32,
        duration_ms: f32,
        easing:      EaseFunction,
    },
}

impl CameraMove {
    pub const fn duration_ms(&self) -> f32 {
        match self {
            Self::ToPosition { duration_ms, .. } | Self::ToOrbit { duration_ms, .. } => {
                *duration_ms
            },
        }
    }

    pub const fn easing(&self) -> EaseFunction {
        match self {
            Self::ToPosition { easing, .. } | Self::ToOrbit { easing, .. } => *easing,
        }
    }

    pub const fn focus(&self) -> Vec3 {
        match self {
            Self::ToPosition { focus, .. } | Self::ToOrbit { focus, .. } => *focus,
        }
    }

    /// Returns the world-space camera position for this move.
    /// For `ToOrbit`, computes the position from orbital parameters.
    pub fn translation(&self) -> Vec3 {
        match self {
            Self::ToPosition { translation, .. } => *translation,
            Self::ToOrbit {
                focus,
                yaw,
                pitch,
                radius,
                ..
            } => {
                let yaw_rot = Quat::from_axis_angle(Vec3::Y, *yaw);
                let pitch_rot = Quat::from_axis_angle(Vec3::X, -*pitch);
                let rotation = yaw_rot * pitch_rot;
                *focus + rotation * Vec3::new(0.0, 0.0, *radius)
            },
        }
    }

    /// Returns the target orbital parameters (yaw, pitch, radius).
    /// For `ToPosition`, decomposes from the world-space offset (may lose yaw at ±PI/2 pitch).
    fn orbital_params(&self) -> (f32, f32, f32) {
        match self {
            Self::ToPosition {
                translation, focus, ..
            } => {
                let offset = *translation - *focus;
                let radius = offset.length();
                let yaw = offset.x.atan2(offset.z);
                let horizontal_dist = offset.x.hypot(offset.z);
                let pitch = offset.y.atan2(horizontal_dist);
                (yaw, pitch, radius)
            },
            Self::ToOrbit {
                yaw, pitch, radius, ..
            } => (*yaw, *pitch, *radius),
        }
    }
}

/// State tracking for the current camera movement
#[derive(Clone, Reflect, Default, Debug)]
enum MoveState {
    InProgress {
        elapsed_ms:   f32,
        start_focus:  Vec3,
        start_pitch:  f32,
        start_radius: f32,
        start_yaw:    f32,
    },
    #[default]
    Ready,
}

/// Component that queues multiple camera movements to execute sequentially
///
/// Simply add this component to a camera entity with a list of movements.
/// The system will automatically process them one by one, removing the component
/// when the queue is empty.
///
/// Camera smoothing is automatically disabled while moves are in progress and
/// restored when the queue completes via the `SmoothnessStash` observer.
#[derive(Component, Reflect, Default)]
#[reflect(Component, Default)]
pub struct CameraMoveList {
    pub moves: VecDeque<CameraMove>,
    state:     MoveState,
}

impl CameraMoveList {
    pub const fn new(moves: VecDeque<CameraMove>) -> Self {
        Self {
            moves,
            state: MoveState::Ready,
        }
    }

    /// Calculates total remaining time in milliseconds for all queued moves
    pub fn remaining_time_ms(&self) -> f32 {
        // Get remaining time for current move
        let current_remaining = match &self.state {
            MoveState::InProgress { elapsed_ms, .. } => {
                if let Some(current_move) = self.moves.front() {
                    (current_move.duration_ms() - elapsed_ms).max(0.0)
                } else {
                    0.0
                }
            },
            MoveState::Ready => self.moves.front().map_or(0.0, CameraMove::duration_ms),
        };

        // Add duration of all remaining moves (skip first since already counted)
        let remaining_queue: f32 = self.moves.iter().skip(1).map(CameraMove::duration_ms).sum();

        current_remaining + remaining_queue
    }
}

/// System that processes camera movement queues with duration-based interpolation
///
/// When a `PanOrbitCamera` has a `CameraMoveList`, interpolates toward the target over
/// the specified duration with easing. When a move completes, automatically moves to the next.
/// Removes the `CameraMoveList` component when all moves are complete.
pub fn process_camera_move_list(
    mut commands: Commands,
    time: Res<Time>,
    mut camera_query: Query<(
        Entity,
        &mut PanOrbitCamera,
        &mut CameraMoveList,
        Option<&ZoomAnimationMarker>,
    )>,
) {
    for (entity, mut pan_orbit, mut queue, zoom_marker) in &mut camera_query {
        // Get the current move from the front of the queue (clone to avoid borrow issues)
        let Some(current_move) = queue.moves.front().cloned() else {
            // Remove components BEFORE triggering events — observers may re-insert
            // `CameraMoveList` (e.g. splash animation chains hold → zoom → spins),
            // and a deferred removal after the trigger would wipe the new one.
            commands.entity(entity).remove::<CameraMoveList>();
            if let Some(marker) = zoom_marker {
                commands.entity(entity).remove::<ZoomAnimationMarker>();
                commands.trigger(ZoomEnd {
                    camera_entity: entity,
                    target_entity: marker.target_entity,
                    margin:        marker.margin,
                    duration_ms:   marker.duration_ms,
                    easing:        marker.easing,
                });
            } else {
                commands.trigger(AnimationEnd {
                    camera_entity: entity,
                });
            }
            continue;
        };

        match &mut queue.state {
            MoveState::Ready => {
                // Disable smoothing for precise control
                pan_orbit.zoom_smoothness = 0.0;
                pan_orbit.pan_smoothness = 0.0;
                pan_orbit.orbit_smoothness = 0.0;

                // Transition to InProgress with captured starting orbital parameters
                queue.state = MoveState::InProgress {
                    elapsed_ms:   0.0,
                    start_focus:  pan_orbit.target_focus,
                    start_radius: pan_orbit.target_radius,
                    start_yaw:    pan_orbit.target_yaw,
                    start_pitch:  pan_orbit.target_pitch,
                };

                if zoom_marker.is_none() {
                    commands.trigger(CameraMoveBegin {
                        camera_entity: entity,
                        camera_move:   current_move.clone(),
                    });
                }
            },
            MoveState::InProgress {
                elapsed_ms,
                start_focus,
                start_radius,
                start_yaw,
                start_pitch,
            } => {
                // Update elapsed time
                *elapsed_ms += time.delta_secs() * 1000.0;

                // Calculate interpolation factor (0.0 to 1.0)
                let t = (*elapsed_ms / current_move.duration_ms()).min(1.0);

                let is_final_frame = t >= 1.0;

                // Extract target orbital parameters
                // `ToOrbit` provides them directly; `ToPosition` decomposes via atan2
                let (canonical_yaw, canonical_pitch, canonical_radius) =
                    current_move.orbital_params();

                // Clamp t to exactly 1.0 if over (important for smooth completion)
                let t_clamped = t.min(1.0);

                // Apply easing function from the move
                let t_interp = current_move.easing().sample_unchecked(t_clamped);

                // Unwrap angles to [-PI, PI] for smooth interpolation (always, including final
                // frame). Using canonical angles on the final frame causes yaw
                // snapping when the atan2 decomposition wraps to the opposite side
                // of the PI boundary.
                let mut yaw_diff = canonical_yaw - *start_yaw;
                yaw_diff = std::f32::consts::TAU.mul_add(
                    -((yaw_diff + std::f32::consts::PI) / std::f32::consts::TAU).floor(),
                    yaw_diff,
                );

                let mut pitch_target = canonical_pitch;
                let pitch_diff_raw = pitch_target - *start_pitch;
                if pitch_diff_raw > std::f32::consts::PI {
                    pitch_target -= std::f32::consts::TAU;
                } else if pitch_diff_raw < -std::f32::consts::PI {
                    pitch_target += std::f32::consts::TAU;
                }
                let pitch_diff = pitch_target - *start_pitch;

                // `ToPosition` and `ToOrbit` are both normalized to orbital params above
                pan_orbit.target_focus = start_focus.lerp(current_move.focus(), t_interp);
                pan_orbit.target_radius =
                    (canonical_radius - *start_radius).mul_add(t_interp, *start_radius);
                pan_orbit.target_yaw = yaw_diff.mul_add(t_interp, *start_yaw);
                pan_orbit.target_pitch = pitch_diff.mul_add(t_interp, *start_pitch);
                pan_orbit.force_update = true;

                // Check if move complete and advance to next
                if is_final_frame {
                    if zoom_marker.is_none() {
                        commands.trigger(CameraMoveEnd {
                            camera_entity: entity,
                            camera_move:   current_move.clone(),
                        });
                    }
                    queue.moves.pop_front();
                    queue.state = MoveState::Ready;
                }
            },
        }
    }
}

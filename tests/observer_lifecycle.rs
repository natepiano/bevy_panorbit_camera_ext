use std::collections::VecDeque;
use std::time::Duration;

use bevy::math::curve::easing::EaseFunction;
use bevy::prelude::*;
use bevy_panorbit_camera::PanOrbitCamera;
use bevy_panorbit_camera_ext::AnimateToFit;
use bevy_panorbit_camera_ext::AnimationBegin;
use bevy_panorbit_camera_ext::AnimationCancelled;
use bevy_panorbit_camera_ext::AnimationConflictPolicy;
use bevy_panorbit_camera_ext::AnimationEnd;
use bevy_panorbit_camera_ext::AnimationRejected;
use bevy_panorbit_camera_ext::CameraInputInterruptBehavior;
use bevy_panorbit_camera_ext::CameraMove;
use bevy_panorbit_camera_ext::CameraMoveList;
use bevy_panorbit_camera_ext::CurrentFitTarget;
use bevy_panorbit_camera_ext::PanOrbitCameraExtPlugin;
use bevy_panorbit_camera_ext::PlayAnimation;
use bevy_panorbit_camera_ext::SetFitTarget;
use bevy_panorbit_camera_ext::ZoomBegin;
use bevy_panorbit_camera_ext::ZoomCancelled;
use bevy_panorbit_camera_ext::ZoomContext;
use bevy_panorbit_camera_ext::ZoomEnd;
use bevy_panorbit_camera_ext::ZoomToFit;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LifecycleEvent {
    AnimationBegin,
    AnimationEnd,
    AnimationCancelled,
    AnimationRejected,
    ZoomBegin,
    ZoomEnd,
    ZoomCancelled,
}

#[derive(Resource, Default, Debug)]
struct EventLog(Vec<LifecycleEvent>);

fn record_animation_begin(_: On<AnimationBegin>, mut log: ResMut<EventLog>) {
    log.0.push(LifecycleEvent::AnimationBegin);
}

fn record_animation_end(_: On<AnimationEnd>, mut log: ResMut<EventLog>) {
    log.0.push(LifecycleEvent::AnimationEnd);
}

fn record_animation_cancelled(_: On<AnimationCancelled>, mut log: ResMut<EventLog>) {
    log.0.push(LifecycleEvent::AnimationCancelled);
}

fn record_animation_rejected(_: On<AnimationRejected>, mut log: ResMut<EventLog>) {
    log.0.push(LifecycleEvent::AnimationRejected);
}

fn record_zoom_begin(_: On<ZoomBegin>, mut log: ResMut<EventLog>) {
    log.0.push(LifecycleEvent::ZoomBegin);
}

fn record_zoom_end(_: On<ZoomEnd>, mut log: ResMut<EventLog>) {
    log.0.push(LifecycleEvent::ZoomEnd);
}

fn record_zoom_cancelled(_: On<ZoomCancelled>, mut log: ResMut<EventLog>) {
    log.0.push(LifecycleEvent::ZoomCancelled);
}

fn add_lifecycle_log_observers(app: &mut App) {
    app.init_resource::<EventLog>();
    app.add_observer(record_animation_begin);
    app.add_observer(record_animation_end);
    app.add_observer(record_animation_cancelled);
    app.add_observer(record_animation_rejected);
    app.add_observer(record_zoom_begin);
    app.add_observer(record_zoom_end);
    app.add_observer(record_zoom_cancelled);
}

fn spawn_fit_camera_and_target(app: &mut App) -> (Entity, Entity) {
    app.init_resource::<Assets<Mesh>>();

    let camera = app.world_mut().spawn((
        PanOrbitCamera::default(),
        Camera::default(),
        Projection::Perspective(PerspectiveProjection::default()),
    ));
    let camera = camera.id();

    let mesh_handle = {
        let mut meshes = app.world_mut().resource_mut::<Assets<Mesh>>();
        meshes.add(Cuboid::new(1.0, 1.0, 1.0))
    };

    let target = app
        .world_mut()
        .spawn((Mesh3d(mesh_handle), GlobalTransform::default()))
        .id();

    (camera, target)
}

fn make_move(duration: Duration) -> CameraMove {
    CameraMove::ToOrbit {
        focus: Vec3::ZERO,
        yaw: 0.0,
        pitch: 0.0,
        radius: 5.0,
        duration,
        easing: EaseFunction::Linear,
    }
}

#[test]
fn play_animation_retrigger_preserves_original_smoothness_stash() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(PanOrbitCameraExtPlugin);

    let camera = PanOrbitCamera {
        zoom_smoothness: 0.25,
        pan_smoothness: 0.5,
        orbit_smoothness: 0.75,
        ..default()
    };

    let camera = app.world_mut().spawn(camera).id();
    let first = PlayAnimation::new(
        camera,
        VecDeque::from([make_move(Duration::from_millis(600))]),
    );
    app.world_mut().trigger(first);
    app.update();

    let camera_after_first = app
        .world()
        .get::<PanOrbitCamera>(camera)
        .expect("camera should exist");
    assert_eq!(camera_after_first.zoom_smoothness, 0.0);
    assert_eq!(camera_after_first.pan_smoothness, 0.0);
    assert_eq!(camera_after_first.orbit_smoothness, 0.0);

    {
        let mut camera_mut = app
            .world_mut()
            .get_mut::<PanOrbitCamera>(camera)
            .expect("camera should exist");
        camera_mut.zoom_smoothness = 9.0;
        camera_mut.pan_smoothness = 8.0;
        camera_mut.orbit_smoothness = 7.0;
    }

    let second = PlayAnimation::new(
        camera,
        VecDeque::from([make_move(Duration::from_millis(600))]),
    );
    app.world_mut().trigger(second);
    app.update();

    // Smoothness should still be zeroed during active animation
    let camera_after_second = app
        .world()
        .get::<PanOrbitCamera>(camera)
        .expect("camera should exist");
    assert_eq!(camera_after_second.zoom_smoothness, 0.0);
    assert_eq!(camera_after_second.pan_smoothness, 0.0);
    assert_eq!(camera_after_second.orbit_smoothness, 0.0);
}

#[test]
fn set_fit_target_event_updates_current_fit_target() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(PanOrbitCameraExtPlugin);

    let camera = app.world_mut().spawn_empty().id();
    let target_a = app.world_mut().spawn_empty().id();
    let target_b = app.world_mut().spawn_empty().id();

    app.world_mut().trigger(SetFitTarget::new(camera, target_a));
    app.update();

    let current = app
        .world()
        .get::<CurrentFitTarget>(camera)
        .expect("fit target should be set by observer");
    assert_eq!(current.0, target_a);

    app.world_mut().trigger(SetFitTarget::new(camera, target_b));
    app.update();

    let current = app
        .world()
        .get::<CurrentFitTarget>(camera)
        .expect("fit target should update on repeated events");
    assert_eq!(current.0, target_b);
}

#[test]
fn direct_camera_move_list_insertion_stashes_and_disables_smoothness() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(PanOrbitCameraExtPlugin);

    let camera = PanOrbitCamera {
        zoom_smoothness: 0.2,
        pan_smoothness: 0.3,
        orbit_smoothness: 0.4,
        ..default()
    };

    let camera = app.world_mut().spawn(camera).id();
    app.world_mut()
        .entity_mut(camera)
        .insert(CameraMoveList::new(VecDeque::from([make_move(
            Duration::from_millis(500),
        )])));
    app.update();

    let camera_after = app
        .world()
        .get::<PanOrbitCamera>(camera)
        .expect("camera should exist");
    assert_eq!(camera_after.zoom_smoothness, 0.0);
    assert_eq!(camera_after.pan_smoothness, 0.0);
    assert_eq!(camera_after.orbit_smoothness, 0.0);
}

#[test]
fn zoom_to_fit_zero_duration_emits_zoom_begin_then_zoom_end_without_animation_queue() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(PanOrbitCameraExtPlugin);
    add_lifecycle_log_observers(&mut app);

    let (camera, target) = spawn_fit_camera_and_target(&mut app);

    app.world_mut()
        .trigger(ZoomToFit::new(camera, target).easing(EaseFunction::Linear));
    app.update();

    let log = app.world().resource::<EventLog>();
    assert_eq!(
        log.0,
        vec![LifecycleEvent::ZoomBegin, LifecycleEvent::ZoomEnd]
    );
    assert!(app.world().get::<CameraMoveList>(camera).is_none());
}

#[test]
fn animate_to_fit_zero_duration_emits_animation_begin_then_end_without_animation_queue() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(PanOrbitCameraExtPlugin);
    add_lifecycle_log_observers(&mut app);

    let (camera, target) = spawn_fit_camera_and_target(&mut app);

    app.world_mut()
        .trigger(AnimateToFit::new(camera, target).easing(EaseFunction::Linear));
    app.update();

    let log = app.world().resource::<EventLog>();
    assert_eq!(
        log.0,
        vec![LifecycleEvent::AnimationBegin, LifecycleEvent::AnimationEnd]
    );
    assert!(app.world().get::<CameraMoveList>(camera).is_none());
}

#[test]
fn interrupt_cancel_emits_cancelled_and_restores_smoothness_without_jumping_to_final() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(PanOrbitCameraExtPlugin);
    add_lifecycle_log_observers(&mut app);

    let camera = PanOrbitCamera {
        zoom_smoothness: 0.25,
        pan_smoothness: 0.5,
        orbit_smoothness: 0.75,
        ..default()
    };
    let camera = app
        .world_mut()
        .spawn((camera, CameraInputInterruptBehavior::Cancel))
        .id();

    let first_move = CameraMove::ToOrbit {
        focus:    Vec3::new(1.0, 2.0, 3.0),
        yaw:      0.4,
        pitch:    -0.2,
        radius:   6.0,
        duration: Duration::from_millis(800),
        easing:   EaseFunction::Linear,
    };
    app.world_mut()
        .trigger(PlayAnimation::new(camera, VecDeque::from([first_move])));
    app.update();

    let sentinel_focus = Vec3::new(100.0, 200.0, 300.0);
    let sentinel_yaw = 1.25;
    let sentinel_pitch = -0.75;
    let sentinel_radius = 12.5;
    {
        let mut camera = app
            .world_mut()
            .get_mut::<PanOrbitCamera>(camera)
            .expect("camera should exist");
        camera.target_focus = sentinel_focus;
        camera.target_yaw = sentinel_yaw;
        camera.target_pitch = sentinel_pitch;
        camera.target_radius = sentinel_radius;
    }

    app.update();
    app.update();

    let log = app.world().resource::<EventLog>();
    assert_eq!(
        log.0,
        vec![
            LifecycleEvent::AnimationBegin,
            LifecycleEvent::AnimationCancelled
        ]
    );
    assert!(app.world().get::<CameraMoveList>(camera).is_none());

    let camera = app
        .world()
        .get::<PanOrbitCamera>(camera)
        .expect("camera should exist");
    assert_eq!(camera.zoom_smoothness, 0.25);
    assert_eq!(camera.pan_smoothness, 0.5);
    assert_eq!(camera.orbit_smoothness, 0.75);
    assert!(camera.enabled);
    assert_eq!(camera.target_focus, sentinel_focus);
    assert_eq!(camera.target_yaw, sentinel_yaw);
    assert_eq!(camera.target_pitch, sentinel_pitch);
    assert_eq!(camera.target_radius, sentinel_radius);
}

#[test]
fn interrupt_complete_emits_end_jumps_to_final_and_restores_smoothness() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(PanOrbitCameraExtPlugin);
    add_lifecycle_log_observers(&mut app);

    let camera = PanOrbitCamera {
        zoom_smoothness: 0.15,
        pan_smoothness: 0.35,
        orbit_smoothness: 0.55,
        ..default()
    };
    let camera = app
        .world_mut()
        .spawn((camera, CameraInputInterruptBehavior::Complete))
        .id();

    let first_move = CameraMove::ToOrbit {
        focus:    Vec3::new(1.0, 0.0, 0.0),
        yaw:      0.1,
        pitch:    0.2,
        radius:   4.0,
        duration: Duration::from_millis(900),
        easing:   EaseFunction::Linear,
    };
    let final_move = CameraMove::ToOrbit {
        focus:    Vec3::new(9.0, 8.0, 7.0),
        yaw:      0.9,
        pitch:    -0.4,
        radius:   11.0,
        duration: Duration::from_millis(900),
        easing:   EaseFunction::Linear,
    };
    app.world_mut().trigger(PlayAnimation::new(
        camera,
        VecDeque::from([first_move, final_move.clone()]),
    ));
    app.update();

    {
        let mut camera = app
            .world_mut()
            .get_mut::<PanOrbitCamera>(camera)
            .expect("camera should exist");
        camera.target_focus = Vec3::new(-1.0, -1.0, -1.0);
        camera.target_yaw = -1.0;
        camera.target_pitch = -1.0;
        camera.target_radius = 2.0;
    }

    app.update();
    app.update();

    let log = app.world().resource::<EventLog>();
    assert_eq!(
        log.0,
        vec![LifecycleEvent::AnimationBegin, LifecycleEvent::AnimationEnd]
    );
    assert!(app.world().get::<CameraMoveList>(camera).is_none());

    let camera = app
        .world()
        .get::<PanOrbitCamera>(camera)
        .expect("camera should exist");
    assert_eq!(camera.zoom_smoothness, 0.15);
    assert_eq!(camera.pan_smoothness, 0.35);
    assert_eq!(camera.orbit_smoothness, 0.55);
    assert!(camera.enabled);
    match final_move {
        CameraMove::ToOrbit {
            focus,
            yaw,
            pitch,
            radius,
            ..
        } => {
            assert_eq!(camera.target_focus, focus);
            assert_eq!(camera.target_yaw, yaw);
            assert_eq!(camera.target_pitch, pitch);
            assert_eq!(camera.target_radius, radius);
        },
        CameraMove::ToPosition { .. } => unreachable!("test uses ToOrbit final move"),
    }
}

#[test]
fn interrupt_ignore_keeps_animation_running_and_emits_no_interrupt_events() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(PanOrbitCameraExtPlugin);
    add_lifecycle_log_observers(&mut app);

    let camera = app
        .world_mut()
        .spawn((
            PanOrbitCamera::default(),
            CameraInputInterruptBehavior::Ignore,
        ))
        .id();

    app.world_mut().trigger(PlayAnimation::new(
        camera,
        VecDeque::from([make_move(Duration::from_millis(5000))]),
    ));
    app.update();

    // Isolate interrupt behavior events.
    app.world_mut().resource_mut::<EventLog>().0.clear();

    let sentinel_focus = Vec3::new(999.0, 888.0, 777.0);
    let sentinel_yaw = 1.25;
    let sentinel_pitch = -0.75;
    let sentinel_radius = 12.5;
    {
        let mut camera = app
            .world_mut()
            .get_mut::<PanOrbitCamera>(camera)
            .expect("camera should exist");
        camera.target_focus = sentinel_focus;
        camera.target_yaw = sentinel_yaw;
        camera.target_pitch = sentinel_pitch;
        camera.target_radius = sentinel_radius;
    }

    app.update();
    app.update();

    let log = app.world().resource::<EventLog>();
    assert_eq!(log.0, Vec::<LifecycleEvent>::new());
    assert!(app.world().get::<CameraMoveList>(camera).is_some());

    let camera = app
        .world()
        .get::<PanOrbitCamera>(camera)
        .expect("camera should exist");
    assert!(!camera.enabled);
    assert_ne!(camera.target_focus, sentinel_focus);
    assert_ne!(camera.target_yaw, sentinel_yaw);
    assert_ne!(camera.target_pitch, sentinel_pitch);
    assert_ne!(camera.target_radius, sentinel_radius);
}

#[test]
fn interrupt_default_is_ignore() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(PanOrbitCameraExtPlugin);
    add_lifecycle_log_observers(&mut app);

    // No CameraInputInterruptBehavior component inserted — should default to Ignore.
    let camera = app.world_mut().spawn(PanOrbitCamera::default()).id();

    app.world_mut().trigger(PlayAnimation::new(
        camera,
        VecDeque::from([make_move(Duration::from_millis(5000))]),
    ));
    app.update();
    app.world_mut().resource_mut::<EventLog>().0.clear();

    let camera_before_interrupt = app
        .world()
        .get::<PanOrbitCamera>(camera)
        .expect("camera should exist");
    assert!(!camera_before_interrupt.enabled);

    {
        let mut camera = app
            .world_mut()
            .get_mut::<PanOrbitCamera>(camera)
            .expect("camera should exist");
        camera.target_focus = Vec3::new(50.0, 60.0, 70.0);
    }

    app.update();

    let log = app.world().resource::<EventLog>();
    assert_eq!(log.0, Vec::<LifecycleEvent>::new());
    assert!(app.world().get::<CameraMoveList>(camera).is_some());
}

#[test]
fn interrupt_ignore_restores_original_enabled_state_after_completion() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(PanOrbitCameraExtPlugin);
    add_lifecycle_log_observers(&mut app);

    let camera = PanOrbitCamera {
        enabled: false,
        ..default()
    };
    let camera = app
        .world_mut()
        .spawn((camera, CameraInputInterruptBehavior::Ignore))
        .id();

    app.world_mut().trigger(PlayAnimation::new(
        camera,
        VecDeque::from([make_move(Duration::ZERO)]),
    ));

    // Start animation and stash state.
    app.update();
    // Drain queue and remove CameraMoveList, triggering restore.
    app.update();
    app.update();

    let camera = app
        .world()
        .get::<PanOrbitCamera>(camera)
        .expect("camera should exist");
    assert!(!camera.enabled);
}

#[test]
fn normal_completion_restores_smoothness_after_queue_finishes() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(PanOrbitCameraExtPlugin);
    add_lifecycle_log_observers(&mut app);

    let camera = PanOrbitCamera {
        zoom_smoothness: 0.45,
        pan_smoothness: 0.55,
        orbit_smoothness: 0.65,
        ..default()
    };
    let camera = app.world_mut().spawn(camera).id();

    app.world_mut().trigger(PlayAnimation::new(
        camera,
        VecDeque::from([make_move(Duration::ZERO)]),
    ));
    app.update();
    app.update();
    app.update();

    let log = app.world().resource::<EventLog>();
    assert_eq!(
        log.0,
        vec![LifecycleEvent::AnimationBegin, LifecycleEvent::AnimationEnd]
    );
    assert!(app.world().get::<CameraMoveList>(camera).is_none());

    let camera = app
        .world()
        .get::<PanOrbitCamera>(camera)
        .expect("camera should exist");
    assert_eq!(camera.zoom_smoothness, 0.45);
    assert_eq!(camera.pan_smoothness, 0.55);
    assert_eq!(camera.orbit_smoothness, 0.65);
}

#[test]
fn conflict_last_wins_animation_cancels_animation() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(PanOrbitCameraExtPlugin);
    add_lifecycle_log_observers(&mut app);

    let camera = app
        .world_mut()
        .spawn((PanOrbitCamera::default(), AnimationConflictPolicy::LastWins))
        .id();

    // Start first animation (long duration so it's still in-flight)
    app.world_mut().trigger(PlayAnimation::new(
        camera,
        VecDeque::from([make_move(Duration::from_millis(5000))]),
    ));
    app.update();

    // Clear the log to isolate the second trigger's events
    app.world_mut().resource_mut::<EventLog>().0.clear();

    // Trigger second animation while first is in-flight
    app.world_mut().trigger(PlayAnimation::new(
        camera,
        VecDeque::from([make_move(Duration::from_millis(500))]),
    ));
    app.update();

    let log = app.world().resource::<EventLog>();
    assert_eq!(
        log.0,
        vec![
            LifecycleEvent::AnimationCancelled,
            LifecycleEvent::AnimationBegin,
        ]
    );
}

#[test]
fn conflict_first_wins_rejects_second() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(PanOrbitCameraExtPlugin);
    add_lifecycle_log_observers(&mut app);

    let camera = app
        .world_mut()
        .spawn((
            PanOrbitCamera::default(),
            AnimationConflictPolicy::FirstWins,
        ))
        .id();

    // Start first animation
    app.world_mut().trigger(PlayAnimation::new(
        camera,
        VecDeque::from([make_move(Duration::from_millis(5000))]),
    ));
    app.update();

    // Clear the log
    app.world_mut().resource_mut::<EventLog>().0.clear();

    // Trigger second animation — should be rejected
    app.world_mut().trigger(PlayAnimation::new(
        camera,
        VecDeque::from([make_move(Duration::from_millis(500))]),
    ));
    app.update();

    let log = app.world().resource::<EventLog>();
    assert_eq!(log.0, vec![LifecycleEvent::AnimationRejected]);

    // Original queue should still be present
    assert!(app.world().get::<CameraMoveList>(camera).is_some());
}

#[test]
fn conflict_first_wins_allows_after_completion() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(PanOrbitCameraExtPlugin);
    add_lifecycle_log_observers(&mut app);

    let camera = app
        .world_mut()
        .spawn((
            PanOrbitCamera::default(),
            AnimationConflictPolicy::FirstWins,
        ))
        .id();

    // Start a zero-duration animation (completes instantly)
    app.world_mut().trigger(PlayAnimation::new(
        camera,
        VecDeque::from([make_move(Duration::ZERO)]),
    ));
    app.update();
    app.update();
    app.update();

    // Clear the log
    app.world_mut().resource_mut::<EventLog>().0.clear();

    // New animation should succeed since queue is gone
    app.world_mut().trigger(PlayAnimation::new(
        camera,
        VecDeque::from([make_move(Duration::ZERO)]),
    ));
    app.update();
    app.update();
    app.update();

    let log = app.world().resource::<EventLog>();
    assert_eq!(
        log.0,
        vec![LifecycleEvent::AnimationBegin, LifecycleEvent::AnimationEnd]
    );
}

#[test]
fn conflict_default_is_last_wins() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(PanOrbitCameraExtPlugin);
    add_lifecycle_log_observers(&mut app);

    // No AnimationConflictPolicy component — should default to LastWins
    let camera = app.world_mut().spawn(PanOrbitCamera::default()).id();

    // Start first animation
    app.world_mut().trigger(PlayAnimation::new(
        camera,
        VecDeque::from([make_move(Duration::from_millis(5000))]),
    ));
    app.update();

    // Clear the log
    app.world_mut().resource_mut::<EventLog>().0.clear();

    // Trigger second animation — should cancel first (LastWins behavior)
    app.world_mut().trigger(PlayAnimation::new(
        camera,
        VecDeque::from([make_move(Duration::from_millis(500))]),
    ));
    app.update();

    let log = app.world().resource::<EventLog>();
    assert_eq!(
        log.0,
        vec![
            LifecycleEvent::AnimationCancelled,
            LifecycleEvent::AnimationBegin,
        ]
    );
}

fn make_zoom_context() -> ZoomContext {
    ZoomContext {
        target:   Entity::PLACEHOLDER,
        margin:   0.1,
        duration: Duration::from_millis(500),
        easing:   EaseFunction::Linear,
    }
}

// ---------------------------------------------------------------------------
// Zoom lifecycle: cancellation / rejection / interrupt scenarios
// ---------------------------------------------------------------------------

#[test]
fn zoom_animated_first_wins_rejection_emits_only_animation_rejected() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(PanOrbitCameraExtPlugin);
    add_lifecycle_log_observers(&mut app);

    let (camera, target) = spawn_fit_camera_and_target(&mut app);
    app.world_mut()
        .entity_mut(camera)
        .insert(AnimationConflictPolicy::FirstWins);

    // Start a long animation so it's still in-flight
    app.world_mut().trigger(PlayAnimation::new(
        camera,
        VecDeque::from([make_move(Duration::from_millis(5000))]),
    ));
    app.update();

    // Clear the log to isolate the zoom trigger's events
    app.world_mut().resource_mut::<EventLog>().0.clear();

    // Trigger ZoomToFit (animated) — should be rejected with no ZoomBegin leak
    app.world_mut().trigger(
        ZoomToFit::new(camera, target)
            .duration(Duration::from_millis(500))
            .easing(EaseFunction::Linear),
    );
    app.update();

    let log = app.world().resource::<EventLog>();
    assert_eq!(log.0, vec![LifecycleEvent::AnimationRejected]);
}

#[test]
fn zoom_animated_last_wins_cancels_plain_animation() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(PanOrbitCameraExtPlugin);
    add_lifecycle_log_observers(&mut app);

    let camera = app
        .world_mut()
        .spawn((PanOrbitCamera::default(), AnimationConflictPolicy::LastWins))
        .id();

    // Start a long plain animation (no zoom context)
    app.world_mut().trigger(PlayAnimation::new(
        camera,
        VecDeque::from([make_move(Duration::from_millis(5000))]),
    ));
    app.update();

    app.world_mut().resource_mut::<EventLog>().0.clear();

    // Trigger zoom animation — should cancel plain, then begin zoom
    app.world_mut().trigger(
        PlayAnimation::new(
            camera,
            VecDeque::from([make_move(Duration::from_millis(500))]),
        )
        .zoom_context(make_zoom_context()),
    );
    app.update();

    let log = app.world().resource::<EventLog>();
    assert_eq!(
        log.0,
        vec![
            LifecycleEvent::AnimationCancelled,
            LifecycleEvent::ZoomBegin,
            LifecycleEvent::AnimationBegin,
        ]
    );
}

#[test]
fn zoom_animated_last_wins_cancels_in_flight_zoom() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(PanOrbitCameraExtPlugin);
    add_lifecycle_log_observers(&mut app);

    let camera = app
        .world_mut()
        .spawn((PanOrbitCamera::default(), AnimationConflictPolicy::LastWins))
        .id();

    // Start a long zoom animation
    app.world_mut().trigger(
        PlayAnimation::new(
            camera,
            VecDeque::from([make_move(Duration::from_millis(5000))]),
        )
        .zoom_context(make_zoom_context()),
    );
    app.update();

    app.world_mut().resource_mut::<EventLog>().0.clear();

    // Trigger another zoom — should cancel in-flight zoom, then begin new zoom
    app.world_mut().trigger(
        PlayAnimation::new(
            camera,
            VecDeque::from([make_move(Duration::from_millis(500))]),
        )
        .zoom_context(make_zoom_context()),
    );
    app.update();

    let log = app.world().resource::<EventLog>();
    assert_eq!(
        log.0,
        vec![
            LifecycleEvent::AnimationCancelled,
            LifecycleEvent::ZoomCancelled,
            LifecycleEvent::ZoomBegin,
            LifecycleEvent::AnimationBegin,
        ]
    );
}

#[test]
fn zoom_animated_cancel_interrupt_emits_cancelled_and_zoom_cancelled() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(PanOrbitCameraExtPlugin);
    add_lifecycle_log_observers(&mut app);

    let camera = app
        .world_mut()
        .spawn((
            PanOrbitCamera::default(),
            CameraInputInterruptBehavior::Cancel,
        ))
        .id();

    // Start a zoom animation
    app.world_mut().trigger(
        PlayAnimation::new(
            camera,
            VecDeque::from([make_move(Duration::from_millis(5000))]),
        )
        .zoom_context(make_zoom_context()),
    );
    app.update();

    // Simulate external user input by modifying camera targets
    {
        let mut camera = app
            .world_mut()
            .get_mut::<PanOrbitCamera>(camera)
            .expect("camera should exist");
        camera.target_focus = Vec3::new(100.0, 200.0, 300.0);
        camera.target_yaw = 1.25;
        camera.target_pitch = -0.75;
        camera.target_radius = 12.5;
    }

    app.update();
    app.update();

    let log = app.world().resource::<EventLog>();
    assert_eq!(
        log.0,
        vec![
            LifecycleEvent::ZoomBegin,
            LifecycleEvent::AnimationBegin,
            LifecycleEvent::AnimationCancelled,
            LifecycleEvent::ZoomCancelled,
        ]
    );
    assert!(app.world().get::<CameraMoveList>(camera).is_none());
}

#[test]
fn zoom_animated_complete_interrupt_emits_end_and_zoom_end() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(PanOrbitCameraExtPlugin);
    add_lifecycle_log_observers(&mut app);

    let camera = app
        .world_mut()
        .spawn((
            PanOrbitCamera::default(),
            CameraInputInterruptBehavior::Complete,
        ))
        .id();

    // Start a zoom animation
    app.world_mut().trigger(
        PlayAnimation::new(
            camera,
            VecDeque::from([make_move(Duration::from_millis(5000))]),
        )
        .zoom_context(make_zoom_context()),
    );
    app.update();

    // Simulate external user input
    {
        let mut camera = app
            .world_mut()
            .get_mut::<PanOrbitCamera>(camera)
            .expect("camera should exist");
        camera.target_focus = Vec3::new(-1.0, -1.0, -1.0);
        camera.target_yaw = -1.0;
        camera.target_pitch = -1.0;
        camera.target_radius = 2.0;
    }

    app.update();
    app.update();

    let log = app.world().resource::<EventLog>();
    assert_eq!(
        log.0,
        vec![
            LifecycleEvent::ZoomBegin,
            LifecycleEvent::AnimationBegin,
            LifecycleEvent::AnimationEnd,
            LifecycleEvent::ZoomEnd,
        ]
    );
    assert!(app.world().get::<CameraMoveList>(camera).is_none());
}

#[test]
fn zoom_animated_normal_completion_emits_full_lifecycle() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(PanOrbitCameraExtPlugin);
    add_lifecycle_log_observers(&mut app);

    let camera = app.world_mut().spawn(PanOrbitCamera::default()).id();

    // Use a zero-duration CameraMove so the queue drains immediately
    app.world_mut().trigger(
        PlayAnimation::new(camera, VecDeque::from([make_move(Duration::ZERO)]))
            .zoom_context(make_zoom_context()),
    );
    app.update();
    app.update();
    app.update();

    let log = app.world().resource::<EventLog>();
    assert_eq!(
        log.0,
        vec![
            LifecycleEvent::ZoomBegin,
            LifecycleEvent::AnimationBegin,
            LifecycleEvent::AnimationEnd,
            LifecycleEvent::ZoomEnd,
        ]
    );
    assert!(app.world().get::<CameraMoveList>(camera).is_none());
}

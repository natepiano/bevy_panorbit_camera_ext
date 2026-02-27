use std::collections::VecDeque;
use std::time::Duration;

use bevy::math::curve::easing::EaseFunction;
use bevy::prelude::*;
use bevy_panorbit_camera::PanOrbitCamera;
use bevy_panorbit_camera_ext::AnimateToFit;
use bevy_panorbit_camera_ext::AnimationBegin;
use bevy_panorbit_camera_ext::AnimationCancelled;
use bevy_panorbit_camera_ext::AnimationEnd;
use bevy_panorbit_camera_ext::CameraMove;
use bevy_panorbit_camera_ext::CameraMoveList;
use bevy_panorbit_camera_ext::CurrentFitTarget;
use bevy_panorbit_camera_ext::InterruptBehavior;
use bevy_panorbit_camera_ext::PanOrbitCameraExtPlugin;
use bevy_panorbit_camera_ext::PlayAnimation;
use bevy_panorbit_camera_ext::SetFitTarget;
use bevy_panorbit_camera_ext::SmoothnessStash;
use bevy_panorbit_camera_ext::ZoomBegin;
use bevy_panorbit_camera_ext::ZoomEnd;
use bevy_panorbit_camera_ext::ZoomToFit;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LifecycleEvent {
    AnimationBegin,
    AnimationEnd,
    AnimationCancelled,
    ZoomBegin,
    ZoomEnd,
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

fn record_zoom_begin(_: On<ZoomBegin>, mut log: ResMut<EventLog>) {
    log.0.push(LifecycleEvent::ZoomBegin);
}

fn record_zoom_end(_: On<ZoomEnd>, mut log: ResMut<EventLog>) {
    log.0.push(LifecycleEvent::ZoomEnd);
}

fn add_lifecycle_log_observers(app: &mut App) {
    app.init_resource::<EventLog>();
    app.add_observer(record_animation_begin);
    app.add_observer(record_animation_end);
    app.add_observer(record_animation_cancelled);
    app.add_observer(record_zoom_begin);
    app.add_observer(record_zoom_end);
}

fn spawn_fit_camera_and_target(app: &mut App) -> (Entity, Entity) {
    app.init_resource::<Assets<Mesh>>();

    let camera = app.world_mut().spawn((
        PanOrbitCamera::default(),
        Camera::default(),
        Projection::Perspective(PerspectiveProjection::default()),
    ));
    let camera_entity = camera.id();

    let mesh_handle = {
        let mut meshes = app.world_mut().resource_mut::<Assets<Mesh>>();
        meshes.add(Cuboid::new(1.0, 1.0, 1.0))
    };

    let target_entity = app
        .world_mut()
        .spawn((Mesh3d(mesh_handle), GlobalTransform::default()))
        .id();

    (camera_entity, target_entity)
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

    let mut camera = PanOrbitCamera::default();
    camera.zoom_smoothness = 0.25;
    camera.pan_smoothness = 0.5;
    camera.orbit_smoothness = 0.75;

    let camera_entity = app.world_mut().spawn(camera).id();
    let first = PlayAnimation::new(
        camera_entity,
        VecDeque::from([make_move(Duration::from_millis(600))]),
    );
    app.world_mut().trigger(first);
    app.update();

    let stash = *app
        .world()
        .get::<SmoothnessStash>(camera_entity)
        .expect("stash should be inserted on first animation start");
    assert_eq!(stash.zoom, 0.25);
    assert_eq!(stash.pan, 0.5);
    assert_eq!(stash.orbit, 0.75);

    let camera_after_first = app
        .world()
        .get::<PanOrbitCamera>(camera_entity)
        .expect("camera should exist");
    assert_eq!(camera_after_first.zoom_smoothness, 0.0);
    assert_eq!(camera_after_first.pan_smoothness, 0.0);
    assert_eq!(camera_after_first.orbit_smoothness, 0.0);

    {
        let mut camera_mut = app
            .world_mut()
            .get_mut::<PanOrbitCamera>(camera_entity)
            .expect("camera should exist");
        camera_mut.zoom_smoothness = 9.0;
        camera_mut.pan_smoothness = 8.0;
        camera_mut.orbit_smoothness = 7.0;
    }

    let second = PlayAnimation::new(
        camera_entity,
        VecDeque::from([make_move(Duration::from_millis(600))]),
    );
    app.world_mut().trigger(second);
    app.update();

    let stash_after_second = *app
        .world()
        .get::<SmoothnessStash>(camera_entity)
        .expect("stash should remain present during active animation");
    assert_eq!(stash_after_second.zoom, 0.25);
    assert_eq!(stash_after_second.pan, 0.5);
    assert_eq!(stash_after_second.orbit, 0.75);
}

#[test]
fn set_fit_target_event_updates_current_fit_target() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(PanOrbitCameraExtPlugin);

    let camera_entity = app.world_mut().spawn_empty().id();
    let target_a = app.world_mut().spawn_empty().id();
    let target_b = app.world_mut().spawn_empty().id();

    app.world_mut()
        .trigger(SetFitTarget::new(camera_entity, target_a));
    app.update();

    let current = app
        .world()
        .get::<CurrentFitTarget>(camera_entity)
        .expect("fit target should be set by observer");
    assert_eq!(current.0, target_a);

    app.world_mut()
        .trigger(SetFitTarget::new(camera_entity, target_b));
    app.update();

    let current = app
        .world()
        .get::<CurrentFitTarget>(camera_entity)
        .expect("fit target should update on repeated events");
    assert_eq!(current.0, target_b);
}

#[test]
fn direct_camera_move_list_insertion_stashes_and_disables_smoothness() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(PanOrbitCameraExtPlugin);

    let mut camera = PanOrbitCamera::default();
    camera.zoom_smoothness = 0.2;
    camera.pan_smoothness = 0.3;
    camera.orbit_smoothness = 0.4;

    let camera_entity = app.world_mut().spawn(camera).id();
    app.world_mut()
        .entity_mut(camera_entity)
        .insert(CameraMoveList::new(VecDeque::from([make_move(
            Duration::from_millis(500),
        )])));
    app.update();

    let stash = *app
        .world()
        .get::<SmoothnessStash>(camera_entity)
        .expect("stash should be inserted for direct CameraMoveList additions");
    assert_eq!(stash.zoom, 0.2);
    assert_eq!(stash.pan, 0.3);
    assert_eq!(stash.orbit, 0.4);

    let camera_after = app
        .world()
        .get::<PanOrbitCamera>(camera_entity)
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

    let (camera_entity, target_entity) = spawn_fit_camera_and_target(&mut app);

    app.world_mut().trigger(ZoomToFit::new(
        camera_entity,
        target_entity,
        0.1,
        Duration::ZERO,
        EaseFunction::Linear,
    ));
    app.update();

    let log = app.world().resource::<EventLog>();
    assert_eq!(
        log.0,
        vec![LifecycleEvent::ZoomBegin, LifecycleEvent::ZoomEnd]
    );
    assert!(app.world().get::<CameraMoveList>(camera_entity).is_none());
}

#[test]
fn animate_to_fit_zero_duration_emits_animation_begin_then_end_without_animation_queue() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(PanOrbitCameraExtPlugin);
    add_lifecycle_log_observers(&mut app);

    let (camera_entity, target_entity) = spawn_fit_camera_and_target(&mut app);

    app.world_mut().trigger(AnimateToFit::new(
        camera_entity,
        target_entity,
        0.0,
        0.0,
        0.1,
        Duration::ZERO,
        EaseFunction::Linear,
    ));
    app.update();

    let log = app.world().resource::<EventLog>();
    assert_eq!(
        log.0,
        vec![LifecycleEvent::AnimationBegin, LifecycleEvent::AnimationEnd]
    );
    assert!(app.world().get::<CameraMoveList>(camera_entity).is_none());
}

#[test]
fn interrupt_cancel_emits_cancelled_and_restores_smoothness_without_jumping_to_final() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(PanOrbitCameraExtPlugin);
    add_lifecycle_log_observers(&mut app);

    let mut camera = PanOrbitCamera::default();
    camera.zoom_smoothness = 0.25;
    camera.pan_smoothness = 0.5;
    camera.orbit_smoothness = 0.75;
    let camera_entity = app
        .world_mut()
        .spawn((camera, InterruptBehavior::Cancel))
        .id();

    let first_move = CameraMove::ToOrbit {
        focus:    Vec3::new(1.0, 2.0, 3.0),
        yaw:      0.4,
        pitch:    -0.2,
        radius:   6.0,
        duration: Duration::from_millis(800),
        easing:   EaseFunction::Linear,
    };
    app.world_mut().trigger(PlayAnimation::new(
        camera_entity,
        VecDeque::from([first_move]),
    ));
    app.update();

    let sentinel_focus = Vec3::new(100.0, 200.0, 300.0);
    let sentinel_yaw = 1.25;
    let sentinel_pitch = -0.75;
    let sentinel_radius = 12.5;
    {
        let mut camera = app
            .world_mut()
            .get_mut::<PanOrbitCamera>(camera_entity)
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
    assert!(app.world().get::<CameraMoveList>(camera_entity).is_none());
    assert!(app.world().get::<SmoothnessStash>(camera_entity).is_none());

    let camera = app
        .world()
        .get::<PanOrbitCamera>(camera_entity)
        .expect("camera should exist");
    assert_eq!(camera.zoom_smoothness, 0.25);
    assert_eq!(camera.pan_smoothness, 0.5);
    assert_eq!(camera.orbit_smoothness, 0.75);
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

    let mut camera = PanOrbitCamera::default();
    camera.zoom_smoothness = 0.15;
    camera.pan_smoothness = 0.35;
    camera.orbit_smoothness = 0.55;
    let camera_entity = app
        .world_mut()
        .spawn((camera, InterruptBehavior::Complete))
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
        camera_entity,
        VecDeque::from([first_move, final_move.clone()]),
    ));
    app.update();

    {
        let mut camera = app
            .world_mut()
            .get_mut::<PanOrbitCamera>(camera_entity)
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
    assert!(app.world().get::<CameraMoveList>(camera_entity).is_none());
    assert!(app.world().get::<SmoothnessStash>(camera_entity).is_none());

    let camera = app
        .world()
        .get::<PanOrbitCamera>(camera_entity)
        .expect("camera should exist");
    assert_eq!(camera.zoom_smoothness, 0.15);
    assert_eq!(camera.pan_smoothness, 0.35);
    assert_eq!(camera.orbit_smoothness, 0.55);
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
fn normal_completion_restores_smoothness_after_queue_finishes() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(PanOrbitCameraExtPlugin);
    add_lifecycle_log_observers(&mut app);

    let mut camera = PanOrbitCamera::default();
    camera.zoom_smoothness = 0.45;
    camera.pan_smoothness = 0.55;
    camera.orbit_smoothness = 0.65;
    let camera_entity = app.world_mut().spawn(camera).id();

    app.world_mut().trigger(PlayAnimation::new(
        camera_entity,
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
    assert!(app.world().get::<CameraMoveList>(camera_entity).is_none());
    assert!(app.world().get::<SmoothnessStash>(camera_entity).is_none());

    let camera = app
        .world()
        .get::<PanOrbitCamera>(camera_entity)
        .expect("camera should exist");
    assert_eq!(camera.zoom_smoothness, 0.45);
    assert_eq!(camera.pan_smoothness, 0.55);
    assert_eq!(camera.orbit_smoothness, 0.65);
}

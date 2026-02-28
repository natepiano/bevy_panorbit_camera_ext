//! Demonstrates clicking on meshes to zoom-to-fit using `bevy_panorbit_camera_ext`.
//!
//! - Click a mesh to select it and zoom the camera to frame it
//! - Click the ground to deselect and zoom out to the full scene
//! - Drag a mesh to rotate it
//! - Selected meshes show a gizmo outline
//! - Press 'D' to toggle debug visualization of zoom-to-fit bounds

use std::collections::VecDeque;
use std::f32::consts::PI;
use std::time::Duration;

use bevy::camera::ScalingMode;
use bevy::color::palettes::basic::SILVER;
use bevy::color::palettes::css::DEEP_SKY_BLUE;
use bevy::light::CascadeShadowConfig;
use bevy::light::CascadeShadowConfigBuilder;
use bevy::light::DirectionalLightShadowMap;
use bevy::math::curve::easing::EaseFunction;
use bevy::prelude::*;
use bevy_brp_extras::BrpExtrasPlugin;
use bevy_panorbit_camera::PanOrbitCamera;
use bevy_panorbit_camera::PanOrbitCameraPlugin;
use bevy_panorbit_camera::TrackpadBehavior;
use bevy_panorbit_camera_ext::AnimateToFit;
use bevy_panorbit_camera_ext::AnimationBegin;
use bevy_panorbit_camera_ext::AnimationCancelled;
use bevy_panorbit_camera_ext::AnimationEnd;
use bevy_panorbit_camera_ext::CameraMove;
use bevy_panorbit_camera_ext::CameraMoveBegin;
use bevy_panorbit_camera_ext::CameraMoveEnd;
use bevy_panorbit_camera_ext::FitVisualizationBegin;
use bevy_panorbit_camera_ext::FitVisualizationEnd;
use bevy_panorbit_camera_ext::InputInterruptBehavior;
use bevy_panorbit_camera_ext::PanOrbitCameraExtPlugin;
use bevy_panorbit_camera_ext::PlayAnimation;
use bevy_panorbit_camera_ext::ToggleFitVisualization;
use bevy_panorbit_camera_ext::ZoomBegin;
use bevy_panorbit_camera_ext::ZoomCancelled;
use bevy_panorbit_camera_ext::ZoomEnd;
use bevy_panorbit_camera_ext::ZoomToFit;

const ZOOM_DURATION_MS: f32 = 500.0;
const ZOOM_MARGIN_MESH: f32 = 0.25;
const ZOOM_MARGIN_SCENE: f32 = 0.08;
const GIZMO_SCALE: f32 = 1.03;
const DRAG_SENSITIVITY: f32 = 0.02;
const MESH_CENTER_Y: f32 = 1.0;
const EVENT_LOG_FONT_SIZE: f32 = 14.0;
const EVENT_LINE_LIFETIME_SECS: f32 = 8.0;
const ANIMATE_FIT_DURATION_MS: f32 = 1200.0;
const CAMERA_START_YAW: f32 = -0.2;
const CAMERA_START_PITCH: f32 = 0.4;
const ORBIT_MOVE_DURATION_MS: f32 = 800.0;
const CASCADE_MAX_DISTANCE_PERSPECTIVE: f32 = 20.0;
const CASCADE_MAX_DISTANCE_ORTHOGRAPHIC: f32 = 40.0;

fn cascade_shadow_config_perspective() -> CascadeShadowConfig {
    CascadeShadowConfigBuilder {
        maximum_distance: CASCADE_MAX_DISTANCE_PERSPECTIVE,
        first_cascade_far_bound: 5.0,
        ..default()
    }
    .build()
}

fn cascade_shadow_config_orthographic() -> CascadeShadowConfig {
    CascadeShadowConfigBuilder {
        num_cascades: 4,
        maximum_distance: CASCADE_MAX_DISTANCE_ORTHOGRAPHIC,
        first_cascade_far_bound: 4.0,
        ..default()
    }
    .build()
}

#[derive(States, Debug, Clone, PartialEq, Eq, Hash, Default)]
enum AppState {
    #[default]
    Loading,
    Running,
}

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins,
            PanOrbitCameraPlugin,
            PanOrbitCameraExtPlugin,
            MeshPickingPlugin,
            BrpExtrasPlugin::default(),
        ))
        .insert_resource(DirectionalLightShadowMap { size: 4096 })
        .init_state::<AppState>()
        .init_resource::<ActiveEasing>()
        .init_resource::<DebugVisualizationActive>()
        .init_resource::<EventLog>()
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            initial_fit_to_scene.run_if(in_state(AppState::Loading)),
        )
        .add_systems(
            Update,
            (
                draw_selection_gizmo,
                toggle_debug_visualization,
                toggle_projection,
                randomize_easing,
                animate_camera,
                animate_fit_to_scene,
                toggle_interrupt_behavior,
                update_event_log_text,
            ),
        )
        .add_observer(log_animation_start)
        .add_observer(log_animation_begin)
        .add_observer(log_animation_cancelled)
        .add_observer(log_camera_move_start)
        .add_observer(log_camera_move_end)
        .add_observer(log_zoom_begin)
        .add_observer(log_zoom_end)
        .add_observer(log_zoom_cancelled)
        .add_observer(log_fit_visualization_begin)
        .add_observer(log_fit_visualization_end)
        .run();
}

#[derive(Component)]
struct Selected;

#[derive(Component)]
struct EventLogNode;

#[derive(Component)]
struct InputInterruptBehaviorLabel;

struct EventLine {
    text:      String,
    timestamp: f32,
}

#[derive(Resource)]
struct ActiveEasing(EaseFunction);

impl Default for ActiveEasing {
    fn default() -> Self { Self(EaseFunction::CubicOut) }
}

const ALL_EASINGS: &[EaseFunction] = &[
    EaseFunction::Linear,
    EaseFunction::QuadraticIn,
    EaseFunction::QuadraticOut,
    EaseFunction::QuadraticInOut,
    EaseFunction::CubicIn,
    EaseFunction::CubicOut,
    EaseFunction::CubicInOut,
    EaseFunction::QuarticIn,
    EaseFunction::QuarticOut,
    EaseFunction::QuarticInOut,
    EaseFunction::QuinticIn,
    EaseFunction::QuinticOut,
    EaseFunction::QuinticInOut,
    EaseFunction::SmoothStepIn,
    EaseFunction::SmoothStepOut,
    EaseFunction::SmoothStep,
    EaseFunction::SmootherStepIn,
    EaseFunction::SmootherStepOut,
    EaseFunction::SmootherStep,
    EaseFunction::SineIn,
    EaseFunction::SineOut,
    EaseFunction::SineInOut,
    EaseFunction::CircularIn,
    EaseFunction::CircularOut,
    EaseFunction::CircularInOut,
    EaseFunction::ExponentialIn,
    EaseFunction::ExponentialOut,
    EaseFunction::ExponentialInOut,
    EaseFunction::ElasticIn,
    EaseFunction::ElasticOut,
    EaseFunction::ElasticInOut,
    EaseFunction::BackIn,
    EaseFunction::BackOut,
    EaseFunction::BackInOut,
    EaseFunction::BounceIn,
    EaseFunction::BounceOut,
    EaseFunction::BounceInOut,
];

#[derive(Resource, Default)]
struct DebugVisualizationActive(bool);

#[derive(Resource, Default)]
struct EventLog {
    lines: Vec<EventLine>,
    dirty: bool,
}

#[derive(Component)]
enum MeshShape {
    Cuboid(Vec3),
    Sphere(f32),
    Torus {
        minor_radius: f32,
        major_radius: f32,
    },
}

#[derive(Resource)]
struct SceneEntities {
    camera:       Entity,
    scene_bounds: Entity,
    light:        Entity,
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Ground plane (clickable from above — deselects and zooms to scene bounds)
    let ground = commands
        .spawn((
            Mesh3d(meshes.add(Plane3d::default().mesh().size(12.0, 12.0))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::from(SILVER).with_alpha(0.85),
                alpha_mode: AlphaMode::Blend,
                double_sided: true,
                cull_mode: None,
                ..default()
            })),
        ))
        .observe(on_ground_clicked)
        .id();

    // Underside plane (clickable from below — deselects and animates back to scene)
    commands
        .spawn((
            Mesh3d(meshes.add(Plane3d::default().mesh().size(12.0, 12.0))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::srgba(0.0, 0.0, 0.0, 0.0),
                alpha_mode: AlphaMode::Blend,
                unlit: true,
                ..default()
            })),
            Transform::from_rotation(Quat::from_rotation_x(PI)),
        ))
        .observe(on_below_clicked);

    // Directional light
    let light = commands
        .spawn((
            DirectionalLight {
                illuminance: 1500.0,
                shadows_enabled: true,
                ..default()
            },
            cascade_shadow_config_perspective(),
            Transform::from_rotation(Quat::from_euler(EulerRot::ZYX, 0.0, PI / 4.0, -PI / 4.0)),
        ))
        .id();

    // Cuboid
    let cuboid_size = Vec3::new(1.0, 1.0, 1.0);
    commands
        .spawn((
            Mesh3d(meshes.add(Cuboid::new(cuboid_size.x, cuboid_size.y, cuboid_size.z))),
            MeshMaterial3d(materials.add(Color::srgb(0.5, 0.5, 0.9))),
            Transform::from_xyz(-2.5, MESH_CENTER_Y, 0.0),
            MeshShape::Cuboid(cuboid_size),
        ))
        .observe(on_mesh_clicked)
        .observe(on_mesh_dragged);

    // Sphere
    let sphere_radius = 0.5;
    commands
        .spawn((
            Mesh3d(meshes.add(Sphere::new(sphere_radius).mesh().uv(128, 64))),
            MeshMaterial3d(materials.add(Color::srgb(0.9, 0.3, 0.2))),
            Transform::from_xyz(0.0, MESH_CENTER_Y, 0.0),
            MeshShape::Sphere(sphere_radius),
        ))
        .observe(on_mesh_clicked)
        .observe(on_mesh_dragged);

    // Torus
    let torus_minor = 0.25;
    let torus_major = 0.75;
    commands
        .spawn((
            Mesh3d(
                meshes.add(
                    Torus::new(torus_minor, torus_major)
                        .mesh()
                        .minor_resolution(64)
                        .major_resolution(64),
                ),
            ),
            MeshMaterial3d(materials.add(Color::srgb(0.3, 0.8, 0.4))),
            Transform::from_xyz(2.5, MESH_CENTER_Y, 0.0),
            MeshShape::Torus {
                minor_radius: torus_minor,
                major_radius: torus_major,
            },
        ))
        .observe(on_mesh_clicked)
        .observe(on_mesh_dragged);

    // Camera (middle-click orbit, shift+middle pan, trackpad support)
    let camera = commands
        .spawn(PanOrbitCamera {
            button_orbit: MouseButton::Middle,
            button_pan: MouseButton::Middle,
            modifier_pan: Some(KeyCode::ShiftLeft),
            trackpad_behavior: TrackpadBehavior::BlenderLike {
                modifier_pan:  Some(KeyCode::ShiftLeft),
                modifier_zoom: Some(KeyCode::ControlLeft),
            },
            trackpad_pinch_to_zoom_enabled: true,
            yaw: Some(CAMERA_START_YAW),
            pitch: Some(CAMERA_START_PITCH),
            ..default()
        })
        .id();

    // Instructions
    commands.spawn((
        Text::new("Click a mesh to zoom-to-fit\nClick the ground to zoom back out\n\nPress:\n'P' toggle projection\n'D' debug visualization\n'F' animate fit to scene\n'A' animate camera\n'R' randomize easing\n'C' reset to 'CubicOut' easing\n'I' toggle interrupt behavior"),
        TextFont {
            font_size: 13.0,
            ..default()
        },
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(12.0),
            left: Val::Px(12.0),
            ..default()
        },
    ));

    // Interrupt behavior hint (bottom-left)
    commands.spawn((
        Text::new(interrupt_behavior_hint_text(InputInterruptBehavior::Cancel)),
        TextFont {
            font_size: 13.0,
            ..default()
        },
        TextColor(Color::srgba(0.7, 0.7, 0.7, 0.7)),
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(12.0),
            left: Val::Px(12.0),
            ..default()
        },
        InputInterruptBehaviorLabel,
    ));

    // Event log display (top-right, grows downward)
    commands.spawn((
        Text::new(""),
        TextFont {
            font_size: EVENT_LOG_FONT_SIZE,
            ..default()
        },
        TextColor(Color::srgba(0.0, 1.0, 0.0, 0.9)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(12.0),
            right: Val::Px(12.0),
            ..default()
        },
        EventLogNode,
    ));

    commands.insert_resource(SceneEntities {
        camera,
        scene_bounds: ground,
        light,
    });
}

fn initial_fit_to_scene(
    mut commands: Commands,
    scene: Res<SceneEntities>,
    mesh_query: Query<&Mesh3d>,
    meshes: Res<Assets<Mesh>>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    let Ok(mesh3d) = mesh_query.get(scene.scene_bounds) else {
        return;
    };
    if meshes.get(&mesh3d.0).is_none() {
        return;
    }
    commands.trigger(
        AnimateToFit::new(scene.camera, scene.scene_bounds)
            .yaw(CAMERA_START_YAW)
            .pitch(CAMERA_START_PITCH)
            .margin(ZOOM_MARGIN_SCENE)
            .easing(EaseFunction::QuadraticInOut),
    );
    next_state.set(AppState::Running);
}

fn on_mesh_clicked(
    click: On<Pointer<Click>>,
    mut commands: Commands,
    scene: Res<SceneEntities>,
    selected: Query<Entity, With<Selected>>,
    active_easing: Res<ActiveEasing>,
) {
    for entity in &selected {
        commands.entity(entity).remove::<Selected>();
    }

    let clicked = click.entity;
    commands.entity(clicked).insert(Selected);
    commands.trigger(
        ZoomToFit::new(scene.camera, clicked)
            .margin(ZOOM_MARGIN_MESH)
            .duration(Duration::from_secs_f32(ZOOM_DURATION_MS / 1000.0))
            .easing(active_easing.0),
    );
}

fn on_ground_clicked(
    _click: On<Pointer<Click>>,
    mut commands: Commands,
    scene: Res<SceneEntities>,
    selected: Query<Entity, With<Selected>>,
    active_easing: Res<ActiveEasing>,
) {
    for entity in &selected {
        commands.entity(entity).remove::<Selected>();
    }

    commands.trigger(
        ZoomToFit::new(scene.camera, scene.scene_bounds)
            .margin(ZOOM_MARGIN_SCENE)
            .duration(Duration::from_secs_f32(ZOOM_DURATION_MS / 1000.0))
            .easing(active_easing.0),
    );
}

fn on_below_clicked(
    _click: On<Pointer<Click>>,
    mut commands: Commands,
    scene: Res<SceneEntities>,
    selected: Query<Entity, With<Selected>>,
    active_easing: Res<ActiveEasing>,
) {
    for entity in &selected {
        commands.entity(entity).remove::<Selected>();
    }

    commands.trigger(
        AnimateToFit::new(scene.camera, scene.scene_bounds)
            .yaw(CAMERA_START_YAW)
            .pitch(CAMERA_START_PITCH)
            .margin(ZOOM_MARGIN_SCENE)
            .duration(Duration::from_secs_f32(ANIMATE_FIT_DURATION_MS / 1000.0))
            .easing(active_easing.0),
    );
}

fn on_mesh_dragged(drag: On<Pointer<Drag>>, mut transforms: Query<&mut Transform>) {
    if let Ok(mut transform) = transforms.get_mut(drag.entity) {
        transform.rotate_y(drag.delta.x * DRAG_SENSITIVITY);
        transform.rotate_x(drag.delta.y * DRAG_SENSITIVITY);
    }
}

fn toggle_debug_visualization(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    mut active: ResMut<DebugVisualizationActive>,
    scene: Res<SceneEntities>,
) {
    if keyboard.just_pressed(KeyCode::KeyD) {
        active.0 = !active.0;
        commands.trigger(ToggleFitVisualization::new(scene.camera));
    }
}

fn draw_selection_gizmo(
    mut gizmos: Gizmos,
    debug_active: Res<DebugVisualizationActive>,
    query: Query<(&Transform, &MeshShape), With<Selected>>,
) {
    // Hide selection gizmo when debug visualization is active
    if debug_active.0 {
        return;
    }

    let color = Color::from(DEEP_SKY_BLUE);
    for (transform, shape) in &query {
        match shape {
            MeshShape::Cuboid(size) => {
                gizmos.cube(
                    Transform::from_translation(transform.translation)
                        .with_rotation(transform.rotation)
                        .with_scale(*size * GIZMO_SCALE),
                    color,
                );
            },
            MeshShape::Sphere(radius) => {
                gizmos.sphere(
                    Isometry3d::new(transform.translation, transform.rotation),
                    radius * GIZMO_SCALE,
                    color,
                );
            },
            MeshShape::Torus {
                minor_radius,
                major_radius,
            } => {
                gizmos.primitive_3d(
                    &Torus::new(*minor_radius * GIZMO_SCALE, *major_radius * GIZMO_SCALE),
                    Isometry3d::new(transform.translation, transform.rotation),
                    color,
                );
            },
        }
    }
}

fn animate_camera(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    scene: Res<SceneEntities>,
    easing: Res<ActiveEasing>,
    camera_query: Query<&PanOrbitCamera>,
) {
    if !keyboard.just_pressed(KeyCode::KeyA) {
        return;
    }

    let Ok(camera) = camera_query.get(scene.camera) else {
        return;
    };

    let e = easing.0;
    let yaw = camera.target_yaw;
    let pitch = camera.target_pitch;
    let radius = camera.target_radius;
    let focus = camera.target_focus;
    let half_pi = PI / 2.0;

    // 4 camera moves that orbit PI/2 at a time from the current position
    let camera_moves = VecDeque::from([
        CameraMove::ToOrbit {
            focus,
            yaw: yaw + half_pi,
            pitch,
            radius,
            duration: Duration::from_secs_f32(ORBIT_MOVE_DURATION_MS / 1000.0),
            easing: e,
        },
        CameraMove::ToOrbit {
            focus,
            yaw: yaw + half_pi * 2.0,
            pitch,
            radius,
            duration: Duration::from_secs_f32(ORBIT_MOVE_DURATION_MS / 1000.0),
            easing: e,
        },
        CameraMove::ToOrbit {
            focus,
            yaw: yaw + half_pi * 3.0,
            pitch,
            radius,
            duration: Duration::from_secs_f32(ORBIT_MOVE_DURATION_MS / 1000.0),
            easing: e,
        },
        CameraMove::ToOrbit {
            focus,
            yaw: yaw + half_pi * 4.0,
            pitch,
            radius,
            duration: Duration::from_secs_f32(ORBIT_MOVE_DURATION_MS / 1000.0),
            easing: e,
        },
    ]);

    commands.trigger(PlayAnimation::new(scene.camera, camera_moves));
}

fn randomize_easing(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut easing: ResMut<ActiveEasing>,
    time: Res<Time>,
    mut log: ResMut<EventLog>,
) {
    if keyboard.just_pressed(KeyCode::KeyR) {
        let index = (time.elapsed_secs() * 1000.0) as usize % ALL_EASINGS.len();
        easing.0 = ALL_EASINGS[index];
        log.push(format!("Easing: {:#?}", easing.0), &time);
    }
    if keyboard.just_pressed(KeyCode::KeyC) {
        easing.0 = EaseFunction::CubicOut;
        log.push("Easing: reset to CubicOut".into(), &time);
    }
}

fn animate_fit_to_scene(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    scene: Res<SceneEntities>,
    easing: Res<ActiveEasing>,
) {
    if !keyboard.just_pressed(KeyCode::KeyF) {
        return;
    }

    commands.trigger(
        AnimateToFit::new(scene.camera, scene.scene_bounds)
            .yaw(CAMERA_START_YAW)
            .pitch(CAMERA_START_PITCH)
            .margin(ZOOM_MARGIN_SCENE)
            .duration(Duration::from_secs_f32(ANIMATE_FIT_DURATION_MS / 1000.0))
            .easing(easing.0),
    );
}

/// Toggles between perspective and orthographic projection, then re-fits the scene.
///
/// The fit is deferred one frame via `pending_fit` because `PanOrbitCamera` needs to
/// process the projection change (syncing radius ↔ orthographic scale) before the
/// fit calculation can produce correct results.
#[allow(clippy::too_many_arguments)]
fn toggle_projection(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    scene: Res<SceneEntities>,
    active_easing: Res<ActiveEasing>,
    mut camera_query: Query<(&mut Projection, &mut PanOrbitCamera)>,
    time: Res<Time>,
    mut log: ResMut<EventLog>,
    mut pending_fit: Local<bool>,
) {
    // Deferred fit: projection was changed last frame, `PanOrbitCamera` has now synced.
    if *pending_fit {
        *pending_fit = false;
        commands.trigger(
            AnimateToFit::new(scene.camera, scene.scene_bounds)
                .yaw(CAMERA_START_YAW)
                .pitch(CAMERA_START_PITCH)
                .margin(ZOOM_MARGIN_SCENE)
                .duration(Duration::from_secs_f32(ANIMATE_FIT_DURATION_MS / 1000.0))
                .easing(active_easing.0),
        );
        return;
    }

    if !keyboard.just_pressed(KeyCode::KeyP) {
        return;
    }
    let Ok((mut projection, mut camera)) = camera_query.single_mut() else {
        return;
    };
    match *projection {
        Projection::Perspective(_) => {
            *projection = Projection::from(OrthographicProjection {
                scaling_mode: ScalingMode::FixedVertical {
                    viewport_height: 1.0,
                },
                far: 40.0,
                ..OrthographicProjection::default_3d()
            });
            commands
                .entity(scene.light)
                .insert(cascade_shadow_config_orthographic());
            log.push("Projection: Orthographic".into(), &time);
        },
        Projection::Orthographic(_) => {
            *projection = Projection::Perspective(PerspectiveProjection::default());
            commands
                .entity(scene.light)
                .insert(cascade_shadow_config_perspective());
            log.push("Projection: Perspective".into(), &time);
        },
        _ => {},
    }
    camera.force_update = true;
    *pending_fit = true;
}

fn interrupt_behavior_hint_text(behavior: InputInterruptBehavior) -> String {
    match behavior {
        InputInterruptBehavior::Cancel => {
            "InterruptBehavior::Cancel - camera input during animation will cancel it".into()
        },
        InputInterruptBehavior::Complete => {
            "InputInterruptBehavior::Complete - camera input during animation will jump to final position"
                .into()
        },
    }
}

fn toggle_interrupt_behavior(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    scene: Res<SceneEntities>,
    mut behavior_query: Query<&mut InputInterruptBehavior>,
    mut hint_query: Query<&mut Text, With<InputInterruptBehaviorLabel>>,
    time: Res<Time>,
    mut log: ResMut<EventLog>,
) {
    if !keyboard.just_pressed(KeyCode::KeyI) {
        return;
    }

    let Ok(mut behavior) = behavior_query.get_mut(scene.camera) else {
        // No `InputInterruptBehavior` on camera yet — insert one (toggled from default)
        commands
            .entity(scene.camera)
            .insert(InputInterruptBehavior::Complete);
        for mut text in &mut hint_query {
            **text = interrupt_behavior_hint_text(InputInterruptBehavior::Complete);
        }
        log.push("InputInterruptBehavior: Complete".into(), &time);
        return;
    };

    let new_behavior = match *behavior {
        InputInterruptBehavior::Cancel => InputInterruptBehavior::Complete,
        InputInterruptBehavior::Complete => InputInterruptBehavior::Cancel,
    };
    *behavior = new_behavior;

    for mut text in &mut hint_query {
        **text = interrupt_behavior_hint_text(new_behavior);
    }
    log.push(format!("InputInterruptBehavior: {new_behavior:?}"), &time);
}

// ============================================================================
// Event log
// ============================================================================

impl EventLog {
    fn push(&mut self, text: String, time: &Time) {
        self.lines.push(EventLine {
            text,
            timestamp: time.elapsed_secs(),
        });
        self.dirty = true;
    }
}

fn fmt_vec3(v: Vec3) -> String { format!("({:.1}, {:.1}, {:.1})", v.x, v.y, v.z) }

fn log_animation_start(event: On<AnimationBegin>, time: Res<Time>, mut log: ResMut<EventLog>) {
    log.push(
        format!("AnimationBegin\n  source={:?}", event.source),
        &time,
    );
}

fn log_animation_begin(_event: On<AnimationEnd>, time: Res<Time>, mut log: ResMut<EventLog>) {
    log.push("AnimationEnd".into(), &time);
}

fn log_camera_move_start(event: On<CameraMoveBegin>, time: Res<Time>, mut log: ResMut<EventLog>) {
    log.push(
        format!(
            "CameraMoveBegin\n  translation={}\n  focus={}\n  duration={:.0}ms\n  easing={:?}",
            fmt_vec3(event.camera_move.translation()),
            fmt_vec3(event.camera_move.focus()),
            event.camera_move.duration_ms(),
            event.camera_move.easing(),
        ),
        &time,
    );
}

fn log_camera_move_end(_event: On<CameraMoveEnd>, time: Res<Time>, mut log: ResMut<EventLog>) {
    log.push("CameraMoveEnd".to_string(), &time);
}

fn log_zoom_begin(event: On<ZoomBegin>, time: Res<Time>, mut log: ResMut<EventLog>) {
    log.push(
        format!(
            "ZoomBegin\n  margin={:.2}\n  duration={:.0}ms\n  easing={:?}",
            event.margin,
            event.duration.as_secs_f32() * 1000.0,
            event.easing,
        ),
        &time,
    );
}

fn log_zoom_end(_event: On<ZoomEnd>, time: Res<Time>, mut log: ResMut<EventLog>) {
    log.push("ZoomEnd".to_string(), &time);
}

fn log_animation_cancelled(
    event: On<AnimationCancelled>,
    time: Res<Time>,
    mut log: ResMut<EventLog>,
) {
    log.push(
        format!(
            "AnimationCancelled\n  source={:?}\n  move_translation={}\n  move_focus={}",
            event.source,
            fmt_vec3(event.camera_move.translation()),
            fmt_vec3(event.camera_move.focus()),
        ),
        &time,
    );
}

fn log_zoom_cancelled(_event: On<ZoomCancelled>, time: Res<Time>, mut log: ResMut<EventLog>) {
    log.push("ZoomCancelled".to_string(), &time);
}

fn log_fit_visualization_begin(
    event: On<FitVisualizationBegin>,
    time: Res<Time>,
    mut log: ResMut<EventLog>,
) {
    log.push(
        format!("FitVisualizationBegin\n  camera={:?}", event.camera_entity),
        &time,
    );
}

fn log_fit_visualization_end(
    event: On<FitVisualizationEnd>,
    time: Res<Time>,
    mut log: ResMut<EventLog>,
) {
    log.push(
        format!("FitVisualizationEnd\n  camera={:?}", event.camera_entity),
        &time,
    );
}

fn update_event_log_text(
    time: Res<Time>,
    mut log: ResMut<EventLog>,
    mut query: Query<&mut Text, With<EventLogNode>>,
) {
    let now = time.elapsed_secs();
    let prev_len = log.lines.len();
    log.lines
        .retain(|line| now - line.timestamp < EVENT_LINE_LIFETIME_SECS);
    let expired = log.lines.len() != prev_len;

    if !log.dirty && !expired {
        return;
    }
    log.dirty = false;

    let display: String = log
        .lines
        .iter()
        .map(|line| line.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");

    for mut text in &mut query {
        **text = display.clone();
    }
}

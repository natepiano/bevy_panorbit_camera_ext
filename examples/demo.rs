//! Demonstrates clicking on meshes to zoom-to-fit using `bevy_panorbit_camera_ext`.
//!
//! - Click a mesh to select it and zoom the camera to frame it
//! - Click the ground to deselect and zoom out to the full scene
//! - Drag a mesh to rotate it
//! - Selected meshes show a gizmo outline
//! - Press 'D' to toggle debug visualization of zoom-to-fit bounds

use std::collections::VecDeque;
use std::f32::consts::PI;

use bevy::color::palettes::basic::SILVER;
use bevy::color::palettes::css::DEEP_SKY_BLUE;
use bevy::math::curve::easing::EaseFunction;
use bevy::prelude::*;
use bevy_brp_extras::BrpExtrasPlugin;
use bevy_panorbit_camera::PanOrbitCamera;
use bevy_panorbit_camera::PanOrbitCameraPlugin;
use bevy_panorbit_camera::TrackpadBehavior;
use bevy_panorbit_camera_ext::AnimateToFit;
use bevy_panorbit_camera_ext::AnimationBegin;
use bevy_panorbit_camera_ext::AnimationEnd;
use bevy_panorbit_camera_ext::CameraMove;
use bevy_panorbit_camera_ext::CameraMoveBegin;
use bevy_panorbit_camera_ext::CameraMoveEnd;
use bevy_panorbit_camera_ext::FitTargetGizmo;
use bevy_panorbit_camera_ext::FitTargetVisualizationPlugin;
use bevy_panorbit_camera_ext::PanOrbitCameraExtPlugin;
use bevy_panorbit_camera_ext::PlayAnimation;
use bevy_panorbit_camera_ext::ZoomBegin;
use bevy_panorbit_camera_ext::ZoomEnd;
use bevy_panorbit_camera_ext::ZoomToFit;

const ZOOM_DURATION_MS: f32 = 500.0;
const ZOOM_MARGIN: f32 = 0.25;
const GIZMO_SCALE: f32 = 1.03;
const DRAG_SENSITIVITY: f32 = 0.02;
const MESH_CENTER_Y: f32 = 1.0;
const EVENT_LOG_FONT_SIZE: f32 = 14.0;
const EVENT_LINE_LIFETIME_SECS: f32 = 8.0;
const ANIMATE_FIT_DURATION_MS: f32 = 1200.0;
const CAMERA_START_PITCH: f32 = 0.6;
const ORBIT_MOVE_DURATION_MS: f32 = 800.0;
const ORBIT_RADIUS: f32 = 8.0;
const ORBIT_FOCUS: Vec3 = Vec3::new(0.0, 1.0, 0.0);
const ORBIT_HEIGHT: f32 = 3.0;

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
            FitTargetVisualizationPlugin,
            MeshPickingPlugin,
            BrpExtrasPlugin::default(),
        ))
        .init_state::<AppState>()
        .init_resource::<ActiveEasing>()
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
                randomize_easing,
                animate_camera,
                animate_fit_to_scene,
                update_event_log_text,
            ),
        )
        .add_observer(log_animation_start)
        .add_observer(log_animation_begin)
        .add_observer(log_camera_move_start)
        .add_observer(log_camera_move_end)
        .add_observer(log_zoom_begin)
        .add_observer(log_zoom_end)
        .run();
}

#[derive(Component)]
struct Selected;

#[derive(Component)]
struct EventLogNode;

struct EventLine {
    text:      String,
    timestamp: f32,
}

#[derive(Resource)]
struct ActiveEasing(EaseFunction);

impl Default for ActiveEasing {
    fn default() -> Self { Self(EaseFunction::BackOut) }
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
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Ground plane (clickable — clicking it deselects and zooms out)
    commands
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
        .observe(on_ground_clicked);

    // Directional light
    commands.spawn((
        DirectionalLight {
            illuminance: 1500.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::ZYX, 0.0, PI / 4.0, -PI / 4.0)),
    ));

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

    // Invisible scene bounds sphere (zoom-out target)
    let scene_bounds = commands
        .spawn((
            Mesh3d(meshes.add(Sphere::new(3.5).mesh().uv(128, 64))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::srgba(0.0, 0.0, 0.0, 0.0),
                alpha_mode: AlphaMode::Blend,
                unlit: true,
                ..default()
            })),
            Transform::from_xyz(0.0, 1.0, 0.0),
            Pickable::IGNORE,
        ))
        .id();

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
            radius: Some(8.0),
            pitch: Some(CAMERA_START_PITCH),
            ..default()
        })
        .id();

    // Instructions
    commands.spawn((
        Text::new("Press:\n'D' debug visualization\n'F' animate fit to scene\n'A' animate camera\n'R' randomize easing\n'B' reset to 'BackOut' easing"),
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
        scene_bounds,
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
    commands.trigger(ZoomToFit::new(
        scene.camera,
        scene.scene_bounds,
        ZOOM_MARGIN,
        0.0,
        EaseFunction::QuadraticInOut,
    ));
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
    commands.trigger(ZoomToFit::new(
        scene.camera,
        clicked,
        ZOOM_MARGIN,
        ZOOM_DURATION_MS,
        active_easing.0,
    ));
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

    commands.trigger(ZoomToFit::new(
        scene.camera,
        scene.scene_bounds,
        ZOOM_MARGIN,
        ZOOM_DURATION_MS,
        active_easing.0,
    ));
}

fn on_mesh_dragged(drag: On<Pointer<Drag>>, mut transforms: Query<&mut Transform>) {
    if let Ok(mut transform) = transforms.get_mut(drag.entity) {
        transform.rotate_y(drag.delta.x * DRAG_SENSITIVITY);
        transform.rotate_x(drag.delta.y * DRAG_SENSITIVITY);
    }
}

fn toggle_debug_visualization(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut config_store: ResMut<GizmoConfigStore>,
) {
    if keyboard.just_pressed(KeyCode::KeyD) {
        let (config, _) = config_store.config_mut::<FitTargetGizmo>();
        config.enabled = !config.enabled;
    }
}

fn draw_selection_gizmo(
    mut gizmos: Gizmos,
    config_store: Res<GizmoConfigStore>,
    query: Query<(&Transform, &MeshShape), With<Selected>>,
) {
    // Hide selection gizmo when debug visualization is active
    let (debug_config, _) = config_store.config::<FitTargetGizmo>();
    if debug_config.enabled {
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
) {
    if !keyboard.just_pressed(KeyCode::KeyA) {
        return;
    }

    let e = easing.0;

    // 4 moves that orbit around the focus point
    let moves = VecDeque::from([
        CameraMove::ToPosition {
            translation: Vec3::new(ORBIT_RADIUS, ORBIT_HEIGHT, 0.0) + ORBIT_FOCUS,
            focus:       ORBIT_FOCUS,
            duration_ms: ORBIT_MOVE_DURATION_MS,
            easing:      e,
        },
        CameraMove::ToPosition {
            translation: Vec3::new(0.0, ORBIT_HEIGHT, -ORBIT_RADIUS) + ORBIT_FOCUS,
            focus:       ORBIT_FOCUS,
            duration_ms: ORBIT_MOVE_DURATION_MS,
            easing:      e,
        },
        CameraMove::ToPosition {
            translation: Vec3::new(-ORBIT_RADIUS, ORBIT_HEIGHT, 0.0) + ORBIT_FOCUS,
            focus:       ORBIT_FOCUS,
            duration_ms: ORBIT_MOVE_DURATION_MS,
            easing:      e,
        },
        CameraMove::ToPosition {
            translation: Vec3::new(0.0, ORBIT_HEIGHT, ORBIT_RADIUS) + ORBIT_FOCUS,
            focus:       ORBIT_FOCUS,
            duration_ms: ORBIT_MOVE_DURATION_MS,
            easing:      e,
        },
    ]);

    commands.trigger(PlayAnimation::new(scene.camera, moves));
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
    if keyboard.just_pressed(KeyCode::KeyB) {
        easing.0 = EaseFunction::BackOut;
        log.push("Easing: reset to Backout".into(), &time);
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

    commands.trigger(AnimateToFit::new(
        scene.camera,
        scene.scene_bounds,
        0.0,
        CAMERA_START_PITCH,
        ZOOM_MARGIN,
        ANIMATE_FIT_DURATION_MS,
        easing.0,
    ));
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

fn log_animation_start(_event: On<AnimationBegin>, time: Res<Time>, mut log: ResMut<EventLog>) {
    log.push("AnimationBegin".into(), &time);
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
            "ZoomBegin\n  margin={:.1}\n  duration={:.0}ms\n  easing={:?}",
            event.margin, event.duration_ms, event.easing,
        ),
        &time,
    );
}

fn log_zoom_end(_event: On<ZoomEnd>, time: Res<Time>, mut log: ResMut<EventLog>) {
    log.push("ZoomEnd".to_string(), &time);
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

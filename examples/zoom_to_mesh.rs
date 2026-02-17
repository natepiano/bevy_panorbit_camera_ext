//! Demonstrates clicking on meshes to zoom-to-fit using `bevy_panorbit_camera_ext`.
//!
//! - Click a mesh to select it and zoom the camera to frame it
//! - Click the ground to deselect and zoom out to the full scene
//! - Drag a mesh to rotate it
//! - Selected meshes show a gizmo outline
//! - Press 'D' to toggle debug visualization of zoom-to-fit bounds

use std::f32::consts::PI;

use bevy::color::palettes::basic::SILVER;
use bevy::color::palettes::css::DEEP_SKY_BLUE;
use bevy::prelude::*;
use bevy_brp_extras::BrpExtrasPlugin;
use bevy_panorbit_camera::PanOrbitCamera;
use bevy_panorbit_camera::PanOrbitCameraPlugin;
use bevy_panorbit_camera::TrackpadBehavior;
use bevy_panorbit_camera_ext::CameraExtPlugin;
use bevy_panorbit_camera_ext::FitTargetGizmo;
use bevy_panorbit_camera_ext::FitTargetVisualizationPlugin;
use bevy_panorbit_camera_ext::ZoomToFit;

const ZOOM_DURATION_MS: f32 = 500.0;
const ZOOM_MARGIN: f32 = 0.25;
const GIZMO_SCALE: f32 = 1.03;
const DRAG_SENSITIVITY: f32 = 0.02;
const MESH_CENTER_Y: f32 = 1.0;

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins,
            PanOrbitCameraPlugin,
            CameraExtPlugin,
            FitTargetVisualizationPlugin,
            MeshPickingPlugin,
            BrpExtrasPlugin::default(),
        ))
        .add_systems(Startup, setup)
        .add_systems(Update, (draw_selection_gizmo, toggle_debug_visualization))
        .run();
}

#[derive(Component)]
struct Selected;

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
            Mesh3d(meshes.add(Plane3d::default().mesh().size(50.0, 50.0))),
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
            Mesh3d(meshes.add(Sphere::new(sphere_radius))),
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
            Mesh3d(meshes.add(Torus::new(torus_minor, torus_major))),
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
            Mesh3d(meshes.add(Sphere::new(5.0))),
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
            pitch: Some(0.3),
            ..default()
        })
        .id();

    // Instructions
    commands.spawn((
        Text::new("Press 'D' for debug visualization"),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(12.0),
            left: Val::Px(12.0),
            ..default()
        },
    ));

    commands.insert_resource(SceneEntities {
        camera,
        scene_bounds,
    });
}

fn on_mesh_clicked(
    click: On<Pointer<Click>>,
    mut commands: Commands,
    scene: Res<SceneEntities>,
    selected: Query<Entity, With<Selected>>,
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
    ));
}

fn on_ground_clicked(
    _click: On<Pointer<Click>>,
    mut commands: Commands,
    scene: Res<SceneEntities>,
    selected: Query<Entity, With<Selected>>,
) {
    for entity in &selected {
        commands.entity(entity).remove::<Selected>();
    }

    commands.trigger(ZoomToFit::new(
        scene.camera,
        scene.scene_bounds,
        ZOOM_MARGIN,
        ZOOM_DURATION_MS,
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

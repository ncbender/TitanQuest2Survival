//! Titan Quest II Survivors – asset viewer stage.
//!
//! usage:
//!   tq2_survivors [model.glb] [animation-name-substring]     single animated model
//!   tq2_survivors --scene <export dir> [model.glb] [anim]    map + animated model
//! keys: WASD pan, Q/E rotate, R/F zoom
//! env TQ2_SCREENSHOT=<path>: save a screenshot after TQ2_SCREENSHOT_DELAY s (default 4) and exit
mod tq2_materials;
mod tq2_scene;

use bevy::gltf::Gltf;
use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};

const EXPORT_ROOT: &str = "../cache/export";

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let scene = match args.iter().position(|a| a == "--scene") {
        Some(i) => { args.remove(i); Some(args.remove(i)) }
        None => None,
    };
    let mut args = args.into_iter();
    let model = args.next().unwrap_or_else(|| "boarman/boarman_brute_animated.glb".into());
    let anim = args.next().unwrap_or_else(|| "none_idle".into());
    App::new()
        .add_plugins(DefaultPlugins.set(AssetPlugin {
            // Extracted assets live in the git-ignored cache/ folder.
            file_path: EXPORT_ROOT.into(),
            ..default()
        }))
        .add_plugins(tq2_materials::Tq2MaterialsPlugin)
        .insert_resource(Viewer { scene, model, anim, gltf: None, spawned: false })
        .add_systems(Startup, setup)
        .add_systems(Update, (spawn_when_loaded, start_animation, camera_controls, auto_screenshot))
        .run();
}

#[derive(Resource)]
struct Viewer {
    scene: Option<String>,
    model: String,
    anim: String,
    gltf: Option<Handle<Gltf>>,
    spawned: bool,
}

#[derive(Resource)]
struct Anim {
    graph: Handle<AnimationGraph>,
    node: AnimationNodeIndex,
}

/// Isometric-style rig: the camera orbits `focus` at `distance`, pitched down by `pitch`.
#[derive(Component)]
struct CameraRig {
    focus: Vec3,
    yaw: f32,
    pitch: f32,
    distance: f32,
}

fn setup(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut viewer: ResMut<Viewer>,
    mut library: ResMut<tq2_materials::MaterialLibrary>,
) {
    viewer.gltf = Some(assets.load(viewer.model.clone()));
    if let Some(dir) = viewer.model.split('/').next() {
        library.load_dir(EXPORT_ROOT, dir);
    }
    if let Some(dir) = &viewer.scene {
        library.load_dir(EXPORT_ROOT, dir);
    }

    let mut rig = CameraRig { focus: Vec3::Y, yaw: 0.0, pitch: 0.35, distance: 4.5 };
    if let Some(dir) = viewer.scene.clone() {
        if let Some(center) = tq2_scene::spawn_scene(&mut commands, &assets, EXPORT_ROOT, &dir) {
            rig = CameraRig { focus: center, yaw: 0.6, pitch: 0.75, distance: 35.0 };
        }
    }
    commands.insert_resource(ClearColor(Color::srgb(0.55, 0.68, 0.82)));
    commands.spawn((Camera3d::default(), Transform::default(), rig));
    commands.spawn((
        DirectionalLight { illuminance: 10000.0, shadow_maps_enabled: true, ..default() },
        Transform::from_xyz(3.0, 6.0, 4.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.insert_resource(GlobalAmbientLight { brightness: 600.0, ..default() });
}

fn spawn_when_loaded(
    mut commands: Commands,
    mut viewer: ResMut<Viewer>,
    gltfs: Res<Assets<Gltf>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    rig: Query<&CameraRig>,
) {
    if viewer.spawned {
        return;
    }
    let Some(gltf) = viewer.gltf.as_ref().and_then(|h| gltfs.get(h)) else { return };
    let mut names: Vec<_> = gltf.named_animations.keys().map(|k| k.to_string()).collect();
    names.sort();
    info!("{} animations", names.len());
    let chosen = names.iter().find(|n| n.contains(&viewer.anim)).or(names.first()).cloned();
    if let Some(name) = chosen {
        info!("playing {name}");
        let (graph, node) = AnimationGraph::from_clip(gltf.named_animations[name.as_str()].clone());
        commands.insert_resource(Anim { graph: graphs.add(graph), node });
    }
    let at = rig.single().map(|r| r.focus).unwrap_or_default();
    commands.spawn((WorldAssetRoot(gltf.scenes[0].clone()), Transform::from_translation(at)));
    viewer.spawned = true;
}

fn start_animation(
    mut commands: Commands,
    anim: Option<Res<Anim>>,
    mut players: Query<(Entity, &mut AnimationPlayer), Added<AnimationPlayer>>,
) {
    let Some(anim) = anim else { return };
    for (entity, mut player) in &mut players {
        player.play(anim.node).repeat();
        commands.entity(entity).insert(AnimationGraphHandle(anim.graph.clone()));
    }
}

fn camera_controls(time: Res<Time>, keys: Res<ButtonInput<KeyCode>>, mut cam: Query<(&mut Transform, &mut CameraRig)>) {
    let Ok((mut t, mut rig)) = cam.single_mut() else { return };
    let dt = time.delta_secs();
    let axis = |pos: KeyCode, neg: KeyCode| keys.pressed(pos) as i32 as f32 - keys.pressed(neg) as i32 as f32;
    rig.yaw += axis(KeyCode::KeyQ, KeyCode::KeyE) * 1.5 * dt;
    rig.distance *= 1.0 + axis(KeyCode::KeyF, KeyCode::KeyR) * 1.5 * dt;
    let forward = Vec3::new(-rig.yaw.sin(), 0.0, -rig.yaw.cos());
    let right = Vec3::new(forward.z * -1.0, 0.0, forward.x);
    let pan = forward * axis(KeyCode::KeyW, KeyCode::KeyS) + right * axis(KeyCode::KeyD, KeyCode::KeyA);
    let speed = rig.distance * 0.8 * dt;
    rig.focus += pan * speed;
    let offset = Vec3::new(rig.yaw.sin() * rig.pitch.cos(), rig.pitch.sin(), rig.yaw.cos() * rig.pitch.cos()) * rig.distance;
    *t = Transform::from_translation(rig.focus + offset).looking_at(rig.focus, Vec3::Y);
}

fn auto_screenshot(mut commands: Commands, time: Res<Time>, mut done: Local<bool>, mut exit: MessageWriter<AppExit>) {
    let Ok(path) = std::env::var("TQ2_SCREENSHOT") else { return };
    let delay: f32 = std::env::var("TQ2_SCREENSHOT_DELAY").ok().and_then(|s| s.parse().ok()).unwrap_or(4.0);
    let t = time.elapsed_secs();
    if t > delay && !*done {
        commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path));
        *done = true;
    }
    if t > delay + 2.0 {
        exit.write(AppExit::Success);
    }
}

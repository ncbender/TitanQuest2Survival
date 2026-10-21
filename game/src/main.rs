//! Milestone 1 viewer: loads a glTF baked from the local Titan Quest II install
//! (see tools/Tq2Extract) and plays one of its original animations.
//!
//! usage: tq2_survivors [model.glb] [animation-name-substring]
//! env TQ2_SCREENSHOT=<path>: save a screenshot after 4 s and exit (for automated checks)
use bevy::gltf::Gltf;
use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};

fn main() {
    let mut args = std::env::args().skip(1);
    let model = args.next().unwrap_or_else(|| "boarman/boarman_brute_animated.glb".into());
    let anim = args.next().unwrap_or_else(|| "none_idle".into());
    App::new()
        .add_plugins(DefaultPlugins.set(AssetPlugin {
            // Extracted assets live in the git-ignored cache/ folder.
            file_path: "../cache/export".into(),
            ..default()
        }))
        .insert_resource(Viewer { model, anim, gltf: None, spawned: false })
        .add_systems(Startup, setup)
        .add_systems(Update, (spawn_when_loaded, start_animation, orbit_camera, auto_screenshot))
        .run();
}

#[derive(Resource)]
struct Viewer {
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

fn setup(mut commands: Commands, assets: Res<AssetServer>, mut viewer: ResMut<Viewer>) {
    viewer.gltf = Some(assets.load(viewer.model.clone()));

    commands.spawn((Camera3d::default(), Transform::from_xyz(0.0, 1.8, 4.0).looking_at(Vec3::Y, Vec3::Y)));
    commands.spawn((
        DirectionalLight { illuminance: 9000.0, shadow_maps_enabled: true, ..default() },
        Transform::from_xyz(3.0, 6.0, 4.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.insert_resource(GlobalAmbientLight { brightness: 500.0, ..default() });
}

fn spawn_when_loaded(
    mut commands: Commands,
    mut viewer: ResMut<Viewer>,
    gltfs: Res<Assets<Gltf>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
) {
    if viewer.spawned {
        return;
    }
    let Some(gltf) = viewer.gltf.as_ref().and_then(|h| gltfs.get(h)) else { return };
    let mut names: Vec<_> = gltf.named_animations.keys().map(|k| k.to_string()).collect();
    names.sort();
    info!("{} animations: {:?}", names.len(), names);
    let chosen = names.iter().find(|n| n.contains(&viewer.anim)).or(names.first()).cloned();
    if let Some(name) = chosen {
        info!("playing {name}");
        let (graph, node) = AnimationGraph::from_clip(gltf.named_animations[name.as_str()].clone());
        commands.insert_resource(Anim { graph: graphs.add(graph), node });
    }
    commands.spawn(WorldAssetRoot(gltf.scenes[0].clone()));
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

fn orbit_camera(time: Res<Time>, keys: Res<ButtonInput<KeyCode>>, mut cam: Query<&mut Transform, With<Camera3d>>) {
    let Ok(mut t) = cam.single_mut() else { return };
    let mut yaw = 0.0;
    if keys.pressed(KeyCode::KeyA) { yaw += 1.0; }
    if keys.pressed(KeyCode::KeyD) { yaw -= 1.0; }
    let zoom = if keys.pressed(KeyCode::KeyW) { -1.0 } else if keys.pressed(KeyCode::KeyS) { 1.0 } else { 0.0 };
    t.rotate_around(Vec3::ZERO, Quat::from_rotation_y(yaw * time.delta_secs()));
    let dir = t.translation.normalize();
    t.translation += dir * zoom * 3.0 * time.delta_secs();
    t.look_at(Vec3::Y, Vec3::Y);
}

fn auto_screenshot(mut commands: Commands, time: Res<Time>, mut done: Local<bool>, mut exit: MessageWriter<AppExit>) {
    let Ok(path) = std::env::var("TQ2_SCREENSHOT") else { return };
    let t = time.elapsed_secs();
    if t > 4.0 && !*done {
        commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path));
        *done = true;
    }
    if t > 6.0 {
        exit.write(AppExit::Success);
    }
}

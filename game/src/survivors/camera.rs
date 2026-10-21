//! Isometric follow camera. Q/E rotate, mouse wheel / R-F zoom.
use bevy::input::mouse::MouseWheel;
use bevy::prelude::*;

use super::player::Hero;
use super::Arena;
use crate::ground::Ground;
use crate::tq2_scene::MapProp;

#[derive(Component)]
pub struct CameraRig {
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
    focus: Vec3,
}

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, spawn_camera)
        .add_systems(Update, tag_occluders)
        .add_systems(PostUpdate, (follow.before(TransformSystems::Propagate), hide_occluders.after(TransformSystems::Propagate)));
}

fn spawn_camera(mut commands: Commands) {
    commands.insert_resource(ClearColor(Color::srgb(0.55, 0.68, 0.82)));
    commands.spawn((
        Camera3d::default(),
        Transform::default(),
        CameraRig {
            yaw: 0.6,
            pitch: 0.95,
            distance: std::env::var("TQ2_ZOOM").ok().and_then(|z| z.parse().ok()).unwrap_or(24.0),
            focus: Vec3::ZERO,
        },
    ));
    commands.spawn((
        DirectionalLight { illuminance: 7000.0, shadow_maps_enabled: true, ..default() },
        Transform::from_xyz(3.0, 6.0, 4.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.insert_resource(GlobalAmbientLight { brightness: 450.0, ..default() });
}

fn follow(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut wheel: MessageReader<MouseWheel>,
    arena: Res<Arena>,
    ground: Res<Ground>,
    hero: Query<&Transform, (With<Hero>, Without<CameraRig>)>,
    mut cam: Query<(&mut Transform, &mut CameraRig)>,
) {
    let Ok((mut t, mut rig)) = cam.single_mut() else { return };
    let dt = time.delta_secs();
    let axis = |a: KeyCode, b: KeyCode| keys.pressed(a) as i32 as f32 - keys.pressed(b) as i32 as f32;
    rig.yaw += axis(KeyCode::KeyQ, KeyCode::KeyE) * 1.5 * dt;
    let scroll: f32 = wheel.read().map(|w| w.y).sum();
    rig.distance = (rig.distance * (1.0 - scroll * 0.1) * (1.0 + axis(KeyCode::KeyF, KeyCode::KeyR) * 1.5 * dt)).clamp(8.0, 80.0);
    if let Ok(hero) = hero.single() {
        let lerp = (8.0 * dt).min(1.0);
        rig.focus = rig.focus.lerp(hero.translation, lerp);
    } else {
        // Main menu: slowly circle the arena.
        rig.yaw += 0.08 * dt;
        rig.focus = Vec3::new(arena.start.x, ground.stand(arena.start), arena.start.y);
    }
    let offset = Vec3::new(rig.yaw.sin() * rig.pitch.cos(), rig.pitch.sin(), rig.yaw.cos() * rig.pitch.cos()) * rig.distance;
    *t = Transform::from_translation(rig.focus + offset).looking_at(rig.focus, Vec3::Y);
}

/// A map mesh that may hide the hero from the camera.
#[derive(Component)]
struct Occluder;

fn tag_occluders(
    mut commands: Commands,
    meshes: Query<Entity, Added<Mesh3d>>,
    parents: Query<&ChildOf>,
    props: Query<(), With<MapProp>>,
) {
    for e in &meshes {
        if parents.iter_ancestors(e).any(|a| props.contains(a)) {
            commands.entity(e).insert(Occluder);
        }
    }
}

/// Hides map meshes that stand between the camera and the hero (rocks, trees, walls),
/// so the isometric view never loses the hero behind tall scenery.
fn hide_occluders(
    cam: Query<&GlobalTransform, With<CameraRig>>,
    hero: Query<&GlobalTransform, (With<Hero>, Without<CameraRig>)>,
    mut occluders: Query<(&bevy::camera::primitives::Aabb, &GlobalTransform, &mut Visibility), With<Occluder>>,
) {
    let (Ok(cam), Ok(hero)) = (cam.single(), hero.single()) else { return };
    let from = cam.translation();
    let to = hero.translation() + Vec3::Y * 1.0;
    let ray = to - from;
    let len2 = ray.length_squared().max(1e-6);
    for (aabb, transform, mut visibility) in &mut occluders {
        let center = transform.transform_point(Vec3::from(aabb.center));
        let radius = (Vec3::from(aabb.half_extents) * transform.scale()).length();
        // Landscape and other huge meshes are never hidden.
        let t = (center - from).dot(ray) / len2;
        let blocking = radius < 25.0
            && (0.05..0.97).contains(&t)
            && center.distance(from + ray * t) < radius * 0.75 + 0.6
            && center.y + radius * 0.5 > to.y;
        let wanted = if blocking { Visibility::Hidden } else { Visibility::Inherited };
        if *visibility != wanted {
            *visibility = wanted;
        }
    }
}

//! Isometric follow camera. Q/E rotate, mouse wheel / R-F zoom.
use bevy::input::mouse::MouseWheel;
use bevy::prelude::*;

use super::player::Hero;

#[derive(Component)]
pub struct CameraRig {
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
    focus: Vec3,
}

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, spawn_camera).add_systems(PostUpdate, follow.before(TransformSystems::Propagate));
}

fn spawn_camera(mut commands: Commands) {
    commands.insert_resource(ClearColor(Color::srgb(0.55, 0.68, 0.82)));
    commands.spawn((
        Camera3d::default(),
        Transform::default(),
        CameraRig { yaw: 0.6, pitch: 0.95, distance: 24.0, focus: Vec3::ZERO },
    ));
    commands.spawn((
        DirectionalLight { illuminance: 11000.0, shadow_maps_enabled: true, ..default() },
        Transform::from_xyz(3.0, 6.0, 4.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.insert_resource(GlobalAmbientLight { brightness: 650.0, ..default() });
}

fn follow(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut wheel: MessageReader<MouseWheel>,
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
    }
    let offset = Vec3::new(rig.yaw.sin() * rig.pitch.cos(), rig.pitch.sin(), rig.yaw.cos() * rig.pitch.cos()) * rig.distance;
    *t = Transform::from_translation(rig.focus + offset).looking_at(rig.focus, Vec3::Y);
}

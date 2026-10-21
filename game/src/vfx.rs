//! Sprite effects from TQ2's VFX flipbooks (camera-facing quads): the 5x5 explosion sheet
//! `t_explosion_01_05x05_c` gives fireballs (its first frames) and fire explosions.
use bevy::math::Affine2;
use bevy::prelude::*;

const SHEET: &str = "vfx/textures/t_explosion_01_05x05_c.png";
const COLUMNS: u32 = 5;
const FRAMES: u32 = 25;
/// Frames after this one are mostly smoke.
const EXPLOSION_LAST: u32 = 11;

/// One shared material per flipbook frame (uv_transform picks the cell), plus the quad.
#[derive(Resource)]
pub struct Flipbook {
    frames: Vec<Handle<StandardMaterial>>,
    quad: Handle<Mesh>,
}

/// Plays frames `first..=last` of the explosion sheet at `fps`.
#[derive(Component)]
pub struct Sprite {
    first: u32,
    last: u32,
    fps: f32,
    age: f32,
    looping: bool,
    shown: u32,
}

/// Turns towards the camera every frame.
#[derive(Component)]
pub struct Billboard;

pub struct VfxPlugin;

impl Plugin for VfxPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup).add_systems(PostUpdate, (animate, face_camera).before(TransformSystems::Propagate));
    }
}

fn setup(mut commands: Commands, assets: Res<AssetServer>, mut materials: ResMut<Assets<StandardMaterial>>, mut meshes: ResMut<Assets<Mesh>>) {
    let texture = assets.load(SHEET);
    let cell = 1.0 / COLUMNS as f32;
    let frames = (0..FRAMES)
        .map(|i| {
            materials.add(StandardMaterial {
                base_color_texture: Some(texture.clone()),
                uv_transform: Affine2::from_scale_angle_translation(
                    Vec2::splat(cell),
                    0.0,
                    Vec2::new((i % COLUMNS) as f32 * cell, (i / COLUMNS) as f32 * cell),
                ),
                // Bright and unlit: these are fire, not lit surfaces.
                base_color: Color::linear_rgb(2.5, 2.2, 2.0),
                unlit: true,
                alpha_mode: AlphaMode::Blend,
                double_sided: true,
                cull_mode: None,
                ..default()
            })
        })
        .collect();
    commands.insert_resource(Flipbook { frames, quad: meshes.add(Rectangle::new(1.0, 1.0)) });
}

impl Flipbook {
    /// A looping fireball sprite (to put on a projectile).
    pub fn fireball(&self, size: f32) -> impl Bundle {
        self.sprite(Sprite { first: 0, last: 4, fps: 14.0, age: 0.0, looping: true, shown: 0 }, Vec3::ZERO, size)
    }

    /// A one-shot fire explosion at `at` (the sheet's fiery frames, before the smoke); despawns when done.
    pub fn explosion(&self, at: Vec3, size: f32) -> impl Bundle {
        self.sprite(Sprite { first: 0, last: EXPLOSION_LAST, fps: 30.0, age: 0.0, looping: false, shown: 0 }, at, size)
    }

    fn sprite(&self, sprite: Sprite, at: Vec3, size: f32) -> impl Bundle {
        (
            Mesh3d(self.quad.clone()),
            MeshMaterial3d(self.frames[sprite.first as usize].clone()),
            Transform::from_translation(at).with_scale(Vec3::splat(size)),
            // Flat quads would throw square shadows.
            bevy::light::NotShadowCaster,
            Billboard,
            sprite,
        )
    }
}

fn animate(
    mut commands: Commands,
    time: Res<Time>,
    flipbook: Option<Res<Flipbook>>,
    mut sprites: Query<(Entity, &mut Sprite, &mut MeshMaterial3d<StandardMaterial>)>,
) {
    let Some(flipbook) = flipbook else { return };
    for (e, mut s, mut material) in &mut sprites {
        s.age += time.delta_secs();
        let count = s.last - s.first + 1;
        let step = (s.age * s.fps) as u32;
        if !s.looping && step >= count {
            commands.entity(e).despawn();
            continue;
        }
        let frame = s.first + step % count;
        if frame != s.shown {
            s.shown = frame;
            material.0 = flipbook.frames[frame as usize].clone();
        }
    }
}

fn face_camera(
    camera: Query<&Transform, (With<Camera3d>, Without<Billboard>)>,
    parents: Query<&GlobalTransform>,
    mut sprites: Query<(&mut Transform, Option<&ChildOf>), With<Billboard>>,
) {
    let Ok(cam) = camera.single() else { return };
    for (mut t, parent) in &mut sprites {
        // Undo the parent's rotation so the quad ends up facing the camera in world space.
        let parent_rotation = parent.and_then(|p| parents.get(p.parent()).ok()).map_or(Quat::IDENTITY, |g| g.rotation());
        t.rotation = parent_rotation.inverse() * cam.rotation;
    }
}

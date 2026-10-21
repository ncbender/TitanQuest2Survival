//! Skill effects from TQ2's VFX textures (ground decals, glows, lightning bolts, beams),
//! tinted per damage element. Materials are shared per texture × element, so effects grow in
//! and shrink away instead of fading.
use bevy::math::Affine2;
use bevy::prelude::*;
use std::collections::HashMap;

use crate::vfx::Billboard;

/// Damage element of a skill; picks the tint of its effects.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Element {
    Fire,
    Cold,
    Lightning,
    Poison,
    Physical,
    Spirit,
}

impl Element {
    pub const ALL: [Element; 6] = [Element::Fire, Element::Cold, Element::Lightning, Element::Poison, Element::Physical, Element::Spirit];

    /// Unlit tint of the TQ2 masks.
    pub fn tint(self) -> Color {
        match self {
            // Additive on bright sand: keep channels low enough that the hue survives.
            Element::Fire => Color::linear_rgb(1.3, 0.45, 0.1),
            Element::Cold => Color::linear_rgb(0.3, 0.7, 1.3),
            Element::Lightning => Color::linear_rgb(0.8, 0.85, 1.4),
            Element::Poison => Color::linear_rgb(0.3, 1.0, 0.15),
            Element::Physical => Color::linear_rgb(0.8, 0.6, 0.35),
            Element::Spirit => Color::linear_rgb(0.15, 1.0, 0.8),
        }
    }

    pub fn light(self) -> Color {
        match self {
            Element::Fire => Color::srgb(1.0, 0.5, 0.15),
            Element::Cold => Color::srgb(0.4, 0.7, 1.0),
            Element::Lightning => Color::srgb(0.7, 0.75, 1.0),
            Element::Poison => Color::srgb(0.4, 1.0, 0.3),
            Element::Physical => Color::srgb(1.0, 0.85, 0.6),
            Element::Spirit => Color::srgb(0.3, 1.0, 0.85),
        }
    }
}

/// TQ2 VFX textures used for skill effects.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Tex {
    /// Ice Nova burst decal: a radial blast.
    Burst,
    Frost,
    /// Call Lightning impact scorch.
    Scorch,
    PoisonPool,
    Cracks,
    /// Ice-storm swirl (cyclones, vortices).
    Vortex,
    /// Ice Nova ring (expanding rings, auras).
    Ring,
    Glow,
    Snowflake,
    Bolt,
    Branch,
    Puff,
    Spark,
}

impl Tex {
    const ALL: [Tex; 13] = [
        Tex::Burst,
        Tex::Frost,
        Tex::Scorch,
        Tex::PoisonPool,
        Tex::Cracks,
        Tex::Vortex,
        Tex::Ring,
        Tex::Glow,
        Tex::Snowflake,
        Tex::Bolt,
        Tex::Branch,
        Tex::Puff,
        Tex::Spark,
    ];

    /// Texture file and atlas cell (UV offset, UV size).
    fn source(self) -> (&'static str, Vec2, Vec2) {
        let (file, offset, size) = match self {
            Tex::Burst => ("t_ice_nova_decal_a", Vec2::ZERO, Vec2::ONE),
            Tex::Frost => ("t_decal_frostbite_albedo", Vec2::ZERO, Vec2::ONE),
            Tex::Scorch => ("t_st_call_lightning_impact_decal_a", Vec2::ZERO, Vec2::ONE),
            Tex::PoisonPool => ("t_ic_alchemist_grenade_poison_decal_a", Vec2::ZERO, Vec2::ONE),
            Tex::Cracks => ("t_ice_cracks_01_a", Vec2::ZERO, Vec2::ONE),
            Tex::Vortex => ("t_un_skel_evoker_ice_storm_decal_a", Vec2::ZERO, Vec2::ONE),
            Tex::Ring => ("t_ice_nova_ring_01_a", Vec2::ZERO, Vec2::ONE),
            Tex::Glow => ("t_circle_01_cold_a", Vec2::ZERO, Vec2::splat(0.5)),
            Tex::Snowflake => ("t_ice_snowfalke_atlas_a", Vec2::ZERO, Vec2::splat(0.5)),
            Tex::Bolt => ("t_lightning_bolt_background_01_a", Vec2::new(0.75, 0.0), Vec2::new(0.25, 1.0)),
            Tex::Branch => ("t_lightning_branches_02_a", Vec2::new(0.0, 0.25), Vec2::new(1.0, 0.25)),
            Tex::Puff => ("t_poison_particles_a", Vec2::ZERO, Vec2::splat(1.0 / 3.0)),
            Tex::Spark => ("t_sparks_block_2x2_a", Vec2::ZERO, Vec2::splat(0.5)),
        };
        (file, offset, size)
    }
}

/// Shared materials for every texture × element, plus the quad.
#[derive(Resource)]
pub struct Fx {
    materials: HashMap<(Tex, Element), Handle<StandardMaterial>>,
    quad: Handle<Mesh>,
}

/// A short-lived effect: grows from `from` to `to` × its spawn scale and shrinks away at the end.
#[derive(Component)]
pub struct FxLife {
    pub age: f32,
    pub life: f32,
    pub from: f32,
    pub to: f32,
}

#[derive(Component)]
struct BaseScale(Vec3);

pub struct FxPlugin;

impl Plugin for FxPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup).add_systems(Update, (start_fx, age_fx).chain());
    }
}

fn setup(mut commands: Commands, assets: Res<AssetServer>, mut materials: ResMut<Assets<StandardMaterial>>, mut meshes: ResMut<Assets<Mesh>>) {
    let mut map = HashMap::new();
    for tex in Tex::ALL {
        let (file, offset, size) = tex.source();
        let image: Handle<Image> = assets.load(format!("vfx/textures/{file}.png"));
        for el in Element::ALL {
            let material = materials.add(StandardMaterial {
                base_color_texture: Some(image.clone()),
                base_color: el.tint(),
                uv_transform: Affine2::from_scale_angle_translation(size, 0.0, offset),
                unlit: true,
                alpha_mode: AlphaMode::Add,
                double_sided: true,
                cull_mode: None,
                ..default()
            });
            map.insert((tex, el), material);
        }
    }
    commands.insert_resource(Fx { materials: map, quad: meshes.add(Rectangle::new(1.0, 1.0)) });
}

const FLAT: Quat = Quat::from_xyzw(-std::f32::consts::FRAC_1_SQRT_2, 0.0, 0.0, std::f32::consts::FRAC_1_SQRT_2);

impl Fx {
    fn mesh(&self, tex: Tex, el: Element) -> (Mesh3d, MeshMaterial3d<StandardMaterial>, bevy::light::NotShadowCaster) {
        (Mesh3d(self.quad.clone()), MeshMaterial3d(self.materials[&(tex, el)].clone()), bevy::light::NotShadowCaster)
    }

    /// A decal flat on the ground (`at` = ground point), `radius` metres, living `life` s.
    pub fn decal(&self, tex: Tex, el: Element, at: Vec3, radius: f32, life: f32) -> impl Bundle {
        (
            self.mesh(tex, el),
            Transform::from_translation(at + Vec3::Y * 0.1).with_rotation(FLAT).with_scale(Vec3::splat(radius * 2.0)),
            FxLife { age: 0.0, life, from: 0.6, to: 1.0 },
        )
    }

    /// A camera-facing sprite without lifetime (e.g. a projectile's or summon's body; parent it).
    pub fn glow(&self, tex: Tex, el: Element, size: f32) -> impl Bundle {
        (self.mesh(tex, el), Transform::from_scale(Vec3::splat(size)), Billboard)
    }

    /// A camera-facing flash at `at` that pops and shrinks away.
    pub fn flash(&self, tex: Tex, el: Element, at: Vec3, size: f32, life: f32) -> impl Bundle {
        (self.mesh(tex, el), Transform::from_translation(at).with_scale(Vec3::splat(size)), Billboard, FxLife { age: 0.0, life, from: 0.5, to: 1.0 })
    }

    /// A lightning bolt from the sky down to `ground`.
    pub fn bolt(&self, el: Element, ground: Vec3, height: f32) -> impl Bundle {
        (
            self.mesh(Tex::Bolt, el),
            Transform::from_translation(ground + Vec3::Y * height * 0.5).with_scale(Vec3::new(height * 0.3, height, 1.0)),
            Billboard,
            FxLife { age: 0.0, life: 0.3, from: 1.0, to: 1.0 },
        )
    }

    /// A flat strip from `from` to `to` (beams, chains, lightning arcs).
    pub fn beam(&self, tex: Tex, el: Element, from: Vec3, to: Vec3, width: f32, life: f32) -> impl Bundle {
        let d = to - from;
        let yaw = f32::atan2(-d.z, d.x);
        (
            self.mesh(tex, el),
            Transform::from_translation((from + to) * 0.5)
                .with_rotation(Quat::from_rotation_y(yaw) * FLAT)
                .with_scale(Vec3::new(d.length().max(0.1), width, 1.0)),
            FxLife { age: 0.0, life, from: 1.0, to: 1.0 },
        )
    }
}

/// Remembers each new effect's spawn scale.
fn start_fx(mut commands: Commands, new: Query<(Entity, &Transform), Added<FxLife>>) {
    for (e, t) in &new {
        commands.entity(e).insert(BaseScale(t.scale));
    }
}

fn age_fx(mut commands: Commands, time: Res<Time>, mut q: Query<(Entity, &mut FxLife, &BaseScale, &mut Transform)>) {
    for (e, mut fx, base, mut t) in &mut q {
        fx.age += time.delta_secs();
        if fx.age >= fx.life {
            commands.entity(e).despawn();
            continue;
        }
        let k = fx.age / fx.life;
        let grow = fx.from + (fx.to - fx.from) * (k / 0.3).min(1.0);
        let out = ((1.0 - k) / 0.25).min(1.0);
        t.scale = base.0 * grow * out;
    }
}

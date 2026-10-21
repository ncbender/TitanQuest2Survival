//! Gear: the equipped weapon decides the hero's auto-attack and stance, armour changes the
//! visible body parts and reduces damage. Elites and bosses drop chests; opening one offers
//! three pieces of gear to choose from.
use bevy::prelude::*;

use super::player::Hero;
use super::{GameState, Pos, RunEntity};
use crate::character::{Attachment, DesiredParts};
use crate::ground::Ground;

/// How a weapon attacks automatically.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum AttackKind {
    /// Homing-free fire bolts at the nearest enemy (staff).
    Bolt,
    /// Fast, long-range arrows that pierce (bow).
    Arrow,
    /// Melee sweep in an arc towards the nearest enemy (sword).
    Sweep,
}

pub struct WeaponDef {
    pub name: &'static str,
    pub mesh: &'static str,
    pub bone: &'static str,
    /// Whether the mesh must be flipped 180° about X on its bone (head/blade along +Y).
    pub flip: bool,
    pub idle: &'static str,
    pub run: &'static str,
    pub kind: AttackKind,
    pub cooldown: f32,
    pub damage: f32,
    pub range: f32,
    /// Projectile speed (m/s) or sweep radius (m).
    pub speed_or_radius: f32,
    pub pierce: u32,
    /// TQ2 FMOD events for firing/swinging and for hitting.
    pub fire_sound: &'static str,
    pub hit_sound: &'static str,
}

pub const STAFF: WeaponDef = WeaponDef {
    name: "Staff of Apollo",
    mesh: "weapons/TQ2/Content/TQ2/Character/Human/Weapon/Staff2H/Unique/Staff2H_Staff_Of_Apollo_01/sm_staff2h_staff_of_apollo_01.glb",
    bone: "HandAttach_R",
    flip: true,
    idle: "a_male_staff2h_idle_combat_01",
    run: "a_male_staff2h_run_01",
    kind: AttackKind::Bolt,
    cooldown: 0.9,
    damage: 10.0,
    range: 18.0,
    speed_or_radius: 22.0,
    pierce: 0,
    fire_sound: "event:/weapons/whoosh/sfx_weapon_whoosh_staff2h_fire",
    hit_sound: "event:/weapons/hit/sfx_weapon_hit_staff_projectile_fire",
};

pub const BOW: WeaponDef = WeaponDef {
    name: "Arch of Iris",
    mesh: "weapons/TQ2/Content/TQ2/Character/Human/Weapon/Bow2H/Unique/Bow2H_Arch_Of_Iris_Narrow_01/sk_bow2h_arch_of_iris_narrow_01.glb",
    bone: "HandAttach_L",
    flip: false,
    idle: "a_male_bow_idle_combat_01",
    run: "a_male_bow_run_01",
    kind: AttackKind::Arrow,
    cooldown: 0.65,
    damage: 8.0,
    range: 26.0,
    speed_or_radius: 34.0,
    pierce: 2,
    fire_sound: "event:/weapons/whoosh/sfx_weapon_whoosh_bow",
    hit_sound: "event:/weapons/hit/sfx_weapon_hit_arrow",
};

pub const SWORD: WeaponDef = WeaponDef {
    name: "Battlemage Sword",
    mesh: "weapons/TQ2/Content/TQ2/Character/Human/Weapon/Sword1H/Unique/Sword1H_Battlemage_Sword_01/sm_sword1h_battlemage_sword_01.glb",
    bone: "HandAttach_R",
    flip: true,
    idle: "a_male_sword_idle_combat_01",
    run: "a_male_sword_run_01",
    kind: AttackKind::Sweep,
    cooldown: 0.75,
    damage: 16.0,
    range: 4.0,
    speed_or_radius: 3.2,
    pierce: 0,
    fire_sound: "event:/weapons/whoosh/sfx_weapon_whoosh_sword1h",
    hit_sound: "event:/weapons/hit/sfx_weapon_hit_sword1h",
};

pub const WEAPONS: &[&WeaponDef] = &[&STAFF, &BOW, &SWORD];

pub struct ArmorDef {
    pub name: &'static str,
    pub parts: &'static [&'static str],
    /// Fraction of enemy damage absorbed.
    pub armor: f32,
}

pub const ADVENTURER: ArmorDef = ArmorDef {
    name: "Adventurer's Garb",
    parts: &["hero/sk_male_torso_greek_adventurer_01_animated.glb", "hero/sk_male_leg_greek_adventurer_01_animated.glb"],
    armor: 0.0,
};

pub const SCHOLAR: ArmorDef = ArmorDef {
    name: "Scholar's Robes",
    parts: &["hero/sk_male_torso_greek_scholar_02_01_animated.glb", "hero/sk_male_leg_greek_scholar_02_01_animated.glb"],
    armor: 0.12,
};

pub const RANGER: ArmorDef = ArmorDef {
    name: "Ranger's Armour",
    parts: &[
        "hero/sk_male_torso_greek_ranger_02_animated.glb",
        "hero/sk_male_leg_greek_ranger_02_animated.glb",
        "hero/sk_male_shoulder_greek_ranger_02_animated.glb",
    ],
    armor: 0.25,
};

pub const ARMORS: &[&ArmorDef] = &[&ADVENTURER, &SCHOLAR, &RANGER];

/// Parts every hero wears regardless of armour.
const HERO_BASE: &[&str] = &["hero/sk_male_base_animated.glb", "hero/sk_human_male_hair_01_animated.glb"];

pub const CHEST: &str = "props/TQ2/Content/TQ2/Environment/Greek/Props/Chest/sm_chest_01a.glb";

#[derive(Component, Clone, Copy)]
pub struct Equipment {
    pub weapon: &'static WeaponDef,
    pub armor: &'static ArmorDef,
}

impl Default for Equipment {
    fn default() -> Self {
        Self { weapon: &STAFF, armor: &ADVENTURER }
    }
}

impl Equipment {
    pub fn parts(&self) -> DesiredParts {
        DesiredParts(HERO_BASE.iter().chain(self.armor.parts).copied().collect())
    }

    pub fn attachment(&self) -> Attachment {
        let w = self.weapon;
        let rotation = if w.flip { Quat::from_rotation_x(std::f32::consts::PI) } else { Quat::IDENTITY };
        Attachment { bone: w.bone, scene: w.mesh, rotation }
    }
}

/// A piece of gear offered from a chest.
#[derive(Clone, Copy)]
pub enum Gear {
    Weapon(&'static WeaponDef),
    Armor(&'static ArmorDef),
}

impl WeaponDef {
    /// What the weapon's auto-attack does, for menus.
    pub fn attack_text(&self) -> &'static str {
        match self.kind {
            AttackKind::Bolt => "fireballs that explode on impact",
            AttackKind::Arrow => "fast arrows that pierce",
            AttackKind::Sweep => "melee sweep in an arc",
        }
    }
}

impl Gear {
    pub fn title(self) -> String {
        match self {
            Gear::Weapon(w) => format!("Weapon: {} – {}", w.name, w.attack_text()),
            Gear::Armor(a) => format!("Armour: {} – {:.0} % less damage taken", a.name, a.armor * 100.0),
        }
    }
}

/// The weapon picked in character creation; every run starts with it.
#[derive(Resource)]
pub struct StartingWeapon(pub &'static WeaponDef);

impl Default for StartingWeapon {
    fn default() -> Self {
        Self(&STAFF)
    }
}

/// A chest on the ground (TQ2 Greek chest).
#[derive(Component)]
pub struct LootChest;

/// The gear offered by the chest being opened.
#[derive(Resource, Default)]
pub struct LootOffer(pub Vec<Gear>);

pub fn plugin(app: &mut App) {
    app.init_resource::<LootOffer>()
        .init_resource::<StartingWeapon>()
        .add_systems(Update, (apply_equipment, place_chests))
        .add_systems(FixedUpdate, open_chests.run_if(in_state(GameState::Playing)));
}

/// Turns equipment changes into visible parts and the carried weapon.
fn apply_equipment(mut commands: Commands, changed: Query<(Entity, &Equipment), Changed<Equipment>>) {
    for (hero, eq) in &changed {
        commands.entity(hero).insert((eq.parts(), eq.attachment()));
    }
}

pub fn drop_chest(commands: &mut Commands, assets: &AssetServer, at: Vec2) {
    commands.spawn((
        LootChest,
        RunEntity,
        Pos(at),
        WorldAssetRoot(assets.load(GltfAssetLabel::Scene(0).from_asset(CHEST))),
        Transform::default(),
    ));
}

fn place_chests(ground: Res<Ground>, mut chests: Query<(&Pos, &mut Transform), (With<LootChest>, Changed<Pos>)>) {
    for (pos, mut t) in &mut chests {
        t.translation = Vec3::new(pos.0.x, ground.stand(pos.0), pos.0.y);
    }
}

/// Walking into a chest opens it: up to three pieces of gear the hero does not wear yet.
fn open_chests(
    mut commands: Commands,
    mut offer: ResMut<LootOffer>,
    mut next: ResMut<NextState<GameState>>,
    hero: Query<(&Pos, &Equipment), With<Hero>>,
    chests: Query<(Entity, &Pos), With<LootChest>>,
) {
    let Ok((hero, eq)) = hero.single() else { return };
    let Some((chest, _)) = chests.iter().find(|(_, p)| p.0.distance(hero.0) < 1.6) else { return };
    commands.entity(chest).despawn();
    let mut pool: Vec<Gear> = WEAPONS
        .iter()
        .filter(|w| !std::ptr::eq(**w, eq.weapon))
        .map(|w| Gear::Weapon(w))
        .chain(ARMORS.iter().filter(|a| !std::ptr::eq(**a, eq.armor)).map(|a| Gear::Armor(a)))
        .collect();
    fastrand::shuffle(&mut pool);
    pool.truncate(3);
    info!("chest offers: {:?}", pool.iter().map(|g| g.title()).collect::<Vec<_>>());
    offer.0 = pool;
    next.set(GameState::Loot);
}

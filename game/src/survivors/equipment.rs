//! Gear: the main-hand weapon decides the hero's auto-attack, the off hand (second one-handed
//! weapon, shield or talisman) modifies it, and the armour changes the visible body parts.
//! Two-handed weapons leave the off hand empty. The weapon combination picks TQ2's stance
//! (idle/run/attack clips), e.g. sword + shield.
//!
//! Gear is only changed outside a run (armoury, see menu.rs). Elites and bosses drop chests;
//! the item picked from a chest goes to the stash when the run ends (see profile.rs).
use bevy::prelude::*;

use super::player::Hero;
use super::{GameState, Pos, RunEntity};
use crate::character::{Attachment, Attachments, DesiredParts};
use crate::ground::Ground;

/// How a weapon attacks automatically.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum AttackKind {
    /// Fireballs at the nearest enemy that explode on impact (staff).
    Bolt,
    /// Fast, long-range arrows that pierce (bow).
    Arrow,
    /// Melee sweep in an arc towards the nearest enemy (swords, axes).
    Sweep,
}

/// TQ2's animation set for one weapon combination.
pub struct Stance {
    pub idle: &'static str,
    pub run: &'static str,
    /// Attack clips, played in turn on the upper body.
    pub attacks: &'static [&'static str],
    /// TQ2's skill clips of this stance, by kind of skill.
    pub casts: Casts,
}

/// Cast clips for the kinds of skills (see `abilities::Shape::cast_kind`).
pub struct Casts {
    pub projectile: &'static str,
    pub area: &'static str,
    pub ground: &'static str,
    pub buff: &'static str,
    pub summon: &'static str,
    pub dash: &'static str,
    pub melee: &'static str,
}

/// The kind of motion a skill needs.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum CastKind {
    Projectile,
    Area,
    Ground,
    Buff,
    Summon,
    Dash,
    Melee,
}

impl Casts {
    pub fn clip(&self, kind: CastKind) -> &'static str {
        match kind {
            CastKind::Projectile => self.projectile,
            CastKind::Area => self.area,
            CastKind::Ground => self.ground,
            CastKind::Buff => self.buff,
            CastKind::Summon => self.summon,
            CastKind::Dash => self.dash,
            CastKind::Melee => self.melee,
        }
    }
}

macro_rules! stance {
    ($set:literal, $proj:literal, $area:literal, $ground:literal, $buff:literal, $summon:literal, $dash:literal, $melee:literal) => {
        Stance {
            idle: concat!("a_male_", $set, "_idle_combat_01"),
            run: concat!("a_male_", $set, "_run_01"),
            attacks: &[
                concat!("a_male_", $set, "_attack_01_01"),
                concat!("a_male_", $set, "_attack_02_01"),
                concat!("a_male_", $set, "_attack_03_01"),
            ],
            casts: Casts {
                projectile: concat!("a_male_", $set, "_", $proj),
                area: concat!("a_male_", $set, "_", $area),
                ground: concat!("a_male_", $set, "_", $ground),
                buff: concat!("a_male_", $set, "_", $buff),
                summon: concat!("a_male_", $set, "_", $summon),
                dash: concat!("a_male_", $set, "_", $dash),
                melee: concat!("a_male_", $set, "_", $melee),
            },
        }
    };
}

// Clips per stance (tools/assets.json bakes them into the hero).
const STAFF_STANCE: Stance =
    stance!("staff2h", "barrage_01", "chthonic_pulse_01", "grenade_throw_01", "forcefield_01", "summon_sentry_01", "blink_01", "armor_eruption_01");
const BOW_STANCE: Stance =
    stance!("bow", "barrage_01", "chthonic_pulse_01", "grenade_throw_01", "cast_doom_horn_01", "summon_sentry_01", "blink_01", "reap_01");
const AXE_STANCE: Stance =
    stance!("axe2h", "trickshot_01", "cast_stomp_01", "grenade_throw_01", "forcefield_01", "summon_sentry_01", "blink_01", "whirlwind_01");
const SWORD_STANCE: Stance =
    stance!("sword", "cast_lightning_bolt_01", "cast_ice_nova_01", "grenade_throw_01", "cast_cyclone_01", "summon_sentry_01", "blink_01", "whirlwind_01");
const SWORD_SHIELD_STANCE: Stance =
    stance!("sword+shield", "barrage_01", "chthonic_pulse_01", "grenade_throw_01", "forcefield_01", "summon_sentry_01", "blink_01", "whirlwind_01");
const DUAL_SWORD_STANCE: Stance =
    stance!("sword+sword", "barrage_01", "cast_stomp_01", "grenade_throw_01", "forcefield_01", "summon_sentry_01", "blink_01", "whirlwind_01");
const SWORD_TALISMAN_STANCE: Stance =
    stance!("sword+talisman", "barrage_01", "chthonic_pulse_01", "grenade_throw_01", "forcefield_01", "summon_sentry_01", "blink_01", "bladespin_01");

pub struct WeaponDef {
    pub id: &'static str,
    pub name: &'static str,
    /// TQ2's rendered item icon (cache/export/items/textures).
    pub icon: &'static str,
    pub mesh: &'static str,
    pub two_handed: bool,
    /// Bone it is carried on when in the main hand.
    pub bone: &'static str,
    /// Whether the mesh must be flipped 180° about X on its bone (head/blade along +Y).
    pub flip: bool,
    /// Stance when wielded alone (two-handed, or one-handed with an empty off hand).
    pub stance: &'static Stance,
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
    id: "fotia",
    name: "Fotia",
    icon: "t_icon_weapon_weapon_staff2h_fotia",
    mesh: "weapons/TQ2/Content/TQ2/Character/Human/Weapon/Staff2H/Unique/Staff2H_Fotia_01/sm_staff2h_fotia_01.glb",
    two_handed: true,
    bone: "HandAttach_R",
    flip: true,
    stance: &STAFF_STANCE,
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
    id: "arch_of_iris",
    name: "Arch of Iris",
    icon: "t_icon_weapon_weapon_bow2h_arch_of_iris",
    mesh: "weapons/TQ2/Content/TQ2/Character/Human/Weapon/Bow2H/Unique/Bow2H_Arch_Of_Iris_Narrow_01/sk_bow2h_arch_of_iris_narrow_01.glb",
    two_handed: true,
    bone: "HandAttach_L",
    flip: false,
    stance: &BOW_STANCE,
    kind: AttackKind::Arrow,
    cooldown: 0.65,
    damage: 8.0,
    range: 26.0,
    speed_or_radius: 34.0,
    pierce: 2,
    fire_sound: "event:/weapons/whoosh/sfx_weapon_whoosh_bow",
    hit_sound: "event:/weapons/hit/sfx_weapon_hit_arrow",
};

pub const AXE: WeaponDef = WeaponDef {
    id: "harpe",
    name: "Harpe",
    icon: "t_icon_weapon_weapon_axe2h_harpe",
    mesh: "weapons/TQ2/Content/TQ2/Character/Human/Weapon/Axe2H/Unique/Axe2H_Harpe_01/sm_axe2h_harpe_01.glb",
    two_handed: true,
    bone: "HandAttach_R",
    flip: true,
    stance: &AXE_STANCE,
    kind: AttackKind::Sweep,
    cooldown: 1.1,
    damage: 28.0,
    range: 4.8,
    speed_or_radius: 3.9,
    pierce: 0,
    fire_sound: "event:/weapons/whoosh/sfx_weapon_whoosh_axe2h",
    hit_sound: "event:/weapons/hit/sfx_weapon_hit_axe2h",
};

pub const SWORD: WeaponDef = WeaponDef {
    id: "battlemage_sword",
    name: "Battlemage Sword",
    icon: "t_icon_weapon_weapon_sword1h_battlemage_sword",
    mesh: "weapons/TQ2/Content/TQ2/Character/Human/Weapon/Sword1H/Unique/Sword1H_Battlemage_Sword_01/sm_sword1h_battlemage_sword_01.glb",
    two_handed: false,
    bone: "HandAttach_R",
    flip: true,
    stance: &SWORD_STANCE,
    kind: AttackKind::Sweep,
    cooldown: 0.75,
    damage: 16.0,
    range: 4.0,
    speed_or_radius: 3.2,
    pierce: 0,
    fire_sound: "event:/weapons/whoosh/sfx_weapon_whoosh_sword1h",
    hit_sound: "event:/weapons/hit/sfx_weapon_hit_sword1h",
};

pub const PELEUS: WeaponDef = WeaponDef {
    id: "sword_of_peleus",
    name: "Sword of Peleus",
    icon: "t_icon_weapon_weapon_sword1h_sword_of_peleus",
    mesh: "weapons/TQ2/Content/TQ2/Character/Human/Weapon/Sword1H/Unique/Sword1H_Sword_Of_Peleus_01/sm_sword1h_sword_of_peleus_01.glb",
    damage: 18.0,
    cooldown: 0.8,
    ..SWORD
};

pub const WEAPONS: &[&WeaponDef] = &[&STAFF, &BOW, &AXE, &SWORD, &PELEUS];

/// Off-hand shield: absorbs damage.
pub struct ShieldDef {
    pub id: &'static str,
    pub name: &'static str,
    pub icon: &'static str,
    pub mesh: &'static str,
    pub block: f32,
}

pub const AENEAS_SHIELD: ShieldDef = ShieldDef {
    id: "shield_of_aeneas",
    name: "Shield of Aeneas",
    icon: "t_icon_weapon_itemicon_weapon_shield1h_shield_of_aeneas",
    mesh: "weapons/TQ2/Content/TQ2/Character/Human/Weapon/Shield1H/Unique/Shield1H_Shield_Of_Aeneas_01/sm_shield1h_shield_of_aeneas_01.glb",
    block: 0.2,
};

/// Off-hand talisman: strengthens skills (Fire Nova, Meteor).
pub struct TalismanDef {
    pub id: &'static str,
    pub name: &'static str,
    pub icon: &'static str,
    pub mesh: &'static str,
    pub skill_damage: f32,
}

pub const ARCANE_MIRROR: TalismanDef = TalismanDef {
    id: "arcane_mirror",
    name: "Arcane Mirror",
    icon: "t_icon_weapon_itemicon_weapon_talisman1h_arcane_mirror",
    mesh: "weapons/TQ2/Content/TQ2/Character/Human/Weapon/Talisman1H/Unique/Talisman1H_Arcane_Mirror_01/sk_talisman1h_arcane_mirror_01.glb",
    skill_damage: 0.4,
};

/// Where an armour piece is worn (index into `Equipment::pieces`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ArmorSlot {
    Head = 0,
    Chest = 1,
    Shoulders = 2,
    Arms = 3,
    Legs = 4,
}

impl ArmorSlot {
    pub fn label(self) -> &'static str {
        match self {
            ArmorSlot::Head => "Helmet",
            ArmorSlot::Chest => "Chest armour",
            ArmorSlot::Shoulders => "Shoulders",
            ArmorSlot::Arms => "Arms",
            ArmorSlot::Legs => "Legs",
        }
    }
}

/// An armour piece: one TQ2 mesh on the hero (more TQ2 sets in armory.rs).
pub struct ArmorDef {
    pub id: &'static str,
    pub name: &'static str,
    pub icon: &'static str,
    pub slot: ArmorSlot,
    pub parts: &'static [&'static str],
    /// Fraction of enemy damage absorbed.
    pub armor: f32,
}

pub const ADVENTURER: ArmorDef = ArmorDef {
    id: "adventurer",
    name: "Adventurer's Tunic",
    icon: "t_icon_armor_torso_greek_adventurer_01_battlemage_none",
    slot: ArmorSlot::Chest,
    parts: &["hero/sk_male_torso_greek_adventurer_01_animated.glb"],
    armor: 0.0,
};

pub const ADVENTURER_LEGS: ArmorDef = ArmorDef {
    id: "adventurer_legs",
    name: "Adventurer's Trousers",
    icon: "t_icon_armor_leg_greek_adventurer_02_01_battlemage_none",
    slot: ArmorSlot::Legs,
    parts: &["hero/sk_male_leg_greek_adventurer_01_animated.glb"],
    armor: 0.0,
};

pub const SCHOLAR: ArmorDef = ArmorDef {
    id: "scholar",
    name: "Scholar's Robe",
    icon: "t_icon_armor_torso_greek_scholar_02_01_battlemage_none",
    slot: ArmorSlot::Chest,
    parts: &["hero/sk_male_torso_greek_scholar_02_01_animated.glb"],
    armor: 0.04,
};

pub const SCHOLAR_LEGS: ArmorDef = ArmorDef {
    id: "scholar_legs",
    name: "Scholar's Sandals",
    icon: "t_icon_armor_leg_greek_scholar_02_01_battlemage_none",
    slot: ArmorSlot::Legs,
    parts: &["hero/sk_male_leg_greek_scholar_02_01_animated.glb"],
    armor: 0.02,
};

pub const RANGER: ArmorDef = ArmorDef {
    id: "ranger",
    name: "Ranger's Jerkin",
    icon: "t_icon_armor_torso_greek_ranger_02_battlemage_none",
    slot: ArmorSlot::Chest,
    parts: &["hero/sk_male_torso_greek_ranger_02_animated.glb"],
    armor: 0.07,
};

pub const RANGER_LEGS: ArmorDef = ArmorDef {
    id: "ranger_legs",
    name: "Ranger's Boots",
    icon: "t_icon_armor_leg_greek_ranger_02_battlemage_none",
    slot: ArmorSlot::Legs,
    parts: &["hero/sk_male_leg_greek_ranger_02_animated.glb"],
    armor: 0.04,
};

pub const RANGER_SHOULDERS: ArmorDef = ArmorDef {
    id: "ranger_shoulders",
    name: "Ranger's Shoulderguards",
    icon: "t_icon_armor_shoulder_greek_ranger_02_battlemage_none",
    slot: ArmorSlot::Shoulders,
    parts: &["hero/sk_male_shoulder_greek_ranger_02_animated.glb"],
    armor: 0.03,
};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum JewelryKind {
    Ring,
    Amulet,
}

/// A ring (critical chance) or an amulet (health).
pub struct JewelryDef {
    pub id: &'static str,
    pub name: &'static str,
    pub icon: &'static str,
    pub kind: JewelryKind,
    /// Ring: critical chance (0.03 = +3 %); amulet: health.
    pub value: f32,
}

// More TQ2 uniques. Stats vary around their weapon class.

pub const CHROMATIC_STAFF: WeaponDef = WeaponDef {
    id: "chromatic_staff",
    name: "Chromatic Staff",
    icon: "t_icon_weapon_weapon_staff2h_chromatic_staff",
    mesh: "weapons/TQ2/Content/TQ2/Character/Human/Weapon/Staff2H/Unique/Staff2H_Chromatic_Staff_01/sm_staff2h_chromatic_staff_01.glb",
    damage: 9.0,
    cooldown: 0.75,
    ..STAFF
};

pub const GRACE_OF_EOS: WeaponDef = WeaponDef {
    id: "grace_of_eos",
    name: "Grace of Eos",
    icon: "t_icon_weapon_weapon_staff2h_grace_of_eos",
    mesh: "weapons/TQ2/Content/TQ2/Character/Human/Weapon/Staff2H/Unique/Staff2H_Grace_Of_Eos_01/sm_staff2h_grace_of_eos_01.glb",
    damage: 12.0,
    range: 20.0,
    ..STAFF
};

pub const HADES_SCEPTER: WeaponDef = WeaponDef {
    id: "hades_scepter",
    name: "Hades' Scepter",
    icon: "t_icon_weapon_weapon_staff2h_hades_scepter",
    mesh: "weapons/TQ2/Content/TQ2/Character/Human/Weapon/Staff2H/Unique/Staff2H_Hades_Scepter_01/sm_staff2h_hades_scepter_01.glb",
    damage: 15.0,
    cooldown: 1.15,
    ..STAFF
};

pub const ILEKTRISMOS: WeaponDef = WeaponDef {
    id: "ilektrismos",
    name: "Ilektrismos",
    icon: "t_icon_weapon_weapon_staff2h_ilektrismos",
    mesh: "weapons/TQ2/Content/TQ2/Character/Human/Weapon/Staff2H/Unique/Staff2H_Ilektrismos_01/sm_staff2h_ilektrismos_01.glb",
    damage: 8.0,
    cooldown: 0.6,
    speed_or_radius: 30.0,
    ..STAFF
};

pub const BARRAGE: WeaponDef = WeaponDef {
    id: "barrage",
    name: "Barrage",
    icon: "t_icon_weapon_weapon_bow2h_barrage",
    mesh: "weapons/TQ2/Content/TQ2/Character/Human/Weapon/Bow2H/Unique/Bow2H_Barrage_Wide_01/sk_bow2h_barrage_wide_01.glb",
    damage: 6.5,
    cooldown: 0.45,
    ..BOW
};

pub const BOW_OF_EURYTUS: WeaponDef = WeaponDef {
    id: "bow_of_eurytus",
    name: "Bow of Eurytus",
    icon: "t_icon_weapon_weapon_bow2h_bow_of_eurytus",
    mesh: "weapons/TQ2/Content/TQ2/Character/Human/Weapon/Bow2H/Unique/Bow2H_Bow_Of_Eurytus_Narrow_01/sk_bow2h_bow_of_eurytus_narrow_01.glb",
    damage: 11.0,
    cooldown: 0.85,
    pierce: 3,
    ..BOW
};

pub const SPECTRAL_WARBOW: WeaponDef = WeaponDef {
    id: "spectral_warbow",
    name: "Spectral Warbow",
    icon: "t_icon_weapon_weapon_bow2h_spectral_warbow",
    mesh: "weapons/TQ2/Content/TQ2/Character/Human/Weapon/Bow2H/Unique/Bow2H_Spectral_Warbow_Narrow_01/sk_bow2h_spectral_warbow_narrow_01.glb",
    damage: 9.0,
    range: 30.0,
    pierce: 4,
    ..BOW
};

pub const TEUCERS_WARBOW: WeaponDef = WeaponDef {
    id: "teucers_warbow",
    name: "Teucer's Warbow",
    icon: "t_icon_weapon_weapon_bow2h_teucers_warbow",
    mesh: "weapons/TQ2/Content/TQ2/Character/Human/Weapon/Bow2H/Unique/Bow2H_Teucers_Warbow_Narrow_01/sk_bow2h_teucers_warbow_narrow_01.glb",
    damage: 10.0,
    ..BOW
};

pub const BONEHACK: WeaponDef = WeaponDef {
    id: "bonehack",
    name: "Bonehack",
    icon: "t_icon_weapon_weapon_axe2h_bonehack",
    mesh: "weapons/TQ2/Content/TQ2/Character/Human/Weapon/Axe2H/Unique/Axe2H_Bonehack_01/sm_axe2h_bonehack_01.glb",
    damage: 24.0,
    cooldown: 0.95,
    ..AXE
};

pub const HYDRA_FANG: WeaponDef = WeaponDef {
    id: "hydra_fang",
    name: "Hydra Fang",
    icon: "t_icon_weapon_weapon_axe2h_hydra_fang",
    mesh: "weapons/TQ2/Content/TQ2/Character/Human/Weapon/Axe2H/Unique/Axe2H_Hydra_Fang_01/sm_axe2h_hydra_fang_01.glb",
    damage: 31.0,
    speed_or_radius: 4.3,
    ..AXE
};

pub const MEGALOS: WeaponDef = WeaponDef {
    id: "megalos",
    name: "Megalos",
    icon: "t_icon_weapon_weapon_axe2h_megalos",
    mesh: "weapons/TQ2/Content/TQ2/Character/Human/Weapon/Axe2H/Unique/Axe2H_Megalos_01/sm_axe2h_megalos_01.glb",
    damage: 36.0,
    cooldown: 1.35,
    speed_or_radius: 4.6,
    ..AXE
};

pub const RIMESCYTHE: WeaponDef = WeaponDef {
    id: "rimescythe",
    name: "Rimescythe",
    icon: "t_icon_weapon_weapon_axe2h_rimescythe",
    mesh: "weapons/TQ2/Content/TQ2/Character/Human/Weapon/Axe2H/Unique/Axe2H_Rimescythe_01/sm_axe2h_rimescythe_01.glb",
    damage: 27.0,
    ..AXE
};

pub const ATLANTEAN_SABRE: WeaponDef = WeaponDef {
    id: "atlantean_sabre",
    name: "Atlantean Sabre",
    icon: "t_icon_weapon_weapon_sword1h_atlantean_sabre",
    mesh: "weapons/TQ2/Content/TQ2/Character/Human/Weapon/Sword1h/Unique/Sword1H_Atlantean_Sabre_01/sm_sword1h_atlantean_sabre_01.glb",
    damage: 15.0,
    cooldown: 0.65,
    ..SWORD
};

pub const DOXA_KALO: WeaponDef = WeaponDef {
    id: "doxa_kalo",
    name: "Doxa Kalo",
    icon: "t_icon_weapon_weapon_sword1h_doxa_kalo",
    mesh: "weapons/TQ2/Content/TQ2/Character/Human/Weapon/Sword1h/Unique/Sword1H_Doxa_Kalo_01/sm_sword1h_doxa_kalo_01.glb",
    damage: 17.0,
    ..SWORD
};

pub const SOUL_CARVER: WeaponDef = WeaponDef {
    id: "soul_carver",
    name: "Soul Carver",
    icon: "t_icon_weapon_weapon_sword1h_soul_carver",
    mesh: "weapons/TQ2/Content/TQ2/Character/Human/Weapon/Sword1h/Unique/Sword1H_Soul_Carver_01/sm_sword1h_soul_carver_01.glb",
    damage: 20.0,
    cooldown: 0.9,
    ..SWORD
};

pub const SWORD_OF_LIGHT: WeaponDef = WeaponDef {
    id: "sword_of_light",
    name: "Sword of Light",
    icon: "t_icon_weapon_weapon_sword1h_sword_of_light",
    mesh: "weapons/TQ2/Content/TQ2/Character/Human/Weapon/Sword1h/Unique/Sword1H_Sword_Of_Light_01/sm_sword1h_sword_of_light_01.glb",
    damage: 19.0,
    speed_or_radius: 3.5,
    ..SWORD
};

pub const AEGIS_OF_LACONIA: ShieldDef = ShieldDef {
    id: "aegis_of_laconia",
    name: "Aegis of Laconia",
    icon: "t_icon_weapon_itemicon_weapon_shield1h_aegis_of_laconia",
    mesh: "weapons/TQ2/Content/TQ2/Character/Human/Weapon/Shield1h/Unique/Shield1H_Aegis_Of_Laconia_01/sm_shield1h_aegis_of_laconia_01.glb",
    block: 0.26,
};

pub const DEIMOS: ShieldDef = ShieldDef {
    id: "deimos",
    name: "Deimos",
    icon: "t_icon_weapon_itemicon_weapon_shield1h_deimos",
    mesh: "weapons/TQ2/Content/TQ2/Character/Human/Weapon/Shield1h/Unique/Shield1H_Deimos_01/sm_shield1h_deimos_01.glb",
    block: 0.18,
};

pub const MOON_DISC: ShieldDef = ShieldDef {
    id: "moon_disc",
    name: "Moon Disc",
    icon: "t_icon_weapon_itemicon_weapon_shield1h_moon_disc",
    mesh: "weapons/TQ2/Content/TQ2/Character/Human/Weapon/Shield1h/Unique/Shield1H_Moon_Disc_01/sm_shield1h_moon_disc_01.glb",
    block: 0.16,
};

pub const THORNWALL: ShieldDef = ShieldDef {
    id: "thornwall",
    name: "Thornwall",
    icon: "t_icon_weapon_itemicon_weapon_shield1h_thornwall",
    mesh: "weapons/TQ2/Content/TQ2/Character/Human/Weapon/Shield1h/Unique/Shield1H_Thornwall_01/sm_shield1h_thornwall_01.glb",
    block: 0.22,
};

pub const BLOOD_GEM: TalismanDef = TalismanDef {
    id: "blood_gem",
    name: "Blood Gem",
    icon: "t_icon_weapon_itemicon_weapon_talisman1h_blood_gem",
    mesh: "weapons/TQ2/Content/TQ2/Character/Human/Weapon/Talisman1H/Unique/Talisman1H_Blood_Gem_01/sk_talisman1h_blood_gem_01.glb",
    skill_damage: 0.35,
};

pub const CHAOS_CUBE: TalismanDef = TalismanDef {
    id: "chaos_cube",
    name: "Chaos Cube",
    icon: "t_icon_weapon_itemicon_weapon_talisman1h_chaos_cube",
    mesh: "weapons/TQ2/Content/TQ2/Character/Human/Weapon/Talisman1H/Unique/Talisman1H_Chaos_Cube_01/sk_talisman1h_chaos_cube_01.glb",
    skill_damage: 0.5,
};

pub const GORGON_HEAD: TalismanDef = TalismanDef {
    id: "gorgon_head",
    name: "Gorgon Head",
    icon: "t_icon_weapon_itemicon_weapon_talisman1h_gorgon_head",
    mesh: "weapons/TQ2/Content/TQ2/Character/Human/Weapon/Talisman1H/Unique/Talisman1H_Gorgon_Head_01/sk_talisman1h_gorgon_head_01.glb",
    skill_damage: 0.45,
};

pub const MOLTEN_ORB: TalismanDef = TalismanDef {
    id: "molten_orb",
    name: "Molten Orb",
    icon: "t_icon_weapon_itemicon_weapon_talisman1h_molten_orb",
    mesh: "weapons/TQ2/Content/TQ2/Character/Human/Weapon/Talisman1H/Unique/Talisman1H_Molten_Orb_01/sk_talisman1h_molten_orb_01.glb",
    skill_damage: 0.4,
};

/// Parts every hero wears regardless of armour.
const HERO_BASE: &[&str] = &["hero/sk_male_base_animated.glb", "hero/sk_human_male_hair_01_animated.glb"];

pub const CHEST: &str = "props/TQ2/Content/TQ2/Environment/Greek/Props/Chest/sm_chest_01a.glb";

/// Any piece of gear (what the stash holds and chests offer).
#[derive(Clone, Copy)]
pub enum Item {
    Weapon(&'static WeaponDef),
    Shield(&'static ShieldDef),
    Talisman(&'static TalismanDef),
    Armor(&'static ArmorDef),
    Jewelry(&'static JewelryDef),
}

/// Every item in the game.
pub const ITEMS: &[Item] = &[
    Item::Weapon(&STAFF),
    Item::Weapon(&BOW),
    Item::Weapon(&AXE),
    Item::Weapon(&SWORD),
    Item::Weapon(&PELEUS),
    Item::Shield(&AENEAS_SHIELD),
    Item::Talisman(&ARCANE_MIRROR),
    Item::Armor(&ADVENTURER),
    Item::Armor(&ADVENTURER_LEGS),
    Item::Armor(&SCHOLAR),
    Item::Armor(&SCHOLAR_LEGS),
    Item::Armor(&RANGER),
    Item::Armor(&RANGER_LEGS),
    Item::Armor(&RANGER_SHOULDERS),
    Item::Weapon(&CHROMATIC_STAFF),
    Item::Weapon(&GRACE_OF_EOS),
    Item::Weapon(&HADES_SCEPTER),
    Item::Weapon(&ILEKTRISMOS),
    Item::Weapon(&BARRAGE),
    Item::Weapon(&BOW_OF_EURYTUS),
    Item::Weapon(&SPECTRAL_WARBOW),
    Item::Weapon(&TEUCERS_WARBOW),
    Item::Weapon(&BONEHACK),
    Item::Weapon(&HYDRA_FANG),
    Item::Weapon(&MEGALOS),
    Item::Weapon(&RIMESCYTHE),
    Item::Weapon(&ATLANTEAN_SABRE),
    Item::Weapon(&DOXA_KALO),
    Item::Weapon(&SOUL_CARVER),
    Item::Weapon(&SWORD_OF_LIGHT),
    Item::Shield(&AEGIS_OF_LACONIA),
    Item::Shield(&DEIMOS),
    Item::Shield(&MOON_DISC),
    Item::Shield(&THORNWALL),
    Item::Talisman(&BLOOD_GEM),
    Item::Talisman(&CHAOS_CUBE),
    Item::Talisman(&GORGON_HEAD),
    Item::Talisman(&MOLTEN_ORB),
];

/// Every item: the hand-made ones and the generated armour sets and jewelry.
pub fn all_items() -> impl Iterator<Item = Item> {
    ITEMS.iter().chain(super::armory::ITEMS).copied()
}

impl Item {
    pub fn id(self) -> &'static str {
        match self {
            Item::Weapon(w) => w.id,
            Item::Shield(s) => s.id,
            Item::Talisman(t) => t.id,
            Item::Armor(a) => a.id,
            Item::Jewelry(j) => j.id,
        }
    }

    pub fn from_id(id: &str) -> Option<Item> {
        // Items renamed in later versions keep working in old profiles.
        let id = match id {
            "staff_of_apollo" => "fotia",
            "sickle_of_kronos" => "harpe",
            "shield_of_heracles" => "shield_of_aeneas",
            other => other,
        };
        all_items().find(|i| i.id() == id)
    }

    /// TQ2's item icon (below the asset root).
    pub fn icon(self) -> String {
        let name = match self {
            Item::Weapon(w) => w.icon,
            Item::Shield(s) => s.icon,
            Item::Talisman(t) => t.icon,
            Item::Armor(a) => a.icon,
            Item::Jewelry(j) => j.icon,
        };
        format!("items/textures/{name}.png")
    }

    pub fn name(self) -> &'static str {
        match self {
            Item::Weapon(w) => w.name,
            Item::Shield(s) => s.name,
            Item::Talisman(t) => t.name,
            Item::Armor(a) => a.name,
            Item::Jewelry(j) => j.name,
        }
    }
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

/// What the off hand holds next to a one-handed weapon.
#[derive(Clone, Copy)]
pub enum OffHand {
    Weapon(&'static WeaponDef),
    Shield(&'static ShieldDef),
    Talisman(&'static TalismanDef),
}

impl OffHand {
    pub fn from_item(item: Item) -> Option<OffHand> {
        match item {
            Item::Weapon(w) if !w.two_handed => Some(OffHand::Weapon(w)),
            Item::Shield(s) => Some(OffHand::Shield(s)),
            Item::Talisman(t) => Some(OffHand::Talisman(t)),
            _ => None,
        }
    }
}

/// Dual wielding attacks this much faster.
const DUAL_WIELD_COOLDOWN: f32 = 0.7;

/// What the hero wears. Invariant: a two-handed main weapon means an empty off hand.
#[derive(Component, Clone, Copy)]
pub struct Equipment {
    pub main: &'static WeaponDef,
    pub off: Option<OffHand>,
    /// Armour pieces by `ArmorSlot`, with their quality.
    pub pieces: [Option<(&'static ArmorDef, f32)>; 5],
    /// Two rings and an amulet, with their quality.
    pub jewelry: [Option<(&'static JewelryDef, f32)>; 3],
    /// Quality rolls of the worn items (base value multipliers, see items.rs).
    pub main_quality: f32,
    pub off_quality: f32,
    /// Sum of the worn items' affixes.
    pub bonus: super::items::Bonus,
}

impl Default for Equipment {
    fn default() -> Self {
        let mut pieces = [None; 5];
        pieces[ArmorSlot::Chest as usize] = Some((&ADVENTURER, 1.0));
        pieces[ArmorSlot::Legs as usize] = Some((&ADVENTURER_LEGS, 1.0));
        Self { main: &STAFF, off: None, pieces, jewelry: [None; 3], main_quality: 1.0, off_quality: 1.0, bonus: default() }
    }
}

impl Equipment {
    /// Puts `item` in the main hand; a two-handed weapon empties the off hand.
    pub fn equip_main(&mut self, weapon: &'static WeaponDef) {
        self.main = weapon;
        if weapon.two_handed {
            self.off = None;
        }
    }

    /// Puts `off` in the off hand; only possible next to a one-handed weapon.
    pub fn equip_off(&mut self, off: Option<OffHand>) -> bool {
        if self.main.two_handed && off.is_some() {
            return false;
        }
        self.off = off;
        true
    }

    pub fn stance(&self) -> &'static Stance {
        match self.off {
            _ if self.main.two_handed => self.main.stance,
            None => self.main.stance,
            Some(OffHand::Weapon(_)) => &DUAL_SWORD_STANCE,
            Some(OffHand::Shield(_)) => &SWORD_SHIELD_STANCE,
            Some(OffHand::Talisman(_)) => &SWORD_TALISMAN_STANCE,
        }
    }

    /// Seconds between auto-attacks (before upgrades).
    pub fn cooldown(&self) -> f32 {
        let dual = matches!(self.off, Some(OffHand::Weapon(_)));
        self.main.cooldown * if dual { DUAL_WIELD_COOLDOWN } else { 1.0 }
    }

    /// Fraction of enemy damage absorbed by armour, shield and resistance affixes.
    pub fn damage_reduction(&self) -> f32 {
        let block = match self.off {
            Some(OffHand::Shield(s)) => s.block * self.off_quality,
            _ => 0.0,
        };
        let armor: f32 = self.pieces.iter().flatten().map(|(a, q)| a.armor * q).sum();
        (armor + block + self.bonus.reduction).min(0.75)
    }

    /// Damage of one weapon hit before damage bonuses: base × quality + flat affix damage.
    pub fn weapon_damage(&self) -> f32 {
        self.main.damage * self.main_quality + self.bonus.flat_damage
    }

    /// Multiplier for skill damage (talisman).
    pub fn skill_damage(&self) -> f32 {
        match self.off {
            Some(OffHand::Talisman(t)) => 1.0 + t.skill_damage * self.off_quality,
            _ => 1.0,
        }
    }

    /// Puts an armour piece in its slot.
    pub fn wear(&mut self, piece: &'static ArmorDef, quality: f32) {
        self.pieces[piece.slot as usize] = Some((piece, quality));
    }

    /// Critical chance from rings.
    pub fn jewelry_crit(&self) -> f32 {
        self.jewelry.iter().flatten().filter(|(j, _)| j.kind == JewelryKind::Ring).map(|(j, q)| j.value * q).sum()
    }

    /// Health from the amulet.
    pub fn jewelry_health(&self) -> f32 {
        self.jewelry.iter().flatten().filter(|(j, _)| j.kind == JewelryKind::Amulet).map(|(j, q)| j.value * q).sum()
    }

    pub fn parts(&self) -> DesiredParts {
        // A helmet replaces the hair.
        let helmet = self.pieces[ArmorSlot::Head as usize].is_some();
        let base = HERO_BASE.iter().copied().filter(|p| !(helmet && p.contains("hair")));
        DesiredParts(base.chain(self.pieces.iter().flatten().flat_map(|(a, _)| a.parts.iter().copied())).collect())
    }

    pub fn attachments(&self) -> Attachments {
        let flip = |f: bool| if f { Quat::from_rotation_x(std::f32::consts::PI) } else { Quat::IDENTITY };
        let mut list = vec![Attachment { bone: self.main.bone, scene: self.main.mesh, rotation: flip(self.main.flip) }];
        let left = |scene, rotation| Attachment { bone: "HandAttach_L", scene, rotation };
        match self.off {
            // HandAttach_L is mirrored against HandAttach_R, so the blade flip is inverted.
            Some(OffHand::Weapon(w)) => list.push(left(w.mesh, flip(!w.flip))),
            Some(OffHand::Shield(s)) => list.push(left(s.mesh, Quat::IDENTITY)),
            Some(OffHand::Talisman(t)) => list.push(left(t.mesh, Quat::IDENTITY)),
            None => {}
        }
        Attachments(list)
    }
}

/// A chest on the ground (TQ2 Greek chest); `luck` raises the rarity of its items.
#[derive(Component)]
pub struct LootChest {
    pub luck: f32,
}

/// The items offered by the chest being opened.
#[derive(Resource, Default)]
pub struct LootOffer(pub Vec<super::items::ItemInstance>);

/// Items picked from chests during this run; they go to the stash when the run ends.
#[derive(Resource, Default)]
pub struct RunLoot(pub Vec<super::items::ItemInstance>);

pub fn plugin(app: &mut App) {
    app.init_resource::<LootOffer>()
        .init_resource::<RunLoot>()
        .add_systems(Update, (apply_equipment, place_chests))
        .add_systems(FixedUpdate, open_chests.run_if(in_state(GameState::Playing)));
}

/// Turns equipment changes into visible parts and carried weapons.
fn apply_equipment(mut commands: Commands, changed: Query<(Entity, &Equipment), Changed<Equipment>>) {
    for (hero, eq) in &changed {
        commands.entity(hero).insert((eq.parts(), eq.attachments()));
    }
}

pub fn drop_chest(commands: &mut Commands, assets: &AssetServer, at: Vec2, luck: f32) {
    commands.spawn((
        LootChest { luck },
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

/// Walking into a chest opens it: three rolled items (rarity by the chest's luck) to choose
/// one from.
fn open_chests(
    mut commands: Commands,
    mut offer: ResMut<LootOffer>,
    mut next: ResMut<NextState<GameState>>,
    book: Res<super::items::AffixBook>,
    hero: Query<&Pos, With<Hero>>,
    chests: Query<(Entity, &Pos, &LootChest)>,
) {
    let Ok(hero) = hero.single() else { return };
    let Some((chest, _, c)) = chests.iter().find(|(_, p, _)| p.0.distance(hero.0) < 1.6) else { return };
    commands.entity(chest).despawn();
    // Uids are assigned when the run is banked.
    offer.0 = (0..3).map(|_| super::items::roll_loot(0, c.luck, &book)).collect();
    info!("chest offers: {:?}", offer.0.iter().map(|i| format!("{} ({:?})", i.name(&book), i.rarity)).collect::<Vec<_>>());
    next.set(GameState::Loot);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_handed_empties_off_hand() {
        let mut eq = Equipment { main: &SWORD, off: Some(OffHand::Shield(&AENEAS_SHIELD)), ..default() };
        eq.equip_main(&AXE);
        assert!(eq.off.is_none());
        assert!(!eq.equip_off(Some(OffHand::Talisman(&ARCANE_MIRROR))));
        eq.equip_main(&SWORD);
        assert!(eq.equip_off(Some(OffHand::Weapon(&PELEUS))));
        assert_eq!(eq.stance().idle, DUAL_SWORD_STANCE.idle);
    }

    #[test]
    fn only_one_handed_weapons_go_off_hand() {
        assert!(OffHand::from_item(Item::Weapon(&BOW)).is_none());
        assert!(OffHand::from_item(Item::Weapon(&PELEUS)).is_some());
        assert!(OffHand::from_item(Item::Armor(&RANGER)).is_none());
    }

    #[test]
    fn item_ids_are_unique() {
        let all: Vec<Item> = all_items().collect();
        for (i, a) in all.iter().enumerate() {
            assert!(all[i + 1..].iter().all(|b| a.id() != b.id()), "{}", a.id());
            assert!(Item::from_id(a.id()).is_some());
        }
    }
}

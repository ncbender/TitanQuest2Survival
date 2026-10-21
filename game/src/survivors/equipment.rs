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
}

macro_rules! stance {
    ($set:literal) => {
        Stance {
            idle: concat!("a_male_", $set, "_idle_combat_01"),
            run: concat!("a_male_", $set, "_run_01"),
            attacks: &[
                concat!("a_male_", $set, "_attack_01_01"),
                concat!("a_male_", $set, "_attack_02_01"),
                concat!("a_male_", $set, "_attack_03_01"),
            ],
        }
    };
}

const STAFF_STANCE: Stance = stance!("staff2h");
const BOW_STANCE: Stance = stance!("bow");
const AXE_STANCE: Stance = stance!("axe2h");
const SWORD_STANCE: Stance = stance!("sword");
const SWORD_SHIELD_STANCE: Stance = stance!("sword+shield");
const DUAL_SWORD_STANCE: Stance = stance!("sword+sword");
const SWORD_TALISMAN_STANCE: Stance = stance!("sword+talisman");

pub struct WeaponDef {
    pub id: &'static str,
    pub name: &'static str,
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
    id: "staff_of_apollo",
    name: "Staff of Apollo",
    mesh: "weapons/TQ2/Content/TQ2/Character/Human/Weapon/Staff2H/Unique/Staff2H_Staff_Of_Apollo_01/sm_staff2h_staff_of_apollo_01.glb",
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
    id: "sickle_of_kronos",
    name: "Sickle of Kronos",
    mesh: "weapons/TQ2/Content/TQ2/Character/Human/Weapon/Axe2H/Unique/Axe2H_Sickle_Of_Kronos_01/sm_axe2h_sickle_of_kronos_01.glb",
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
    pub mesh: &'static str,
    pub block: f32,
}

pub const HERACLES_SHIELD: ShieldDef = ShieldDef {
    id: "shield_of_heracles",
    name: "Shield of Heracles",
    mesh: "weapons/TQ2/Content/TQ2/Character/Human/Weapon/Shield1H/Unique/Shield1H_Shield_Of_Heracles_01/sm_shield1h_shield_of_heracles_01.glb",
    block: 0.2,
};

/// Off-hand talisman: strengthens skills (Fire Nova, Meteor).
pub struct TalismanDef {
    pub id: &'static str,
    pub name: &'static str,
    pub mesh: &'static str,
    pub skill_damage: f32,
}

pub const ARCANE_MIRROR: TalismanDef = TalismanDef {
    id: "arcane_mirror",
    name: "Arcane Mirror",
    mesh: "weapons/TQ2/Content/TQ2/Character/Human/Weapon/Talisman1H/Unique/Talisman1H_Arcane_Mirror_01/sk_talisman1h_arcane_mirror_01.glb",
    skill_damage: 0.4,
};

pub struct ArmorDef {
    pub id: &'static str,
    pub name: &'static str,
    pub parts: &'static [&'static str],
    /// Fraction of enemy damage absorbed.
    pub armor: f32,
}

pub const ADVENTURER: ArmorDef = ArmorDef {
    id: "adventurer",
    name: "Adventurer's Garb",
    parts: &["hero/sk_male_torso_greek_adventurer_01_animated.glb", "hero/sk_male_leg_greek_adventurer_01_animated.glb"],
    armor: 0.0,
};

pub const SCHOLAR: ArmorDef = ArmorDef {
    id: "scholar",
    name: "Scholar's Robes",
    parts: &["hero/sk_male_torso_greek_scholar_02_01_animated.glb", "hero/sk_male_leg_greek_scholar_02_01_animated.glb"],
    armor: 0.12,
};

pub const RANGER: ArmorDef = ArmorDef {
    id: "ranger",
    name: "Ranger's Armour",
    parts: &[
        "hero/sk_male_torso_greek_ranger_02_animated.glb",
        "hero/sk_male_leg_greek_ranger_02_animated.glb",
        "hero/sk_male_shoulder_greek_ranger_02_animated.glb",
    ],
    armor: 0.25,
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
}

/// Every item in the game.
pub const ITEMS: &[Item] = &[
    Item::Weapon(&STAFF),
    Item::Weapon(&BOW),
    Item::Weapon(&AXE),
    Item::Weapon(&SWORD),
    Item::Weapon(&PELEUS),
    Item::Shield(&HERACLES_SHIELD),
    Item::Talisman(&ARCANE_MIRROR),
    Item::Armor(&ADVENTURER),
    Item::Armor(&SCHOLAR),
    Item::Armor(&RANGER),
];

impl Item {
    pub fn id(self) -> &'static str {
        match self {
            Item::Weapon(w) => w.id,
            Item::Shield(s) => s.id,
            Item::Talisman(t) => t.id,
            Item::Armor(a) => a.id,
        }
    }

    pub fn from_id(id: &str) -> Option<Item> {
        ITEMS.iter().copied().find(|i| i.id() == id)
    }

    pub fn name(self) -> &'static str {
        match self {
            Item::Weapon(w) => w.name,
            Item::Shield(s) => s.name,
            Item::Talisman(t) => t.name,
            Item::Armor(a) => a.name,
        }
    }

    /// One line for menus: kind and what it does.
    pub fn title(self) -> String {
        match self {
            Item::Weapon(w) => format!("{} ({}) – {}", w.name, if w.two_handed { "two-handed" } else { "one-handed" }, w.attack_text()),
            Item::Shield(s) => format!("{} (shield) – blocks {:.0} % of damage", s.name, s.block * 100.0),
            Item::Talisman(t) => format!("{} (talisman) – +{:.0} % skill damage", t.name, t.skill_damage * 100.0),
            Item::Armor(a) => format!("{} (armour) – {:.0} % less damage taken", a.name, a.armor * 100.0),
        }
    }

    pub fn same(self, other: Item) -> bool {
        self.id() == other.id()
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
    pub fn item(self) -> Item {
        match self {
            OffHand::Weapon(w) => Item::Weapon(w),
            OffHand::Shield(s) => Item::Shield(s),
            OffHand::Talisman(t) => Item::Talisman(t),
        }
    }

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
    pub armor: &'static ArmorDef,
}

impl Default for Equipment {
    fn default() -> Self {
        Self { main: &STAFF, off: None, armor: &ADVENTURER }
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

    /// Fraction of enemy damage absorbed by armour and shield.
    pub fn damage_reduction(&self) -> f32 {
        let block = match self.off {
            Some(OffHand::Shield(s)) => s.block,
            _ => 0.0,
        };
        (self.armor.armor + block).min(0.75)
    }

    /// Multiplier for skill damage (talisman).
    pub fn skill_damage(&self) -> f32 {
        match self.off {
            Some(OffHand::Talisman(t)) => 1.0 + t.skill_damage,
            _ => 1.0,
        }
    }

    pub fn parts(&self) -> DesiredParts {
        DesiredParts(HERO_BASE.iter().chain(self.armor.parts).copied().collect())
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

/// A chest on the ground (TQ2 Greek chest).
#[derive(Component)]
pub struct LootChest;

/// The items offered by the chest being opened.
#[derive(Resource, Default)]
pub struct LootOffer(pub Vec<Item>);

/// Items picked from chests during this run; they go to the stash when the run ends.
#[derive(Resource, Default)]
pub struct RunLoot(pub Vec<Item>);

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

/// Walking into a chest opens it: three random items to choose one from.
fn open_chests(
    mut commands: Commands,
    mut offer: ResMut<LootOffer>,
    mut next: ResMut<NextState<GameState>>,
    hero: Query<&Pos, With<Hero>>,
    chests: Query<(Entity, &Pos), With<LootChest>>,
) {
    let Ok(hero) = hero.single() else { return };
    let Some((chest, _)) = chests.iter().find(|(_, p)| p.0.distance(hero.0) < 1.6) else { return };
    commands.entity(chest).despawn();
    let mut pool = ITEMS.to_vec();
    fastrand::shuffle(&mut pool);
    pool.truncate(3);
    info!("chest offers: {:?}", pool.iter().map(|i| i.name()).collect::<Vec<_>>());
    offer.0 = pool;
    next.set(GameState::Loot);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_handed_empties_off_hand() {
        let mut eq = Equipment { main: &SWORD, off: Some(OffHand::Shield(&HERACLES_SHIELD)), armor: &ADVENTURER };
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
        for (i, a) in ITEMS.iter().enumerate() {
            assert!(ITEMS[i + 1..].iter().all(|b| !a.same(*b)), "{}", a.id());
            assert!(Item::from_id(a.id()).is_some());
        }
    }
}

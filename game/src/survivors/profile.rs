//! The save: a stash of rolled items shared by all characters, and the characters themselves,
//! each with its own TQ2 masteries, learned skills and passives, unspent points and loadout
//! (worn items by id; an item can only be worn by one character at a time). Saved as JSON in
//! `save/profile.json` in the project folder (git-ignored). Gear and skills only change between
//! runs; at the end of a run the picked chest items go to the shared stash and the run's points
//! to the character that played it.
//!
//! `Profile` dereferences to the active character, so `profile.masteries` etc. read and write
//! the character being played.
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::ops::{Deref, DerefMut};
use std::path::PathBuf;

use super::equipment::{Equipment, Item, OffHand, RunLoot, ADVENTURER, WeaponDef};
use super::items::{AffixBook, ItemInstance};
use super::{GameState, Run};

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub struct Character {
    /// False only for the placeholder used while no character exists.
    pub created: bool,
    pub name: String,
    /// Chosen TQ2 masteries (ids like "storm"), at most two.
    pub masteries: Vec<String>,
    /// Unspent points (TQ2 has separate points for skills and passives).
    pub active_points: u32,
    pub passive_points: u32,
    /// Learned active skills (TQ2 ability id) → level.
    pub actives: BTreeMap<String, u32>,
    /// Learned passives → level.
    pub passives: BTreeMap<String, u32>,
    /// Learned skills that cast themselves during runs.
    pub equipped: Vec<String>,
    /// Worn items (stash ids).
    pub main: Option<u64>,
    pub off: Option<u64>,
    pub armor: Option<u64>,
    pub runs: u32,
    pub best_time: f32,
}

static NO_CHARACTER: Character = Character {
    created: false,
    name: String::new(),
    masteries: Vec::new(),
    active_points: 0,
    passive_points: 0,
    actives: BTreeMap::new(),
    passives: BTreeMap::new(),
    equipped: Vec::new(),
    main: None,
    off: None,
    armor: None,
    runs: 0,
    best_time: 0.0,
};

impl Character {
    pub fn active_level(&self, id: &str) -> u32 {
        self.actives.get(id).copied().unwrap_or(0)
    }

    pub fn passive_level(&self, id: &str) -> u32 {
        self.passives.get(id).copied().unwrap_or(0)
    }

    fn worn(&self) -> [Option<u64>; 3] {
        [self.main, self.off, self.armor]
    }
}

#[derive(Resource, Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub struct Profile {
    /// Items of all characters.
    pub stash: Vec<ItemInstance>,
    pub characters: Vec<Character>,
    /// Index of the character being played.
    pub active: usize,
    pub next_uid: u64,
}

impl Deref for Profile {
    type Target = Character;
    fn deref(&self) -> &Character {
        self.characters.get(self.active).unwrap_or(&NO_CHARACTER)
    }
}

impl DerefMut for Profile {
    fn deref_mut(&mut self) -> &mut Character {
        if self.characters.is_empty() {
            self.characters.push(Character::default());
        }
        self.active = self.active.min(self.characters.len() - 1);
        &mut self.characters[self.active]
    }
}

/// Where the profile is saved; test runs use a separate file so they never touch the real one.
#[derive(Resource)]
struct ProfilePath(PathBuf);

/// Points and items credited by the last finished run (shown on the end screen).
#[derive(Resource, Default)]
pub struct LastReward {
    pub active: u32,
    pub passive: u32,
    pub items: Vec<(String, super::items::Rarity)>,
}

/// Greek names for new heroes.
const NAMES: &[&str] = &["Leander", "Kallias", "Theron", "Nikias", "Lysander", "Damon", "Orestes", "Alkaios", "Phaidon", "Ione", "Kassia", "Thalia"];

impl Profile {
    pub fn new_uid(&mut self) -> u64 {
        self.next_uid += 1;
        self.next_uid
    }

    /// Adds a new character (starting weapon and adventurer's garb go to the stash) and plays it.
    pub fn new_hero(&mut self, weapon: &'static WeaponDef) {
        let name = NAMES.iter().find(|n| !self.characters.iter().any(|c| c.name == **n)).map_or_else(|| format!("Hero {}", self.characters.len() + 1), |n| n.to_string());
        let w = self.new_uid();
        let a = self.new_uid();
        self.stash.push(ItemInstance::plain(w, Item::Weapon(weapon)));
        self.stash.push(ItemInstance::plain(a, Item::Armor(&ADVENTURER)));
        self.characters.push(Character { created: true, name, main: Some(w), armor: Some(a), ..default() });
        self.active = self.characters.len() - 1;
    }

    /// Removes a character; its items stay in the stash.
    pub fn delete_character(&mut self, index: usize) {
        if index < self.characters.len() {
            self.characters.remove(index);
            self.active = self.active.min(self.characters.len().saturating_sub(1));
        }
    }

    pub fn item(&self, uid: u64) -> Option<&ItemInstance> {
        self.stash.iter().find(|i| i.uid == uid)
    }

    /// The character (index) wearing an item.
    pub fn worn_by(&self, uid: u64) -> Option<usize> {
        self.characters.iter().position(|c| c.worn().contains(&Some(uid)))
    }

    /// The active character's loadout with quality rolls and affix bonuses.
    pub fn equipment(&self, book: &AffixBook) -> Equipment {
        let mut eq = Equipment::default();
        let get = |uid: Option<u64>| uid.and_then(|u| self.item(u)).and_then(|i| Some((i, i.item()?)));
        if let Some((inst, Item::Weapon(w))) = get(self.main) {
            eq.equip_main(w);
            eq.main_quality = inst.quality;
            inst.add_to(&mut eq.bonus, book);
        }
        if let Some((inst, Item::Armor(a))) = get(self.armor) {
            eq.armor = a;
            eq.armor_quality = inst.quality;
            inst.add_to(&mut eq.bonus, book);
        }
        if let Some((inst, item)) = get(self.off) {
            if eq.equip_off(OffHand::from_item(item)) {
                eq.off_quality = inst.quality;
                inst.add_to(&mut eq.bonus, book);
            }
        }
        eq
    }

    /// Puts a stash item into a slot of the active character (None empties the slot).
    pub fn equip(&mut self, slot: Slot, uid: Option<u64>) -> Result<(), String> {
        let item = match uid {
            Some(u) => {
                let inst = self.item(u).ok_or("unknown item")?;
                let item = inst.item().ok_or("unknown item base")?;
                if !slot.fits(item) {
                    return Err("does not fit this slot".into());
                }
                if let Some(other) = self.worn_by(u).filter(|o| *o != self.active) {
                    return Err(format!("worn by {}", self.characters[other].name));
                }
                Some(item)
            }
            None if slot == Slot::Main => return Err("a weapon is needed".into()),
            None => None,
        };
        let main_two_handed = |p: &Profile| p.main.and_then(|u| p.item(u)).and_then(|i| i.item()).is_some_and(|i| matches!(i, Item::Weapon(w) if w.two_handed));
        let c: &mut Character = self;
        // Take the item out of whatever slot of this character held it.
        for s in [&mut c.main, &mut c.off, &mut c.armor] {
            if uid.is_some() && *s == uid {
                *s = None;
            }
        }
        match slot {
            Slot::Main => {
                c.main = uid;
                if matches!(item, Some(Item::Weapon(w)) if w.two_handed) {
                    c.off = None;
                }
            }
            Slot::Off => {
                if uid.is_some() && main_two_handed(self) {
                    return Err("a two-handed weapon needs both hands".into());
                }
                self.off = uid;
            }
            Slot::Armor => self.armor = uid,
        }
        // The main hand must never be empty.
        if self.main.is_none() {
            let weapon = self.stash.iter().find(|i| matches!(i.item(), Some(Item::Weapon(_))) && self.worn_by(i.uid).is_none()).map(|i| i.uid);
            self.main = weapon;
        }
        Ok(())
    }

    /// Converts saves of earlier versions (one hero, items as id strings) to this format.
    fn from_legacy(v: &serde_json::Value) -> Profile {
        let mut p = Profile::default();
        let s = |k: &str| v.get(k).and_then(|x| x.as_str()).map(str::to_string);
        if !v.get("created").and_then(|x| x.as_bool()).unwrap_or(false) {
            return p;
        }
        // Old item fields were ids as text; they are converted below, not deserialised.
        let mut fields = v.clone();
        if let Some(o) = fields.as_object_mut() {
            for k in ["main", "off", "armor", "stash"] {
                o.remove(k);
            }
        }
        let mut c: Character = serde_json::from_value(fields).unwrap_or_default();
        c.created = true;
        c.name = NAMES[0].into();
        let (main, off, armor) = (s("main"), s("off"), s("armor"));
        let (mut wm, mut wo, mut wa) = (None, None, None);
        for id in v.get("stash").and_then(|x| x.as_array()).into_iter().flatten().filter_map(|x| x.as_str()) {
            let Some(item) = Item::from_id(id) else { continue };
            let uid = p.new_uid();
            p.stash.push(ItemInstance::plain(uid, item));
            let canonical = Some(item.id().to_string());
            let same = |x: &Option<String>| x.as_deref().and_then(Item::from_id).map(|i| i.id().to_string()) == canonical;
            if wm.is_none() && same(&main) {
                wm = Some(uid);
            } else if wo.is_none() && same(&off) {
                wo = Some(uid);
            } else if wa.is_none() && same(&armor) {
                wa = Some(uid);
            }
        }
        (c.main, c.off, c.armor) = (wm, wo, wa);
        // The placeholder skill tree of the first versions becomes passive points.
        let favor = v.get("favor").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
        let spent: u32 = v.get("skills").and_then(|x| x.as_object()).map_or(0, |m| m.values().filter_map(|x| x.as_u64()).sum::<u64>() as u32);
        if favor + spent > 0 {
            c.passive_points += favor + spent;
            c.active_points += 1;
        }
        p.characters.push(c);
        p
    }
}

/// Equipment slots of the armoury.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Slot {
    #[default]
    Main,
    Off,
    Armor,
}

impl Slot {
    pub fn fits(self, item: Item) -> bool {
        match self {
            Slot::Main => matches!(item, Item::Weapon(_)),
            Slot::Off => OffHand::from_item(item).is_some(),
            Slot::Armor => matches!(item, Item::Armor(_)),
        }
    }
}

/// Points for a run: (active, passive). Passive: one per level gained and per full minute,
/// five for surviving. Active: one per run plus one per three levels, two for surviving.
pub fn points_for(run: &Run) -> (u32, u32) {
    let levels = run.level.saturating_sub(1);
    let active = 1 + levels / 3 + if run.won { 2 } else { 0 };
    let passive = levels + run.time as u32 / 60 + if run.won { 5 } else { 0 };
    (active, passive)
}

pub fn plugin(app: &mut App) {
    let path = profile_path();
    let profile = load(&path);
    info!("profile {} ({} characters, {} items)", path.display(), profile.characters.len(), profile.stash.len());
    app.insert_resource(profile)
        .insert_resource(ProfilePath(path))
        .init_resource::<LastReward>()
        .add_systems(OnEnter(GameState::GameOver), bank_run)
        // Leaving a run through the pause menu banks it too.
        .add_systems(OnEnter(GameState::MainMenu), bank_run)
        .add_systems(Last, save.run_if(resource_changed::<Profile>));
}

/// `save/profile.json` next to the `cache` folder (TQ2_PROFILE overrides it). Automated test
/// runs default to `save/test_profile.json`.
fn profile_path() -> PathBuf {
    if let Ok(p) = std::env::var("TQ2_PROFILE") {
        return p.into();
    }
    let export = PathBuf::from(crate::export_root());
    let dir = export.parent().and_then(|p| p.parent()).map(|p| p.join("save")).unwrap_or_else(|| "save".into());
    dir.join(if super::is_test_run() { "test_profile.json" } else { "profile.json" })
}

/// Reads the profile (converting old formats); a file that does not parse is kept as `*.bad`
/// instead of being overwritten by the next save.
fn load(path: &std::path::Path) -> Profile {
    let Ok(text) = std::fs::read_to_string(path) else { return Profile::default() };
    // Editors (and PowerShell 5) may write a UTF-8 byte order mark.
    let parsed = serde_json::from_str::<serde_json::Value>(text.trim_start_matches('\u{feff}')).and_then(|v| {
        if v.get("characters").is_some() {
            serde_json::from_value::<Profile>(v)
        } else {
            Ok(Profile::from_legacy(&v))
        }
    });
    match parsed {
        Ok(profile) => profile,
        Err(e) => {
            let backup = path.with_extension("json.bad");
            error!("profile {} is unreadable ({e}); kept as {}", path.display(), backup.display());
            let _ = std::fs::copy(path, backup);
            Profile::default()
        }
    }
}

fn save(profile: Res<Profile>, path: Res<ProfilePath>) {
    if profile.characters.is_empty() && profile.stash.is_empty() {
        return;
    }
    let write = || -> std::io::Result<()> {
        if let Some(dir) = path.0.parent() {
            std::fs::create_dir_all(dir)?;
        }
        // Write a temporary file first so a crash never leaves a half-written profile.
        let tmp = path.0.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_string_pretty(&*profile).unwrap_or_default())?;
        std::fs::rename(&tmp, &path.0)
    };
    if let Err(e) = write() {
        error!("saving profile {}: {e}", path.0.display());
    }
}

/// Credits a finished (or abandoned) run once: loot to the shared stash, points to the hero.
pub fn bank_run(
    mut run: ResMut<Run>,
    mut loot: ResMut<RunLoot>,
    mut profile: ResMut<Profile>,
    mut reward: ResMut<LastReward>,
    book: Res<AffixBook>,
) {
    if run.banked || run.level == 0 {
        return;
    }
    run.banked = true;
    let (active, passive) = points_for(&run);
    let mut items = vec![];
    for mut item in loot.0.drain(..) {
        item.uid = profile.new_uid();
        items.push((item.name(&book), item.rarity));
        profile.stash.push(item);
    }
    profile.active_points += active;
    profile.passive_points += passive;
    profile.runs += 1;
    profile.best_time = profile.best_time.max(run.time);
    info!("run banked: +{active} active / +{passive} passive points, items {items:?}");
    *reward = LastReward { active, passive, items };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::survivors::equipment::{AENEAS_SHIELD, AXE, PELEUS, SWORD};

    fn add(p: &mut Profile, item: Item) -> u64 {
        let uid = p.new_uid();
        p.stash.push(ItemInstance::plain(uid, item));
        uid
    }

    #[test]
    fn characters_share_the_stash_but_not_worn_items() {
        let mut p = Profile::default();
        p.new_hero(&SWORD);
        let shield = add(&mut p, Item::Shield(&AENEAS_SHIELD));
        p.equip(Slot::Off, Some(shield)).unwrap();
        p.masteries.push("storm".into());
        p.new_hero(&PELEUS);
        assert_eq!(p.characters.len(), 2);
        assert!(p.masteries.is_empty(), "masteries belong to the character");
        assert!(p.equip(Slot::Off, Some(shield)).unwrap_err().contains("worn by"));
        p.active = 0;
        p.equip(Slot::Off, None).unwrap();
        p.active = 1;
        p.equip(Slot::Off, Some(shield)).unwrap();
        assert!(p.equipment(&AffixBook::default()).off.is_some());
    }

    #[test]
    fn two_handed_main_empties_the_off_hand() {
        let mut p = Profile::default();
        p.new_hero(&SWORD);
        let shield = add(&mut p, Item::Shield(&AENEAS_SHIELD));
        let axe = add(&mut p, Item::Weapon(&AXE));
        p.equip(Slot::Off, Some(shield)).unwrap();
        p.equip(Slot::Main, Some(axe)).unwrap();
        assert!(p.off.is_none());
        assert!(p.equip(Slot::Off, Some(shield)).is_err());
    }

    #[test]
    fn points() {
        let run = Run { level: 7, time: 185.0, won: false, ..default() };
        assert_eq!(points_for(&run), (1 + 2, 6 + 3));
    }

    #[test]
    fn legacy_save_is_converted() {
        let v: serde_json::Value = serde_json::from_str(
            r#"{"created":true,"stash":["staff_of_apollo","shield_of_heracles","battlemage_sword"],"main":"battlemage_sword","off":"shield_of_heracles","armor":"adventurer","masteries":["storm"],"favor":3,"skills":{"might":2}}"#,
        )
        .unwrap();
        let p = Profile::from_legacy(&v);
        assert_eq!(p.characters.len(), 1);
        assert_eq!(p.stash.len(), 3);
        assert_eq!(p.masteries, vec!["storm".to_string()]);
        assert_eq!((p.passive_points, p.active_points), (5, 1));
        let eq = p.equipment(&AffixBook::default());
        assert_eq!(eq.main.id, SWORD.id);
        assert!(eq.off.is_some());
    }
}

//! The persistent hero (meta progression): the stash of found gear, the loadout chosen in
//! the armoury, unspent favour and the skill tree. Saved as JSON in `save/profile.json` in the
//! project folder (git-ignored). Gear only changes between runs; at the end of a run the
//! picked chest items go to the stash and the run's favour is credited.
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

use super::equipment::{Equipment, Item, OffHand, RunLoot, ADVENTURER, STAFF, WeaponDef};
use super::{GameState, Run};

#[derive(Resource, Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub struct Profile {
    /// False until character creation; nothing is saved before that.
    pub created: bool,
    /// Item ids, one entry per copy (two copies of a sword allow dual wielding it).
    pub stash: Vec<String>,
    pub main: String,
    pub off: Option<String>,
    pub armor: String,
    /// Unspent skill points.
    pub favor: u32,
    /// Skill-tree node id → rank.
    pub skills: BTreeMap<String, u32>,
    pub runs: u32,
    pub best_time: f32,
}

/// Where the profile is saved; test runs use a separate file so they never touch the real one.
#[derive(Resource)]
struct ProfilePath(PathBuf);

/// Favour and items credited by the last finished run (shown on the end screen).
#[derive(Resource, Default)]
pub struct LastReward {
    pub favor: u32,
    pub items: Vec<&'static str>,
}

impl Profile {
    pub fn new_hero(weapon: &'static WeaponDef) -> Self {
        Self {
            created: true,
            stash: vec![weapon.id.into(), ADVENTURER.id.into()],
            main: weapon.id.into(),
            armor: ADVENTURER.id.into(),
            ..default()
        }
    }

    pub fn copies(&self, item: Item) -> usize {
        self.stash.iter().filter(|id| *id == item.id()).count()
    }

    /// The saved loadout; anything unknown or missing from the stash falls back to defaults.
    pub fn equipment(&self) -> Equipment {
        let mut eq = Equipment::default();
        if let Some(Item::Weapon(w)) = Item::from_id(&self.main) {
            eq.equip_main(w);
        } else if let Some(w) = self.stash.iter().find_map(|id| match Item::from_id(id) {
            Some(Item::Weapon(w)) => Some(w),
            _ => None,
        }) {
            eq.equip_main(w);
        } else {
            eq.equip_main(&STAFF);
        }
        if let Some(Item::Armor(a)) = Item::from_id(&self.armor) {
            eq.armor = a;
        }
        if let Some(off) = self.off.as_deref().and_then(Item::from_id).and_then(OffHand::from_item) {
            // A second copy is needed to dual-wield the same sword.
            let needed = if off.item().same(Item::Weapon(eq.main)) { 2 } else { 1 };
            if self.copies(off.item()) >= needed {
                eq.equip_off(Some(off));
            }
        }
        eq
    }

    pub fn set_equipment(&mut self, eq: &Equipment) {
        self.main = eq.main.id.into();
        self.off = eq.off.map(|o| o.item().id().into());
        self.armor = eq.armor.id.into();
    }

    /// Copies of `item` not worn in a slot other than `except`.
    pub fn free_copies(&self, item: Item, eq: &Equipment, except: Slot) -> usize {
        let worn = [
            (Slot::Main, Some(Item::Weapon(eq.main))),
            (Slot::Off, eq.off.map(OffHand::item)),
            (Slot::Armor, Some(Item::Armor(eq.armor))),
        ]
        .into_iter()
        .filter(|(slot, worn)| *slot != except && worn.is_some_and(|w| w.same(item)))
        .count();
        self.copies(item).saturating_sub(worn)
    }

    pub fn rank(&self, node: &str) -> u32 {
        self.skills.get(node).copied().unwrap_or(0)
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

/// Favour for a run: one per level gained, one per full minute, five for surviving.
pub fn favor_for(run: &Run) -> u32 {
    run.level.saturating_sub(1) + run.time as u32 / 60 + if run.won { 5 } else { 0 }
}

pub fn plugin(app: &mut App) {
    let path = profile_path();
    let profile = load(&path);
    info!("profile {}", path.display());
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

/// Reads the profile; a file that does not parse is kept as `*.bad` instead of being
/// overwritten by the next save.
fn load(path: &std::path::Path) -> Profile {
    let Ok(text) = std::fs::read_to_string(path) else { return Profile::default() };
    // Editors (and PowerShell 5) may write a UTF-8 byte order mark.
    match serde_json::from_str::<Profile>(text.trim_start_matches('\u{feff}')) {
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
    if !profile.created {
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

/// Credits a finished (or abandoned) run once: loot to the stash, favour to spend.
pub fn bank_run(mut run: ResMut<Run>, mut loot: ResMut<RunLoot>, mut profile: ResMut<Profile>, mut reward: ResMut<LastReward>) {
    if run.banked || run.level == 0 {
        return;
    }
    run.banked = true;
    let favor = favor_for(&run);
    let mut items = vec![];
    for item in loot.0.drain(..) {
        profile.stash.push(item.id().into());
        items.push(item.name());
    }
    profile.favor += favor;
    profile.runs += 1;
    profile.best_time = profile.best_time.max(run.time);
    info!("run banked: +{favor} favour, items {items:?}");
    *reward = LastReward { favor, items };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::survivors::equipment::{HERACLES_SHIELD, PELEUS, SWORD};

    #[test]
    fn dual_wielding_one_sword_needs_two_copies() {
        let mut p = Profile::new_hero(&SWORD);
        p.off = Some(SWORD.id.into());
        assert!(p.equipment().off.is_none());
        p.stash.push(SWORD.id.into());
        assert!(p.equipment().off.is_some());
    }

    #[test]
    fn loadout_round_trip() {
        let mut p = Profile::new_hero(&PELEUS);
        p.stash.push(HERACLES_SHIELD.id.into());
        let mut eq = p.equipment();
        eq.equip_off(Some(OffHand::Shield(&HERACLES_SHIELD)));
        p.set_equipment(&eq);
        let back = p.equipment();
        assert!(back.off.is_some() && back.main.id == PELEUS.id);
    }

    #[test]
    fn favor() {
        let run = Run { level: 6, time: 185.0, won: false, ..default() };
        assert_eq!(favor_for(&run), 5 + 3);
    }
}

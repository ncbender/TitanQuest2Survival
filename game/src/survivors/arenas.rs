//! Arenas generated from TQ2 pieces (mapgen.rs), each
//! with its own enemy waves and boss. Surviving an arena unlocks the next; later arenas are
//! tougher and drop better loot.
use bevy::prelude::*;

use super::bestiary::{self as b, EnemyDef, Spawn};

pub struct ArenaDef {
    pub id: &'static str,
    pub name: &'static str,
    /// Export folder below cache/export, or `gen:<kit>` for an arena built by mapgen.rs.
    pub scene: &'static str,
    pub blurb: &'static str,
    pub roster: &'static [Spawn],
    pub boss: &'static EnemyDef,
    /// Enemy health/damage multiplier.
    pub tier: f32,
    /// Chest loot multiplier.
    pub luck: f32,
}

const fn s(def: &'static EnemyDef, from: f32, weight: u32) -> Spawn {
    Spawn { def, from, weight }
}

pub const ARENAS: &[ArenaDef] = &[
    ArenaDef {
        id: "coast",
        name: "Aegean Coast",
        scene: "gen:coast",
        blurb: "A quiet beach, until the boarmen come down from the hills.",
        roster: &[s(&b::SPIDER, 0.0, 3), s(&b::BOARMAN, 0.0, 3), s(&b::CRAB, 60.0, 2), s(&b::GIANT_BOAR, 120.0, 1), s(&b::SKELETON, 180.0, 2), s(&b::HARPY, 240.0, 2)],
        boss: &b::CYCLOPS,
        tier: 1.0,
        luck: 1.0,
    },
    ArenaDef {
        id: "forest",
        name: "Arcadian Woods",
        scene: "gen:woods",
        blurb: "Wolves hunt between the trees. Something larger hunts the wolves.",
        roster: &[s(&b::WOLF, 0.0, 4), s(&b::BOARMAN, 0.0, 2), s(&b::BOARMAN_MAGE, 60.0, 2), s(&b::GIANT_BAT, 120.0, 2), s(&b::GIANT_BOAR, 180.0, 1), s(&b::LION, 240.0, 1)],
        boss: &b::NEMEAN_LION,
        tier: 1.4,
        luck: 1.3,
    },
    ArenaDef {
        id: "graveyard",
        name: "Necropolis",
        scene: "gen:necropolis",
        blurb: "The dead do not rest here. Neither will you.",
        roster: &[s(&b::SKELETON, 0.0, 4), s(&b::GIANT_BAT, 0.0, 2), s(&b::SPIDER, 60.0, 2), s(&b::LICH, 150.0, 2), s(&b::WOLF, 240.0, 1)],
        boss: &b::LICH_LORD,
        tier: 1.9,
        luck: 1.7,
    },
    ArenaDef {
        id: "village",
        name: "Burnt Village",
        scene: "gen:village",
        blurb: "Scorpions nest in the ruins, crocolisks crawl up from the river.",
        roster: &[s(&b::SCORPION, 0.0, 3), s(&b::CRAB, 0.0, 2), s(&b::HARPY, 60.0, 2), s(&b::BOARMAN_MAGE, 120.0, 2), s(&b::CROCOLISK, 200.0, 1), s(&b::LION, 300.0, 1)],
        boss: &b::ELDER_CROCOLISK,
        tier: 2.5,
        luck: 2.2,
    },
];

pub fn by_id(id: &str) -> &'static ArenaDef {
    ARENAS.iter().find(|a| a.id == id).unwrap_or(&ARENAS[0])
}

/// Arenas whose map (or, for generated arenas, every source of its kit) was exported.
pub fn available(a: &ArenaDef) -> bool {
    let root = crate::export_root();
    let has = |dir: &str| std::path::Path::new(&root).join(dir).join("scene.json").is_file();
    match a.scene.strip_prefix("gen:") {
        Some(id) => crate::mapgen::kit(id).is_some(),
        None => has(a.scene),
    }
}

/// The map to load for a run: generated arenas get a fresh seed every run.
pub fn map_for_run(a: &ArenaDef) -> String {
    if a.scene.starts_with("gen:") { format!("{}:{}", a.scene, fastrand::u64(..)) } else { a.scene.to_string() }
}

/// The arena whose map `scene` is (generated maps carry a seed after the kit).
pub fn for_map(scene: &str) -> Option<&'static ArenaDef> {
    ARENAS.iter().find(|a| scene == a.scene || scene.strip_prefix(a.scene).is_some_and(|rest| rest.starts_with(':')))
}

/// The first arena is always open; every other one opens when the one before was survived.
pub fn unlocked(a: &ArenaDef, cleared: &[String]) -> bool {
    let i = ARENAS.iter().position(|x| x.id == a.id).unwrap_or(0);
    i == 0 || cleared.iter().any(|c| c == ARENAS[i - 1].id)
}

/// The arena of the current run.
#[derive(Resource)]
pub struct CurrentArena(pub &'static ArenaDef);

impl Default for CurrentArena {
    fn default() -> Self {
        Self(&ARENAS[0])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arenas_unlock_in_order() {
        assert!(unlocked(&ARENAS[0], &[]));
        assert!(!unlocked(&ARENAS[1], &[]));
        assert!(unlocked(&ARENAS[1], &["coast".into()]));
        assert!(!unlocked(&ARENAS[2], &["coast".into()]));
    }

    #[test]
    fn every_roster_has_an_opening_enemy() {
        for a in ARENAS {
            assert!(a.roster.iter().any(|s| s.from == 0.0), "{}", a.id);
        }
    }
}

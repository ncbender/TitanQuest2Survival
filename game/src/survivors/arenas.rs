//! Arenas: regions of TQ2's world (exported with `Tq2Extract region`) or its menu scenes, each
//! with its own enemy waves and boss. Surviving an arena unlocks the next; later arenas are
//! tougher and drop better loot.
use bevy::prelude::*;

use super::bestiary::{self as b, EnemyDef, Spawn};

pub struct ArenaDef {
    pub id: &'static str,
    pub name: &'static str,
    /// Export folder below cache/export.
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
        scene: "menu2",
        blurb: "A quiet beach, until the boarmen come down from the hills.",
        roster: &[s(&b::SPIDER, 0.0, 3), s(&b::BOARMAN, 0.0, 3), s(&b::CRAB, 60.0, 2), s(&b::GIANT_BOAR, 120.0, 1), s(&b::SKELETON, 180.0, 2), s(&b::HARPY, 240.0, 2)],
        boss: &b::CYCLOPS,
        tier: 1.0,
        luck: 1.0,
    },
    ArenaDef {
        id: "forest",
        name: "Arcadian Woods",
        scene: "forest",
        blurb: "Wolves hunt between the trees. Something larger hunts the wolves.",
        roster: &[s(&b::WOLF, 0.0, 4), s(&b::BOARMAN, 0.0, 2), s(&b::BOARMAN_MAGE, 60.0, 2), s(&b::GIANT_BAT, 120.0, 2), s(&b::GIANT_BOAR, 180.0, 1), s(&b::LION, 240.0, 1)],
        boss: &b::NEMEAN_LION,
        tier: 1.4,
        luck: 1.3,
    },
    ArenaDef {
        id: "graveyard",
        name: "Necropolis",
        scene: "graveyard",
        blurb: "The dead do not rest here. Neither will you.",
        roster: &[s(&b::SKELETON, 0.0, 4), s(&b::GIANT_BAT, 0.0, 2), s(&b::SPIDER, 60.0, 2), s(&b::LICH, 150.0, 2), s(&b::WOLF, 240.0, 1)],
        boss: &b::LICH_LORD,
        tier: 1.9,
        luck: 1.7,
    },
    ArenaDef {
        id: "village",
        name: "Burnt Village",
        scene: "village",
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

/// Arenas whose map was exported (the others are listed as missing).
pub fn available(a: &ArenaDef) -> bool {
    std::path::Path::new(&crate::export_root()).join(a.scene).join("scene.json").is_file()
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

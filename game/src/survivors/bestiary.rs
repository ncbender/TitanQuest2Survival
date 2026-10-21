//! Enemy types: original TQ2 creatures (parts + clips from `tools/assets.json`) with
//! Survivors stats. Multipliers are relative to the base values in `config`. Which types
//! appear where and when is up to the arena (`arenas.rs`).

pub struct EnemyDef {
    pub name: &'static str,
    pub parts: &'static [&'static str],
    pub run: &'static str,
    /// Playback speed of the run clip (spiders only have a walk cycle).
    pub run_speed: f32,
    pub attack: &'static str,
    /// Played once on spawn while the enemy stands still (skeletons rise from the ground).
    pub spawn: Option<(&'static str, f32)>,
    pub hp: f32,
    pub speed: f32,
    pub damage: f32,
    pub radius: f32,
    pub scale: f32,
    pub xp: u32,
}

pub const SPIDER: EnemyDef = EnemyDef {
    name: "Spider",
    parts: &["spider/sk_spider_01_animated.glb"],
    run: "a_spider_01_walk_01",
    run_speed: 1.8,
    attack: "a_spider_01_attack_01",
    spawn: None,
    hp: 0.5,
    speed: 1.4,
    damage: 0.6,
    radius: 0.6,
    scale: 1.0,
    xp: 1,
};

pub const BOARMAN: EnemyDef = EnemyDef {
    name: "Boarman",
    parts: &[
        "boarman/sk_boarman_brute_01_animated.glb",
        "boarman/sk_boarman_brute_mane_01_animated.glb",
        "boarman/sk_boarman_brute_armhair_01_animated.glb",
    ],
    run: "a_boarman_club+club_run_01",
    run_speed: 1.0,
    attack: "a_boarman_club+shield_attack_01",
    spawn: None,
    hp: 1.0,
    speed: 1.0,
    damage: 1.0,
    radius: 0.7,
    scale: 1.0,
    xp: 1,
};

pub const GIANT_BOAR: EnemyDef = EnemyDef {
    name: "Giant Boar",
    parts: &[
        "giant_boar/sk_giant_boar_01_animated.glb",
        "giant_boar/sk_giant_boar_01_tusk_01_animated.glb",
        "giant_boar/sk_giant_boar_01_hair_01_animated.glb",
    ],
    run: "a_giant_boar_run_01",
    run_speed: 1.0,
    attack: "a_giant_boar_aa_regular_01",
    spawn: None,
    hp: 3.5,
    speed: 1.25,
    damage: 1.6,
    radius: 1.1,
    scale: 1.0,
    xp: 3,
};

pub const SKELETON: EnemyDef = EnemyDef {
    name: "Skeleton",
    parts: &["skeleton/sk_skeleton_01_body_01_animated.glb", "skeleton/sk_skeleton_01_head_01_animated.glb"],
    run: "a_skeleton_01_unarmed_run_01",
    run_speed: 1.0,
    attack: "a_skeleton_sword_attack_01",
    spawn: Some(("a_skeleton_01_unarmed_spawn_01", 1.6)),
    hp: 1.8,
    speed: 0.9,
    damage: 1.2,
    radius: 0.5,
    scale: 1.0,
    xp: 2,
};

pub const HARPY: EnemyDef = EnemyDef {
    name: "Harpy",
    parts: &["harpy/sk_harpy_01_animated.glb"],
    run: "a_harpy_01_run_flapping_01",
    run_speed: 1.0,
    attack: "a_harpy_01_attack_01",
    spawn: None,
    hp: 0.9,
    speed: 1.5,
    damage: 0.9,
    radius: 0.6,
    scale: 1.0,
    xp: 2,
};

/// Boss. TQ2 ships no attack clips for Cyclops_01, so it swings with its combat idle.
pub const CYCLOPS: EnemyDef = EnemyDef {
    name: "Cyclops",
    parts: &[
        "cyclops/sk_cyclops_01_animated.glb",
        "cyclops/sk_cyclops_01_hair_01_animated.glb",
        "cyclops/sk_cyclops_01_horn_01_animated.glb",
    ],
    run: "a_cyclops_01_staff2h_run_01",
    run_speed: 1.0,
    attack: "a_cyclops_01_staff2h_combat_idle_01",
    spawn: None,
    hp: 60.0,
    speed: 0.85,
    damage: 3.0,
    radius: 1.8,
    scale: 1.0,
    xp: 50,
};

pub const WOLF: EnemyDef = EnemyDef {
    name: "Wolf",
    parts: &["wolf/sk_wolf_01_animated.glb"],
    run: "a_wolf_run_01",
    run_speed: 1.0,
    attack: "a_wolf_attack_01",
    spawn: None,
    hp: 0.7,
    speed: 1.6,
    damage: 0.8,
    radius: 0.6,
    scale: 1.0,
    xp: 1,
};

pub const SCORPION: EnemyDef = EnemyDef {
    name: "Scorpion",
    parts: &["scorpion/sk_scorpion_01_animated.glb"],
    run: "a_scorpion_run_01",
    run_speed: 1.0,
    attack: "a_scorpion_aa_quick_claw_left_01",
    spawn: Some(("a_scorpion_ambush_spawn_01", 1.3)),
    hp: 1.5,
    speed: 1.0,
    damage: 1.1,
    radius: 0.8,
    scale: 1.0,
    xp: 2,
};

pub const GIANT_BAT: EnemyDef = EnemyDef {
    name: "Giant Bat",
    parts: &["giant_bat/sk_giant_bat_01_animated.glb"],
    run: "a_giant_bat_01_walk",
    run_speed: 1.4,
    attack: "a_giant_bat_01_regular_attack_01",
    spawn: None,
    hp: 0.5,
    speed: 1.7,
    damage: 0.6,
    radius: 0.5,
    scale: 1.0,
    xp: 1,
};

pub const CRAB: EnemyDef = EnemyDef {
    name: "Crab",
    parts: &["crab/sk_crab_01_01_animated.glb"],
    run: "a_crab_run_fwd_01",
    run_speed: 1.0,
    attack: "a_crab_attack_01",
    spawn: Some(("a_crab_spawn_01", 1.0)),
    hp: 1.2,
    speed: 0.9,
    damage: 0.9,
    radius: 0.7,
    scale: 1.0,
    xp: 1,
};

pub const LION: EnemyDef = EnemyDef {
    name: "Lion",
    parts: &["lion/sk_lion_01_body_animated.glb", "lion/sk_lion_01_mane_01_animated.glb"],
    run: "a_lion_run_01",
    run_speed: 1.0,
    attack: "a_lion_attack_02",
    spawn: None,
    hp: 3.0,
    speed: 1.4,
    damage: 1.6,
    radius: 1.0,
    scale: 1.0,
    xp: 3,
};

pub const CROCOLISK: EnemyDef = EnemyDef {
    name: "Crocolisk",
    parts: &["crocolisk/sk_crocolisk_01_animated.glb"],
    run: "a_crocolisk_run_01",
    run_speed: 1.0,
    attack: "a_crocolisk_attack_01",
    spawn: None,
    hp: 3.5,
    speed: 1.0,
    damage: 1.8,
    radius: 1.1,
    scale: 1.0,
    xp: 3,
};

pub const BOARMAN_MAGE: EnemyDef = EnemyDef {
    name: "Boarman Shaman",
    parts: &[
        "boarman_mage/sk_boarman_mage_01_animated.glb",
        "boarman_mage/sk_boarman_mage_beard_01_animated.glb",
        "boarman_mage/sk_boarman_mage_mane_01_animated.glb",
    ],
    run: "a_boarman_staff_2h_run_01",
    run_speed: 1.0,
    attack: "a_boarman_staff_2h_attack_01",
    spawn: None,
    hp: 1.3,
    speed: 1.0,
    damage: 1.3,
    radius: 0.7,
    scale: 1.0,
    xp: 2,
};

pub const LICH: EnemyDef = EnemyDef {
    name: "Lich",
    parts: &["lich/sk_lich_02_animated.glb"],
    run: "a_lich_02_run_01",
    run_speed: 1.0,
    attack: "a_lich_02_attack_staff_01",
    spawn: None,
    hp: 2.2,
    speed: 0.9,
    damage: 1.5,
    radius: 0.6,
    scale: 1.0,
    xp: 3,
};

/// Boss of the graveyard.
pub const LICH_LORD: EnemyDef = EnemyDef {
    name: "Lich Lord",
    parts: &["lich_lord/sk_lich_03_animated.glb"],
    run: "a_lich_03_run_01",
    run_speed: 1.0,
    attack: "a_lich_03_attack_01",
    spawn: None,
    hp: 70.0,
    speed: 0.8,
    damage: 3.2,
    radius: 1.2,
    scale: 1.6,
    xp: 60,
};

/// Boss of the forest: an oversized lion.
pub const NEMEAN_LION: EnemyDef = EnemyDef {
    name: "Nemean Lion",
    hp: 65.0,
    speed: 1.0,
    damage: 3.0,
    radius: 1.9,
    scale: 1.9,
    xp: 60,
    ..LION
};

/// Boss of the swamp: an oversized crocolisk.
pub const ELDER_CROCOLISK: EnemyDef = EnemyDef {
    name: "Elder Crocolisk",
    hp: 80.0,
    speed: 0.8,
    damage: 3.4,
    radius: 2.2,
    scale: 2.0,
    xp: 60,
    ..CROCOLISK
};

/// Every type (gallery test).
pub const ALL: &[&EnemyDef] = &[
    &SPIDER, &BOARMAN, &GIANT_BOAR, &SKELETON, &HARPY, &CYCLOPS, &WOLF, &SCORPION, &GIANT_BAT, &CRAB, &LION, &CROCOLISK, &BOARMAN_MAGE, &LICH, &LICH_LORD,
];

/// A type in an arena's waves: joins from run time `from` (seconds) with spawn `weight`.
pub struct Spawn {
    pub def: &'static EnemyDef,
    pub from: f32,
    pub weight: u32,
}

/// Picks a wave enemy type unlocked at run time `t`, weighted by `weight`.
pub fn pick(roster: &[Spawn], t: f32) -> &'static EnemyDef {
    let open: Vec<&Spawn> = roster.iter().filter(|s| s.from <= t).collect();
    let total: u32 = open.iter().map(|s| s.weight).sum();
    let mut roll = fastrand::u32(0..total.max(1));
    for s in &open {
        if roll < s.weight {
            return s.def;
        }
        roll -= s.weight;
    }
    roster.first().map_or(&BOARMAN, |s| s.def)
}

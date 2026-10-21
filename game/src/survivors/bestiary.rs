//! Enemy types: original TQ2 creatures (parts + clips from `tools/assets.json`) with
//! Survivors stats. Multipliers are relative to the base values in `config`.

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
    /// Run time (seconds) from which this type joins the waves, and its spawn weight.
    pub from: f32,
    pub weight: u32,
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
    from: 0.0,
    weight: 3,
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
    from: 0.0,
    weight: 3,
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
    from: 120.0,
    weight: 1,
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
    from: 180.0,
    weight: 2,
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
    from: 240.0,
    weight: 2,
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
    from: f32::MAX,
    weight: 0,
};

/// Types that join regular waves.
pub const WAVES: &[&EnemyDef] = &[&SPIDER, &BOARMAN, &GIANT_BOAR, &SKELETON, &HARPY];

/// Picks a wave enemy type unlocked at run time `t`, weighted by `weight`.
pub fn pick(t: f32) -> &'static EnemyDef {
    let open: Vec<&EnemyDef> = WAVES.iter().copied().filter(|d| d.from <= t).collect();
    let total: u32 = open.iter().map(|d| d.weight).sum();
    let mut roll = fastrand::u32(0..total.max(1));
    for d in &open {
        if roll < d.weight {
            return d;
        }
        roll -= d.weight;
    }
    &BOARMAN
}

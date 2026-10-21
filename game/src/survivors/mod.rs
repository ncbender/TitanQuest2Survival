//! The Survivors game loop. Simulation state lives in plain components on the ground
//! plane (`Pos`, `Vel`, `Health`…) and runs in `FixedUpdate`; visuals only follow it.
//! Keeping the simulation separate is what later lets a host run it for two players.
mod bench;
mod bestiary;
mod camera;
mod combat;
mod enemies;
mod flowfield;
mod hud;
mod menu;
mod player;
mod progression;

use bevy::prelude::*;

use crate::audio::Audio;
use crate::ground::Ground;

/// Original TQ2 assets used by the game (paths below cache/export).
pub mod content {
    pub const HERO_PARTS: &[&str] = &[
        "hero/sk_male_base_animated.glb",
        "hero/sk_male_torso_greek_adventurer_01_animated.glb",
        "hero/sk_male_leg_greek_adventurer_01_animated.glb",
        "hero/sk_human_male_hair_01_animated.glb",
    ];
    pub const HERO_IDLE: &str = "a_male_staff2h_idle_combat_01";
    pub const HERO_RUN: &str = "a_male_staff2h_run_01";
    /// Staff of Apollo (TQ2 unique staff) in the hero's right hand.
    pub const HERO_WEAPON: &str =
        "weapons/TQ2/Content/TQ2/Character/Human/Weapon/Staff2H/Unique/Staff2H_Staff_Of_Apollo_01/sm_staff2h_staff_of_apollo_01.glb";
    pub const HERO_WEAPON_BONE: &str = "HandAttach_R";

    pub const BOLT: &str = "props/TQ2/Content/TQ2/Character/Abilities/Forge_Sentry_01/sm_forge_sentry_projectile_01.glb";
    pub const SHARD: &str = "props/TQ2/Content/TQ2/Character/Crafting/In_World/Mystical_Shards/sm_mystic_shards.glb";
    /// Forge Call ability ring (TQ2) used as the Nova shockwave; outer radius in metres.
    pub const NOVA_RING: &str = "props/TQ2/Content/TQ2/Character/Abilities/Forge_Call_01/sm_forge_call_ring_outer.glb";
    pub const NOVA_RING_RADIUS: f32 = 0.16;

    /// TQ2 FMOD events (see `tq2_survivors --list-sounds`).
    pub mod sound {
        pub const MENU_MUSIC: &str = "event:/music/mus_mainmenu";
        pub const RUN_MUSIC: &str = "event:/music/mus_combat_generic_01";
        pub const BOSS_MUSIC: &str = "event:/music/mus_centaur_chapter_bossfight_centaurus";
        pub const BOLT_HIT: &str = "event:/weapons/hit/sfx_weapon_hit_staff_projectile_fire";
        pub const NOVA: &str = "event:/abilities/player/masteries/spirit/death_nova/sfx_ability_player_death_nova_commit";
        pub const KILL: &str = "event:/weapons/hit/sfx_weapon_hit_generic_medium";
        pub const SHARD: &str = "event:/loot/pickup/misc/sfx_loot_pickup_gold";
        pub const LEVEL_UP: &str = "event:/ui/menu/mastery_screen/sfx_ui_menu_mastery_screen_ability_level_up_release";
        pub const PICK: &str = "event:/ui/menu/button/sfx_ui_menu_button_gemstone_release";
        pub const SKELETON_RISE: &str = "event:/effects/creature/undead/skeleton/sfx_effect_creature_un_skel_spawn";
        pub const BOSS_ARRIVES: &str = "event:/abilities/boss/centaurus/sfx_ability_boss_centaurus_shockwave_impact";
    }

    /// Export folders whose materials.json the game loads.
    pub const MATERIAL_DIRS: &[&str] = &["hero", "boarman", "spider", "giant_boar", "skeleton", "harpy", "cyclops", "props", "weapons"];
}

/// Tunables for a run. Kept in one place so balancing does not mean hunting through systems.
pub mod config {
    pub const RUN_SECONDS: f32 = 600.0;
    pub const PLAYER_HP: f32 = 100.0;
    pub const PLAYER_SPEED: f32 = 6.0;
    pub const PICKUP_RADIUS: f32 = 3.5;

    pub const SPAWN_RING: (f32, f32) = (22.0, 30.0);
    pub const MAX_ENEMIES: usize = 400;
    pub const ENEMY_HP: f32 = 18.0;
    pub const ENEMY_SPEED: f32 = 3.4;
    pub const ENEMY_DAMAGE: f32 = 7.0;
    pub const ENEMY_RADIUS: f32 = 0.7;
    pub const ATTACK_RANGE: f32 = 1.7;
    pub const ATTACK_COOLDOWN: f32 = 1.4;
    /// Minute marks (seconds) where a boss joins the horde.
    pub const BOSS_TIMES: &[f32] = &[300.0, 540.0];
}

#[derive(States, Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GameState {
    #[default]
    MainMenu,
    Playing,
    LevelUp,
    Paused,
    GameOver,
}

/// Starts a fresh run from any state (menu, end screen, tests).
#[derive(Message)]
pub struct StartRun;

/// Position on the ground plane (world x, z). Height comes from `Ground`.
#[derive(Component, Default, Clone, Copy)]
pub struct Pos(pub Vec2);

#[derive(Component, Default, Clone, Copy)]
pub struct Vel(pub Vec2);

#[derive(Component, Clone, Copy)]
pub struct Radius(pub f32);

#[derive(Component)]
pub struct Health {
    pub hp: f32,
    pub max: f32,
}

/// Everything spawned for a run; despawned on restart.
#[derive(Component)]
pub struct RunEntity;

/// Time, kills and level of the current run.
#[derive(Resource, Default)]
pub struct Run {
    pub time: f32,
    pub kills: u32,
    pub level: u32,
    pub xp: u32,
    pub xp_next: u32,
    pub won: bool,
}

/// Where the arena is and where the hero starts.
#[derive(Resource)]
pub struct Arena {
    pub start: Vec2,
}

pub struct SurvivorsPlugin;

impl Plugin for SurvivorsPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<GameState>()
            .init_resource::<Run>()
            .add_message::<StartRun>()
            .add_plugins((camera::plugin, player::plugin, enemies::plugin, combat::plugin, progression::plugin, hud::plugin))
            .add_plugins((menu::plugin, bench::plugin, flowfield::plugin))
            .add_systems(Startup, skip_menu_in_tests)
            .add_systems(OnEnter(GameState::MainMenu), clear_run)
            .add_systems(Update, ((start_run, follow_ground, restart).chain(), music));
    }
}

/// Automated tests (screenshots, autoplay, benchmarks) start a run without the menu.
fn skip_menu_in_tests(mut start: MessageWriter<StartRun>) {
    let tests = ["TQ2_SCREENSHOT", "TQ2_AUTOPLAY", "TQ2_BENCH", "TQ2_TEST_GALLERY"];
    // TQ2_MENU=1 keeps the menu (for screenshots of it).
    if std::env::var_os("TQ2_MENU").is_none() && tests.iter().any(|v| std::env::var_os(v).is_some()) {
        start.write(StartRun);
    }
}

/// Resets every piece of run state and spawns the hero.
fn start_run(
    mut commands: Commands,
    mut requests: MessageReader<StartRun>,
    mut next: ResMut<NextState<GameState>>,
    mut run: ResMut<Run>,
    mut stats: ResMut<player::HeroStats>,
    arena: Res<Arena>,
    entities: Query<Entity, With<RunEntity>>,
) {
    if requests.read().count() == 0 {
        return;
    }
    for e in &entities {
        commands.entity(e).despawn();
    }
    *run = Run { level: 1, xp_next: progression::xp_for_level(1), ..default() };
    *stats = player::HeroStats::default();
    enemies::reset(&mut commands);
    combat::reset(&mut commands);
    let hero = player::spawn_hero(&mut commands, arena.start);
    // Test hook: TQ2_TEST_UPGRADES=multishot,multishot,nova applies upgrades at the start.
    if let Ok(list) = std::env::var("TQ2_TEST_UPGRADES") {
        let mut hp = Health { hp: config::PLAYER_HP, max: config::PLAYER_HP };
        for name in list.split(',') {
            match progression::Upgrade::from_name(name.trim()) {
                Some(u) => u.apply(&mut stats, &mut hp),
                None => warn!("unknown upgrade {name}"),
            }
        }
        commands.entity(hero).insert(hp);
    }
    next.set(GameState::Playing);
}

/// Back in the menu, the arena is empty again.
fn clear_run(mut commands: Commands, entities: Query<Entity, With<RunEntity>>) {
    for e in &entities {
        commands.entity(e).despawn();
    }
}

/// Menu music in the menus, combat music during a run, boss music while a boss lives.
fn music(audio: Res<Audio>, state: Res<State<GameState>>, enemies: Query<&enemies::Enemy>) {
    let track = match state.get() {
        GameState::MainMenu | GameState::GameOver => content::sound::MENU_MUSIC,
        _ if enemies.iter().any(|e| e.boss) => content::sound::BOSS_MUSIC,
        _ => content::sound::RUN_MUSIC,
    };
    audio.music(track, 0.6);
}

/// Places every simulated unit on the terrain and turns it towards where it moves.
fn follow_ground(
    time: Res<Time>,
    ground: Res<Ground>,
    mut units: Query<(&Pos, Option<&Vel>, &Radius, &mut Transform), With<Health>>,
) {
    for (pos, vel, radius, mut t) in &mut units {
        // Highest ground under the feet, so feet never sink into a slope.
        t.translation = Vec3::new(pos.0.x, ground.stand_area(pos.0, radius.0 * 0.6), pos.0.y);
        if let Some(v) = vel.filter(|v| v.0.length_squared() > 1e-6) {
            // Exported TQ2 characters face +Z (toe bones lie at +Z in the bind pose).
            let target = Quat::from_rotation_y(f32::atan2(v.0.x, v.0.y));
            t.rotation = t.rotation.slerp(target, (12.0 * time.delta_secs()).min(1.0));
        }
    }
}

fn restart(keys: Res<ButtonInput<KeyCode>>, state: Res<State<GameState>>, mut start: MessageWriter<StartRun>) {
    if *state.get() == GameState::GameOver && keys.just_pressed(KeyCode::KeyR) {
        start.write(StartRun);
    }
}

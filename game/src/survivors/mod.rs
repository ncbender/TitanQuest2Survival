//! The Survivors game loop. Simulation state lives in plain components on the ground
//! plane (`Pos`, `Vel`, `Health`…) and runs in `FixedUpdate`; visuals only follow it.
//! Keeping the simulation separate is what later lets a host run it for two players.
mod abilities;
mod armory;
pub mod arenas;
mod bench;
mod bestiary;
mod camera;
mod charstats;
mod combat;
mod elites;
mod enemies;
mod equipment;
mod flowfield;
mod hud;
mod inventory;
pub mod items;
pub mod mastery_data;
mod mechanics;
mod menu;
mod player;
mod profile;
mod progression;
mod settings;
mod skills;
mod skilltree;
mod ui;

use bevy::prelude::*;

use crate::audio::Audio;
use crate::ground::Ground;

/// Original TQ2 assets used by the game (paths below cache/export).
pub mod content {
    // Hero parts, weapons and armour sets live in equipment.rs.

    pub const ARROW: &str = "props/TQ2/Content/TQ2/Character/Human/Weapon/Bow2H/Projectile/sm_projectile_greek_narrow_01.glb";
    pub const SHARD: &str = "props/TQ2/Content/TQ2/Character/Crafting/In_World/Mystical_Shards/sm_mystic_shards.glb";

    /// TQ2 FMOD events (see `tq2_survivors --list-sounds`).
    pub mod sound {
        pub const MENU_MUSIC: &str = "event:/music/mus_mainmenu";
        pub const RUN_MUSIC: &str = "event:/music/mus_combat_generic_01";
        pub const BOSS_MUSIC: &str = "event:/music/mus_centaur_chapter_bossfight_centaurus";
        pub const NOVA: &str = "event:/abilities/player/masteries/spirit/death_nova/sfx_ability_player_death_nova_commit";
        pub const KILL: &str = "event:/weapons/hit/sfx_weapon_hit_generic_medium";
        pub const SHARD: &str = "event:/loot/pickup/misc/sfx_loot_pickup_gold";
        pub const LEVEL_UP: &str = "event:/ui/menu/mastery_screen/sfx_ui_menu_mastery_screen_ability_level_up_release";
        pub const PICK: &str = "event:/ui/menu/button/sfx_ui_menu_button_gemstone_release";
        pub const SKELETON_RISE: &str = "event:/effects/creature/undead/skeleton/sfx_effect_creature_un_skel_spawn";
        pub const BOSS_ARRIVES: &str = "event:/abilities/boss/centaurus/sfx_ability_boss_centaurus_shockwave_impact";
    }

    /// Export folders whose materials.json the game loads.
    pub const MATERIAL_DIRS: &[&str] = &[
        "hero", "boarman", "spider", "giant_boar", "skeleton", "harpy", "cyclops", "wolf", "scorpion", "giant_bat", "crab", "lion", "crocolisk",
        "boarman_mage", "lich", "lich_lord", "props", "weapons", "armor",
    ];
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
    /// Choosing gear from an opened chest.
    Loot,
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
    /// Loot and favour of this run were credited to the profile.
    pub banked: bool,
    /// Ichor from bosses killed in this run, and how many bosses fell.
    pub ichor: u32,
    pub bosses: u32,
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
            .init_resource::<arenas::CurrentArena>()
            .add_message::<StartRun>()
            .add_plugins((camera::plugin, player::plugin, enemies::plugin, combat::plugin, progression::plugin, hud::plugin))
            .add_plugins((menu::plugin, bench::plugin, flowfield::plugin, equipment::plugin, profile::plugin))
            .add_plugins((mastery_data::plugin, abilities::plugin, skilltree::plugin, mechanics::plugin, inventory::plugin, elites::plugin))
            .insert_resource(items::AffixBook::load(&crate::export_root()))
            .add_systems(Update, (ui::scale_to_window, ui::show_scrollbars))
            .insert_resource(crate::character::ClipAllowlist(bestiary::used_clips()))
            .add_plugins(settings::plugin)
            .add_systems(Startup, skip_menu_in_tests)
            .add_systems(OnEnter(GameState::MainMenu), clear_run)
            .add_systems(Update, ((crate::map::sync, start_run, follow_ground, restart).chain(), music));
    }
}

/// Screenshots, autoplay or benchmarks are running (they use a separate test profile).
pub fn is_test_run() -> bool {
    ["TQ2_SCREENSHOT", "TQ2_AUTOPLAY", "TQ2_BENCH", "TQ2_TEST_GALLERY"].iter().any(|v| std::env::var_os(v).is_some())
}

/// Automated tests start a run without the menu (TQ2_MENU keeps the menu for screenshots).
fn skip_menu_in_tests(mut start: MessageWriter<StartRun>) {
    if std::env::var_os("TQ2_MENU").is_none() && is_test_run() {
        start.write(StartRun);
    }
}

/// Resets every piece of run state and spawns the hero.
fn start_run(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut requests: MessageReader<StartRun>,
    mut next: ResMut<NextState<GameState>>,
    mut run: ResMut<Run>,
    mut stats: ResMut<player::HeroStats>,
    arena: Res<Arena>,
    mut profile: ResMut<profile::Profile>,
    mut loot: ResMut<equipment::RunLoot>,
    book: Res<mastery_data::MasteryBook>,
    affixes: Res<items::AffixBook>,
    entities: Query<Entity, With<RunEntity>>,
    map: Res<crate::map::LoadedMap>,
    selected: Res<crate::map::SelectedMap>,
) {
    // A newly selected arena is loaded first (map::sync); the request waits for it.
    if !map.ready(&selected) || requests.read().count() == 0 {
        return;
    }
    for e in &entities {
        commands.entity(e).despawn();
    }
    *run = Run { level: 1, xp_next: progression::xp_for_level(1), ..default() };
    // The loaded map decides the arena (`--scene` can load any arena's map directly).
    let arena_def = arenas::for_map(&selected.0).unwrap_or_else(|| arenas::by_id(&profile.arena));
    info!("arena: {}", arena_def.name);
    commands.insert_resource(arenas::CurrentArena(arena_def));
    *stats = player::HeroStats::default();
    loot.0.clear();
    commands.insert_resource(abilities::Buffs::default());
    enemies::reset(&mut commands);
    combat::reset(&mut commands);
    if !profile.created {
        // Tests (and a first start without the menu) get a staff hero.
        profile.new_hero(&equipment::STAFF);
    }
    // Same numbers as the character sheet: base values, passives, gear.
    let (start_stats, mut hp, mut gear) = charstats::starting(&profile, &book, &affixes);
    *stats = start_stats;
    let (loadout, pool) = run_loadout(&profile, &book);
    commands.insert_resource(loadout);
    commands.insert_resource(pool);
    // Test hook: TQ2_TEST_CHEST=1 drops a chest right next to the hero.
    if std::env::var_os("TQ2_TEST_CHEST").is_some() {
        let luck = std::env::var("TQ2_TEST_CHEST").ok().and_then(|v| v.parse().ok()).unwrap_or(1.0);
        equipment::drop_chest(&mut commands, &assets, arena.start + Vec2::X, luck);
    }
    // Test hook: TQ2_TEST_GEAR=sword,shield,ranger (item ids or short names) - the first
    // weapon goes to the main hand, the next one-handed item to the off hand.
    if let Ok(list) = std::env::var("TQ2_TEST_GEAR") {
        let mut main_set = false;
        for name in list.split(',').map(str::trim) {
            match equipment::Item::from_id(name).or_else(|| test_gear_alias(name)) {
                Some(equipment::Item::Weapon(w)) if !main_set => {
                    gear.equip_main(w);
                    gear.off = None;
                    main_set = true;
                }
                Some(equipment::Item::Armor(a)) => gear.wear(a, 1.0),
                Some(item) => {
                    if !equipment::OffHand::from_item(item).is_some_and(|o| gear.equip_off(Some(o))) {
                        warn!("{} does not fit the off hand", item.name());
                    }
                }
                None => warn!("unknown gear {name}"),
            }
        }
    }
    // Test hook: TQ2_TEST_UPGRADES=multishot,multishot,nova applies upgrades at the start.
    if let Ok(list) = std::env::var("TQ2_TEST_UPGRADES") {
        for name in list.split(',') {
            match progression::Upgrade::from_name(name.trim()) {
                Some(u) => u.apply(&mut stats, &mut hp),
                None => warn!("unknown upgrade {name}"),
            }
        }
    }
    mechanics::reset(&mut commands, &stats, &hp);
    let hero = player::spawn_hero(&mut commands, arena.start, gear);
    commands.entity(hero).insert(hp);
    // Test hook: TQ2_TEST_LEVELUP=1 opens the level-up window right away (screenshots).
    if std::env::var_os("TQ2_TEST_LEVELUP").is_some() {
        run.level = 2;
        next.set(GameState::LevelUp);
    } else {
        next.set(GameState::Playing);
    }
}

/// The run's skills: none at the start, the learned ones are offered on level-ups (the pool).
/// TQ2_TEST_SKILLS=Ice Shards,Fissure,… (TQ2 skill names, for tests) starts with those instead.
fn run_loadout(profile: &profile::Profile, book: &mastery_data::MasteryBook) -> (abilities::Loadout, progression::SkillPool) {
    let mut slots = vec![];
    let test: Option<Vec<String>> = std::env::var("TQ2_TEST_SKILLS").ok().map(|s| s.split(',').map(|n| n.trim().to_ascii_lowercase()).collect());
    let wanted: Vec<(String, u32)> = match &test {
        Some(names) => book
            .0
            .iter()
            .flat_map(|m| m.tiers.iter().flat_map(|t| t.actives.iter()))
            .filter(|a| !a.wip && names.contains(&a.name.to_ascii_lowercase()))
            .map(|a| (a.id.clone(), 1))
            .collect(),
        None => vec![],
    };
    let pool = profile
        .actives
        .iter()
        .filter(|(id, lvl)| **lvl > 0 && abilities::spec(id).is_some())
        .filter_map(|(id, lvl)| book.active(id).map(|(_, _, info)| progression::SkillOffer { id: id.clone(), name: info.name.clone(), level: *lvl }))
        .collect();
    for (id, level) in wanted {
        match (abilities::spec(&id), book.active(&id)) {
            (Some(spec), Some((_, _, info))) => slots.push(abilities::Slot::new(spec, info.name.clone(), level)),
            _ => warn!("skill {id} is not playable"),
        }
    }
    info!("run skills: {:?}", slots.iter().map(|s| format!("{} {}", s.name, s.level)).collect::<Vec<_>>());
    (abilities::Loadout(slots), progression::SkillPool(pool))
}

/// Short names for TQ2_TEST_GEAR.
fn test_gear_alias(name: &str) -> Option<equipment::Item> {
    use equipment::*;
    Some(match name {
        "staff" => Item::Weapon(&STAFF),
        "bow" => Item::Weapon(&BOW),
        "axe" => Item::Weapon(&AXE),
        "sword" => Item::Weapon(&SWORD),
        "peleus" => Item::Weapon(&PELEUS),
        "shield" => Item::Shield(&AENEAS_SHIELD),
        "talisman" => Item::Talisman(&ARCANE_MIRROR),
        _ => return None,
    })
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

/// Turns a unit towards `dir` for `time` seconds (attacking), regardless of where it walks.
#[derive(Component)]
pub struct Facing {
    pub dir: Vec2,
    pub time: f32,
}

/// Places every simulated unit on the terrain and turns it towards where it moves (or where
/// it attacks).
fn follow_ground(
    time: Res<Time>,
    ground: Res<Ground>,
    mut units: Query<(&Pos, Option<&Vel>, &Radius, &mut Transform, Option<&mut Facing>), With<Health>>,
) {
    let dt = time.delta_secs();
    for (pos, vel, radius, mut t, facing) in &mut units {
        // Highest ground under the feet, so feet never sink into a slope.
        t.translation = Vec3::new(pos.0.x, ground.stand_area(pos.0, radius.0 * 0.6), pos.0.y);
        let attack = facing.and_then(|mut f| {
            f.time -= dt;
            (f.time > 0.0).then_some(f.dir)
        });
        let moving = vel.map(|v| v.0).filter(|v| v.length_squared() > 0.1);
        // Running units keep facing where they run; an attack turns them at most 50° towards
        // the target (the legs cannot strafe, and turning further makes the body flip back
        // and forth with every swing).
        let dir = match (attack, moving) {
            (Some(aim), Some(run)) => {
                let angle = run.angle_to(aim).clamp(-0.87, 0.87);
                Some(Vec2::from_angle(angle).rotate(run))
            }
            (Some(aim), None) => Some(aim),
            (None, run) => run.or(vel.map(|v| v.0)),
        };
        if let Some(dir) = dir.filter(|d| d.length_squared() > 1e-6) {
            // Exported TQ2 characters face +Z (toe bones lie at +Z in the bind pose).
            let target = Quat::from_rotation_y(f32::atan2(dir.x, dir.y));
            t.rotation = t.rotation.slerp(target, (14.0 * dt).min(1.0));
        }
    }
}

fn restart(
    keys: Res<ButtonInput<KeyCode>>,
    state: Res<State<GameState>>,
    arena: Res<arenas::CurrentArena>,
    mut choice: ResMut<hud::EndChoice>,
    mut map: ResMut<crate::map::SelectedMap>,
    mut next: ResMut<NextState<GameState>>,
    mut start: MessageWriter<StartRun>,
) {
    let clicked = choice.0.take();
    if *state.get() == GameState::GameOver && clicked == Some(false) {
        next.set(GameState::MainMenu);
        return;
    }
    if *state.get() == GameState::GameOver && (keys.just_pressed(KeyCode::KeyR) || clicked == Some(true)) {
        // A generated arena is rebuilt with a new seed.
        if arena.0.scene.starts_with("gen:") {
            map.0 = arenas::map_for_run(arena.0);
        }
        start.write(StartRun);
    }
}

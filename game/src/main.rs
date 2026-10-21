//! Titan Quest II Survivors.
//!
//! usage: tq2_survivors [--scene <export dir>]   (default: menu2)
//!        tq2_survivors --list-sounds            print TQ2's FMOD event paths
//! keys: WASD move, Q/E rotate camera, mouse wheel / R-F zoom, 1-3 pick upgrade, R restart
//! env TQ2_SCREENSHOT=<path>: save a screenshot after TQ2_SCREENSHOT_DELAY s (default 4) and exit
//! env TQ2_AUTOPLAY=1: the hero runs in circles by itself and picks upgrades (tests, benchmarks)
//! env TQ2_AUTOPICK=1: level-ups pick the first offer (hero stays under manual/no control)
//! env TQ2_TEST_UPGRADES=multishot,nova,…: apply upgrades at the start of a run
//! env TQ2_TEST_GALLERY=1: spawn one of every enemy type next to the hero
//! env TQ2_BENCH=<n>: spawn n enemies, measure 10 s of frame times into cache/bench.txt, exit
mod audio;
mod character;
mod fonts;
mod ground;
mod obstacles;
mod survivors;
mod tq2_materials;
mod tq2_scene;

use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};

use ground::Ground;
use survivors::{Arena, SurvivorsPlugin};

const EXPORT_ROOT: &str = "../cache/export";

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--list-sounds") {
        // Development aid: print every FMOD event path in TQ2's banks.
        for path in audio::Audio::load().event_paths() {
            println!("{path}");
        }
        return;
    }
    let scene = args.iter().position(|a| a == "--scene").and_then(|i| args.get(i + 1)).cloned().unwrap_or_else(|| "menu2".into());

    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(AssetPlugin {
        // Extracted assets live in the git-ignored cache/ folder.
        file_path: EXPORT_ROOT.into(),
        ..default()
    }))
    .add_plugins((tq2_materials::Tq2MaterialsPlugin, character::CharacterPlugin, audio::AudioPlugin))
    .add_systems(Update, auto_screenshot);

    fonts::install(&mut app, EXPORT_ROOT);
    load_world(&mut app, &scene);
    app.add_plugins(SurvivorsPlugin).run();
}

/// Spawns the map, loads its ground and picks a start point on land near its centre.
fn load_world(app: &mut App, scene: &str) {
    let world = app.world_mut();
    let assets = world.resource::<AssetServer>().clone();
    let mut library = world.resource_mut::<tq2_materials::MaterialLibrary>();
    for dir in survivors::content::MATERIAL_DIRS.iter().copied().chain([scene]) {
        library.load_dir(EXPORT_ROOT, dir);
    }
    let mut commands = world.commands();
    let mut info = tq2_scene::spawn_scene(&mut commands, &assets, EXPORT_ROOT, scene);
    world.flush();

    let water = info.as_ref().and_then(|i| i.water);
    let obstacles = info.as_mut().map(|i| std::mem::take(&mut i.obstacles)).unwrap_or_default();
    let ground = Ground::load(&format!("{EXPORT_ROOT}/{scene}/ground.json"), water).unwrap_or_else(Ground::flat);
    let center = info.map(|i| Vec2::new(i.center.x, i.center.z)).unwrap_or_default();
    // Spiral outwards from the map centre until we stand on land.
    let start = (0..400)
        .map(|i| center + Vec2::from_angle(i as f32 * 0.7) * (i as f32 * 0.25))
        .find(|p| ground.is_land(*p) && obstacles.resolve(*p, 0.6, ground.stand(*p)) == *p)
        .unwrap_or(center);
    info!("start at {start}");
    world.insert_resource(ground);
    world.insert_resource(obstacles);
    world.insert_resource(Arena { start });
}

fn auto_screenshot(mut commands: Commands, time: Res<Time>, mut done: Local<bool>, mut exit: MessageWriter<AppExit>) {
    let Ok(path) = std::env::var("TQ2_SCREENSHOT") else { return };
    let delay: f32 = std::env::var("TQ2_SCREENSHOT_DELAY").ok().and_then(|s| s.parse().ok()).unwrap_or(4.0);
    let t = time.elapsed_secs();
    if t > delay && !*done {
        commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path));
        *done = true;
    }
    if t > delay + 2.0 {
        exit.write(AppExit::Success);
    }
}

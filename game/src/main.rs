//! Titan Quest II Survivors.
//!
//! usage: tq2_survivors [--scene <export dir>]   (load that map and its arena directly; default: menu2)
//!        tq2_survivors --list-sounds            print TQ2's FMOD event paths
//! keys: WASD move, Q/E rotate camera, mouse wheel / R-F zoom, 1-3 pick upgrade, R restart
//! env TQ2_SCREENSHOT=<path>: save a screenshot after TQ2_SCREENSHOT_DELAY s (default 4) and exit
//! env TQ2_AUTOPLAY=1: the hero runs in circles by itself and picks upgrades (tests, benchmarks)
//! env TQ2_AUTOPICK=1: level-ups pick the first offer (hero stays under manual/no control)
//! env TQ2_TEST_UPGRADES=multishot,nova,…: apply upgrades at the start of a run
//! env TQ2_TEST_GEAR=sword,shield,ranger: start with this gear
//! env TQ2_TEST_GALLERY=1: spawn one of every enemy type next to the hero
//! env TQ2_TEST_END=<s>: end the run after s seconds (end screen, loot banking)
//! env TQ2_MENU=1|hero|armory|skills|choose|characters|arenas|settings (+ TQ2_MENU_SELECT=tier,index[,p]): open a menu page
//! env TQ2_TEST_SKILLS=Ice Shards,Fissure,…: run with these TQ2 skills instead of the profile's
//! env TQ2_TEST_CHEST=<luck> / TQ2_TEST_LEVELUP=1: chest next to the hero / open the level-up window
//! env TQ2_ZOOM=<m> / TQ2_PITCH=<rad>: camera distance and angle (close-ups)
//! env TQ2_PROFILE=<path>: profile file (default save/profile.json; test runs use save/test_profile.json)
//! env TQ2_BENCH=<n>: spawn n enemies, measure 10 s of frame times into cache/bench.txt, exit
mod audio;
mod character;
mod fonts;
mod fx;
mod ground;
mod map;
mod mapgen;
mod obstacles;
mod survivors;
mod terrain;
mod tq2_materials;
mod tq2_scene;
mod vfx;

use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};

use survivors::SurvivorsPlugin;

/// Where `Tq2Extract` put the exported assets: `TQ2_EXPORT_ROOT`, or the first `cache/export`
/// found walking up from the executable or the working directory (works for `cargo run`
/// and for starting the .exe directly).
fn export_root() -> String {
    if let Ok(root) = std::env::var("TQ2_EXPORT_ROOT") {
        return root;
    }
    let starts = [std::env::current_exe().ok(), std::env::current_dir().ok()];
    starts
        .into_iter()
        .flatten()
        .flat_map(|p| p.ancestors().map(std::path::Path::to_path_buf).collect::<Vec<_>>())
        .map(|dir| dir.join("cache").join("export"))
        .find(|dir| dir.is_dir())
        .map(|dir| dir.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|| "../cache/export".into())
}

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

    let root = export_root();
    info!("assets from {root}");
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(AssetPlugin {
        // Extracted assets live in the git-ignored cache/ folder.
        file_path: root.clone(),
        ..default()
    }))
    .add_plugins((tq2_materials::Tq2MaterialsPlugin, character::CharacterPlugin, audio::AudioPlugin, vfx::VfxPlugin, fx::FxPlugin))
    .add_systems(Update, auto_screenshot);

    app.add_plugins(terrain::TerrainPlugin);
    fonts::install(&mut app, &root);
    load_shared_materials(&mut app, &root);
    app.add_plugins(SurvivorsPlugin);
    // `--scene` overrides the arena's map (development, screenshots).
    if args.iter().any(|a| a == "--scene") {
        app.insert_resource(map::SelectedMap(scene.clone()));
    }
    map::load(app.world_mut(), &scene);
    app.run();
}

/// Materials of characters, weapons and props (maps add their own when loaded).
fn load_shared_materials(app: &mut App, root: &str) {
    let mut library = app.world_mut().resource_mut::<tq2_materials::MaterialLibrary>();
    for dir in survivors::content::MATERIAL_DIRS {
        library.load_dir(root, dir);
    }
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

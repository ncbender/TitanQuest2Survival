//! Benchmark mode (env TQ2_BENCH=<enemies>): spawns that many animated Boarmen around the
//! hero, keeps the hero alive, measures frame times for 10 s and appends the result to
//! cache/bench.txt, then exits. Gallery mode (env TQ2_TEST_GALLERY=1) spawns one of every
//! enemy type next to the hero. Nothing here runs in normal play.
use bevy::prelude::*;
use std::io::Write;

use super::bestiary;
use super::enemies::{spawn_boarman_at, spawn_enemy, Enemy};
use super::player::Hero;
use super::{config, Health, Pos};

const WARMUP: f32 = 12.0;
const MEASURE: f32 = 10.0;

#[derive(Resource)]
struct Bench {
    enemies: usize,
    spawned: bool,
    frames: Vec<f32>,
}

pub fn plugin(app: &mut App) {
    if std::env::var_os("TQ2_TEST_GALLERY").is_some() {
        app.add_systems(Update, gallery);
    }
    let Some(n) = std::env::var("TQ2_BENCH").ok().and_then(|v| v.parse().ok()) else { return };
    app.insert_resource(Bench { enemies: n, spawned: false, frames: vec![] }).add_systems(Update, run_bench);
}

fn run_bench(
    mut commands: Commands,
    time: Res<Time<Real>>,
    mut bench: ResMut<Bench>,
    mut hero: Query<(&Pos, &mut Health), With<Hero>>,
    enemies: Query<(), With<Enemy>>,
    mut exit: MessageWriter<AppExit>,
) {
    let t = time.elapsed_secs();
    let Ok((hero_pos, mut hp)) = hero.single_mut() else { return };
    hp.hp = hp.max;
    if !bench.spawned && t > 2.0 {
        bench.spawned = true;
        for i in 0..bench.enemies {
            let angle = i as f32 * 2.399; // golden angle: even spread
            // TQ2_BENCH_SPREAD=<m>: spread them wider (like a real horde, partly off screen).
            let spread: f32 = std::env::var("TQ2_BENCH_SPREAD").ok().and_then(|v| v.parse().ok()).unwrap_or(14.0);
            let dist = 8.0 + spread * (i as f32 / bench.enemies.max(1) as f32).sqrt();
            spawn_boarman_at(&mut commands, hero_pos.0 + Vec2::from_angle(angle) * dist, config::ENEMY_HP * 1000.0);
        }
    }
    if t > WARMUP && t < WARMUP + MEASURE {
        bench.frames.push(time.delta_secs());
    } else if t >= WARMUP + MEASURE {
        let mut f = bench.frames.clone();
        f.sort_by(f32::total_cmp);
        let avg = f.iter().sum::<f32>() / f.len().max(1) as f32;
        let p95 = f.get(f.len() * 95 / 100).copied().unwrap_or(0.0);
        let line = format!(
            "enemies={} alive={} frames={} avg_fps={:.1} avg_ms={:.2} p95_ms={:.2} build={}",
            bench.enemies,
            enemies.iter().count(),
            f.len(),
            1.0 / avg.max(1e-6),
            avg * 1000.0,
            p95 * 1000.0,
            if cfg!(debug_assertions) { "dev" } else { "release" }
        );
        info!("BENCH {line}");
        if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open("../cache/bench.txt") {
            let _ = writeln!(file, "{line}");
        }
        exit.write(AppExit::Success);
    }
}

fn gallery(mut commands: Commands, time: Res<Time<Real>>, mut done: Local<bool>, hero: Query<&Pos, With<Hero>>) {
    let Ok(hero) = hero.single() else { return };
    if *done || time.elapsed_secs() < 1.0 {
        return;
    }
    *done = true;
    let defs = bestiary::ALL.iter().copied();
    for (i, def) in defs.enumerate() {
        let at = hero.0 + Vec2::from_angle(i as f32 * 1.05) * 9.0;
        spawn_enemy(&mut commands, def, at, 1.0, false, false);
    }
}

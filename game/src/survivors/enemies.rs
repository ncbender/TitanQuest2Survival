//! Enemy waves: spawning on a ring around the hero, chasing with separation, melee attacks.
use bevy::prelude::*;
use std::collections::HashMap;

use super::bestiary::{self, EnemyDef};
use super::player::Hero;
use super::{config, GameState, Health, Pos, Radius, Run, RunEntity, Vel};
use crate::character::{attach_parts, CharacterAnim};
use crate::ground::Ground;
use crate::obstacles::Obstacles;

#[derive(Component)]
pub struct Enemy {
    pub def: &'static EnemyDef,
    pub damage: f32,
    pub speed: f32,
    pub xp: u32,
    pub attack_timer: f32,
    /// Seconds left in the spawn animation; the enemy does not move meanwhile.
    pub spawning: f32,
    pub boss: bool,
}

/// Uniform grid of enemy positions, rebuilt every tick; used for separation and hits.
#[derive(Resource, Default)]
pub struct EnemyGrid {
    cells: HashMap<(i32, i32), Vec<(Entity, Vec2, f32)>>,
}

const CELL: f32 = 2.0;

impl EnemyGrid {
    fn key(p: Vec2) -> (i32, i32) {
        ((p.x / CELL).floor() as i32, (p.y / CELL).floor() as i32)
    }

    /// Enemies whose centre lies within `r` (+ their radius) of `p`.
    pub fn near(&self, p: Vec2, r: f32) -> impl Iterator<Item = (Entity, Vec2, f32)> + '_ {
        let reach = ((r + 2.0) / CELL).ceil() as i32;
        let (cx, cz) = Self::key(p);
        (-reach..=reach)
            .flat_map(move |dx| (-reach..=reach).map(move |dz| (cx + dx, cz + dz)))
            .filter_map(|k| self.cells.get(&k))
            .flatten()
            .copied()
            .filter(move |(_, q, rad)| q.distance_squared(p) <= (r + rad) * (r + rad))
    }
}

/// Enemy simulation; combat runs after it so hits use this tick's grid.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct EnemySet;

#[derive(Resource)]
struct Director {
    spawn_timer: f32,
    bosses_spawned: usize,
}

pub fn plugin(app: &mut App) {
    app.init_resource::<EnemyGrid>()
        .insert_resource(Director { spawn_timer: 0.0, bosses_spawned: 0 })
        .add_systems(
            FixedUpdate,
            (rebuild_grid, spawn_waves, chase_and_attack).chain().in_set(EnemySet).run_if(in_state(GameState::Playing)),
        );
}

fn rebuild_grid(mut grid: ResMut<EnemyGrid>, enemies: Query<(Entity, &Pos, &Radius), With<Enemy>>) {
    grid.cells.clear();
    for (e, pos, r) in &enemies {
        grid.cells.entry(EnemyGrid::key(pos.0)).or_default().push((e, pos.0, r.0));
    }
}

fn spawn_waves(
    mut commands: Commands,
    time: Res<Time>,
    run: Res<Run>,
    ground: Res<Ground>,
    obstacles: Res<Obstacles>,
    mut director: ResMut<Director>,
    hero: Query<&Pos, With<Hero>>,
    enemies: Query<(), With<Enemy>>,
) {
    let Ok(hero) = hero.single() else { return };
    if std::env::var_os("TQ2_BENCH").is_some() {
        return;
    }
    let t = run.time;
    // Pressure ramps up over the run: faster spawning, more per wave, tougher enemies.
    let interval = (1.6 - t / 200.0).max(0.25);
    let per_wave = 1 + (t / 45.0) as usize;
    let cap = ((30.0 + t * 0.6) as usize).min(config::MAX_ENEMIES);
    let scale = 1.0 + t / 100.0;

    if director.bosses_spawned < config::BOSS_TIMES.len() && t >= config::BOSS_TIMES[director.bosses_spawned] {
        director.bosses_spawned += 1;
        if let Some(p) = ring_point(&ground, &obstacles, hero.0, bestiary::CYCLOPS.radius) {
            spawn_enemy(&mut commands, &bestiary::CYCLOPS, p, scale, false);
        }
    }

    director.spawn_timer -= time.delta_secs();
    if director.spawn_timer > 0.0 {
        return;
    }
    director.spawn_timer = interval;
    let alive = enemies.iter().count();
    for i in 0..per_wave.min(cap.saturating_sub(alive)) {
        let def = bestiary::pick(t);
        let Some(p) = ring_point(&ground, &obstacles, hero.0, def.radius) else { continue };
        // Every 15th spawn after the first minute is an elite.
        let elite = t > 60.0 && (run.kills as usize + alive + i) % 15 == 0;
        spawn_enemy(&mut commands, def, p, scale, elite);
    }
}

/// A random point on the spawn ring that is on land and not inside an obstacle.
fn ring_point(ground: &Ground, obstacles: &Obstacles, center: Vec2, radius: f32) -> Option<Vec2> {
    let b = ground.bounds();
    (0..12).find_map(|_| {
        let angle = fastrand::f32() * std::f32::consts::TAU;
        let dist = config::SPAWN_RING.0 + fastrand::f32() * (config::SPAWN_RING.1 - config::SPAWN_RING.0);
        let p = center + Vec2::from_angle(angle) * dist;
        (b.contains(p) && ground.is_land(p) && obstacles.resolve(p, radius, ground.stand(p)) == p).then_some(p)
    })
}

/// A regular Boarman with the given HP (used by the benchmark).
pub fn spawn_boarman_at(commands: &mut Commands, at: Vec2, hp: f32) {
    let e = spawn_enemy(commands, &bestiary::BOARMAN, at, 1.0, false);
    commands.entity(e).insert(Health { hp, max: hp });
}

/// Spawns `def` at `at`; `scale` is the run's difficulty multiplier.
pub fn spawn_enemy(commands: &mut Commands, def: &'static EnemyDef, at: Vec2, scale: f32, elite: bool) -> Entity {
    let boss = def.weight == 0;
    let (hp_mult, size, xp_mult) = if elite { (6.0, 1.4, 5) } else { (1.0, 1.0, 1) };
    let hp = config::ENEMY_HP * def.hp * scale * hp_mult;
    let (anim, spawning) = match def.spawn {
        Some((clip, secs)) => (CharacterAnim::once(clip), secs),
        None => (CharacterAnim { speed: def.run_speed, ..CharacterAnim::looping(def.run) }, 0.0),
    };
    let e = commands
        .spawn((
            Enemy {
                def,
                damage: config::ENEMY_DAMAGE * def.damage * scale.sqrt(),
                speed: config::ENEMY_SPEED * def.speed * (0.9 + fastrand::f32() * 0.2),
                xp: def.xp * xp_mult,
                attack_timer: 0.0,
                spawning,
                boss,
            },
            RunEntity,
            Pos(at),
            Vel::default(),
            Radius(def.radius * size),
            Health { hp, max: hp },
            anim,
            Transform::from_scale(Vec3::splat(def.scale * size)),
            Visibility::default(),
        ))
        .id();
    attach_parts(commands, e, def.parts);
    e
}

fn chase_and_attack(
    time: Res<Time>,
    grid: Res<EnemyGrid>,
    ground: Res<Ground>,
    obstacles: Res<Obstacles>,
    mut hero: Query<(&Pos, &mut Health), (With<Hero>, Without<Enemy>)>,
    mut enemies: Query<(Entity, &mut Pos, &mut Vel, &Radius, &mut Enemy, &mut CharacterAnim), Without<Hero>>,
) {
    let Ok((hero_pos, mut hero_hp)) = hero.single_mut() else { return };
    let dt = time.delta_secs();
    for (e, mut pos, mut vel, radius, mut enemy, mut anim) in &mut enemies {
        let def = enemy.def;
        if enemy.spawning > 0.0 {
            enemy.spawning -= dt;
            vel.0 = Vec2::ZERO;
            continue;
        }
        enemy.attack_timer -= dt;
        let to_hero = hero_pos.0 - pos.0;
        let dist = to_hero.length();
        let reach = config::ATTACK_RANGE + radius.0 - config::ENEMY_RADIUS;

        // Push away from neighbours so the horde spreads instead of stacking.
        let mut push = Vec2::ZERO;
        for (other, p, r) in grid.near(pos.0, radius.0) {
            if other == e {
                continue;
            }
            let d = pos.0 - p;
            let overlap = radius.0 + r - d.length();
            if overlap > 0.0 {
                push += d.normalize_or(Vec2::X) * overlap;
            }
        }

        if dist <= reach {
            vel.0 = to_hero.normalize_or_zero() * 0.01; // keep facing the hero
            if enemy.attack_timer <= 0.0 {
                enemy.attack_timer = config::ATTACK_COOLDOWN;
                hero_hp.hp -= enemy.damage;
                *anim = CharacterAnim::once(def.attack);
            }
        } else {
            vel.0 = to_hero / dist.max(0.001) * enemy.speed;
            if enemy.attack_timer < config::ATTACK_COOLDOWN - 1.0 && anim.clip != def.run {
                *anim = CharacterAnim { speed: def.run_speed, ..CharacterAnim::looping(def.run) };
            }
        }
        let next = ground.clamp_move(pos.0, pos.0 + (vel.0 + push * 4.0) * dt);
        pos.0 = obstacles.resolve(next, radius.0, ground.stand(next));
    }
}

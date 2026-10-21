//! Weapon auto-attacks, damage, deaths and XP drops. Skills live in abilities.rs.
use bevy::prelude::*;

use super::abilities::{Ailments, Buffs};
use super::content::{ARROW, SHARD};
use crate::vfx::Flipbook;
use super::equipment::{drop_chest, AttackKind, Equipment};
use super::enemies::{Enemy, EnemyGrid};
use super::player::{Hero, HeroStats};
use super::progression::XpShard;
use super::{GameState, Health, Pos, Run, RunEntity};
use super::content::sound;
use crate::audio::Audio;
use crate::character::UpperBodyAction;
use crate::ground::Ground;
use crate::obstacles::Obstacles;

#[derive(Component)]
pub struct Projectile {
    vel: Vec2,
    damage: f32,
    crit: bool,
    hit_sound: &'static str,
    /// Fireballs explode where they hit.
    explodes: bool,
    pierce: u32,
    life: f32,
    hit: Vec<Entity>,
}

/// Enemy in its death fade (simulation already removed it).
#[derive(Component)]
/// A dead enemy thrown like a ragdoll: its pose freezes, it flies away from the hero, tips
/// over, bounces once or twice and sinks into the ground.
struct Dying {
    age: f32,
    vel: Vec3,
    /// Axis it tips over around (horizontal, across its flight).
    axis: Vec3,
    /// Tip-over angle so far and its target (lying on the side).
    tilt: f32,
    spin: f32,
    landed: bool,
}

/// Seconds until a body starts sinking, and until it is gone.
const LIE: f32 = 1.4;
const GONE: f32 = 2.3;
const GRAVITY: f32 = 22.0;

/// A hit to show as a floating damage number.
#[derive(Message)]
pub struct Hit {
    pub at: Vec2,
    pub amount: f32,
    pub crit: bool,
}

#[derive(Resource, Default)]
struct Weapons {
    /// Cooldown of the equipped weapon's auto-attack.
    attack_timer: f32,
    /// Which of the stance's attack clips plays next.
    attack_index: usize,
}

/// Angle between neighbouring projectiles of a multishot (radians, ~11°).
const BOLT_SPREAD: f32 = 0.2;
/// Sword sweep arc (radians) and the extra width per additional projectile upgrade.
const SWEEP_ARC: f32 = 2.4;
const SWEEP_ARC_PER_EXTRA: f32 = 0.35;
pub fn plugin(app: &mut App) {
    app.init_resource::<Weapons>()
        .add_message::<Hit>()
        .add_systems(
            FixedUpdate,
            (auto_attack, move_projectiles, kill_dead, check_game_over)
                .chain()
                .after(super::enemies::EnemySet)
                .run_if(in_state(GameState::Playing)),
        )
        .add_systems(Update, (place_projectiles, fade_dying));
}

/// Fresh weapon cooldowns for a new run.
pub fn reset(commands: &mut Commands) {
    commands.insert_resource(Weapons::default());
}

/// Damage of one hit (base already includes damage bonuses) and whether it crit.
pub fn roll_damage(base: f32, stats: &HeroStats) -> (f32, bool) {
    let crit = fastrand::f32() < stats.crit;
    (base * (1.0 + stats.rage_bonus) * if crit { stats.crit_mult } else { 1.0 }, crit)
}

/// The equipped weapon's auto-attack: projectiles (staff, bow) or a melee sweep (sword).
fn auto_attack(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<AssetServer>,
    audio: Res<Audio>,
    ground: Res<Ground>,
    stats: Res<HeroStats>,
    mut weapons: ResMut<Weapons>,
    grid: Res<EnemyGrid>,
    hero: Query<(Entity, &Pos, &super::Vel, &Equipment, Option<&super::abilities::CastLock>), With<Hero>>,
    mut enemies: Query<(&mut Health, &Ailments), With<Enemy>>,
    mut hits: MessageWriter<Hit>,
    flipbook: Res<Flipbook>,
    buffs: Res<Buffs>,
) {
    let Ok((hero_entity, hero, hero_vel, eq, cast_lock)) = hero.single() else { return };
    let w = eq.main;
    let base = eq.weapon_damage() * stats.damage * buffs.damage();
    weapons.attack_timer -= time.delta_secs();
    if weapons.attack_timer > 0.0 {
        return;
    }
    // Aim at the nearest enemy in range.
    let Some(target) = grid.near(hero.0, w.range).map(|(_, p, _)| p).min_by(|a, b| {
        a.distance_squared(hero.0).total_cmp(&b.distance_squared(hero.0))
    }) else {
        return;
    };
    weapons.attack_timer = eq.cooldown() * stats.cooldown / buffs.attack_speed();
    // TQ2's attack clips of the current stance, in turn, on the upper body.
    let attacks = eq.stance().attacks;
    let clip = attacks[weapons.attack_index % attacks.len()];
    weapons.attack_index += 1;
    let aim = (target - hero.0).normalize_or(Vec2::X);
    // Standing heroes swing with the whole body; running ones with the upper body only.
    let full_body = hero_vel.0.length_squared() < 0.1;
    // A skill animation that is still playing is not cut off by a weapon swing.
    if cast_lock.is_some_and(|l| l.0 > time.elapsed_secs()) {
        commands.entity(hero_entity).insert(super::Facing { dir: aim, time: 0.45 });
    } else {
        commands.entity(hero_entity).insert((
            UpperBodyAction { clip, max_duration: weapons.attack_timer.max(0.35), full_body },
            super::Facing { dir: aim, time: 0.45 },
        ));
    }
    let at = Vec3::new(hero.0.x, ground.stand(hero.0) + 1.2, hero.0.y);
    audio.play(w.fire_sound, Some(at), 0.5, 0.1);

    match w.kind {
        AttackKind::Bolt | AttackKind::Arrow => {
            // Extra projectiles fan out evenly around the aim direction.
            let fire = w.kind == AttackKind::Bolt;
            let arrow = assets.load(GltfAssetLabel::Scene(0).from_asset(ARROW));
            let n = stats.projectiles as f32;
            for i in 0..stats.projectiles {
                let dir = Vec2::from_angle((i as f32 - (n - 1.0) / 2.0) * BOLT_SPREAD).rotate(aim);
                let (damage, crit) = roll_damage(base, &stats);
                let projectile = Projectile {
                    vel: dir * w.speed_or_radius,
                    damage,
                    crit,
                    hit_sound: w.hit_sound,
                    explodes: fire,
                    pierce: w.pierce + stats.pierce,
                    life: w.range / w.speed_or_radius,
                    hit: vec![],
                };
                let mut entity = commands.spawn((projectile, RunEntity, Pos(hero.0 + dir * 0.6)));
                if fire {
                    // Fireball: TQ2 flame flipbook sprite with a small light of its own.
                    entity.insert((Transform::default(), Visibility::default())).with_children(|p| {
                        p.spawn(flipbook.fireball(1.7));
                        p.spawn(PointLight { color: Color::srgb(1.0, 0.5, 0.15), intensity: 2.0e5, range: 6.0, ..default() });
                    });
                } else {
                    entity.insert((WorldAssetRoot(arrow.clone()), Transform::from_scale(Vec3::splat(1.6))));
                }
            }
        }
        AttackKind::Sweep => {
            // Everything in an arc in front of the hero; each multishot upgrade widens the arc.
            let radius = w.speed_or_radius * stats.area;
            let half_arc = (SWEEP_ARC + SWEEP_ARC_PER_EXTRA * (stats.projectiles - 1) as f32).min(std::f32::consts::TAU) / 2.0;
            let mut struck = 0;
            for (e, p, _) in grid.near(hero.0, radius) {
                let to = p - hero.0;
                if to.length_squared() > 0.25 && aim.angle_to(to).abs() > half_arc {
                    continue;
                }
                if let Ok((mut hp, ail)) = enemies.get_mut(e) {
                    let (amount, crit) = roll_damage(base, &stats);
                    let amount = amount * ail.taken();
                    hp.hp -= amount;
                    hits.write(Hit { at: p, amount, crit });
                    struck += 1;
                }
            }
            if struck > 0 {
                audio.play(w.hit_sound, Some(at), 0.6, 0.05);
            }
        }
    }
}

fn move_projectiles(
    mut commands: Commands,
    time: Res<Time>,
    grid: Res<EnemyGrid>,
    ground: Res<Ground>,
    obstacles: Res<Obstacles>,
    mut projectiles: Query<(Entity, &mut Pos, &mut Projectile)>,
    mut enemies: Query<(&mut Health, &Ailments), With<Enemy>>,
    mut hit_events: MessageWriter<Hit>,
    audio: Res<Audio>,
    flipbook: Res<Flipbook>,
) {
    let dt = time.delta_secs();
    for (e, mut pos, mut p) in &mut projectiles {
        pos.0 += p.vel * dt;
        p.life -= dt;
        // Bolts fly at chest height and stop at rocks, walls and trees.
        let mut done = p.life <= 0.0 || obstacles.hits(pos.0, ground.stand(pos.0) + 1.2);
        if !done {
            let hits: Vec<Entity> = grid.near(pos.0, 0.4).map(|(e, _, _)| e).filter(|e| !p.hit.contains(e)).collect();
            for target in hits {
                if let Ok((mut hp, ail)) = enemies.get_mut(target) {
                    let amount = p.damage * ail.taken();
                    hp.hp -= amount;
                    hit_events.write(Hit { at: pos.0, amount, crit: p.crit });
                    let at = Vec3::new(pos.0.x, ground.stand(pos.0) + 1.0, pos.0.y);
                    audio.play(p.hit_sound, Some(at), 0.5, 0.07);
                    if p.explodes {
                        commands.spawn((flipbook.explosion(at, 1.8), RunEntity));
                    }
                    p.hit.push(target);
                    if p.hit.len() as u32 > p.pierce {
                        done = true;
                        break;
                    }
                }
            }
        }
        if done {
            commands.entity(e).despawn();
        }
    }
}

fn kill_dead(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut run: ResMut<Run>,
    audio: Res<Audio>,
    ground: Res<Ground>,
    arena: Res<super::arenas::CurrentArena>,
    stats: Res<HeroStats>,
    mut mech: ResMut<super::mechanics::Mechanics>,
    mut hero: Query<&mut Health, With<Hero>>,
    hero_positions: Query<&Pos, With<Hero>>,
    dead: Query<(Entity, &Health, &Pos, &Enemy, &super::Radius), Without<Hero>>,
) {
    let hero_pos = hero_positions.single().ok().map(|p| p.0);
    for (e, hp, pos, enemy, radius) in &dead {
        // Execution: wounded enemies below the threshold die (not bosses).
        let threshold = if enemy.boss { 0.0 } else { hp.max * stats.execute };
        if hp.hp > threshold {
            continue;
        }
        run.kills += 1;
        super::mechanics::on_kill(&mut mech, &stats, &mut hero);
        audio.play(sound::KILL, Some(Vec3::new(pos.0.x, ground.stand(pos.0) + 1.0, pos.0.y)), 0.6, 0.08);
        // Leave the simulation now; the body sinks away in `fade_dying`.
        // Thrown away from the hero; big creatures fly less far.
        let away = hero_pos.map_or(Vec2::X, |h| (pos.0 - h).normalize_or(Vec2::X));
        let heavy = (radius.0 / 0.7).max(1.0);
        let push = (5.0 + fastrand::f32() * 3.0) / heavy;
        let dir3 = Vec3::new(away.x, 0.0, away.y);
        commands.entity(e).remove::<(Enemy, Health, Pos)>().insert((
            Dying {
                age: 0.0,
                vel: dir3 * push + Vec3::Y * (4.0 + fastrand::f32() * 2.5) / heavy.sqrt(),
                axis: Vec3::Y.cross(dir3).normalize_or(Vec3::X),
                tilt: 0.0,
                spin: (fastrand::f32() - 0.5) * 3.0,
                landed: false,
            },
            crate::character::AnimationFrozen,
        ));
        // Bosses drop Ichor for mastery tiers (the second boss of a run two).
        if enemy.boss {
            run.bosses += 1;
            run.ichor += if run.bosses >= 2 { 2 } else { 1 };
            info!("boss killed: {} Ichor this run", run.ichor);
        }
        // Elites and bosses leave a chest with gear to choose from.
        if enemy.elite || enemy.boss {
            info!("chest dropped at {:.0} s", run.time);
            // Bosses drop better chests, and loot improves over the run.
            let luck = if enemy.boss { 2.5 } else { 1.3 } * (1.0 + run.time / 600.0) * arena.0.luck;
            drop_chest(&mut commands, &assets, pos.0, luck);
        }
        let shards = if enemy.boss { 10 } else { 1 };
        for i in 0..shards {
            let offset = if shards > 1 { Vec2::from_angle(i as f32) * 1.5 } else { Vec2::ZERO };
            commands.spawn((
                XpShard(enemy.xp.div_ceil(shards)),
                RunEntity,
                Pos(pos.0 + offset),
                WorldAssetRoot(assets.load(GltfAssetLabel::Scene(0).from_asset(SHARD))),
                Transform::from_scale(Vec3::splat(0.6)),
            ));
        }
    }
}

fn check_game_over(
    mut next: ResMut<NextState<GameState>>,
    mut run: ResMut<Run>,
    hero: Query<&Health, With<Hero>>,
) {
    // Test hook: TQ2_TEST_END=<seconds> ends the run early as a defeat (end screen, banking).
    let test_end = std::env::var("TQ2_TEST_END").ok().and_then(|s| s.parse::<f32>().ok());
    if let Ok(hp) = hero.single() {
        if hp.hp <= 0.0 || test_end.is_some_and(|t| run.time >= t) {
            next.set(GameState::GameOver);
        } else if run.time >= super::config::RUN_SECONDS {
            run.won = true;
            next.set(GameState::GameOver);
        }
    }
}

fn place_projectiles(ground: Res<Ground>, mut q: Query<(&Pos, &Projectile, &mut Transform)>) {
    for (pos, p, mut t) in &mut q {
        t.translation = Vec3::new(pos.0.x, ground.stand(pos.0) + 1.2, pos.0.y);
        t.rotation = Quat::from_rotation_y(f32::atan2(-p.vel.y, p.vel.x));
    }
}

fn fade_dying(mut commands: Commands, time: Res<Time>, ground: Res<Ground>, fx: Res<crate::fx::Fx>, mut q: Query<(Entity, &mut Dying, &mut Transform)>) {
    let dt = time.delta_secs();
    for (e, mut d, mut t) in &mut q {
        d.age += dt;
        if d.age > GONE {
            commands.entity(e).despawn();
            continue;
        }
        let floor = ground.stand(Vec2::new(t.translation.x, t.translation.z));
        if d.age < LIE {
            // Flight with gravity; on the ground it bounces once, then slides to a stop.
            d.vel.y -= GRAVITY * dt;
            t.translation += d.vel * dt;
            if t.translation.y <= floor {
                t.translation.y = floor;
                if d.vel.y < -3.0 {
                    if !d.landed {
                        commands.spawn(fx.flash(crate::fx::Tex::Puff, crate::fx::Element::Physical, t.translation + Vec3::Y * 0.3, 1.6, 0.45));
                    }
                    d.vel.y *= -0.3;
                    d.landed = true;
                } else {
                    d.vel.y = 0.0;
                }
                d.vel.x *= (1.0 - 6.0 * dt).max(0.0);
                d.vel.z *= (1.0 - 6.0 * dt).max(0.0);
            }
            // Tip over onto the side (about 80°) while flying, with a little spin.
            let step = (1.4 - d.tilt).min(5.0 * dt).max(0.0);
            d.tilt += step;
            let (axis, spin) = (d.axis, d.spin);
            t.rotation = Quat::from_axis_angle(axis, step) * Quat::from_rotation_y(spin * dt) * t.rotation;
        } else {
            // Sink into the ground.
            t.translation.y = floor - (d.age - LIE) / (GONE - LIE) * 1.6;
        }
    }
}

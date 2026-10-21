//! Hero attacks (auto-aimed bolts, Nova), damage, deaths and XP drops.
use bevy::prelude::*;

use super::content::{ARROW, NOVA_RING, NOVA_RING_RADIUS, SHARD};
use crate::vfx::Flipbook;
use super::equipment::{drop_chest, AttackKind, Equipment};
use super::enemies::{Enemy, EnemyGrid};
use super::player::{Hero, HeroStats};
use super::progression::XpShard;
use super::{GameState, Health, Pos, Run, RunEntity};
use super::content::sound;
use crate::audio::Audio;
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
struct Dying(f32);

/// Expanding ring shown when Nova fires.
#[derive(Component)]
struct NovaRing {
    age: f32,
    radius: f32,
}

/// A hit to show as a floating damage number.
#[derive(Message)]
pub struct Hit {
    pub at: Vec2,
    pub amount: f32,
    pub crit: bool,
}

/// Fire flash of a Nova pulse (a real light, fading out).
#[derive(Component)]
struct NovaFlash(f32);

#[derive(Resource, Default)]
struct Weapons {
    /// Cooldown of the equipped weapon's auto-attack.
    attack_timer: f32,
    nova_timer: f32,
}

/// Angle between neighbouring projectiles of a multishot (radians, ~11°).
const BOLT_SPREAD: f32 = 0.2;
/// Sword sweep arc (radians) and the extra width per additional projectile upgrade.
const SWEEP_ARC: f32 = 2.4;
const SWEEP_ARC_PER_EXTRA: f32 = 0.35;
const NOVA_COOLDOWN: f32 = 3.5;
const NOVA_DAMAGE: f32 = 12.0;
const NOVA_RADIUS: f32 = 4.0;

pub fn plugin(app: &mut App) {
    app.init_resource::<Weapons>()
        .add_message::<Hit>()
        .add_systems(
            FixedUpdate,
            (auto_attack, fire_nova, move_projectiles, kill_dead, check_game_over)
                .chain()
                .after(super::enemies::EnemySet)
                .run_if(in_state(GameState::Playing)),
        )
        .add_systems(Update, (place_projectiles, fade_dying, animate_nova, fade_nova_flash));
}

/// Fresh weapon cooldowns for a new run.
pub fn reset(commands: &mut Commands) {
    commands.insert_resource(Weapons::default());
}

/// Damage of one hit and whether it crit.
fn roll_damage(base: f32, stats: &HeroStats) -> (f32, bool) {
    let crit = fastrand::f32() < stats.crit;
    (base * stats.damage * if crit { 2.0 } else { 1.0 }, crit)
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
    hero: Query<(&Pos, &Equipment), With<Hero>>,
    mut enemies: Query<&mut Health, With<Enemy>>,
    mut hits: MessageWriter<Hit>,
    flipbook: Res<Flipbook>,
) {
    let Ok((hero, eq)) = hero.single() else { return };
    let w = eq.weapon;
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
    weapons.attack_timer = w.cooldown * stats.cooldown;
    let aim = (target - hero.0).normalize_or(Vec2::X);
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
                let (damage, crit) = roll_damage(w.damage, &stats);
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
                if let Ok(mut hp) = enemies.get_mut(e) {
                    let (amount, crit) = roll_damage(w.damage, &stats);
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

fn fire_nova(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<AssetServer>,
    stats: Res<HeroStats>,
    mut weapons: ResMut<Weapons>,
    grid: Res<EnemyGrid>,
    ground: Res<Ground>,
    audio: Res<Audio>,
    flipbook: Res<Flipbook>,
    mut hits: MessageWriter<Hit>,
    hero: Query<&Pos, With<Hero>>,
    mut enemies: Query<&mut Health, With<Enemy>>,
) {
    if stats.nova_level == 0 {
        return;
    }
    let Ok(hero) = hero.single() else { return };
    weapons.nova_timer -= time.delta_secs();
    if weapons.nova_timer > 0.0 {
        return;
    }
    weapons.nova_timer = NOVA_COOLDOWN * stats.cooldown / (1.0 + 0.15 * (stats.nova_level - 1) as f32);
    let radius = NOVA_RADIUS * stats.area * (1.0 + 0.1 * stats.nova_level as f32);
    let damage = NOVA_DAMAGE * (1.0 + 0.3 * (stats.nova_level - 1) as f32);
    let mut count = 0;
    for (e, at, _) in grid.near(hero.0, radius) {
        if let Ok(mut hp) = enemies.get_mut(e) {
            let (amount, crit) = roll_damage(damage, &stats);
            hp.hp -= amount;
            hits.write(Hit { at, amount, crit });
            count += 1;
        }
    }
    debug!("nova pulse r={radius:.1} dmg={damage:.0}: {count} enemies hit");
    let at = Vec3::new(hero.0.x, ground.stand(hero.0) + 1.0, hero.0.y);
    audio.play(sound::NOVA, Some(at), 0.9, 0.2);
    commands.spawn((
        NovaFlash(0.0),
        RunEntity,
        PointLight { color: Color::srgb(1.0, 0.55, 0.15), intensity: 4.0e6, range: radius * 3.0, ..default() },
        Transform::from_xyz(hero.0.x, ground.stand(hero.0) + 1.5, hero.0.y),
    ));
    // A ring of TQ2 fire explosions marks the Nova's reach.
    for k in 0..10 {
        let p = hero.0 + Vec2::from_angle(k as f32 * std::f32::consts::TAU / 10.0) * radius * 0.85;
        let at = Vec3::new(p.x, ground.stand(p) + 0.8, p.y);
        commands.spawn((flipbook.explosion(at, 2.2), RunEntity));
    }
    commands.spawn((
        NovaRing { age: 0.0, radius },
        RunEntity,
        Pos(hero.0),
        WorldAssetRoot(assets.load(GltfAssetLabel::Scene(0).from_asset(NOVA_RING))),
        Transform::default(),
    ));
}

fn move_projectiles(
    mut commands: Commands,
    time: Res<Time>,
    grid: Res<EnemyGrid>,
    ground: Res<Ground>,
    obstacles: Res<Obstacles>,
    mut projectiles: Query<(Entity, &mut Pos, &mut Projectile)>,
    mut enemies: Query<&mut Health, With<Enemy>>,
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
                if let Ok(mut hp) = enemies.get_mut(target) {
                    hp.hp -= p.damage;
                    hit_events.write(Hit { at: pos.0, amount: p.damage, crit: p.crit });
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
    dead: Query<(Entity, &Health, &Pos, &Enemy)>,
) {
    for (e, hp, pos, enemy) in &dead {
        if hp.hp > 0.0 {
            continue;
        }
        run.kills += 1;
        audio.play(sound::KILL, Some(Vec3::new(pos.0.x, ground.stand(pos.0) + 1.0, pos.0.y)), 0.6, 0.08);
        // Leave the simulation now; the body sinks away in `fade_dying`.
        commands.entity(e).remove::<(Enemy, Health, Pos)>().insert(Dying(0.0));
        // Elites and bosses leave a chest with gear to choose from.
        if enemy.elite || enemy.boss {
            info!("chest dropped at {:.0} s", run.time);
            drop_chest(&mut commands, &assets, pos.0);
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
    if let Ok(hp) = hero.single() {
        if hp.hp <= 0.0 {
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

fn fade_dying(mut commands: Commands, time: Res<Time>, mut q: Query<(Entity, &mut Dying, &mut Transform)>) {
    for (e, mut d, mut t) in &mut q {
        d.0 += time.delta_secs();
        t.translation.y -= 1.5 * time.delta_secs();
        t.rotation *= Quat::from_rotation_x(-1.5 * time.delta_secs()).slerp(Quat::IDENTITY, (d.0 / 0.5).min(1.0));
        if d.0 > 1.2 {
            commands.entity(e).despawn();
        }
    }
}

fn fade_nova_flash(mut commands: Commands, time: Res<Time>, mut q: Query<(Entity, &mut NovaFlash, &mut PointLight)>) {
    for (e, mut flash, mut light) in &mut q {
        flash.0 += time.delta_secs();
        light.intensity = 4.0e6 * (1.0 - flash.0 / 0.4).max(0.0);
        if flash.0 > 0.4 {
            commands.entity(e).despawn();
        }
    }
}

fn animate_nova(
    mut commands: Commands,
    time: Res<Time>,
    ground: Res<Ground>,
    mut q: Query<(Entity, &Pos, &mut NovaRing, &mut Transform)>,
) {
    for (e, pos, mut ring, mut t) in &mut q {
        ring.age += time.delta_secs();
        // The TQ2 Forge Call ring sweeps out to the Nova radius while spinning.
        let k = (ring.age / 0.35).min(1.0);
        let scale = ring.radius / NOVA_RING_RADIUS * k;
        t.translation = Vec3::new(pos.0.x, ground.stand(pos.0) + 0.4, pos.0.y);
        t.scale = Vec3::new(scale, scale * 0.5, scale);
        t.rotation = Quat::from_rotation_y(ring.age * 12.0);
        if ring.age > 0.5 {
            commands.entity(e).despawn();
        }
    }
}

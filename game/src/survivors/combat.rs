//! Hero attacks (auto-aimed bolts, Nova), damage, deaths and XP drops.
use bevy::prelude::*;

use super::content::{BOLT, NOVA_RING, NOVA_RING_RADIUS, SHARD};
use super::enemies::{Enemy, EnemyGrid};
use super::player::{Hero, HeroStats};
use super::progression::XpShard;
use super::{GameState, Health, Pos, Run, RunEntity};
use crate::ground::Ground;
use crate::obstacles::Obstacles;

#[derive(Component)]
pub struct Projectile {
    vel: Vec2,
    damage: f32,
    crit: bool,
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
    bolt_timer: f32,
    nova_timer: f32,
}

const BOLT_COOLDOWN: f32 = 0.9;
const BOLT_DAMAGE: f32 = 10.0;
const BOLT_SPEED: f32 = 22.0;
const BOLT_RANGE: f32 = 18.0;
/// Angle between neighbouring bolts of a multishot (radians, ~11°).
const BOLT_SPREAD: f32 = 0.2;
const NOVA_COOLDOWN: f32 = 3.5;
const NOVA_DAMAGE: f32 = 12.0;
const NOVA_RADIUS: f32 = 4.0;

pub fn plugin(app: &mut App) {
    app.init_resource::<Weapons>()
        .add_message::<Hit>()
        .add_systems(
            FixedUpdate,
            (fire_bolts, fire_nova, move_projectiles, kill_dead, check_game_over)
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

fn fire_bolts(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<AssetServer>,
    stats: Res<HeroStats>,
    mut weapons: ResMut<Weapons>,
    grid: Res<EnemyGrid>,
    hero: Query<&Pos, With<Hero>>,
) {
    let Ok(hero) = hero.single() else { return };
    weapons.bolt_timer -= time.delta_secs();
    if weapons.bolt_timer > 0.0 {
        return;
    }
    // Aim at the nearest enemy; extra projectiles fan out evenly around that direction.
    let Some(target) = grid.near(hero.0, BOLT_RANGE).map(|(_, p, _)| p).min_by(|a, b| {
        a.distance_squared(hero.0).total_cmp(&b.distance_squared(hero.0))
    }) else {
        return;
    };
    weapons.bolt_timer = BOLT_COOLDOWN * stats.cooldown;
    let aim = (target - hero.0).normalize_or(Vec2::X);
    let scene = assets.load(GltfAssetLabel::Scene(0).from_asset(BOLT));
    let n = stats.projectiles as f32;
    for i in 0..stats.projectiles {
        let dir = Vec2::from_angle((i as f32 - (n - 1.0) / 2.0) * BOLT_SPREAD).rotate(aim);
        let (damage, crit) = roll_damage(BOLT_DAMAGE, &stats);
        commands.spawn((
            Projectile { vel: dir * BOLT_SPEED, damage, crit, pierce: stats.pierce, life: 1.2, hit: vec![] },
            RunEntity,
            Pos(hero.0 + dir * 0.6),
            WorldAssetRoot(scene.clone()),
            Transform::from_scale(Vec3::splat(2.5)),
        ));
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
    commands.spawn((
        NovaFlash(0.0),
        RunEntity,
        PointLight { color: Color::srgb(1.0, 0.55, 0.15), intensity: 4.0e6, range: radius * 3.0, ..default() },
        Transform::from_xyz(hero.0.x, ground.stand(hero.0) + 1.5, hero.0.y),
    ));
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
    dead: Query<(Entity, &Health, &Pos, &Enemy)>,
) {
    for (e, hp, pos, enemy) in &dead {
        if hp.hp > 0.0 {
            continue;
        }
        run.kills += 1;
        // Leave the simulation now; the body sinks away in `fade_dying`.
        commands.entity(e).remove::<(Enemy, Health, Pos)>().insert(Dying(0.0));
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

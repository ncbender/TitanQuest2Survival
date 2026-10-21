//! Hero attacks (auto-aimed bolts, Nova), damage, deaths and XP drops.
use bevy::prelude::*;

use super::content::{BOLT, SHARD};
use super::enemies::{Enemy, EnemyGrid};
use super::player::{Hero, HeroStats};
use super::progression::XpShard;
use super::{GameState, Health, Pos, Run, RunEntity};
use crate::ground::Ground;

#[derive(Component)]
pub struct Projectile {
    vel: Vec2,
    damage: f32,
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

#[derive(Resource, Default)]
struct Weapons {
    bolt_timer: f32,
    nova_timer: f32,
}

#[derive(Resource)]
struct NovaVisual {
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
}

const BOLT_COOLDOWN: f32 = 0.9;
const BOLT_DAMAGE: f32 = 10.0;
const BOLT_SPEED: f32 = 22.0;
const BOLT_RANGE: f32 = 18.0;
const NOVA_COOLDOWN: f32 = 3.5;
const NOVA_DAMAGE: f32 = 12.0;
const NOVA_RADIUS: f32 = 4.0;

pub fn plugin(app: &mut App) {
    app.init_resource::<Weapons>()
        .add_systems(Startup, setup_nova_visual)
        .add_systems(
            FixedUpdate,
            (fire_bolts, fire_nova, move_projectiles, kill_dead, check_game_over)
                .chain()
                .after(super::enemies::EnemySet)
                .run_if(in_state(GameState::Playing)),
        )
        .add_systems(Update, (place_projectiles, fade_dying, animate_nova));
}

fn roll_damage(base: f32, stats: &HeroStats) -> f32 {
    let crit = if fastrand::f32() < stats.crit { 2.0 } else { 1.0 };
    base * stats.damage * crit
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
    // Nearest enemies first; one bolt per target, extra bolts fan out if targets run out.
    let mut targets: Vec<_> = grid.near(hero.0, BOLT_RANGE).map(|(_, p, _)| p).collect();
    if targets.is_empty() {
        return;
    }
    weapons.bolt_timer = BOLT_COOLDOWN * stats.cooldown;
    targets.sort_by(|a, b| a.distance_squared(hero.0).total_cmp(&b.distance_squared(hero.0)));
    let scene = assets.load(GltfAssetLabel::Scene(0).from_asset(BOLT));
    for i in 0..stats.projectiles {
        let target = targets[i as usize % targets.len()];
        let spread = if (i as usize) < targets.len() { 0.0 } else { (i as f32 - 0.5) * 0.25 };
        let dir = Vec2::from_angle(spread).rotate((target - hero.0).normalize_or(Vec2::X));
        commands.spawn((
            Projectile { vel: dir * BOLT_SPEED, damage: roll_damage(BOLT_DAMAGE, &stats), pierce: stats.pierce, life: 1.2, hit: vec![] },
            RunEntity,
            Pos(hero.0 + dir * 0.6),
            WorldAssetRoot(scene.clone()),
            Transform::from_scale(Vec3::splat(1.5)),
        ));
    }
}

fn fire_nova(
    mut commands: Commands,
    time: Res<Time>,
    stats: Res<HeroStats>,
    visual: Res<NovaVisual>,
    mut weapons: ResMut<Weapons>,
    grid: Res<EnemyGrid>,
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
    for (e, _, _) in grid.near(hero.0, radius) {
        if let Ok(mut hp) = enemies.get_mut(e) {
            hp.hp -= roll_damage(damage, &stats);
        }
    }
    commands.spawn((
        NovaRing { age: 0.0, radius },
        RunEntity,
        Pos(hero.0),
        Mesh3d(visual.mesh.clone()),
        MeshMaterial3d(visual.material.clone()),
        Transform::default(),
    ));
}

fn move_projectiles(
    mut commands: Commands,
    time: Res<Time>,
    grid: Res<EnemyGrid>,
    mut projectiles: Query<(Entity, &mut Pos, &mut Projectile)>,
    mut enemies: Query<&mut Health, With<Enemy>>,
) {
    let dt = time.delta_secs();
    for (e, mut pos, mut p) in &mut projectiles {
        pos.0 += p.vel * dt;
        p.life -= dt;
        let mut done = p.life <= 0.0;
        if !done {
            let hits: Vec<Entity> = grid.near(pos.0, 0.4).map(|(e, _, _)| e).filter(|e| !p.hit.contains(e)).collect();
            for target in hits {
                if let Ok(mut hp) = enemies.get_mut(target) {
                    hp.hp -= p.damage;
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

// TEMPORARY visual: Nova uses a plain emissive torus until TQ2 Niagara effects are imported.
fn setup_nova_visual(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    commands.insert_resource(NovaVisual {
        mesh: meshes.add(Torus::new(0.9, 1.0)),
        material: materials.add(StandardMaterial {
            base_color: Color::srgba(0.4, 0.8, 1.0, 0.5),
            emissive: LinearRgba::rgb(1.0, 3.0, 6.0),
            alpha_mode: AlphaMode::Add,
            unlit: true,
            ..default()
        }),
    });
}

fn animate_nova(
    mut commands: Commands,
    time: Res<Time>,
    ground: Res<Ground>,
    mut q: Query<(Entity, &Pos, &mut NovaRing, &mut Transform)>,
) {
    for (e, pos, mut ring, mut t) in &mut q {
        ring.age += time.delta_secs();
        let k = (ring.age / 0.35).min(1.0);
        t.translation = Vec3::new(pos.0.x, ground.stand(pos.0) + 0.3, pos.0.y);
        t.scale = Vec3::new(ring.radius * k, 1.0, ring.radius * k);
        if ring.age > 0.45 {
            commands.entity(e).despawn();
        }
    }
}

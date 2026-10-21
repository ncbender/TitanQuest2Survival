//! The hero: WASD movement relative to the camera, upgradeable stats.
use bevy::prelude::*;

use super::camera::CameraRig;
use super::equipment::Equipment;
use super::{config, GameState, Health, Pos, Radius, RunEntity, Vel};
use crate::character::{CharacterAnim, UpperBodyRig};
use crate::ground::Ground;
use crate::obstacles::Obstacles;

#[derive(Component)]
pub struct Hero;

const HERO_RADIUS: f32 = 0.45;

/// Everything upgrades can change. Multipliers start at 1.
#[derive(Resource, Clone)]
pub struct HeroStats {
    pub damage: f32,
    pub cooldown: f32,
    pub projectiles: u32,
    pub pierce: u32,
    pub speed: f32,
    pub area: f32,
    pub crit: f32,
    pub pickup: f32,
    /// Extra fraction of enemy damage absorbed (passives), on top of gear.
    pub armor: f32,
    /// Multiplier for skill damage (mastery skills).
    pub skill_damage: f32,
    /// Multiplier for skill cooldowns.
    pub skill_cooldown: f32,
    /// Health regenerated per second.
    pub regen: f32,
    /// Bonus to skill ailment chances (0.2 = +20 %).
    pub ailment: f32,
    /// Multiplier for collected experience.
    pub xp: f32,
    /// Damage multiplier of critical hits.
    pub crit_mult: f32,
    /// Damage dealt back to an enemy that hits the hero.
    pub retaliation: f32,
    /// TQ2 mechanics from passives (mechanics.rs): barrier as a fraction of maximum health,
    /// increased Rage effect (0 = no Rage), glancing hit chance, execution threshold (fraction
    /// of enemy health), increased shade generation (0 = no shades).
    pub barrier: f32,
    pub rage: f32,
    pub glancing: f32,
    pub execute: f32,
    pub shade: f32,
    /// Current damage bonus from Rage stacks (updated during the run).
    pub rage_bonus: f32,
}

impl Default for HeroStats {
    fn default() -> Self {
        Self {
            damage: 1.0,
            cooldown: 1.0,
            projectiles: 1,
            pierce: 0,
            speed: 1.0,
            area: 1.0,
            crit: 0.05,
            pickup: 1.0,
            armor: 0.0,
            skill_damage: 1.0,
            skill_cooldown: 1.0,
            regen: 0.0,
            ailment: 0.0,
            xp: 1.0,
            crit_mult: 2.0,
            retaliation: 0.0,
            barrier: 0.0,
            rage: 0.0,
            glancing: 0.0,
            execute: 0.0,
            shade: 0.0,
            rage_bonus: 0.0,
        }
    }
}

pub fn plugin(app: &mut App) {
    app.init_resource::<HeroStats>()
        .add_systems(FixedUpdate, move_hero.run_if(in_state(GameState::Playing)))
        .add_systems(Update, animate_hero);
}

pub fn spawn_hero(commands: &mut Commands, at: Vec2, equipment: Equipment) -> Entity {
    let hero = commands
        .spawn((
            Hero,
            UpperBodyRig,
            RunEntity,
            Pos(at),
            Vel::default(),
            Radius(HERO_RADIUS),
            Health { hp: config::PLAYER_HP, max: config::PLAYER_HP },
            // Parts, carried weapon and stance follow the equipment (see equipment.rs).
            CharacterAnim::looping(equipment.stance().idle),
            equipment,
            Transform::default(),
            Visibility::default(),
        ))
        .id();
    hero
}

fn move_hero(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    stats: Res<HeroStats>,
    buffs: Res<super::abilities::Buffs>,
    ground: Res<Ground>,
    obstacles: Res<Obstacles>,
    rig: Query<&CameraRig>,
    mut hero: Query<(&mut Pos, &mut Vel), With<Hero>>,
) {
    let Ok((mut pos, mut vel)) = hero.single_mut() else { return };
    let yaw = rig.single().map(|r| r.yaw).unwrap_or(0.0);
    let axis = |a: KeyCode, b: KeyCode| keys.pressed(a) as i32 as f32 - keys.pressed(b) as i32 as f32;
    let mut input = Vec2::new(axis(KeyCode::KeyD, KeyCode::KeyA), axis(KeyCode::KeyS, KeyCode::KeyW));
    if std::env::var_os("TQ2_AUTOPLAY").is_some() {
        // Test driver: run a wide circle so the horde follows and the hero keeps fighting.
        input = Vec2::from_angle(time.elapsed_secs() * 0.25);
    }
    // Screen-relative: rotate input by the camera yaw so W always moves "up the screen".
    let dir = Vec2::from_angle(-yaw).rotate(input).normalize_or_zero();
    vel.0 = dir * config::PLAYER_SPEED * stats.speed * buffs.speed();
    let next = pos.0 + vel.0 * time.delta_secs();
    let b = ground.bounds();
    let next = ground.clamp_move(pos.0, next.clamp(b.min + 2.0, b.max - 2.0));
    pos.0 = obstacles.resolve(next, HERO_RADIUS, ground.stand(next));
}

fn animate_hero(mut hero: Query<(&Vel, &Equipment, &mut CharacterAnim), With<Hero>>) {
    for (vel, eq, mut anim) in &mut hero {
        let stance = eq.stance();
        let clip = if vel.0.length_squared() > 0.1 { stance.run } else { stance.idle };
        if anim.clip != clip {
            *anim = CharacterAnim::looping(clip);
        }
    }
}

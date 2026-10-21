//! The hero: WASD movement relative to the camera, upgradeable stats.
use bevy::prelude::*;

use super::camera::CameraRig;
use super::content::{HERO_IDLE, HERO_PARTS, HERO_RUN, HERO_WEAPON, HERO_WEAPON_BONE};
use super::{config, GameState, Health, Pos, Radius, RunEntity, Vel};
use crate::character::{attach_parts, Attachment, CharacterAnim};
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
    pub nova_level: u32,
}

impl Default for HeroStats {
    fn default() -> Self {
        Self { damage: 1.0, cooldown: 1.0, projectiles: 1, pierce: 0, speed: 1.0, area: 1.0, crit: 0.05, pickup: 1.0, nova_level: 0 }
    }
}

pub fn plugin(app: &mut App) {
    app.init_resource::<HeroStats>()
        .add_systems(FixedUpdate, move_hero.run_if(in_state(GameState::Playing)))
        .add_systems(Update, animate_hero);
}

pub fn spawn_hero(commands: &mut Commands, at: Vec2) {
    let hero = commands
        .spawn((
            Hero,
            RunEntity,
            Pos(at),
            Vel::default(),
            Radius(HERO_RADIUS),
            Health { hp: config::PLAYER_HP, max: config::PLAYER_HP },
            CharacterAnim::looping(HERO_IDLE),
            // The staff's head points along its +Y; on HandAttach_R that axis faces down.
            Attachment { bone: HERO_WEAPON_BONE, scene: HERO_WEAPON, rotation: Quat::from_rotation_x(std::f32::consts::PI) },
            Transform::default(),
            Visibility::default(),
        ))
        .id();
    attach_parts(commands, hero, HERO_PARTS);
}

fn move_hero(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    stats: Res<HeroStats>,
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
    vel.0 = dir * config::PLAYER_SPEED * stats.speed;
    let next = pos.0 + vel.0 * time.delta_secs();
    let b = ground.bounds();
    let next = ground.clamp_move(pos.0, next.clamp(b.min + 2.0, b.max - 2.0));
    pos.0 = obstacles.resolve(next, HERO_RADIUS, ground.stand(next));
}

fn animate_hero(mut hero: Query<(&Vel, &mut CharacterAnim), With<Hero>>) {
    for (vel, mut anim) in &mut hero {
        let clip = if vel.0.length_squared() > 0.1 { HERO_RUN } else { HERO_IDLE };
        if anim.clip != clip {
            *anim = CharacterAnim::looping(clip);
        }
    }
}

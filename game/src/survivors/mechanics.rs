//! Simplified versions of TQ2 mechanics that some passives build on: Barrier (absorbs damage,
//! refills when not hit), Rage (kills stack a damage bonus that fades), Glancing hits (take
//! half damage), Execution (enemies below a health threshold die) and Shades (every few kills
//! heal). They do nothing until a passive grants them.
use bevy::prelude::*;

use super::player::{Hero, HeroStats};
use super::{GameState, Health};

/// Seconds without being hit before the barrier refills.
const BARRIER_DELAY: f32 = 3.0;
/// Fraction of the barrier refilled per second.
const BARRIER_REFILL: f32 = 0.25;
const RAGE_MAX: u32 = 10;
/// Damage bonus per Rage stack before passives.
const RAGE_PER_STACK: f32 = 0.015;
/// Seconds after the last kill before Rage is lost.
const RAGE_TIMEOUT: f32 = 4.0;
/// Kills per shade without passives; a shade heals this fraction of maximum health.
const SHADE_KILLS: f32 = 30.0;
const SHADE_HEAL: f32 = 0.06;

#[derive(Resource, Default)]
pub struct Mechanics {
    pub barrier: f32,
    pub barrier_max: f32,
    since_hit: f32,
    pub rage: u32,
    rage_timer: f32,
    shade_progress: f32,
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Mechanics>().add_systems(FixedUpdate, tick.run_if(in_state(GameState::Playing)));
}

/// Fresh state for a run (after passives and gear were applied).
pub fn reset(commands: &mut Commands, stats: &HeroStats, hp: &Health) {
    let max = stats.barrier * hp.max;
    commands.insert_resource(Mechanics { barrier: max, barrier_max: max, ..default() });
}

fn tick(time: Res<Time>, mut mech: ResMut<Mechanics>, mut stats: ResMut<HeroStats>) {
    let dt = time.delta_secs();
    mech.since_hit += dt;
    if mech.since_hit > BARRIER_DELAY && mech.barrier < mech.barrier_max {
        mech.barrier = (mech.barrier + mech.barrier_max * BARRIER_REFILL * dt).min(mech.barrier_max);
    }
    if mech.rage > 0 {
        mech.rage_timer -= dt;
        if mech.rage_timer <= 0.0 {
            mech.rage = 0;
        }
    }
    let bonus = if stats.rage > 0.0 { mech.rage as f32 * RAGE_PER_STACK * (1.0 + stats.rage) } else { 0.0 };
    if (stats.rage_bonus - bonus).abs() > 1e-4 {
        stats.rage_bonus = bonus;
    }
}

/// Damage the hero actually takes from a hit of `amount` (after armour): glancing hits halve
/// it, the barrier absorbs what it can.
pub fn hurt_hero(amount: f32, stats: &HeroStats, mech: &mut Mechanics) -> f32 {
    mech.since_hit = 0.0;
    let mut amount = amount;
    if fastrand::f32() < stats.glancing {
        amount *= 0.5;
    }
    let absorbed = amount.min(mech.barrier);
    mech.barrier -= absorbed;
    amount - absorbed
}

/// A kill: Rage stacks, shade progress (heals the hero when a shade completes).
pub fn on_kill(mech: &mut Mechanics, stats: &HeroStats, hero: &mut Query<&mut Health, With<Hero>>) {
    if stats.rage > 0.0 {
        mech.rage = (mech.rage + 1).min(RAGE_MAX);
        mech.rage_timer = RAGE_TIMEOUT;
    }
    if stats.shade > 0.0 {
        mech.shade_progress += (1.0 + stats.shade) / SHADE_KILLS;
        if mech.shade_progress >= 1.0 {
            mech.shade_progress -= 1.0;
            if let Ok(mut hp) = hero.single_mut() {
                hp.hp = (hp.hp + hp.max * SHADE_HEAL).min(hp.max);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn barrier_absorbs_before_health() {
        let stats = HeroStats::default();
        let mut mech = Mechanics { barrier: 10.0, barrier_max: 10.0, ..default() };
        assert_eq!(hurt_hero(4.0, &stats, &mut mech), 0.0);
        assert_eq!(hurt_hero(10.0, &stats, &mut mech), 4.0);
        assert_eq!(mech.barrier, 0.0);
    }
}

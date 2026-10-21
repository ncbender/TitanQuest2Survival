//! XP shards, level-ups and the upgrade choices.
use bevy::prelude::*;

use super::player::{Hero, HeroStats};
use super::{GameState, Health, Pos, Run};
use crate::ground::Ground;

/// A dropped XP crystal (TQ2 mystic shards) worth this much XP.
#[derive(Component)]
pub struct XpShard(pub u32);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Upgrade {
    Damage,
    AttackSpeed,
    Multishot,
    Pierce,
    MoveSpeed,
    Area,
    Crit,
    Vitality,
    Magnet,
    Nova,
}

impl Upgrade {
    pub const ALL: [Upgrade; 10] = [
        Upgrade::Damage,
        Upgrade::AttackSpeed,
        Upgrade::Multishot,
        Upgrade::Pierce,
        Upgrade::MoveSpeed,
        Upgrade::Area,
        Upgrade::Crit,
        Upgrade::Vitality,
        Upgrade::Magnet,
        Upgrade::Nova,
    ];

    /// Lower-case variant name, e.g. "multishot" (used by test hooks).
    pub fn from_name(name: &str) -> Option<Upgrade> {
        Upgrade::ALL.into_iter().find(|u| format!("{u:?}").eq_ignore_ascii_case(name))
    }

    pub fn title(self, stats: &HeroStats) -> String {
        match self {
            Upgrade::Damage => "Kraft des Titanen: +20 % Schaden".into(),
            Upgrade::AttackSpeed => "Schnelle Hand: +15 % Angriffstempo".into(),
            Upgrade::Multishot => "Schmiedefeuer: +1 Geschoss".into(),
            Upgrade::Pierce => "Durchschlag: Geschosse treffen +1 Gegner".into(),
            Upgrade::MoveSpeed => "Flinke Füße: +10 % Laufgeschwindigkeit".into(),
            Upgrade::Area => "Weite Wirkung: +20 % Flächengröße".into(),
            Upgrade::Crit => "Kritisches Auge: +8 % Krit-Chance".into(),
            Upgrade::Vitality => "Vitalität: +20 max. Leben, volle Heilung".into(),
            Upgrade::Magnet => "Anziehung: +50 % Sammelradius".into(),
            Upgrade::Nova if stats.nova_level == 0 => "Neue Fähigkeit – Nova: Schockwelle um dich herum".into(),
            Upgrade::Nova => format!("Nova Stufe {}: mehr Schaden, Radius, Tempo", stats.nova_level + 1),
        }
    }

    fn available(self, stats: &HeroStats) -> bool {
        match self {
            Upgrade::Multishot => stats.projectiles < 8,
            Upgrade::Pierce => stats.pierce < 5,
            Upgrade::Crit => stats.crit < 0.5,
            Upgrade::AttackSpeed => stats.cooldown > 0.3,
            Upgrade::Nova => stats.nova_level < 6,
            _ => true,
        }
    }

    pub fn apply(self, stats: &mut HeroStats, hero_hp: &mut Health) {
        match self {
            Upgrade::Damage => stats.damage *= 1.2,
            Upgrade::AttackSpeed => stats.cooldown *= 0.85,
            Upgrade::Multishot => stats.projectiles += 1,
            Upgrade::Pierce => stats.pierce += 1,
            Upgrade::MoveSpeed => stats.speed *= 1.1,
            Upgrade::Area => stats.area *= 1.2,
            Upgrade::Crit => stats.crit += 0.08,
            Upgrade::Vitality => {
                hero_hp.max += 20.0;
                hero_hp.hp = hero_hp.max;
            }
            Upgrade::Magnet => stats.pickup *= 1.5,
            Upgrade::Nova => stats.nova_level += 1,
        }
    }
}

/// The three choices offered at the current level-up.
#[derive(Resource, Default)]
pub struct Offer(pub Vec<Upgrade>);

pub fn xp_for_level(level: u32) -> u32 {
    // 6, 10, 14, 19, … - the first level-up comes within the first half minute.
    2 + level * 4 + level * level / 3
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Offer>()
        .add_systems(FixedUpdate, (tick_time, collect_shards).run_if(in_state(GameState::Playing)))
        .add_systems(Update, spin_shards)
        .add_systems(OnEnter(GameState::LevelUp), make_offer);
}

fn tick_time(time: Res<Time>, mut run: ResMut<Run>) {
    run.time += time.delta_secs();
}

fn collect_shards(
    mut commands: Commands,
    time: Res<Time>,
    stats: Res<HeroStats>,
    mut run: ResMut<Run>,
    mut next: ResMut<NextState<GameState>>,
    hero: Query<&Pos, (With<Hero>, Without<XpShard>)>,
    mut shards: Query<(Entity, &mut Pos, &XpShard)>,
) {
    let Ok(hero) = hero.single() else { return };
    let radius = super::config::PICKUP_RADIUS * stats.pickup;
    for (e, mut pos, shard) in &mut shards {
        let d = hero.0 - pos.0;
        let dist = d.length();
        if dist < 0.6 {
            commands.entity(e).despawn();
            run.xp += shard.0;
        } else if dist < radius {
            // Magnet: shards fly to the hero, faster the closer they get.
            pos.0 += d / dist * (8.0 + 20.0 * (1.0 - dist / radius)) * time.delta_secs();
        }
    }
    if run.xp >= run.xp_next {
        run.xp -= run.xp_next;
        run.level += 1;
        run.xp_next = xp_for_level(run.level);
        next.set(GameState::LevelUp);
    }
}

fn make_offer(mut offer: ResMut<Offer>, stats: Res<HeroStats>, run: Res<Run>) {
    let mut pool: Vec<Upgrade> = Upgrade::ALL.into_iter().filter(|u| u.available(&stats)).collect();
    fastrand::shuffle(&mut pool);
    // The first level-up always offers the second ability so every run can find it.
    if run.level == 2 && stats.nova_level == 0 {
        pool.retain(|u| *u != Upgrade::Nova);
        pool.insert(0, Upgrade::Nova);
    }
    offer.0 = pool.into_iter().take(3).collect();
}

fn spin_shards(time: Res<Time>, ground: Res<Ground>, mut q: Query<(&Pos, &mut Transform), With<XpShard>>) {
    let t = time.elapsed_secs();
    for (pos, mut tr) in &mut q {
        tr.translation = Vec3::new(pos.0.x, ground.stand(pos.0) + 0.4 + (t * 3.0 + pos.0.x).sin() * 0.1, pos.0.y);
        tr.rotation = Quat::from_rotation_y(t * 1.5 + pos.0.y);
    }
}

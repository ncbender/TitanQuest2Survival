//! XP shards, level-ups and the upgrade choices: general bonuses, levels of the run's mastery
//! skills and TQ2's modifiers for them.
use bevy::prelude::*;

use super::abilities::{modifier_effects, Loadout, Modifier};
use super::mastery_data::MasteryBook;
use super::player::{Hero, HeroStats};
use super::{GameState, Health, Pos, Run};
use crate::audio::Audio;
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
}

impl Upgrade {
    pub const ALL: [Upgrade; 9] = [
        Upgrade::Damage,
        Upgrade::AttackSpeed,
        Upgrade::Multishot,
        Upgrade::Pierce,
        Upgrade::MoveSpeed,
        Upgrade::Area,
        Upgrade::Crit,
        Upgrade::Vitality,
        Upgrade::Magnet,
    ];

    /// Lower-case variant name, e.g. "multishot" (used by test hooks).
    pub fn from_name(name: &str) -> Option<Upgrade> {
        Upgrade::ALL.into_iter().find(|u| format!("{u:?}").eq_ignore_ascii_case(name))
    }

    pub fn title(self) -> String {
        match self {
            Upgrade::Damage => "Might of the Titans: +20 % damage".into(),
            Upgrade::AttackSpeed => "Quick Hands: +15 % attack speed".into(),
            Upgrade::Multishot => "Forge Fire: +1 projectile".into(),
            Upgrade::Pierce => "Piercing: projectiles hit +1 enemy".into(),
            Upgrade::MoveSpeed => "Swift Feet: +10 % movement speed".into(),
            Upgrade::Area => "Wide Reach: +20 % area size".into(),
            Upgrade::Crit => "Keen Eye: +8 % critical chance".into(),
            Upgrade::Vitality => "Vitality: +20 max health, full heal".into(),
            Upgrade::Magnet => "Attraction: +50 % pickup radius".into(),
        }
    }

    fn available(self, stats: &HeroStats) -> bool {
        match self {
            Upgrade::Multishot => stats.projectiles < 8,
            Upgrade::Pierce => stats.pierce < 5,
            Upgrade::Crit => stats.crit < 0.5,
            Upgrade::AttackSpeed => stats.cooldown > 0.3,
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
        }
    }
}

/// One level-up choice.
#[derive(Clone)]
pub enum Choice {
    Stat(Upgrade),
    /// +1 level for the skill in this loadout slot.
    SkillLevel(usize),
    /// A TQ2 modifier for the skill in this loadout slot.
    Modifier(usize, Modifier),
}

/// Skill levels reachable in a run.
const MAX_RUN_SKILL_LEVEL: u32 = 10;

impl Choice {
    pub fn title(&self, loadout: &Loadout) -> String {
        match self {
            Choice::Stat(u) => u.title(),
            Choice::SkillLevel(i) => {
                let s = &loadout.0[*i];
                format!("{} level {}: +25 % damage", s.name, s.level + 1)
            }
            Choice::Modifier(i, m) => {
                let effects: Vec<&str> = m.effects.iter().map(|e| e.text()).collect();
                format!("{} – {}: {}", loadout.0[*i].name, m.name, effects.join(", "))
            }
        }
    }

    /// TQ2 icon (below the asset root): the skill's or modifier's icon, or a stat icon.
    pub fn icon(&self, loadout: &Loadout, book: &MasteryBook) -> Option<String> {
        use super::mastery_data::{texture, ui_texture};
        let skill = |i: &usize| book.active(loadout.0[*i].spec.id).map(|(_, _, s)| s);
        match self {
            Choice::Stat(u) => Some(ui_texture(match u {
                Upgrade::Damage => "t_icon_mod_increase-damage-weapon01",
                Upgrade::AttackSpeed => "t_icon_mod__increase-attack-speed01",
                Upgrade::Multishot => "t_icon_mod_increase-damage-projectile01",
                Upgrade::Pierce => "t_icon_attribute_cunning01",
                Upgrade::MoveSpeed => "t_icon_attribute_agility01",
                Upgrade::Area => "t_icon_simple_totaldamage01",
                Upgrade::Crit => "t_icon_simple_critchance01",
                Upgrade::Vitality => "t_icon_mod_healthincrease01",
                Upgrade::Magnet => "t_icon_simple_ailmentchance01",
            })),
            Choice::SkillLevel(i) => skill(i).and_then(|s| s.icon.as_deref()).map(texture),
            Choice::Modifier(i, m) => {
                let s = skill(i)?;
                book.modifiers(s).find(|x| x.name == m.name).and_then(|x| x.icon.as_deref()).or(s.icon.as_deref()).map(texture)
            }
        }
    }

    /// Whether the choice improves a mastery skill (shown in a skill frame).
    pub fn is_skill(&self) -> bool {
        !matches!(self, Choice::Stat(_))
    }

    pub fn apply(&self, stats: &mut HeroStats, hp: &mut Health, loadout: &mut Loadout) {
        match self {
            Choice::Stat(u) => u.apply(stats, hp),
            Choice::SkillLevel(i) => loadout.0[*i].level += 1,
            Choice::Modifier(i, m) => loadout.0[*i].mods.push(m.clone()),
        }
    }
}

/// The three choices offered at the current level-up.
#[derive(Resource, Default)]
pub struct Offer(pub Vec<Choice>);

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
    audio: Res<Audio>,
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
            run.xp += (shard.0 as f32 * stats.xp).round() as u32;
            audio.play(super::content::sound::SHARD, None, 0.35, 0.06);
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
        audio.play(super::content::sound::LEVEL_UP, None, 0.9, 0.0);
    }
}

/// Rolls the three choices; the level-up panel must run after this (see hud.rs). With mastery
/// skills in the loadout, at least one choice improves a skill (a level or a TQ2 modifier).
pub fn make_offer(mut offer: ResMut<Offer>, stats: Res<HeroStats>, run: Res<Run>, loadout: Res<Loadout>, book: Res<MasteryBook>) {
    let mut stat_pool: Vec<Choice> = Upgrade::ALL.into_iter().filter(|u| u.available(&stats)).map(Choice::Stat).collect();
    let mut skill_pool: Vec<Choice> = vec![];
    for (i, slot) in loadout.0.iter().enumerate() {
        if slot.level < MAX_RUN_SKILL_LEVEL {
            skill_pool.push(Choice::SkillLevel(i));
        }
        let Some((_, _, info)) = book.active(slot.spec.id) else { continue };
        for m in book.modifiers(info) {
            let effects = modifier_effects(&m.description);
            if !effects.is_empty() && !slot.has_mod(&m.name) {
                skill_pool.push(Choice::Modifier(i, Modifier { name: m.name.clone(), effects }));
            }
        }
    }
    fastrand::shuffle(&mut stat_pool);
    fastrand::shuffle(&mut skill_pool);
    let mut choices: Vec<Choice> = vec![];
    choices.extend(skill_pool.drain(..skill_pool.len().min(if stat_pool.is_empty() { 3 } else { 2 })));
    choices.extend(stat_pool.into_iter().take(3 - choices.len()));
    fastrand::shuffle(&mut choices);
    info!("level {} offer: {:?}", run.level, choices.iter().map(|c| c.title(&loadout)).collect::<Vec<_>>());
    offer.0 = choices;
}

fn spin_shards(time: Res<Time>, ground: Res<Ground>, mut q: Query<(&Pos, &mut Transform), With<XpShard>>) {
    let t = time.elapsed_secs();
    for (pos, mut tr) in &mut q {
        tr.translation = Vec3::new(pos.0.x, ground.stand(pos.0) + 0.4 + (t * 3.0 + pos.0.x).sin() * 0.1, pos.0.y);
        tr.rotation = Quat::from_rotation_y(t * 1.5 + pos.0.y);
    }
}

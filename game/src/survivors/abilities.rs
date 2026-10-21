//! Active skills of TQ2's masteries, cast automatically during a run. Every TQ2 skill (by its
//! ability id) maps to one of a few survivor-style behaviours (`Shape`) with its own numbers,
//! element and look. Elements apply ailments: burn and poison deal damage over time, cold
//! chills (slows), lightning shocks (more damage taken), physical hits stun.
//!
//! Learned skills come from the profile (skill tree); during a run level-ups raise them and
//! add TQ2's modifiers, whose effects are read from their descriptions (`ModEffect`).
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use super::combat::{roll_damage, Hit};
use super::enemies::{Enemy, EnemyGrid, EnemySet};
use super::equipment::Equipment;
use super::player::{Hero, HeroStats};
use super::{GameState, Health, Pos, RunEntity};
use crate::audio::Audio;
use crate::fx::{Element, Fx, Tex};
use crate::ground::Ground;
use crate::vfx::Flipbook;

/// Where a zone sits.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Place {
    /// Placed at the hero, stays there.
    Hero,
    /// Moves with the hero (auras like Earthquake, Ring of Flame).
    Follow,
    /// At an enemy cluster.
    Target,
}

/// Buff while a skill is active (multipliers are bonuses: 0.3 = +30 %).
#[derive(Clone, Copy, Default, Debug)]
pub struct Buff {
    pub damage: f32,
    pub attack_speed: f32,
    pub armor: f32,
    pub speed: f32,
    /// Fraction of max health lost per second (Dark Pact).
    pub hp_cost: f32,
    /// Area damage around the hero when the buff ends (Valor's falling sword), × skill damage.
    pub finale: f32,
}

#[derive(Clone, Copy, Debug)]
pub enum Shape {
    /// Projectiles at the nearest enemy, fanned out; `explode` > 0 = area damage on impact.
    Volley { count: u32, spread: f32, speed: f32, range: f32, pierce: u32, explode: f32 },
    /// Projectiles in all directions.
    Radial { count: u32, speed: f32, range: f32, pierce: u32 },
    /// Hits on enemies after `delay` (lightning from the sky, fire from above).
    Strike { radius: f32, targets: u32, delay: f32, range: f32 },
    /// Blast around the hero.
    Nova { radius: f32 },
    /// Along the line to the nearest enemy.
    Line { length: f32, width: f32 },
    /// An area that deals damage every 0.5 s; `drift` m/s towards enemies.
    Zone { radius: f32, duration: f32, place: Place, drift: f32 },
    /// Random hits in an area around an enemy cluster over `duration`.
    Rain { radius: f32, drops: u32, duration: f32, drop_radius: f32 },
    Boost { duration: f32, buff: Buff },
    /// Companions: shoot (`range` > 4) or fight in melee. Stationary ones stay where placed.
    Summon { count: u32, duration: f32, rate: f32, range: f32, stationary: bool },
    /// Jump to an enemy (cluster) and hit around the landing point; `path` also hits on the way.
    Dash { distance: f32, radius: f32, path: bool },
    /// A heavy hit in an arc towards the nearest enemy.
    Melee { radius: f32, arc: f32 },
    /// Enemies in the area take `bonus` more damage.
    Curse { radius: f32, duration: f32, bonus: f32, at_target: bool },
    /// Jumps from enemy to enemy.
    Chain { jumps: u32, range: f32 },
    /// Orbs circling the hero.
    Orbit { count: u32, radius: f32, duration: f32 },
    /// Mines around an enemy cluster that explode on contact.
    Mines { count: u32, radius: f32, duration: f32 },
}

pub struct SkillSpec {
    /// TQ2 ability id (see masteries.json).
    pub id: &'static str,
    pub element: Element,
    pub shape: Shape,
    pub cooldown: f32,
    /// Damage per hit (per tick for zones). For weapon skills a factor of the weapon's damage.
    pub damage: f32,
    pub weapon: bool,
    /// Chance to apply the element's ailment.
    pub ailment: f32,
}

use Element::*;
use Shape::*;

const fn s(id: &'static str, element: Element, shape: Shape, cooldown: f32, damage: f32, ailment: f32) -> SkillSpec {
    SkillSpec { id, element, shape, cooldown, damage, weapon: false, ailment }
}

const fn w(id: &'static str, element: Element, shape: Shape, cooldown: f32, factor: f32, ailment: f32) -> SkillSpec {
    SkillSpec { id, element, shape, cooldown, damage: factor, weapon: true, ailment }
}

const BUFF: Buff = Buff { damage: 0.0, attack_speed: 0.0, armor: 0.0, speed: 0.0, hp_cost: 0.0, finale: 0.0 };

/// Every playable TQ2 skill.
pub const SKILLS: &[SkillSpec] = &[
    // Storm
    s("bp_bga_player_st_iceshards_p", Cold, Volley { count: 5, spread: 0.16, speed: 24.0, range: 15.0, pierce: 0, explode: 0.0 }, 1.6, 9.0, 0.25),
    s("bp_bga_player_st_calllightning_p", Lightning, Strike { radius: 2.5, targets: 1, delay: 0.25, range: 16.0 }, 1.8, 30.0, 0.4),
    s("bp_bga_player_st_stormNimbus_activate_p", Lightning, Boost { duration: 12.0, buff: Buff { damage: 0.3, ..BUFF } }, 20.0, 0.0, 0.0),
    s("bp_bga_player_st_icenova_p", Cold, Nova { radius: 5.0 }, 5.0, 24.0, 0.8),
    s("bp_bga_player_st_lightningbolt_p", Lightning, Chain { jumps: 4, range: 7.0 }, 2.4, 20.0, 0.35),
    s("bp_bga_player_st_wisp_empower", Lightning, Summon { count: 2, duration: 20.0, rate: 1.2, range: 12.0, stationary: false }, 22.0, 9.0, 0.2),
    s("bp_bga_player_st_cyclone_p", Cold, Zone { radius: 3.0, duration: 5.0, place: Place::Target, drift: 2.5 }, 7.0, 7.0, 0.3),
    s("bp_bga_player_st_blink_p", Lightning, Dash { distance: 7.0, radius: 3.0, path: false }, 6.0, 18.0, 0.6),
    s("bp_bga_player_st_tempest_p", Lightning, Boost { duration: 8.0, buff: Buff { damage: 0.2, speed: 0.2, ..BUFF } }, 20.0, 0.0, 0.0),
    s("bp_bga_player_st_frostbite_p", Cold, Zone { radius: 3.5, duration: 6.0, place: Place::Target, drift: 0.0 }, 8.0, 5.0, 1.0),
    // Earth
    s("bp_bga_player_ea_fissure_p", Physical, Line { length: 10.0, width: 1.8 }, 3.0, 26.0, 0.35),
    s("bp_bga_player_ea_roilingMagma_p", Fire, Volley { count: 1, spread: 0.0, speed: 12.0, range: 16.0, pierce: 0, explode: 3.5 }, 3.5, 36.0, 0.5),
    s("bp_bga_player_ea_earthEnchantment_activate", Fire, Boost { duration: 20.0, buff: Buff { damage: 0.3, ..BUFF } }, 25.0, 0.0, 0.0),
    s("bp_bga_player_ea_flameVolley_p", Fire, Strike { radius: 2.0, targets: 4, delay: 0.5, range: 14.0 }, 3.5, 16.0, 0.3),
    s("bp_bga_player_ea_earthquake_p", Physical, Zone { radius: 6.0, duration: 4.0, place: Place::Follow, drift: 0.0 }, 9.0, 7.0, 0.15),
    s("bp_bga_player_ea_coreDweller_empower", Fire, Summon { count: 1, duration: 25.0, rate: 1.4, range: 2.5, stationary: false }, 26.0, 18.0, 0.3),
    s("bp_bga_player_ea_ignite_p", Fire, Strike { radius: 1.5, targets: 3, delay: 0.0, range: 14.0 }, 3.0, 6.0, 1.0),
    s("bp_bga_player_ea_ringOfFlame", Fire, Zone { radius: 4.5, duration: 8.0, place: Place::Follow, drift: 0.0 }, 12.0, 6.0, 0.2),
    s("bp_bga_player_ea_volcanicEruption_p", Fire, Rain { radius: 5.0, drops: 8, duration: 2.5, drop_radius: 2.2 }, 10.0, 30.0, 0.4),
    s("bp_bga_player_ea_stoneForm_p", Physical, Boost { duration: 5.0, buff: Buff { armor: 0.6, ..BUFF } }, 18.0, 0.0, 0.0),
    // Warfare
    w("bp_bga_player_wf_heavyAttack_melee_p", Physical, Melee { radius: 4.5, arc: 1.4 }, 3.0, 2.6, 0.5),
    w("bp_bga_player_wf_sweepingStrike", Physical, Melee { radius: 4.0, arc: 3.2 }, 2.0, 1.4, 0.15),
    s("bp_bga_player_wf_stomp_p", Physical, Nova { radius: 4.5 }, 4.0, 18.0, 0.6),
    s("bp_bga_player_wf_doomHorn_p", Physical, Curse { radius: 8.0, duration: 6.0, bonus: 0.25, at_target: false }, 12.0, 0.0, 0.0),
    w("bp_bga_player_wf_leapslam", Physical, Dash { distance: 8.0, radius: 3.5, path: false }, 6.0, 2.0, 0.5),
    w("bp_bga_player_wf_barrage_melee_p", Physical, Volley { count: 6, spread: 0.08, speed: 30.0, range: 18.0, pierce: 1, explode: 0.0 }, 3.0, 0.8, 0.1),
    w("bp_bga_player_wf_arrowrain", Physical, Rain { radius: 5.0, drops: 12, duration: 2.0, drop_radius: 1.6 }, 8.0, 0.9, 0.1),
    s("bp_bga_player_wf_battlefieldAwareness_activate_p", Physical, Boost { duration: 15.0, buff: Buff { armor: 0.15, ..BUFF } }, 25.0, 0.0, 0.0),
    s("bp_bga_player_wf_valor_p", Physical, Boost { duration: 8.0, buff: Buff { damage: 0.3, finale: 3.0, ..BUFF } }, 18.0, 20.0, 0.5),
    s("bp_bga_player_wf_warBanner_p", Physical, Boost { duration: 12.0, buff: Buff { damage: 0.2, attack_speed: 0.1, ..BUFF } }, 22.0, 0.0, 0.0),
    // Rogue
    w("bp_bga_player_rg_lethalStrike_p", Physical, Dash { distance: 10.0, radius: 1.6, path: false }, 4.0, 3.0, 0.2),
    s("bp_bga_player_rg_trickShot_p", Physical, Volley { count: 1, spread: 0.0, speed: 30.0, range: 22.0, pierce: 99, explode: 0.0 }, 2.5, 20.0, 0.1),
    s("bp_bga_player_rg_preparation_activate_p", Poison, Boost { duration: 15.0, buff: Buff { damage: 0.2, attack_speed: 0.1, ..BUFF } }, 25.0, 0.0, 0.0),
    s("bp_bga_player_rg_markForDeath_p", Spirit, Curse { radius: 2.5, duration: 8.0, bonus: 0.4, at_target: true }, 8.0, 0.0, 0.0),
    s("bp_bga_player_rg_grenade_p", Fire, Volley { count: 1, spread: 0.0, speed: 14.0, range: 12.0, pierce: 0, explode: 3.0 }, 3.0, 26.0, 0.4),
    s("bp_bga_player_rg_vanish_p", Poison, Boost { duration: 6.0, buff: Buff { damage: 0.3, speed: 0.3, ..BUFF } }, 16.0, 0.0, 0.0),
    s("bp_bga_player_rg_flicker_p", Physical, Dash { distance: 7.0, radius: 1.5, path: true }, 4.0, 18.0, 0.2),
    s("bp_bga_player_rg_shadowClone_summon_p", Spirit, Summon { count: 1, duration: 12.0, rate: 0.8, range: 2.5, stationary: false }, 20.0, 12.0, 0.2),
    s("bp_bga_player_rg_scattershot_p", Poison, Radial { count: 12, speed: 18.0, range: 10.0, pierce: 1 }, 3.0, 10.0, 0.4),
    w("bp_bga_player_rg_bladespin_p", Physical, Nova { radius: 3.5 }, 1.8, 1.2, 0.1),
    // Spirit
    s("bp_bga_player_sp_chthonicPulse", Spirit, Volley { count: 1, spread: 0.0, speed: 7.0, range: 18.0, pierce: 99, explode: 0.0 }, 2.5, 22.0, 0.2),
    w("bp_bga_player_sp_reap_p", Spirit, Melee { radius: 5.0, arc: 1.6 }, 2.0, 1.6, 0.2),
    s("bp_bga_player_sp_eidolonGuard_summon_p", Spirit, Summon { count: 1, duration: 30.0, rate: 1.0, range: 2.5, stationary: false }, 30.0, 14.0, 0.2),
    s("bp_bga_player_sp_deathchillAura_p", Cold, Zone { radius: 6.0, duration: 10.0, place: Place::Follow, drift: 0.0 }, 12.0, 3.0, 0.5),
    s("bp_bga_player_sp_deathwardBeam", Spirit, Line { length: 14.0, width: 1.2 }, 2.5, 26.0, 0.2),
    s("bp_bga_player_sp_keres_sustained_p", Spirit, Summon { count: 3, duration: 15.0, rate: 1.0, range: 10.0, stationary: false }, 20.0, 8.0, 0.2),
    s("bp_bga_player_sp_lich_empower", Spirit, Summon { count: 1, duration: 25.0, rate: 1.5, range: 12.0, stationary: false }, 28.0, 18.0, 0.3),
    s("bp_bga_player_sp_soulVortex", Spirit, Zone { radius: 3.5, duration: 5.0, place: Place::Target, drift: 1.0 }, 8.0, 7.0, 0.2),
    s("bp_bga_player_sp_darkPact_activate_p", Spirit, Boost { duration: 10.0, buff: Buff { damage: 0.35, hp_cost: 0.01, ..BUFF } }, 20.0, 0.0, 0.0),
    s("bp_bga_player_sp_deathNova", Spirit, Nova { radius: 7.0 }, 9.0, 40.0, 0.3),
    // Forge
    s("bp_bga_player_fo_bladeTrap_p", Physical, Zone { radius: 3.0, duration: 10.0, place: Place::Target, drift: 0.0 }, 10.0, 8.0, 0.1),
    s("bp_bga_player_fo_forgeCall", Physical, Orbit { count: 3, radius: 2.5, duration: 8.0 }, 10.0, 12.0, 0.2),
    s("bp_bga_player_fo_minefield_p", Fire, Mines { count: 5, radius: 2.2, duration: 12.0 }, 9.0, 25.0, 0.3),
    s("bp_bga_player_fo_chains", Physical, Line { length: 10.0, width: 1.6 }, 4.0, 15.0, 0.7),
    s("bp_bga_player_fo_sentry_p", Physical, Summon { count: 1, duration: 15.0, rate: 0.7, range: 14.0, stationary: true }, 14.0, 9.0, 0.1),
    s("bp_bga_player_fo_armorEruption", Physical, Radial { count: 10, speed: 20.0, range: 10.0, pierce: 1 }, 8.0, 20.0, 0.3),
    s("bp_bga_player_fo_metalWelding_p", Fire, Boost { duration: 12.0, buff: Buff { damage: 0.3, ..BUFF } }, 20.0, 0.0, 0.0),
    s("bp_bga_player_fo_ironclad_activate_p", Physical, Boost { duration: 15.0, buff: Buff { armor: 0.25, ..BUFF } }, 22.0, 0.0, 0.0),
    s("bp_bga_player_fo_automaton_empower", Fire, Summon { count: 1, duration: 30.0, rate: 1.2, range: 12.0, stationary: false }, 30.0, 15.0, 0.3),
    s("bp_bga_player_fo_forceField", Cold, Zone { radius: 4.0, duration: 6.0, place: Place::Hero, drift: 0.0 }, 14.0, 2.0, 1.0),
    s("bp_bga_player_fo_temperedArsenal_activate_p", Physical, Boost { duration: 15.0, buff: Buff { armor: 0.15, damage: 0.15, ..BUFF } }, 25.0, 0.0, 0.0),
];

pub fn spec(id: &str) -> Option<&'static SkillSpec> {
    SKILLS.iter().find(|s| s.id == id)
}

/// Effects a TQ2 modifier can have in this game (read from its description).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ModEffect {
    Damage,
    Count,
    Area,
    Cooldown,
    Duration,
    Ailment,
    Pierce,
}

impl ModEffect {
    pub fn text(self) -> &'static str {
        match self {
            ModEffect::Damage => "+30 % damage",
            ModEffect::Count => "+1 projectile / hit",
            ModEffect::Area => "+25 % area",
            ModEffect::Cooldown => "20 % shorter cooldown",
            ModEffect::Duration => "+30 % duration",
            ModEffect::Ailment => "+25 % ailment chance",
            ModEffect::Pierce => "+2 pierce",
        }
    }
}

/// What a modifier does here, from the words of its TQ2 description.
pub fn modifier_effects(description: &str) -> Vec<ModEffect> {
    // Placeholder names ({AreaCountMod}) are not words of the text; "+{" marks a count.
    let mut d = String::new();
    let mut depth = 0;
    for c in description.to_ascii_lowercase().chars() {
        match c {
            '{' => {
                depth += 1;
                d.push('{');
            }
            '}' => depth -= 1,
            _ if depth == 0 => d.push(c),
            _ => {}
        }
    }
    let has = |s: &str| d.contains(s);
    let mut out = vec![];
    if has("more damage") || has("increased damage") || has("additional") && has("damage") {
        out.push(ModEffect::Damage);
    }
    let counted = ["projectile", "shard", "fissure", "mine", "orb", "wisp", "arrow", "wave", "jump", "chain", "bounce", "fragment", "bolt", "dagger"];
    // "Projectile Penetration" is about piercing, not about more projectiles.
    if (has("+{") || has("additional") || has("more ")) && counted.iter().any(|w| has(w)) && !has("penetrat") {
        out.push(ModEffect::Count);
    }
    if has("radius") || has("area") || has("larger") || has("length") {
        out.push(ModEffect::Area);
    }
    if has("cooldown") || has("charge") || has("cast speed") || has("refund") {
        out.push(ModEffect::Cooldown);
    }
    if has("duration") || has("lasts") {
        out.push(ModEffect::Duration);
    }
    if has("ailment") || has("stun") || has("chill") || has("freez") || has("ignite") || has("burn") || has("shock") || has("poison") {
        out.push(ModEffect::Ailment);
    }
    if has("penetrat") || has("pierce") {
        out.push(ModEffect::Pierce);
    }
    out.dedup();
    out
}

/// A modifier picked during the run.
#[derive(Clone)]
pub struct Modifier {
    pub name: String,
    pub effects: Vec<ModEffect>,
}

/// A skill in this run's loadout.
pub struct Slot {
    pub spec: &'static SkillSpec,
    pub name: String,
    pub level: u32,
    pub mods: Vec<Modifier>,
    timer: f32,
}

impl Slot {
    pub fn new(spec: &'static SkillSpec, name: String, level: u32) -> Self {
        // Stagger the first casts so not everything fires at once.
        Self { spec, name, level: level.max(1), mods: vec![], timer: 0.5 + fastrand::f32() }
    }

    fn n(&self, e: ModEffect) -> u32 {
        self.mods.iter().flat_map(|m| m.effects.iter()).filter(|x| **x == e).count() as u32
    }

    pub fn has_mod(&self, name: &str) -> bool {
        self.mods.iter().any(|m| m.name == name)
    }

    fn area(&self) -> f32 {
        1.0 + 0.25 * self.n(ModEffect::Area) as f32
    }

    fn duration(&self) -> f32 {
        1.0 + 0.3 * self.n(ModEffect::Duration) as f32
    }

    fn extra(&self) -> u32 {
        self.n(ModEffect::Count)
    }

    fn ailment(&self, stats: &HeroStats) -> f32 {
        self.spec.ailment * (1.0 + 0.25 * self.n(ModEffect::Ailment) as f32 + stats.ailment)
    }

    fn cooldown(&self, stats: &HeroStats) -> f32 {
        self.spec.cooldown * 0.8f32.powi(self.n(ModEffect::Cooldown) as i32) * stats.skill_cooldown
    }

    /// Damage per hit before crits.
    pub fn damage(&self, stats: &HeroStats, eq: &Equipment, buffs: &Buffs) -> f32 {
        let level = 1.0 + 0.25 * (self.level - 1) as f32;
        let mods = 1.0 + 0.3 * self.n(ModEffect::Damage) as f32;
        let base = if self.spec.weapon { self.spec.damage * eq.main.damage * stats.damage } else { self.spec.damage * stats.skill_damage * eq.skill_damage() };
        base * level * mods * buffs.damage()
    }

    /// Values for the skill's TQ2 description placeholders.
    pub fn values(&self, stats: &HeroStats, eq: &Equipment) -> impl Fn(&str) -> Option<f32> + use<> {
        let dmg = self.damage(stats, eq, &Buffs::default());
        let area = self.area();
        let dur = self.duration();
        let extra = self.extra() as f32;
        let shape = self.spec.shape;
        let weapon = self.spec.weapon.then_some(self.spec.damage * 100.0);
        let chance = self.ailment(&HeroStats::default()) * 100.0;
        let ailment_time = ailment_duration(self.spec.element);
        move |key: &str| {
            let k = key.to_ascii_lowercase();
            if k.contains("weapondmg") || k.contains("damagefactor") {
                return weapon;
            }
            if k.contains("chance") {
                return Some(chance.min(100.0));
            }
            if ["freeze", "stun", "chill", "ignite", "burn", "shock"].iter().any(|a| k.contains(a)) && k.contains("duration") {
                return Some(ailment_time);
            }
            if k.contains("mindmg") {
                return Some(dmg * 0.85);
            }
            if k.contains("maxdmg") {
                return Some(dmg * 1.15);
            }
            if k.contains("dmg") || k.contains("damage") {
                return Some(dmg);
            }
            let radius = match shape {
                Strike { radius, .. } | Nova { radius } | Zone { radius, .. } | Rain { radius, .. } | Curse { radius, .. } | Mines { radius, .. } => Some(radius),
                Dash { radius, .. } | Melee { radius, .. } | Orbit { radius, .. } => Some(radius),
                Volley { explode, .. } if explode > 0.0 => Some(explode),
                _ => None,
            };
            if k.contains("radius") {
                return radius.map(|r| r * area);
            }
            if k.contains("length") {
                if let Line { length, .. } = shape {
                    return Some(length * area);
                }
            }
            if k.contains("duration") {
                return match shape {
                    Zone { duration, .. } | Rain { duration, .. } | Boost { duration, .. } | Summon { duration, .. } | Curse { duration, .. } | Orbit { duration, .. } | Mines { duration, .. } => Some(duration * dur),
                    _ => None,
                };
            }
            if k.contains("projcount") || k.contains("projamount") || k.contains("amount") {
                return match shape {
                    Volley { count, .. } | Radial { count, .. } | Orbit { count, .. } | Mines { count, .. } | Summon { count, .. } => Some(count as f32 + extra),
                    _ => None,
                };
            }
            None
        }
    }
}

/// The run's skills (built from the profile at run start).
#[derive(Resource, Default)]
pub struct Loadout(pub Vec<Slot>);

/// Active buffs from skills.
#[derive(Resource, Default)]
pub struct Buffs(Vec<(Buff, f32, f32)>);

impl Buffs {
    pub fn damage(&self) -> f32 {
        1.0 + self.0.iter().map(|(b, _, _)| b.damage).sum::<f32>()
    }
    pub fn attack_speed(&self) -> f32 {
        1.0 + self.0.iter().map(|(b, _, _)| b.attack_speed).sum::<f32>()
    }
    pub fn armor(&self) -> f32 {
        self.0.iter().map(|(b, _, _)| b.armor).sum()
    }
    pub fn speed(&self) -> f32 {
        1.0 + self.0.iter().map(|(b, _, _)| b.speed).sum::<f32>()
    }
}

/// Ailments on an enemy.
#[derive(Component, Default)]
pub struct Ailments {
    /// Damage per second of burn/poison.
    burn: f32,
    burn_t: f32,
    chill_t: f32,
    stun_t: f32,
    shock_t: f32,
    curse: f32,
    curse_t: f32,
    tick: f32,
}

impl Ailments {
    /// Multiplier for damage this enemy takes.
    pub fn taken(&self) -> f32 {
        1.0 + if self.shock_t > 0.0 { 0.15 } else { 0.0 } + if self.curse_t > 0.0 { self.curse } else { 0.0 }
    }

    /// Movement multiplier (stunned enemies stand still, chilled ones are slowed).
    pub fn speed(&self) -> f32 {
        if self.stun_t > 0.0 {
            0.0
        } else if self.chill_t > 0.0 {
            0.45
        } else {
            1.0
        }
    }

    pub fn stunned(&self) -> bool {
        self.stun_t > 0.0
    }

    fn afflict(&mut self, el: Element, damage: f32, chance: f32) {
        if fastrand::f32() >= chance {
            return;
        }
        let t = ailment_duration(el);
        match el {
            Fire => {
                self.burn = self.burn.max(damage * 0.5);
                self.burn_t = t;
            }
            Poison => {
                self.burn = self.burn.max(damage * 0.35);
                self.burn_t = t;
            }
            Cold => self.chill_t = t,
            Lightning => self.shock_t = t,
            Physical => self.stun_t = t,
            Spirit => {
                self.curse = self.curse.max(0.15);
                self.curse_t = t;
            }
        }
    }
}

/// How long an element's ailment lasts (seconds).
fn ailment_duration(el: Element) -> f32 {
    match el {
        Fire => 3.0,
        Poison => 5.0,
        Cold => 2.5,
        Lightning | Spirit => 3.0,
        Physical => 0.8,
    }
}

/// Deals skill damage to one enemy: crits, ailment multipliers, damage number, ailment.
fn deal(hp: &mut Health, ail: &mut Ailments, base: f32, el: Element, chance: f32, at: Vec2, stats: &HeroStats, hits: &mut MessageWriter<Hit>) {
    let (amount, crit) = roll_damage(base, stats);
    let amount = amount * ail.taken();
    hp.hp -= amount;
    hits.write(Hit { at, amount, crit });
    ail.afflict(el, base, chance);
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Loadout>()
        .init_resource::<Buffs>()
        .add_systems(
            FixedUpdate,
            (cast_skills, move_shots, land_strikes, tick_zones, run_rains, run_pets, run_orbs, run_mines, tick_buffs, tick_ailments)
                .chain()
                .after(EnemySet)
                .run_if(in_state(GameState::Playing)),
        )
        .add_systems(Update, place_skill_entities);
}

/// Shared system parameters of the skill systems.
#[derive(SystemParam)]
struct Ctx<'w, 's> {
    commands: Commands<'w, 's>,
    fx: Res<'w, Fx>,
    flipbook: Res<'w, Flipbook>,
    ground: Res<'w, Ground>,
    grid: Res<'w, EnemyGrid>,
    audio: Res<'w, Audio>,
    stats: Res<'w, HeroStats>,
    hits: MessageWriter<'w, Hit>,
}

impl Ctx<'_, '_> {
    fn at(&self, p: Vec2, up: f32) -> Vec3 {
        Vec3::new(p.x, self.ground.stand(p) + up, p.y)
    }

    fn nearest(&self, from: Vec2, range: f32) -> Option<Vec2> {
        self.grid.near(from, range).map(|(_, p, _)| p).min_by(|a, b| a.distance_squared(from).total_cmp(&b.distance_squared(from)))
    }

    /// The enemy with the most others close by (sampled).
    fn cluster(&self, from: Vec2, range: f32) -> Option<Vec2> {
        let candidates: Vec<Vec2> = self.grid.near(from, range).map(|(_, p, _)| p).take(16).collect();
        candidates.iter().copied().max_by_key(|c| self.grid.near(*c, 3.0).count())
    }

    /// Damages every enemy within `radius` of `center`.
    fn area(&mut self, enemies: &mut Query<(&mut Health, &mut Ailments), With<Enemy>>, center: Vec2, radius: f32, dmg: f32, el: Element, chance: f32) -> usize {
        let targets: Vec<(Entity, Vec2)> = self.grid.near(center, radius).map(|(e, p, _)| (e, p)).collect();
        for (e, p) in &targets {
            if let Ok((mut hp, mut ail)) = enemies.get_mut(*e) {
                deal(&mut hp, &mut ail, dmg, el, chance, *p, &self.stats, &mut self.hits);
            }
        }
        targets.len()
    }

    fn sound(&self, el: Element, at: Vec3) {
        let event = match el {
            Fire => "event:/weapons/hit/sfx_weapon_hit_staff_projectile_fire",
            Cold => "event:/weapons/hit/sfx_weapon_hit_staff_projectile_cold",
            Lightning => "event:/weapons/hit/sfx_weapon_hit_staff_projectile_lightning",
            Poison => "event:/weapons/hit/sfx_weapon_hit_staff_projectile_poison",
            Spirit => "event:/weapons/hit/sfx_weapon_hit_staff_projectile_spirit_divine",
            Physical => "event:/weapons/hit/sfx_weapon_hit_axe2h",
        };
        self.audio.play(event, Some(at), 0.5, 0.06);
    }

    /// Impact look of an element at a ground point.
    fn impact(&mut self, el: Element, at: Vec3, radius: f32) {
        let decal = match el {
            Lightning => Tex::Scorch,
            Poison => Tex::PoisonPool,
            Physical => Tex::Cracks,
            Cold => Tex::Burst,
            Spirit | Fire => Tex::Burst,
        };
        self.commands.spawn((self.fx.decal(decal, el, at, radius, 0.7), RunEntity));
        match el {
            Fire => {
                self.commands.spawn((self.flipbook.explosion(at + Vec3::Y * 0.8, radius * 0.9), RunEntity));
            }
            Lightning => {
                self.commands.spawn((self.fx.bolt(el, at, 9.0), RunEntity));
                self.commands.spawn((self.fx.flash(Tex::Spark, el, at + Vec3::Y, radius, 0.3), RunEntity));
            }
            _ => {
                self.commands.spawn((self.fx.flash(Tex::Glow, el, at + Vec3::Y * 0.8, radius * 1.2, 0.35), RunEntity));
            }
        }
        self.sound(el, at);
    }

    /// A projectile body for an element.
    fn projectile_look(&mut self, e: Entity, el: Element, size: f32) {
        let fx = &self.fx;
        let flipbook = &self.flipbook;
        self.commands.entity(e).insert((Transform::default(), Visibility::default())).with_children(|p| {
            match el {
                Fire => {
                    p.spawn(flipbook.fireball(size * 1.4));
                }
                Cold => {
                    p.spawn(fx.glow(Tex::Snowflake, el, size));
                    p.spawn(fx.glow(Tex::Glow, el, size * 1.4));
                }
                Lightning => {
                    p.spawn(fx.glow(Tex::Spark, el, size * 1.3));
                }
                Poison => {
                    p.spawn(fx.glow(Tex::Puff, el, size * 1.3));
                }
                Physical => {
                    p.spawn(fx.glow(Tex::Spark, el, size));
                }
                Spirit => {
                    p.spawn(fx.glow(Tex::Glow, el, size * 1.6));
                }
            }
            if matches!(el, Fire | Lightning | Spirit) {
                p.spawn(PointLight { color: el.light(), intensity: 1.5e5, range: 5.0, ..default() });
            }
        });
    }
}

/// A skill projectile.
#[derive(Component)]
struct Shot {
    vel: Vec2,
    life: f32,
    damage: f32,
    el: Element,
    chance: f32,
    pierce: u32,
    explode: f32,
    hit: Vec<Entity>,
}

/// A hit that lands after a delay (lightning, fire from above, rain drops).
#[derive(Component)]
struct Pending {
    t: f32,
    radius: f32,
    damage: f32,
    el: Element,
    chance: f32,
}

/// A damaging area ticking every 0.5 s.
#[derive(Component)]
struct ActiveZone {
    radius: f32,
    life: f32,
    tick: f32,
    damage: f32,
    el: Element,
    chance: f32,
    follow: bool,
    drift: f32,
}

#[derive(Component)]
struct RainCloud {
    radius: f32,
    drops: u32,
    every: f32,
    timer: f32,
    drop_radius: f32,
    damage: f32,
    el: Element,
    chance: f32,
}

#[derive(Component)]
struct Pet {
    life: f32,
    rate: f32,
    timer: f32,
    range: f32,
    damage: f32,
    el: Element,
    chance: f32,
    stationary: bool,
    angle: f32,
}

#[derive(Component)]
struct Orb {
    angle: f32,
    radius: f32,
    life: f32,
    damage: f32,
    el: Element,
    chance: f32,
    cooldowns: Vec<(Entity, f32)>,
}

#[derive(Component)]
struct Mine {
    armed: f32,
    life: f32,
    radius: f32,
    damage: f32,
    el: Element,
    chance: f32,
}

/// Height above the ground for a skill entity's visual (0 = flat decal).
#[derive(Component)]
struct Hover(f32);

fn cast_skills(
    mut ctx: Ctx,
    time: Res<Time>,
    mut loadout: ResMut<Loadout>,
    mut buffs: ResMut<Buffs>,
    mut hero: Query<(&mut Pos, &Equipment), With<Hero>>,
    mut enemies: Query<(&mut Health, &mut Ailments), With<Enemy>>,
    obstacles: Res<crate::obstacles::Obstacles>,
) {
    let Ok((mut hero_pos, eq)) = hero.single_mut() else { return };
    let dt = time.delta_secs();
    let stats = ctx.stats.clone();
    for slot in loadout.0.iter_mut() {
        slot.timer -= dt;
        if slot.timer > 0.0 {
            continue;
        }
        let spec = slot.spec;
        let el = spec.element;
        let dmg = slot.damage(&stats, eq, &buffs);
        let chance = slot.ailment(&stats);
        let area = slot.area();
        let me = hero_pos.0;
        // Most skills need an enemy in reach; if none, try again shortly.
        let cast = match spec.shape {
            Volley { count, spread, speed, range, pierce, explode } => ctx.nearest(me, range).map(|t| {
                let aim = (t - me).normalize_or(Vec2::X);
                let n = count + slot.extra();
                for i in 0..n {
                    let dir = Vec2::from_angle((i as f32 - (n as f32 - 1.0) / 2.0) * spread.max(0.12 * (n > 1) as u8 as f32)).rotate(aim);
                    let e = ctx
                        .commands
                        .spawn((
                            Shot { vel: dir * speed, life: range / speed, damage: dmg, el, chance, pierce: pierce + 2 * slot.n(ModEffect::Pierce), explode: explode * area, hit: vec![] },
                            Pos(me + dir * 0.8),
                            Hover(1.2),
                            RunEntity,
                        ))
                        .id();
                    ctx.projectile_look(e, el, if explode > 0.0 { 1.4 } else { 0.9 });
                }
            }),
            Radial { count, speed, range, pierce } => ctx.nearest(me, range).map(|_| {
                let n = count + 2 * slot.extra();
                for i in 0..n {
                    let dir = Vec2::from_angle(i as f32 / n as f32 * std::f32::consts::TAU);
                    let e = ctx
                        .commands
                        .spawn((
                            Shot { vel: dir * speed, life: range / speed, damage: dmg, el, chance, pierce: pierce + 2 * slot.n(ModEffect::Pierce), explode: 0.0, hit: vec![] },
                            Pos(me + dir * 0.8),
                            Hover(1.2),
                            RunEntity,
                        ))
                        .id();
                    ctx.projectile_look(e, el, 0.8);
                }
            }),
            Strike { radius, targets, delay, range } => ctx.cluster(me, range).map(|first| {
                let mut spots = vec![first];
                spots.extend(ctx.grid.near(me, range).map(|(_, p, _)| p).filter(|p| p.distance(first) > radius).take((targets + slot.extra()).saturating_sub(1) as usize));
                for (i, p) in spots.into_iter().enumerate() {
                    ctx.commands.spawn((Pending { t: delay + i as f32 * 0.08, radius: radius * area, damage: dmg, el, chance }, Pos(p), RunEntity));
                }
            }),
            Nova { radius } => ctx.nearest(me, radius * area).map(|_| {
                let r = radius * area;
                let at = ctx.at(me, 0.0);
                ctx.area(&mut enemies, me, r, dmg, el, chance);
                let decal = if el == Physical { Tex::Cracks } else { Tex::Burst };
                ctx.commands.spawn((ctx.fx.decal(decal, el, at, r, 0.6), RunEntity));
                ctx.commands.spawn((ctx.fx.decal(Tex::Ring, el, at, r * 1.05, 0.45), RunEntity));
                ctx.audio.play(super::content::sound::NOVA, Some(at), 0.8, 0.2);
            }),
            Line { length, width } => ctx.nearest(me, length * area).map(|t| {
                let dir = (t - me).normalize_or(Vec2::X);
                let len = length * area;
                let end = me + dir * len;
                let targets: Vec<(Entity, Vec2)> = ctx
                    .grid
                    .near(me + dir * len * 0.5, len * 0.5 + width)
                    .filter(|(_, p, r)| {
                        let along = (*p - me).dot(dir).clamp(0.0, len);
                        (me + dir * along).distance(*p) <= width * 0.5 + r
                    })
                    .map(|(e, p, _)| (e, p))
                    .collect();
                for (e, p) in targets {
                    if let Ok((mut hp, mut ail)) = enemies.get_mut(e) {
                        deal(&mut hp, &mut ail, dmg, el, chance, p, &ctx.stats, &mut ctx.hits);
                    }
                }
                let (a, b) = (ctx.at(me, 0.15), ctx.at(end, 0.15));
                let tex = match el {
                    Physical => Tex::Cracks,
                    Lightning | Spirit => Tex::Branch,
                    _ => Tex::Burst,
                };
                ctx.commands.spawn((ctx.fx.beam(tex, el, a, b, width * 1.4, 0.8), RunEntity));
                ctx.sound(el, b);
            }),
            Zone { radius, duration, place, drift } => {
                let center = match place {
                    Place::Hero | Place::Follow => ctx.nearest(me, radius * area + 4.0).map(|_| me),
                    Place::Target => ctx.cluster(me, 16.0),
                };
                center.map(|c| {
                    let r = radius * area;
                    let life = duration * slot.duration();
                    let tex = match el {
                        Poison => Tex::PoisonPool,
                        Physical => Tex::Cracks,
                        Fire => Tex::Ring,
                        Cold if place == Place::Target => Tex::Vortex,
                        Spirit => Tex::Vortex,
                        _ => Tex::Frost,
                    };
                    let at = ctx.at(c, 0.0);
                    ctx.commands.spawn((
                        ctx.fx.decal(tex, el, at, r, life),
                        ActiveZone { radius: r, life, tick: 0.0, damage: dmg, el, chance, follow: place == Place::Follow, drift },
                        Pos(c),
                        Hover(0.0),
                        RunEntity,
                    ));
                })
            }
            Rain { radius, drops, duration, drop_radius } => ctx.cluster(me, 16.0).map(|c| {
                let n = drops + 2 * slot.extra();
                ctx.commands.spawn((
                    RainCloud { radius: radius * area, drops: n, every: duration / n as f32, timer: 0.0, drop_radius: drop_radius * area, damage: dmg, el, chance },
                    Pos(c),
                    RunEntity,
                ));
            }),
            Boost { duration, buff } => {
                buffs.0.push((buff, duration * slot.duration(), dmg));
                let at = ctx.at(me, 0.0);
                ctx.commands.spawn((ctx.fx.decal(Tex::Ring, el, at, 2.5, 0.8), RunEntity));
                ctx.commands.spawn((ctx.fx.flash(Tex::Glow, el, at + Vec3::Y * 1.2, 3.0, 0.6), RunEntity));
                ctx.audio.play(super::content::sound::PICK, Some(at), 0.6, 0.0);
                Some(())
            }
            Summon { count, duration, rate, range, stationary } => {
                let n = count + slot.extra();
                for i in 0..n {
                    let angle = i as f32 / n as f32 * std::f32::consts::TAU;
                    let at = me + Vec2::from_angle(angle) * 2.0;
                    let e = ctx
                        .commands
                        .spawn((
                            Pet { life: duration * slot.duration(), rate, timer: 0.3, range, damage: dmg, el, chance, stationary, angle },
                            Pos(at),
                            Hover(1.4),
                            RunEntity,
                        ))
                        .id();
                    ctx.projectile_look(e, el, 1.3);
                }
                Some(())
            }
            Dash { distance, radius, path } => ctx.cluster(me, distance * 1.6).map(|t| {
                let dir = (t - me).normalize_or(Vec2::X);
                let land = me + dir * (t.distance(me) - 1.0).clamp(0.0, distance);
                if ctx.ground.walkable(land) && obstacles.resolve(land, 0.45, ctx.ground.stand(land)) == land {
                    if path {
                        let from = ctx.at(me, 1.0);
                        let to = ctx.at(land, 1.0);
                        ctx.commands.spawn((ctx.fx.beam(Tex::Branch, el, from, to, 1.5, 0.4), RunEntity));
                        let mid = (me + land) * 0.5;
                        ctx.area(&mut enemies, mid, me.distance(land) * 0.5 + radius * 0.5, dmg, el, chance);
                    }
                    ctx.commands.spawn((ctx.fx.flash(Tex::Glow, el, ctx.at(me, 1.0), 2.5, 0.3), RunEntity));
                    hero_pos.0 = land;
                }
                let r = radius * area;
                ctx.area(&mut enemies, hero_pos.0, r, dmg, el, chance);
                let at = ctx.at(hero_pos.0, 0.0);
                ctx.impact(el, at, r);
            }),
            Melee { radius, arc } => ctx.nearest(me, radius * area).map(|t| {
                let aim = (t - me).normalize_or(Vec2::X);
                let r = radius * area;
                let half = (arc + 0.3 * slot.extra() as f32).min(std::f32::consts::TAU) / 2.0;
                let targets: Vec<(Entity, Vec2)> = ctx.grid.near(me, r).filter(|(_, p, _)| (*p - me).length_squared() < 0.25 || aim.angle_to(*p - me).abs() <= half).map(|(e, p, _)| (e, p)).collect();
                for (e, p) in targets {
                    if let Ok((mut hp, mut ail)) = enemies.get_mut(e) {
                        deal(&mut hp, &mut ail, dmg, el, chance, p, &ctx.stats, &mut ctx.hits);
                    }
                }
                let at = ctx.at(me + aim * r * 0.5, 0.0);
                ctx.commands.spawn((ctx.fx.decal(Tex::Burst, el, at, r * 0.6, 0.35), RunEntity));
                ctx.commands.spawn((ctx.fx.flash(Tex::Spark, el, at + Vec3::Y, r * 0.6, 0.25), RunEntity));
                ctx.sound(el, at);
            }),
            Curse { radius, duration, bonus, at_target } => {
                let center = if at_target { ctx.cluster(me, 16.0) } else { ctx.nearest(me, radius).map(|_| me) };
                center.map(|c| {
                    let r = radius * area;
                    let targets: Vec<Entity> = ctx.grid.near(c, r).map(|(e, _, _)| e).collect();
                    for e in targets {
                        if let Ok((_, mut ail)) = enemies.get_mut(e) {
                            ail.curse = ail.curse.max(bonus);
                            ail.curse_t = duration * slot.duration();
                        }
                    }
                    let at = ctx.at(c, 0.0);
                    ctx.commands.spawn((ctx.fx.decal(Tex::Vortex, Spirit, at, r, 0.9), RunEntity));
                    ctx.audio.play(super::content::sound::BOSS_ARRIVES, Some(at), 0.3, 0.5);
                })
            }
            Chain { jumps, range } => ctx.nearest(me, range * 2.0).map(|first| {
                let mut from = ctx.at(me, 1.2);
                let mut at = first;
                let mut hit: Vec<Entity> = vec![];
                for _ in 0..=(jumps + slot.extra()) {
                    let Some((e, p)) = ctx.grid.near(at, 1.5).filter(|(e, _, _)| !hit.contains(e)).map(|(e, p, _)| (e, p)).next() else { break };
                    let to = ctx.at(p, 1.2);
                    ctx.commands.spawn((ctx.fx.beam(Tex::Branch, el, from, to, 1.2, 0.3), RunEntity));
                    if let Ok((mut hp, mut ail)) = enemies.get_mut(e) {
                        deal(&mut hp, &mut ail, dmg, el, chance, p, &ctx.stats, &mut ctx.hits);
                    }
                    hit.push(e);
                    from = to;
                    let Some(next) = ctx.grid.near(p, range).filter(|(e, _, _)| !hit.contains(e)).map(|(_, q, _)| q).min_by(|a, b| a.distance_squared(p).total_cmp(&b.distance_squared(p))) else { break };
                    at = next;
                }
                ctx.sound(el, from);
            }),
            Orbit { count, radius, duration } => ctx.nearest(me, radius + 6.0).map(|_| {
                let n = count + slot.extra();
                for i in 0..n {
                    let e = ctx
                        .commands
                        .spawn((
                            Orb { angle: i as f32 / n as f32 * std::f32::consts::TAU, radius: radius * area, life: duration * slot.duration(), damage: dmg, el, chance, cooldowns: vec![] },
                            Pos(me),
                            Hover(1.0),
                            RunEntity,
                        ))
                        .id();
                    ctx.projectile_look(e, el, 0.9);
                }
            }),
            Mines { count, radius, duration } => ctx.cluster(me, 14.0).map(|c| {
                let n = count + slot.extra();
                for _ in 0..n {
                    let p = c + Vec2::from_angle(fastrand::f32() * std::f32::consts::TAU) * fastrand::f32() * 3.0;
                    let e = ctx
                        .commands
                        .spawn((Mine { armed: 0.6, life: duration * slot.duration(), radius: radius * area, damage: dmg, el, chance }, Pos(p), Hover(0.3), RunEntity))
                        .id();
                    ctx.projectile_look(e, el, 0.6);
                }
            }),
        };
        if cast.is_some() {
            debug!("cast {} (level {}, {:.0} damage)", slot.name, slot.level, dmg);
        }
        slot.timer = if cast.is_some() { slot.cooldown(&stats) } else { 0.4 };
    }
}

fn move_shots(mut ctx: Ctx, time: Res<Time>, obstacles: Res<crate::obstacles::Obstacles>, mut shots: Query<(Entity, &mut Pos, &mut Shot)>, mut enemies: Query<(&mut Health, &mut Ailments), With<Enemy>>) {
    let dt = time.delta_secs();
    for (e, mut pos, mut shot) in &mut shots {
        pos.0 += shot.vel * dt;
        shot.life -= dt;
        let mut done = shot.life <= 0.0 || obstacles.hits(pos.0, ctx.ground.stand(pos.0) + 1.2);
        if !done {
            let targets: Vec<(Entity, Vec2)> = ctx.grid.near(pos.0, 0.5).filter(|(t, _, _)| !shot.hit.contains(t)).map(|(t, p, _)| (t, p)).collect();
            for (t, p) in targets {
                if shot.explode > 0.0 {
                    done = true;
                    break;
                }
                if let Ok((mut hp, mut ail)) = enemies.get_mut(t) {
                    deal(&mut hp, &mut ail, shot.damage, shot.el, shot.chance, p, &ctx.stats, &mut ctx.hits);
                    shot.hit.push(t);
                    if shot.hit.len() as u32 > shot.pierce {
                        done = true;
                        break;
                    }
                }
            }
        }
        if done {
            if shot.explode > 0.0 {
                ctx.area(&mut enemies, pos.0, shot.explode, shot.damage, shot.el, shot.chance);
                let at = ctx.at(pos.0, 0.0);
                ctx.impact(shot.el, at, shot.explode);
            }
            ctx.commands.entity(e).despawn();
        }
    }
}

fn land_strikes(mut ctx: Ctx, time: Res<Time>, mut pending: Query<(Entity, &Pos, &mut Pending)>, mut enemies: Query<(&mut Health, &mut Ailments), With<Enemy>>) {
    for (e, pos, mut p) in &mut pending {
        p.t -= time.delta_secs();
        if p.t > 0.0 {
            continue;
        }
        ctx.area(&mut enemies, pos.0, p.radius, p.damage, p.el, p.chance);
        let at = ctx.at(pos.0, 0.0);
        ctx.impact(p.el, at, p.radius);
        ctx.commands.entity(e).despawn();
    }
}

fn tick_zones(mut ctx: Ctx, time: Res<Time>, hero: Query<&Pos, (With<Hero>, Without<ActiveZone>)>, mut zones: Query<(Entity, &mut Pos, &mut ActiveZone, &mut Transform)>, mut enemies: Query<(&mut Health, &mut Ailments), With<Enemy>>) {
    let dt = time.delta_secs();
    let hero = hero.single().map(|p| p.0).ok();
    for (e, mut pos, mut zone, mut t) in &mut zones {
        zone.life -= dt;
        if zone.life <= 0.0 {
            ctx.commands.entity(e).despawn();
            continue;
        }
        if zone.follow {
            if let Some(h) = hero {
                pos.0 = h;
            }
        } else if zone.drift > 0.0 {
            if let Some(target) = ctx.nearest(pos.0, 12.0) {
                let step = (target - pos.0).normalize_or_zero() * zone.drift * dt;
                pos.0 += step;
            }
        }
        t.rotate_y(dt * 1.5);
        zone.tick -= dt;
        if zone.tick <= 0.0 {
            zone.tick = 0.5;
            let (r, d, el, c) = (zone.radius, zone.damage, zone.el, zone.chance);
            ctx.area(&mut enemies, pos.0, r, d, el, c);
        }
    }
}

fn run_rains(mut ctx: Ctx, time: Res<Time>, mut rains: Query<(Entity, &Pos, &mut RainCloud)>) {
    for (e, pos, mut rain) in &mut rains {
        rain.timer -= time.delta_secs();
        if rain.timer > 0.0 {
            continue;
        }
        rain.timer = rain.every;
        let p = pos.0 + Vec2::from_angle(fastrand::f32() * std::f32::consts::TAU) * fastrand::f32().sqrt() * rain.radius;
        ctx.commands.spawn((Pending { t: 0.15, radius: rain.drop_radius, damage: rain.damage, el: rain.el, chance: rain.chance }, Pos(p), RunEntity));
        rain.drops -= 1;
        if rain.drops == 0 {
            ctx.commands.entity(e).despawn();
        }
    }
}

fn run_pets(
    mut ctx: Ctx,
    time: Res<Time>,
    hero: Query<&Pos, (With<Hero>, Without<Pet>)>,
    mut pets: Query<(Entity, &mut Pos, &mut Pet)>,
    mut enemies: Query<(&mut Health, &mut Ailments), With<Enemy>>,
) {
    let dt = time.delta_secs();
    let Ok(hero) = hero.single() else { return };
    for (e, mut pos, mut pet) in &mut pets {
        pet.life -= dt;
        if pet.life <= 0.0 {
            ctx.commands.entity(e).despawn();
            continue;
        }
        let melee = pet.range <= 4.0;
        if !pet.stationary {
            // Ranged pets circle the hero, melee pets charge the nearest enemy close to the hero.
            pet.angle += dt * 0.8;
            let home = hero.0 + Vec2::from_angle(pet.angle) * 2.2;
            let goal = if melee { ctx.nearest(hero.0, 9.0).unwrap_or(home) } else { home };
            let step = (goal - pos.0).clamp_length_max(9.0 * dt);
            pos.0 += step;
        }
        pet.timer -= dt;
        if pet.timer > 0.0 {
            continue;
        }
        let Some(target) = ctx.nearest(pos.0, pet.range) else { continue };
        pet.timer = pet.rate;
        if melee {
            let (d, el, c) = (pet.damage, pet.el, pet.chance);
            ctx.area(&mut enemies, target, 1.5, d, el, c);
            let at = ctx.at(target, 1.0);
            ctx.commands.spawn((ctx.fx.flash(Tex::Spark, pet.el, at, 1.6, 0.25), RunEntity));
        } else {
            let dir = (target - pos.0).normalize_or(Vec2::X);
            let shot = ctx
                .commands
                .spawn((
                    Shot { vel: dir * 20.0, life: pet.range / 20.0, damage: pet.damage, el: pet.el, chance: pet.chance, pierce: 0, explode: 0.0, hit: vec![] },
                    Pos(pos.0),
                    Hover(1.4),
                    RunEntity,
                ))
                .id();
            ctx.projectile_look(shot, pet.el, 0.6);
        }
    }
}

fn run_orbs(mut ctx: Ctx, time: Res<Time>, hero: Query<&Pos, (With<Hero>, Without<Orb>)>, mut orbs: Query<(Entity, &mut Pos, &mut Orb)>, mut enemies: Query<(&mut Health, &mut Ailments), With<Enemy>>) {
    let dt = time.delta_secs();
    let Ok(hero) = hero.single() else { return };
    for (e, mut pos, mut orb) in &mut orbs {
        orb.life -= dt;
        if orb.life <= 0.0 {
            ctx.commands.entity(e).despawn();
            continue;
        }
        orb.angle += dt * 3.0;
        pos.0 = hero.0 + Vec2::from_angle(orb.angle) * orb.radius;
        orb.cooldowns.retain_mut(|(_, t)| {
            *t -= dt;
            *t > 0.0
        });
        let targets: Vec<(Entity, Vec2)> = ctx.grid.near(pos.0, 0.7).map(|(t, p, _)| (t, p)).filter(|(t, _)| !orb.cooldowns.iter().any(|(c, _)| c == t)).collect();
        for (t, p) in targets {
            if let Ok((mut hp, mut ail)) = enemies.get_mut(t) {
                deal(&mut hp, &mut ail, orb.damage, orb.el, orb.chance, p, &ctx.stats, &mut ctx.hits);
                orb.cooldowns.push((t, 0.5));
            }
        }
    }
}

fn run_mines(mut ctx: Ctx, time: Res<Time>, mut mines: Query<(Entity, &Pos, &mut Mine)>, mut enemies: Query<(&mut Health, &mut Ailments), With<Enemy>>) {
    let dt = time.delta_secs();
    for (e, pos, mut mine) in &mut mines {
        mine.life -= dt;
        mine.armed -= dt;
        let triggered = mine.armed <= 0.0 && ctx.grid.near(pos.0, 1.0).next().is_some();
        if triggered {
            let (r, d, el, c) = (mine.radius, mine.damage, mine.el, mine.chance);
            ctx.area(&mut enemies, pos.0, r, d, el, c);
            let at = ctx.at(pos.0, 0.0);
            ctx.impact(el, at, r);
        }
        if triggered || mine.life <= 0.0 {
            ctx.commands.entity(e).despawn();
        }
    }
}

/// Buff timers, Dark Pact's health cost, Valor's finale and health regeneration.
fn tick_buffs(mut ctx: Ctx, time: Res<Time>, mut buffs: ResMut<Buffs>, mut hero: Query<(&Pos, &mut Health), (With<Hero>, Without<Enemy>)>, mut enemies: Query<(&mut Health, &mut Ailments), With<Enemy>>) {
    let dt = time.delta_secs();
    let Ok((pos, mut hp)) = hero.single_mut() else { return };
    hp.hp = (hp.hp + ctx.stats.regen * dt).min(hp.max);
    let mut finales = vec![];
    buffs.0.retain_mut(|(b, t, dmg)| {
        *t -= dt;
        if b.hp_cost > 0.0 {
            hp.hp = (hp.hp - b.hp_cost * hp.max * dt).max(1.0);
        }
        if *t <= 0.0 && b.finale > 0.0 {
            finales.push(*dmg * b.finale);
        }
        *t > 0.0
    });
    for dmg in finales {
        ctx.area(&mut enemies, pos.0, 5.0, dmg, Physical, 0.5);
        let at = ctx.at(pos.0, 0.0);
        ctx.impact(Physical, at, 5.0);
    }
}

/// Burn and poison damage, ailment timers.
fn tick_ailments(time: Res<Time>, mut enemies: Query<(&mut Health, &mut Ailments), With<Enemy>>) {
    let dt = time.delta_secs();
    for (mut hp, mut ail) in &mut enemies {
        ail.burn_t -= dt;
        ail.chill_t -= dt;
        ail.stun_t -= dt;
        ail.shock_t -= dt;
        ail.curse_t -= dt;
        if ail.burn_t > 0.0 {
            ail.tick -= dt;
            if ail.tick <= 0.0 {
                ail.tick = 0.5;
                hp.hp -= ail.burn * 0.5 * ail.taken();
            }
        } else {
            ail.burn = 0.0;
        }
    }
}

/// Puts projectiles, pets, orbs and mines at their ground position.
fn place_skill_entities(ground: Res<Ground>, mut q: Query<(&Pos, &Hover, &mut Transform)>) {
    for (pos, hover, mut t) in &mut q {
        t.translation = Vec3::new(pos.0.x, ground.stand(pos.0) + hover.0 + if hover.0 > 0.0 { 0.0 } else { 0.1 }, pos.0.y);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modifiers_are_read_from_their_text() {
        assert_eq!(modifier_effects("Create <b>+{AreaCountMod}|round()</> fissures in a cone."), vec![ModEffect::Count]);
        let e = modifier_effects("<b>{DmgMod_UI}|round()%</> more Damage.\\r\\n<b>{StunChance_UI}|round()%</> chance to <me_Stun>Stun</>");
        assert!(e.contains(&ModEffect::Damage) && e.contains(&ModEffect::Ailment));
    }

    #[test]
    fn skill_ids_are_unique() {
        for (i, a) in SKILLS.iter().enumerate() {
            assert!(SKILLS[i + 1..].iter().all(|b| b.id != a.id), "{}", a.id);
        }
    }
}

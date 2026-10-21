//! TQ2's mastery rules between runs: choose two of six masteries (the second once the first
//! reaches `SECOND_MASTERY_LEVEL`), spend active points on skills and passive points on
//! passives; points spent in a mastery are its level and open its tiers.
//!
//! Passives work through their original description lines: every "{XCurrent}% increased
//! Spell Damage" line becomes a per-level bonus to the matching game stat (`Stat`), and the
//! same numbers fill the description, so what the text says is what the passive does.
use super::abilities;
use super::mastery_data::{MasteryBook, PassiveInfo};
use super::player::HeroStats;
use super::profile::Profile;
use super::Health;

/// Mastery level needed to open each tier.
pub const TIER_LEVEL: [u32; 5] = [0, 5, 12, 20, 30];
pub const SECOND_MASTERY_LEVEL: u32 = 8;
pub const MAX_MASTERIES: usize = 2;
/// Learned skills that cast themselves during a run.
/// Skills a hero can carry in one run (the spell bar).
pub const MAX_EQUIPPED: usize = 4;
pub const ACTIVE_MAX_LEVEL: u32 = 5;

/// Points spent in a mastery (active levels + passive levels).
pub fn mastery_level(profile: &Profile, book: &MasteryBook, mastery: &str) -> u32 {
    let Some(m) = book.mastery(mastery) else { return 0 };
    m.tiers
        .iter()
        .map(|t| t.actives.iter().map(|a| profile.active_level(&a.id)).sum::<u32>() + t.passives.iter().map(|p| profile.passive_level(&p.id)).sum::<u32>())
        .sum()
}

pub fn tier_open(profile: &Profile, book: &MasteryBook, mastery: &str, tier: usize) -> bool {
    mastery_level(profile, book, mastery) >= TIER_LEVEL[tier.min(TIER_LEVEL.len() - 1)]
}

/// Whether another mastery may be chosen now.
pub fn can_choose_mastery(profile: &Profile, book: &MasteryBook) -> bool {
    match profile.masteries.len() {
        0 => true,
        1 => mastery_level(profile, book, &profile.masteries[0]) >= SECOND_MASTERY_LEVEL,
        _ => false,
    }
}

pub fn choose_mastery(profile: &mut Profile, book: &MasteryBook, id: &str) -> Result<(), String> {
    if profile.masteries.iter().any(|m| m == id) {
        return Err("already chosen".into());
    }
    if !can_choose_mastery(profile, book) {
        return Err(format!("second mastery at mastery level {SECOND_MASTERY_LEVEL}"));
    }
    profile.masteries.push(id.into());
    Ok(())
}

/// Why an active skill cannot be learned / raised now, or None.
pub fn active_blocked(profile: &Profile, book: &MasteryBook, id: &str) -> Option<String> {
    let Some((m, tier, skill)) = book.active(id) else { return Some("unknown skill".into()) };
    if skill.wip || abilities::spec(id).is_none() {
        return Some("not playable yet".into());
    }
    if !profile.masteries.contains(&m.id) {
        return Some(format!("choose the {} mastery", m.name));
    }
    if !tier_open(profile, book, &m.id, tier) {
        return Some(format!("needs mastery level {}", TIER_LEVEL[tier]));
    }
    if profile.active_level(id) >= ACTIVE_MAX_LEVEL {
        return Some("maximum level".into());
    }
    if profile.active_points == 0 {
        return Some("no active points".into());
    }
    None
}

pub fn passive_blocked(profile: &Profile, book: &MasteryBook, id: &str) -> Option<String> {
    let Some((m, tier, passive)) = book.passive(id) else { return Some("unknown passive".into()) };
    if lines(passive).is_empty() {
        return Some("not playable yet".into());
    }
    if !profile.masteries.contains(&m.id) {
        return Some(format!("choose the {} mastery", m.name));
    }
    if !tier_open(profile, book, &m.id, tier) {
        return Some(format!("needs mastery level {}", TIER_LEVEL[tier]));
    }
    if profile.passive_level(id) >= passive.max_level.max(1) {
        return Some("maximum level".into());
    }
    if profile.passive_points == 0 {
        return Some("no passive points".into());
    }
    None
}

/// Learns or raises an active skill. Learned skills are offered on level-ups in runs.
pub fn learn_active(profile: &mut Profile, book: &MasteryBook, id: &str) -> Result<(), String> {
    if let Some(why) = active_blocked(profile, book, id) {
        return Err(why);
    }
    profile.active_points -= 1;
    *profile.actives.entry(id.into()).or_default() += 1;
    Ok(())
}

pub fn learn_passive(profile: &mut Profile, book: &MasteryBook, id: &str) -> Result<(), String> {
    if let Some(why) = passive_blocked(profile, book, id) {
        return Err(why);
    }
    profile.passive_points -= 1;
    *profile.passives.entry(id.into()).or_default() += 1;
    Ok(())
}

/// Gives back every point; with `masteries` also forgets the chosen masteries (full respec).
pub fn refund(profile: &mut Profile, masteries: bool) {
    profile.active_points += profile.actives.values().sum::<u32>();
    profile.passive_points += profile.passives.values().sum::<u32>();
    profile.actives.clear();
    profile.passives.clear();
    profile.equipped.clear();
    if masteries {
        profile.masteries.clear();
    }
}

/// Game stats a passive line can raise.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Stat {
    WeaponDamage,
    SkillDamage,
    AllDamage,
    AttackSpeed,
    CastSpeed,
    Crit,
    MaxHealth,
    Regen,
    Armor,
    MoveSpeed,
    Ailment,
    // TQ2 mechanics (mechanics.rs).
    Barrier,
    Rage,
    Glancing,
    Execution,
    Shade,
    Pickup,
}

impl Stat {
    /// Gain per passive level (percent for most, flat for health and regeneration).
    pub fn per_level(self) -> f32 {
        match self {
            Stat::WeaponDamage | Stat::SkillDamage => 3.0,
            Stat::AllDamage => 2.5,
            Stat::AttackSpeed | Stat::CastSpeed => 2.0,
            Stat::Crit => 1.0,
            Stat::MaxHealth => 6.0,
            Stat::Regen => 0.3,
            Stat::Armor | Stat::MoveSpeed => 1.5,
            Stat::Ailment => 4.0,
            Stat::Barrier => 3.0,
            Stat::Rage => 5.0,
            Stat::Glancing => 2.0,
            Stat::Execution => 1.5,
            Stat::Shade => 10.0,
            Stat::Pickup => 6.0,
        }
    }

    /// Which stat a description line names (most specific first).
    fn detect(line: &str) -> Option<Stat> {
        let l = line.to_ascii_lowercase();
        let has = |s: &str| l.contains(s);
        Some(if has("glancing") {
            Stat::Glancing
        } else if has("barrier") {
            Stat::Barrier
        } else if has("me_rage") {
            Stat::Rage
        } else if has("execution threshold") {
            Stat::Execution
        } else if has("me_shade") {
            Stat::Shade
        } else if has("collection range") {
            Stat::Pickup
        } else if has("me_device") || has("me_activation") {
            Stat::SkillDamage
        } else if has("attack speed") {
            Stat::AttackSpeed
        } else if has("cast speed") || has("cooldown") || has("energy") {
            Stat::CastSpeed
        } else if has("critical") {
            Stat::Crit
        } else if has("regenerat") || has("leech") {
            Stat::Regen
        } else if has("health") || has("life") {
            Stat::MaxHealth
        } else if has("armor") || has("armour") || has("resist") || has("less damage") || has("damage reduction") || has("block") {
            Stat::Armor
        } else if has("movement speed") {
            Stat::MoveSpeed
        } else if has("ailment") || has("stun") || has("chill") || has("freez") || has("ignite") || has("burn") || has("shock") || has("poison chance") {
            Stat::Ailment
        } else if has("spell damage") || has("skill damage") || has("summon") {
            Stat::SkillDamage
        } else if has("weapon damage") || has("attack damage") || has("melee") {
            Stat::WeaponDamage
        } else if has("damage") || has("penetration") {
            Stat::AllDamage
        } else {
            return None;
        })
    }
}

/// One "{XCurrent} … <stat>" line of a passive.
#[derive(Debug)]
pub struct Line {
    pub key: String,
    pub stat: Stat,
}

/// Placeholders of a text with the rest of their line.
fn placeholders(text: &str) -> Vec<(String, String)> {
    let text = text.replace("\\r\\n", "\n").replace("\\n", "\n");
    let mut out = vec![];
    let mut rest = text.as_str();
    while let Some(start) = rest.find('{') {
        let Some(len) = rest[start..].find('}') else { break };
        let name = rest[start + 1..start + len].to_string();
        let after = &rest[start + len + 1..];
        let line_end = after.find('\n').unwrap_or(after.len());
        out.push((name, after[..line_end].to_string()));
        rest = after;
    }
    out
}

/// The stat lines of a passive's description (empty = no effect the game can model). TQ2
/// writes the current level's value as "{XCurrent}" or "{X}" and the next level's as
/// "{XNext}" or "{X2}".
pub fn lines(passive: &PassiveInfo) -> Vec<Line> {
    let found = placeholders(&passive.description);
    let names: Vec<&str> = found.iter().map(|(n, _)| n.as_str()).collect();
    let mut out: Vec<Line> = vec![];
    for (name, after) in &found {
        let key = if matches!(name.as_str(), "CurrentLevel" | "NextLevel") || name.ends_with("Next") {
            continue;
        } else if let Some(k) = name.strip_suffix("Current") {
            k.to_string()
        } else if name.strip_suffix('2').is_some_and(|k| names.contains(&k)) {
            continue;
        } else {
            name.clone()
        };
        if key.is_empty() || out.iter().any(|l| l.key == key) {
            continue;
        }
        // Summon lines strengthen summons, i.e. skills.
        let line = if name.starts_with("Summon") { format!("summon {after}") } else { after.clone() };
        if let Some(stat) = Stat::detect(&line) {
            out.push(Line { key, stat });
        }
    }
    out
}

/// Placeholder values for a passive's description at `level`.
pub fn passive_values(passive: &PassiveInfo, level: u32) -> impl Fn(&str) -> Option<f32> {
    let lines = lines(passive);
    move |key: &str| {
        match key {
            "CurrentLevel" => return Some(level as f32),
            "NextLevel" => return Some(level as f32 + 1.0),
            _ => {}
        }
        let (name, lvl) = if let Some(k) = key.strip_suffix("Current") {
            (k, level)
        } else if let Some(k) = key.strip_suffix("Next") {
            (k, level + 1)
        } else if let Some(k) = key.strip_suffix('2').filter(|k| lines.iter().any(|l| l.key == *k)) {
            (k, level + 1)
        } else {
            (key, level)
        };
        lines.iter().find(|l| l.key == name).map(|l| l.stat.per_level() * lvl as f32)
    }
}

/// Applies all learned passives to a fresh run.
pub fn apply(profile: &Profile, book: &MasteryBook, stats: &mut HeroStats, hp: &mut Health) {
    for (id, &level) in &profile.passives {
        let Some((_, _, passive)) = book.passive(id) else { continue };
        for line in lines(passive) {
            let v = line.stat.per_level() * level as f32;
            match line.stat {
                Stat::WeaponDamage => stats.damage *= 1.0 + v / 100.0,
                Stat::SkillDamage => stats.skill_damage *= 1.0 + v / 100.0,
                Stat::AllDamage => {
                    stats.damage *= 1.0 + v / 100.0;
                    stats.skill_damage *= 1.0 + v / 100.0;
                }
                Stat::AttackSpeed => stats.cooldown /= 1.0 + v / 100.0,
                Stat::CastSpeed => stats.skill_cooldown /= 1.0 + v / 100.0,
                Stat::Crit => stats.crit += v / 100.0,
                Stat::MaxHealth => hp.max += v,
                Stat::Regen => stats.regen += v,
                Stat::Armor => stats.armor += v / 100.0,
                Stat::MoveSpeed => stats.speed *= 1.0 + v / 100.0,
                Stat::Ailment => stats.ailment += v / 100.0,
                Stat::Barrier => stats.barrier += v / 100.0,
                Stat::Rage => stats.rage += v / 100.0,
                Stat::Glancing => stats.glancing = (stats.glancing + v / 100.0).min(0.6),
                Stat::Execution => stats.execute = (stats.execute + v / 100.0).min(0.25),
                Stat::Shade => stats.shade += v / 100.0,
                Stat::Pickup => stats.pickup *= 1.0 + v / 100.0,
            }
        }
    }
    hp.hp = hp.max;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn passive(desc: &str) -> PassiveInfo {
        PassiveInfo { id: "p".into(), name: "P".into(), description: desc.into(), icon: None, max_level: 12, feats: vec![] }
    }

    #[test]
    fn passive_lines_follow_the_description() {
        let p = passive("<b_gold>Level {CurrentLevel}:</>\\n<b>{DmgCurrent}|floor()%</> increased Spell Damage\\n<b>{CSCurrent}|floor()%</> increased Cast Speed\\n\\n<b_gold>Level {NextLevel}:</>\\n<b>{DmgNext}|floor()%</> increased Spell Damage");
        let l = lines(&p);
        assert_eq!(l.len(), 2);
        assert_eq!(l[0].stat, Stat::SkillDamage);
        assert_eq!(l[1].stat, Stat::CastSpeed);
        let v = passive_values(&p, 2);
        assert_eq!(v("DmgCurrent"), Some(6.0));
        assert_eq!(v("DmgNext"), Some(9.0));
        assert_eq!(v("NextLevel"), Some(3.0));
    }

    /// Coverage of the real TQ2 data (needs cache/export): `cargo test -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn coverage_of_tq2_data() {
        let book = MasteryBook::load(&crate::export_root());
        for m in &book.0 {
            let passives: Vec<&PassiveInfo> = m.tiers.iter().flat_map(|t| t.passives.iter()).collect();
            let playable = passives.iter().filter(|p| !lines(p).is_empty()).count();
            let mods: Vec<_> = m.tiers.iter().flat_map(|t| t.modifiers.iter()).collect();
            let with_effects = mods.iter().filter(|x| !abilities::modifier_effects(&x.description).is_empty()).count();
            let actives = m.tiers.iter().flat_map(|t| t.actives.iter()).filter(|a| !a.wip);
            let skills = actives.clone().count();
            let specced = actives.filter(|a| abilities::spec(&a.id).is_some()).count();
            println!("{}: skills {specced}/{skills}, passives {playable}/{}, modifiers {with_effects}/{}", m.id, passives.len(), mods.len());
            for p in passives.iter().filter(|p| lines(p).is_empty()) {
                println!("   no stat lines: {}", p.name);
            }
        }
    }

    #[test]
    fn description_components_use_x_and_x2() {
        let p = passive(r"<b_gold>Level {CurrentLevel}:</>\n<b>{IncreasedColdDamage}|fmt(max_frac=1,perc1)</> increased <me_ColdDam>Cold Damage</>\n<b_gold>Level {NextLevel}:</>\n<b>{IncreasedColdDamage2}|fmt(max_frac=1,perc1)</> increased <me_ColdDam>Cold Damage</>");
        let l = lines(&p);
        assert_eq!(l.len(), 1);
        assert_eq!(l[0].stat, Stat::AllDamage);
        let v = passive_values(&p, 1);
        assert_eq!(v("IncreasedColdDamage"), Some(2.5));
        assert_eq!(v("IncreasedColdDamage2"), Some(5.0));
    }

    #[test]
    fn passives_without_stat_lines_are_not_playable() {
        assert!(lines(&passive("Unlocks <me_SpellRage>Spell Rage</>.")).is_empty());
    }
}

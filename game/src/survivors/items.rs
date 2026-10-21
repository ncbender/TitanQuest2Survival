//! Rolled items like TQ2's: a base item (equipment.rs) with a rarity, a quality roll on its
//! base value and prefixes/suffixes. Affix names (per tier), effect texts and which affixes
//! may roll on weapons, armour and off-hands (with TQ2's drop weights) come from TQ2
//! (`Tq2Extract affixes`). TQ2's numeric ranges are not readable, so `Stat::range` supplies
//! this game's values per tier. There are no level requirements: every run starts at level 1.
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::equipment::{Equipment, Item};
use super::mastery_data::render;

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Default)]
pub enum Rarity {
    #[default]
    Common,
    Magical,
    Rare,
    Epic,
    Legendary,
}

impl Rarity {
    pub const ALL: [Rarity; 5] = [Rarity::Common, Rarity::Magical, Rarity::Rare, Rarity::Epic, Rarity::Legendary];

    /// TQ2's rarity names.
    pub fn name(self) -> &'static str {
        match self {
            Rarity::Common => "Common",
            Rarity::Magical => "Magical",
            Rarity::Rare => "Rare",
            Rarity::Epic => "Epic",
            Rarity::Legendary => "Legendary",
        }
    }

    /// Titan Quest's item colours.
    pub fn color(self) -> Color {
        match self {
            Rarity::Common => Color::srgb(0.92, 0.92, 0.9),
            Rarity::Magical => Color::srgb(1.0, 0.86, 0.3),
            Rarity::Rare => Color::srgb(0.35, 0.95, 0.35),
            Rarity::Epic => Color::srgb(0.35, 0.65, 1.0),
            Rarity::Legendary => Color::srgb(0.8, 0.45, 1.0),
        }
    }

    /// Base drop weight (per 1000).
    fn weight(self) -> f32 {
        match self {
            Rarity::Common => 500.0,
            Rarity::Magical => 300.0,
            Rarity::Rare => 140.0,
            Rarity::Epic => 50.0,
            Rarity::Legendary => 10.0,
        }
    }

    /// (prefixes, suffixes) and the affix tier range (1..=6).
    fn affixes(self) -> (usize, usize, u8, u8) {
        match self {
            Rarity::Common => (0, 0, 1, 1),
            Rarity::Magical => (1, 0, 1, 2),
            Rarity::Rare => (1, 1, 2, 3),
            Rarity::Epic => (2, 1, 3, 5),
            Rarity::Legendary => (2, 2, 4, 6),
        }
    }

    /// Bonus on the base value (weapon damage, armour) on top of the quality roll.
    fn base_bonus(self) -> f32 {
        match self {
            Rarity::Common | Rarity::Magical => 0.0,
            Rarity::Rare => 0.05,
            Rarity::Epic => 0.15,
            Rarity::Legendary => 0.3,
        }
    }

    /// Rolls a rarity. `luck` ≥ 1 favours higher rarities (bosses, later in a run).
    pub fn roll(luck: f32) -> Rarity {
        let weights: Vec<f32> = Rarity::ALL.iter().enumerate().map(|(i, r)| r.weight() * luck.powi(i as i32)).collect();
        let mut x = fastrand::f32() * weights.iter().sum::<f32>();
        for (r, w) in Rarity::ALL.iter().zip(&weights) {
            if x < *w {
                return *r;
            }
            x -= w;
        }
        Rarity::Common
    }

    /// Chance of each rarity in percent for `luck`.
    #[cfg(test)]
    pub fn chances(luck: f32) -> Vec<(Rarity, f32)> {
        let weights: Vec<f32> = Rarity::ALL.iter().enumerate().map(|(i, r)| r.weight() * luck.powi(i as i32)).collect();
        let total: f32 = weights.iter().sum();
        Rarity::ALL.iter().zip(weights).map(|(r, w)| (*r, w / total * 100.0)).collect()
    }
}

/// What an affix does in this game (derived from its TQ2 text).
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Stat {
    /// "+X Weapon Fire Damage": flat damage per hit.
    FlatDamage,
    WeaponPct,
    SkillPct,
    /// "% increased Fire/Cold/… Damage", damage over time: all damage.
    DamagePct,
    AttackSpeed,
    CastSpeed,
    Cooldown,
    Crit,
    CritMult,
    /// Might/Agility/Knowledge style attributes: health.
    Health,
    /// Resistances, glancing hits: damage reduction.
    Resist,
    Ailment,
    /// Damage dealt to enemies that hit the hero.
    Retaliation,
}

impl Stat {
    fn detect(text: &str) -> Option<Stat> {
        let t = text.to_ascii_lowercase();
        let has = |s: &str| t.contains(s);
        Some(if has("weapon") && has("damage") && !has("%") {
            Stat::FlatDamage
        } else if has("retaliation") {
            Stat::Retaliation
        } else if has("attack and cast speed") || has("attack speed") {
            Stat::AttackSpeed
        } else if has("cast speed") || has("energy") {
            Stat::CastSpeed
        } else if has("cooldown") {
            Stat::Cooldown
        } else if has("critical hit damage") {
            Stat::CritMult
        } else if has("critical") {
            Stat::Crit
        } else if has("resistance") || has("glancing") {
            Stat::Resist
        } else if has("ailment") {
            Stat::Ailment
        } else if has("attack damage") {
            Stat::WeaponPct
        } else if has("spell damage") || has("summons") {
            Stat::SkillPct
        } else if has("damage") {
            Stat::DamagePct
        } else if has("might") || has("agility") || has("knowledge") || has("health") {
            Stat::Health
        } else {
            return None;
        })
    }

    /// Value range at affix tier 1..=6 (percent unless noted).
    fn range(self, tier: u8) -> (f32, f32) {
        let t = tier.clamp(1, 6) as f32;
        let (lo, hi) = match self {
            Stat::FlatDamage => (1.0, 2.0),
            Stat::WeaponPct | Stat::SkillPct => (4.0, 7.0),
            Stat::DamagePct => (3.0, 5.0),
            Stat::AttackSpeed | Stat::CastSpeed => (2.0, 4.0),
            Stat::Cooldown => (2.0, 3.5),
            Stat::Crit => (1.0, 1.5),
            Stat::CritMult => (6.0, 10.0),
            Stat::Health => (6.0, 10.0),
            Stat::Resist => (1.5, 2.5),
            Stat::Ailment => (5.0, 9.0),
            Stat::Retaliation => (3.0, 5.0),
        };
        (lo * t, hi * t)
    }
}

/// One TQ2 affix (cache/export/items/affixes.json).
#[derive(Deserialize, Clone)]
pub struct AffixInfo {
    pub id: String,
    pub kind: String,
    /// Name per tier (TQ2 has six).
    pub names: Vec<String>,
    pub text: String,
    /// TQ2 drop weight per slot ("weapon", "armor", "offhand").
    pub weights: std::collections::HashMap<String, f32>,
    #[serde(skip)]
    pub stat: Option<Stat>,
}

#[derive(Resource, Default)]
pub struct AffixBook(pub Vec<AffixInfo>);

impl AffixBook {
    pub fn load(root: &str) -> Self {
        let path = format!("{root}/items/affixes.json");
        let list: Vec<AffixInfo> = std::fs::read_to_string(&path).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_else(|| {
            warn!("no affix data at {path}; run `Tq2Extract assets affixes`");
            vec![]
        });
        AffixBook(list.into_iter().map(|mut a| {
            a.stat = Stat::detect(&a.text);
            a
        }).filter(|a| a.stat.is_some() && a.weights.values().any(|w| *w > 0.0)).collect())
    }

    pub fn get(&self, id: &str) -> Option<&AffixInfo> {
        self.0.iter().find(|a| a.id == id)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RolledAffix {
    pub id: String,
    pub tier: u8,
    pub value: f32,
}

/// A concrete item in the stash.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ItemInstance {
    pub uid: u64,
    /// Base item id (equipment.rs).
    pub base: String,
    pub rarity: Rarity,
    /// Roll on the base value (weapon damage, armour), 0.9..1.1.
    pub quality: f32,
    pub affixes: Vec<RolledAffix>,
}

/// Slot key of an item for TQ2's affix weights.
fn slot_key(item: Item) -> &'static str {
    match item {
        Item::Weapon(_) => "weapon",
        Item::Armor(_) => "armor",
        Item::Shield(_) | Item::Talisman(_) => "offhand",
    }
}

impl ItemInstance {
    /// A plain common copy (starting gear).
    pub fn plain(uid: u64, item: Item) -> Self {
        Self { uid, base: item.id().into(), rarity: Rarity::Common, quality: 1.0, affixes: vec![] }
    }

    /// Rolls a random item of `rarity`.
    pub fn roll(uid: u64, item: Item, rarity: Rarity, book: &AffixBook) -> Self {
        let (prefixes, suffixes, lo, hi) = rarity.affixes();
        let slot = slot_key(item);
        let mut affixes = vec![];
        for (kind, count) in [("prefix", prefixes), ("suffix", suffixes)] {
            let mut pool: Vec<&AffixInfo> = book.0.iter().filter(|a| a.kind == kind && a.weights.get(slot).is_some_and(|w| *w > 0.0)).collect();
            for _ in 0..count {
                let total: f32 = pool.iter().map(|a| a.weights[slot]).sum();
                if total <= 0.0 {
                    break;
                }
                let mut x = fastrand::f32() * total;
                let Some(i) = pool.iter().position(|a| {
                    x -= a.weights[slot];
                    x < 0.0
                }) else { break };
                let a = pool.remove(i);
                let tier = fastrand::u8(lo..=hi);
                let (min, max) = a.stat.unwrap().range(tier);
                affixes.push(RolledAffix { id: a.id.clone(), tier, value: (min + fastrand::f32() * (max - min)).round().max(1.0) });
            }
        }
        let quality = 0.9 + fastrand::f32() * 0.2 + rarity.base_bonus();
        Self { uid, base: item.id().into(), rarity, quality, affixes }
    }

    pub fn item(&self) -> Option<Item> {
        Item::from_id(&self.base)
    }

    /// TQ2-style name: "<Prefix> <Base> <Suffix>" with the affixes' tier names.
    pub fn name(&self, book: &AffixBook) -> String {
        let base = self.item().map_or(self.base.clone(), |i| i.name().to_string());
        let tier_name = |r: &RolledAffix| book.get(&r.id).and_then(|a| a.names.get(r.tier as usize - 1).or(a.names.last()).cloned());
        let prefix = self.affixes.iter().filter(|r| book.get(&r.id).is_some_and(|a| a.kind == "prefix")).find_map(tier_name);
        let suffix = self.affixes.iter().filter(|r| book.get(&r.id).is_some_and(|a| a.kind == "suffix")).find_map(tier_name);
        [prefix, Some(base), suffix].into_iter().flatten().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" ")
    }

    /// The affix effect lines (TQ2 texts with the rolled values) as coloured spans.
    pub fn lines(&self, book: &AffixBook) -> Vec<Vec<(String, Color)>> {
        self.affixes
            .iter()
            .filter_map(|r| {
                let a = book.get(&r.id)?;
                let v = r.value;
                Some(render(&a.text, &move |_| Some(v)))
            })
            .collect()
    }

    /// The base line: damage or armour with the quality roll.
    pub fn base_line(&self) -> String {
        match self.item() {
            Some(Item::Weapon(w)) => format!("{:.0} damage · every {:.2} s", w.damage * self.quality, w.cooldown),
            Some(Item::Armor(a)) => format!("{:.0} % less damage taken", a.armor * self.quality * 100.0),
            Some(Item::Shield(s)) => format!("blocks {:.0} % of damage", s.block * self.quality * 100.0),
            Some(Item::Talisman(t)) => format!("+{:.0} % skill damage", t.skill_damage * self.quality * 100.0),
            None => String::new(),
        }
    }

    /// Adds this item's affixes to `bonus`.
    pub fn add_to(&self, bonus: &mut Bonus, book: &AffixBook) {
        for r in &self.affixes {
            let Some(stat) = book.get(&r.id).and_then(|a| a.stat) else { continue };
            let v = r.value / 100.0;
            match stat {
                Stat::FlatDamage => bonus.flat_damage += r.value,
                Stat::WeaponPct => bonus.weapon_pct += v,
                Stat::SkillPct => bonus.skill_pct += v,
                Stat::DamagePct => {
                    bonus.weapon_pct += v;
                    bonus.skill_pct += v;
                }
                Stat::AttackSpeed => bonus.attack_speed += v,
                Stat::CastSpeed | Stat::Cooldown => bonus.cast_speed += v,
                Stat::Crit => bonus.crit += v,
                Stat::CritMult => bonus.crit_mult += v,
                Stat::Health => bonus.health += r.value,
                Stat::Resist => bonus.reduction += v,
                Stat::Ailment => bonus.ailment += v,
                Stat::Retaliation => bonus.retaliation += r.value,
            }
        }
    }
}

/// Sum of the worn items' affixes.
#[derive(Clone, Copy, Default, Debug)]
pub struct Bonus {
    pub flat_damage: f32,
    pub weapon_pct: f32,
    pub skill_pct: f32,
    pub attack_speed: f32,
    pub cast_speed: f32,
    pub crit: f32,
    pub crit_mult: f32,
    pub health: f32,
    pub reduction: f32,
    pub ailment: f32,
    pub retaliation: f32,
}

/// Rolls a random item (any base) for a chest.
pub fn roll_loot(uid: u64, luck: f32, book: &AffixBook) -> ItemInstance {
    let bases = super::equipment::ITEMS;
    let item = bases[fastrand::usize(..bases.len())];
    ItemInstance::roll(uid, item, Rarity::roll(luck), book)
}

/// Applies the equipment's bonuses to a run's stats.
pub fn apply(eq: &Equipment, stats: &mut super::player::HeroStats, hp: &mut super::Health) {
    let b = eq.bonus;
    stats.damage *= 1.0 + b.weapon_pct;
    stats.skill_damage *= 1.0 + b.skill_pct;
    stats.cooldown /= 1.0 + b.attack_speed;
    stats.skill_cooldown /= 1.0 + b.cast_speed;
    stats.crit += b.crit;
    stats.crit_mult += b.crit_mult;
    stats.ailment += b.ailment;
    stats.retaliation += b.retaliation;
    hp.max += b.health;
    hp.hp = hp.max;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn book() -> AffixBook {
        let a = |id: &str, kind: &str, text: &str, slot: &str| AffixInfo {
            id: id.into(),
            kind: kind.into(),
            names: vec!["Numbing".into(), "Biting".into(), "Frigid".into(), "Freezing".into(), "Glacial".into(), "Arctic".into()],
            text: text.into(),
            weights: [(slot.to_string(), 10.0)].into(),
            stat: Stat::detect(text),
        };
        AffixBook(vec![
            a("p", "prefix", "{Value}|fmt(sign=true) Weapon Cold Damage", "weapon"),
            AffixInfo { names: vec!["of Haste".into(); 6], ..a("s", "suffix", "{Value}% increased Attack Speed", "weapon") },
        ])
    }

    #[test]
    fn rare_weapon_has_prefix_and_suffix_named_like_tq2() {
        let it = ItemInstance::roll(1, Item::Weapon(&super::super::equipment::SWORD), Rarity::Rare, &book());
        assert_eq!(it.affixes.len(), 2);
        let name = it.name(&book());
        assert!(name.ends_with("Battlemage Sword of Haste"), "{name}");
        let mut b = Bonus::default();
        it.add_to(&mut b, &book());
        assert!(b.flat_damage > 0.0 && b.attack_speed > 0.0);
    }

    #[test]
    fn stats_are_detected_from_tq2_texts() {
        assert_eq!(Stat::detect("{Value}% increased Attack Damage"), Some(Stat::WeaponPct));
        assert_eq!(Stat::detect("{Value}|fmt(sign=true)% Fire Resistance"), Some(Stat::Resist));
        assert_eq!(Stat::detect("{Value}|fmt(sign=true) Fire Retaliation"), Some(Stat::Retaliation));
        assert_eq!(Stat::detect("{Value}|fmt(sign,perc100) Critical Hit Damage Multiplier"), Some(Stat::CritMult));
    }

    #[test]
    fn luck_shifts_rarity() {
        let normal = Rarity::chances(1.0);
        let lucky = Rarity::chances(2.0);
        assert!(lucky[4].1 > normal[4].1 && lucky[0].1 < normal[0].1);
        assert!((normal.iter().map(|c| c.1).sum::<f32>() - 100.0).abs() < 0.01);
    }
}

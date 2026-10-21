//! The skill tree: permanent passives and unlockable skills, bought with favour earned in
//! runs (see profile.rs). Unlocked skills can then be found and levelled in level-ups.
use super::player::HeroStats;
use super::profile::Profile;
use super::Health;

pub struct SkillNode {
    pub id: &'static str,
    pub name: &'static str,
    /// What one rank does.
    pub effect: &'static str,
    pub max_rank: u32,
    /// Favour per rank.
    pub cost: u32,
    /// Node that needs at least one rank first.
    pub requires: Option<&'static str>,
    /// Column in the tree screen.
    pub branch: usize,
}

pub const BRANCHES: &[&str] = &["Warfare", "Defence", "Fire", "Fortune"];

pub const NODES: &[SkillNode] = &[
    SkillNode { id: "might", name: "Might", effect: "+6 % damage", max_rank: 5, cost: 1, requires: None, branch: 0 },
    SkillNode { id: "haste", name: "Haste", effect: "+5 % attack speed", max_rank: 5, cost: 1, requires: None, branch: 0 },
    SkillNode { id: "keen", name: "Keen Eye", effect: "+3 % critical chance", max_rank: 3, cost: 2, requires: Some("might"), branch: 0 },
    SkillNode { id: "vigor", name: "Vigour", effect: "+15 max health", max_rank: 5, cost: 1, requires: None, branch: 1 },
    SkillNode { id: "fortitude", name: "Fortitude", effect: "+4 % less damage taken", max_rank: 4, cost: 1, requires: None, branch: 1 },
    SkillNode { id: "swift", name: "Swiftness", effect: "+5 % movement speed", max_rank: 3, cost: 2, requires: Some("vigor"), branch: 1 },
    SkillNode {
        id: "fire_nova",
        name: "Fire Nova",
        effect: "unlocks the skill: a ring of flames around you",
        max_rank: 1,
        cost: 1,
        requires: None,
        branch: 2,
    },
    SkillNode {
        id: "meteor",
        name: "Meteor",
        effect: "unlocks the skill: fire falls on nearby enemies",
        max_rank: 1,
        cost: 2,
        requires: Some("fire_nova"),
        branch: 2,
    },
    SkillNode { id: "pyromancy", name: "Pyromancy", effect: "+15 % skill damage", max_rank: 3, cost: 2, requires: Some("fire_nova"), branch: 2 },
    SkillNode { id: "magnet", name: "Attraction", effect: "+20 % pickup radius", max_rank: 3, cost: 1, requires: None, branch: 3 },
    SkillNode { id: "wisdom", name: "Wisdom", effect: "+10 % experience", max_rank: 3, cost: 1, requires: None, branch: 3 },
];

pub fn node(id: &str) -> Option<&'static SkillNode> {
    NODES.iter().find(|n| n.id == id)
}

/// Why `id` cannot be bought right now, or None if it can.
pub fn blocked(profile: &Profile, id: &str) -> Option<String> {
    let n = node(id)?;
    let rank = profile.rank(id);
    if rank >= n.max_rank {
        return Some("maxed".into());
    }
    if let Some(req) = n.requires.filter(|r| profile.rank(r) == 0) {
        return Some(format!("requires {}", node(req).map_or(req, |r| r.name)));
    }
    if profile.favor < n.cost {
        return Some(format!("needs {} favour", n.cost));
    }
    None
}

pub fn buy(profile: &mut Profile, id: &str) -> bool {
    let Some(n) = node(id) else { return false };
    if blocked(profile, id).is_some() {
        return false;
    }
    profile.favor -= n.cost;
    *profile.skills.entry(id.into()).or_default() += 1;
    true
}

/// Gives back every point spent.
pub fn reset(profile: &mut Profile) {
    let spent: u32 = profile.skills.iter().filter_map(|(id, r)| node(id).map(|n| n.cost * r)).sum();
    profile.favor += spent;
    profile.skills.clear();
}

/// Applies the tree to a fresh run's stats.
pub fn apply(profile: &Profile, stats: &mut HeroStats, hp: &mut Health) {
    let r = |id| profile.rank(id) as f32;
    stats.damage *= 1.0 + 0.06 * r("might");
    stats.cooldown /= 1.0 + 0.05 * r("haste");
    stats.crit += 0.03 * r("keen");
    hp.max += 15.0 * r("vigor");
    hp.hp = hp.max;
    stats.armor += 0.04 * r("fortitude");
    stats.speed *= 1.0 + 0.05 * r("swift");
    stats.skill_damage *= 1.0 + 0.15 * r("pyromancy");
    stats.pickup *= 1.0 + 0.2 * r("magnet");
    stats.xp *= 1.0 + 0.1 * r("wisdom");
    stats.nova_unlocked = profile.rank("fire_nova") > 0;
    stats.meteor_unlocked = profile.rank("meteor") > 0;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buying_needs_favour_and_prerequisites() {
        let mut p = Profile { favor: 3, ..Default::default() };
        assert!(!buy(&mut p, "meteor"), "needs fire_nova first");
        assert!(buy(&mut p, "fire_nova"));
        assert!(buy(&mut p, "meteor"));
        assert_eq!(p.favor, 0);
        assert!(!buy(&mut p, "might"));
        reset(&mut p);
        assert_eq!(p.favor, 3);
        assert!(p.skills.is_empty());
    }

    #[test]
    fn requirements_exist() {
        for n in NODES {
            assert!(n.requires.is_none_or(|r| node(r).is_some()), "{}", n.id);
            assert!(n.branch < BRANCHES.len());
        }
    }
}

//! The character sheet: every number that decides a fight, from the same sources as a run
//! (base values, passives, gear, and in a run also level-up bonuses). Shown in the inventory
//! and on the pause screen.
use bevy::prelude::*;

use super::equipment::{AttackKind, Equipment};
use super::items::AffixBook;
use super::mastery_data::{MasteryBook, GOLD, TEXT};
use super::player::HeroStats;
use super::profile::Profile;
use super::ui::{headline, text};
use super::{config, Health};
use crate::fonts::TitleFont;

/// The stats a fresh run starts with: base values, then passives, then gear.
pub fn starting(profile: &Profile, book: &MasteryBook, affixes: &AffixBook) -> (HeroStats, Health, Equipment) {
    let mut stats = HeroStats::default();
    let mut hp = Health { hp: config::PLAYER_HP, max: config::PLAYER_HP };
    super::skills::apply(profile, book, &mut stats, &mut hp);
    let gear = profile.equipment(affixes);
    super::items::apply(&gear, &mut stats, &mut hp);
    (stats, hp, gear)
}

/// Sections of (label, value) rows.
pub fn rows(stats: &HeroStats, hp: &Health, eq: &Equipment) -> Vec<(&'static str, Vec<(&'static str, String)>)> {
    let pct = |v: f32| format!("{:.0} %", v * 100.0);
    let more = |mult: f32| format!("{:+.0} %", (mult - 1.0) * 100.0);
    let w = eq.main;
    let interval = eq.cooldown() * stats.cooldown;
    let hit = eq.weapon_damage() * stats.damage;
    let mut offense = vec![
        ("Weapon", w.name.to_string()),
        ("Damage per hit", format!("{hit:.0}")),
        ("Attacks per second", format!("{:.2}", 1.0 / interval.max(0.01))),
        ("Damage per second", format!("{:.0}", hit / interval.max(0.01) * (1.0 + stats.crit.min(1.0) * (stats.crit_mult - 1.0)))),
        ("Critical chance", pct(stats.crit.min(1.0))),
        ("Critical damage", format!("×{:.2}", stats.crit_mult)),
    ];
    match w.kind {
        AttackKind::Sweep => offense.push(("Sweep radius", format!("{:.1} m", w.speed_or_radius * stats.area))),
        _ => {
            offense.push(("Projectiles", format!("{}", stats.projectiles)));
            offense.push(("Pierces", format!("{} enemies", w.pierce + stats.pierce)));
        }
    }
    let skills = vec![
        ("Skill damage", more(stats.skill_damage * eq.skill_damage())),
        ("Skill cooldowns", format!("{:+.0} %", (stats.skill_cooldown - 1.0) * 100.0)),
        ("Area of effect", more(stats.area)),
        ("Ailment chance", format!("+{:.0} %", stats.ailment * 100.0)),
    ];
    let mut defense = vec![
        ("Health", format!("{:.0} / {:.0}", hp.hp.max(0.0), hp.max)),
        ("Regeneration", format!("{:.1} per second", stats.regen)),
        ("Damage reduction", pct((eq.damage_reduction() + stats.armor).min(0.8))),
    ];
    if stats.barrier > 0.0 {
        defense.push(("Barrier", format!("{:.0}", stats.barrier * hp.max)));
    }
    if stats.glancing > 0.0 {
        defense.push(("Glancing hits", pct(stats.glancing)));
    }
    if stats.retaliation > 0.0 {
        defense.push(("Retaliation", format!("{:.0}", stats.retaliation)));
    }
    let mut utility = vec![
        ("Movement speed", format!("{:.1} m/s", config::PLAYER_SPEED * stats.speed)),
        ("Pickup radius", format!("{:.1} m", config::PICKUP_RADIUS * stats.pickup)),
        ("Experience", more(stats.xp)),
    ];
    if stats.rage > 0.0 {
        utility.push(("Rage effect", more(1.0 + stats.rage)));
    }
    if stats.execute > 0.0 {
        utility.push(("Execution below", pct(stats.execute)));
    }
    if stats.shade > 0.0 {
        utility.push(("Shade generation", more(1.0 + stats.shade)));
    }
    vec![("Offense", offense), ("Skills", skills), ("Defense", defense), ("Utility", utility)]
}

/// Compact form (narrow side columns): plain section titles, smaller rows.
pub fn sheet(parent: &mut ChildSpawnerCommands, title: &TitleFont, stats: &HeroStats, hp: &Health, eq: &Equipment, width: f32) {
    for (section, list) in rows(stats, hp, eq) {
        parent.spawn(Node { width: Val::Px(width), flex_direction: FlexDirection::Column, flex_shrink: 0.0, margin: UiRect::top(Val::Px(4.0)), ..default() }).with_children(|s| {
            s.spawn((Text::new(section), title.at(15.0), TextColor(GOLD)));
            for (label, value) in list {
                s.spawn(Node { justify_content: JustifyContent::SpaceBetween, ..default() }).with_children(|r| {
                    r.spawn((Text::new(label), text(13.0), TextColor(TEXT)));
                    r.spawn((Text::new(value), text(13.0), TextColor(GOLD)));
                });
            }
        });
    }
}

/// Some sections of the sheet as label/value rows under section headlines, `width` px wide.
#[allow(clippy::too_many_arguments)]
pub fn sheet_part(parent: &mut ChildSpawnerCommands, assets: &AssetServer, title: &TitleFont, stats: &HeroStats, hp: &Health, eq: &Equipment, width: f32, sections: std::ops::Range<usize>) {
    let all = rows(stats, hp, eq);
    let end = sections.end.min(all.len());
    for (section, list) in all.into_iter().take(end).skip(sections.start) {
        parent.spawn(Node { width: Val::Px(width), flex_direction: FlexDirection::Column, flex_shrink: 0.0, ..default() }).with_children(|s| {
            headline(s, assets, title, section, 17.0);
            for (label, value) in list {
                s.spawn(Node { justify_content: JustifyContent::SpaceBetween, ..default() }).with_children(|r| {
                    r.spawn((Text::new(label), text(14.0), TextColor(TEXT)));
                    r.spawn((Text::new(value), text(14.0), TextColor(GOLD)));
                });
            }
        });
    }
}

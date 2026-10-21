//! The mastery screen in TQ2's look: choose masteries, learn active skills and passives tier by
//! tier, pick which skills cast in runs. All names, texts, icons and artwork are TQ2's own.
use bevy::prelude::*;

use super::abilities::{self, modifier_effects, Slot};
use super::mastery_data::{render, texture, MasteryBook, GOLD, TEXT};
use super::menu::{MainMenuUi, Page};
use super::player::HeroStats;
use super::profile::Profile;
use super::skills::{self, ACTIVE_MAX_LEVEL, MAX_EQUIPPED, SECOND_MASTERY_LEVEL, TIER_LEVEL};
use super::ui::{self, backdrop, button, panel, rich, roman, sliced, text};
use super::GameState;
use crate::fonts::TitleFont;

/// A skill or passive in the tree (indices into the mastery book).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct NodeRef {
    pub mastery: usize,
    pub tier: usize,
    pub passive: bool,
    pub index: usize,
}

#[derive(Clone, Copy, Debug)]
pub enum TreeAction {
    Tab(usize),
    Select(NodeRef),
    Learn,
    OpenChooser,
    Preview(usize),
    Choose,
    Refund,
    ResetMasteries,
    Back,
}

/// What the mastery screen shows.
#[derive(Resource, Default)]
pub struct TreeView {
    pub tab: Option<usize>,
    pub selected: Option<NodeRef>,
    pub chooser: bool,
    pub preview: Option<usize>,
    /// Feedback for the last action (e.g. why a skill cannot be learned).
    pub message: String,
}

#[derive(Resource, Default)]
struct TreeClicked(Option<TreeAction>);

pub fn plugin(app: &mut App) {
    app.init_resource::<TreeView>().init_resource::<TreeClicked>().add_systems(Update, tree_input.run_if(in_state(GameState::MainMenu)));
}

fn click(action: TreeAction) -> impl Fn(On<PointerClick>, ResMut<TreeClicked>) {
    move |_, mut c| c.0 = Some(action)
}

fn tree_input(mut clicked: ResMut<TreeClicked>, mut view: ResMut<TreeView>, mut profile: ResMut<Profile>, book: Res<MasteryBook>, mut page: ResMut<Page>) {
    let Some(action) = clicked.0.take() else { return };
    let mut result: Result<(), String> = Ok(());
    match action {
        TreeAction::Tab(m) => {
            view.tab = Some(m);
            view.selected = None;
        }
        TreeAction::Select(n) => view.selected = Some(n),
        TreeAction::Learn => {
            if let Some(n) = view.selected {
                let id = node_id(&book, n);
                result = if n.passive { skills::learn_passive(&mut profile, &book, &id) } else { skills::learn_active(&mut profile, &book, &id) };
            }
        }
        TreeAction::OpenChooser => {
            view.chooser = true;
            view.preview = None;
        }
        TreeAction::Preview(m) => view.preview = Some(m),
        TreeAction::Choose => {
            if let Some(m) = view.preview {
                let id = book.0[m].id.clone();
                result = skills::choose_mastery(&mut profile, &book, &id);
                if result.is_ok() {
                    view.chooser = false;
                    view.tab = Some(m);
                    view.selected = None;
                }
            }
        }
        TreeAction::Refund => skills::refund(&mut profile, false),
        TreeAction::ResetMasteries => {
            skills::refund(&mut profile, true);
            view.tab = None;
            view.selected = None;
            view.chooser = true;
        }
        TreeAction::Back => {
            if view.chooser && !profile.masteries.is_empty() {
                view.chooser = false;
            } else {
                *page = Page::Title;
            }
        }
    }
    view.message = result.err().unwrap_or_default();
}

fn node_id(book: &MasteryBook, n: NodeRef) -> String {
    let tier = &book.0[n.mastery].tiers[n.tier];
    if n.passive { tier.passives[n.index].id.clone() } else { tier.actives[n.index].id.clone() }
}

/// Builds the mastery screen (or the mastery chooser).
pub fn build(commands: &mut Commands, assets: &AssetServer, title: &TitleFont, profile: &Profile, book: &MasteryBook, view: &TreeView) {
    if book.0.is_empty() {
        commands.spawn(backdrop(assets, MainMenuUi)).with_children(|p| {
            p.spawn((Text::new("Mastery data missing – run `Tq2Extract assets masteries` (see README)."), text(24.0)));
        });
        return;
    }
    if view.chooser || profile.masteries.is_empty() {
        chooser(commands, assets, title, profile, book, view);
        return;
    }
    let chosen: Vec<usize> = profile.masteries.iter().filter_map(|id| book.0.iter().position(|m| &m.id == id)).collect();
    let tab = view.tab.filter(|t| chosen.contains(t)).unwrap_or(chosen[0]);
    let m = &book.0[tab];
    let level = skills::mastery_level(profile, book, &m.id);

    commands.spawn(backdrop(assets, MainMenuUi)).with_children(|root| {
        // Header: title, points, mastery tabs.
        root.spawn(Node { align_items: AlignItems::Center, column_gap: Val::Px(18.0), ..default() }).with_children(|h| {
            h.spawn((Text::new("Masteries"), title.at(34.0)));
            h.spawn((
                Text::new(format!("Active Points: {}    Passive Points: {}", profile.active_points, profile.passive_points)),
                text(22.0),
                TextColor(GOLD),
                Node { margin: UiRect::horizontal(Val::Px(24.0)), ..default() },
            ));
            for &c in &chosen {
                let info = &book.0[c];
                h.spawn((
                    Node { padding: UiRect::axes(Val::Px(18.0), Val::Px(8.0)), border: UiRect::all(Val::Px(if c == tab { 2.0 } else { 0.0 })), ..default() },
                    BackgroundColor(info.color().with_alpha(0.55)),
                    BorderColor::all(GOLD),
                    children![(Text::new(format!("{}  ({})", info.name, skills::mastery_level(profile, book, &info.id))), title.at(22.0))],
                ))
                .observe(click(TreeAction::Tab(c)));
            }
            if chosen.len() < skills::MAX_MASTERIES {
                let open = skills::can_choose_mastery(profile, book);
                let label = if open { "+ Second Mastery".to_string() } else { format!("Second mastery at level {SECOND_MASTERY_LEVEL}") };
                h.spawn(button(assets, label, 300.0, open, open)).observe(click(TreeAction::OpenChooser));
            }
        });

        root.spawn(Node { flex_grow: 1.0, column_gap: Val::Px(14.0), min_height: Val::Px(0.0), ..default() }).with_children(|body| {
            // Left: artwork, name, level, equipped skills.
            body.spawn(panel(assets, Node { width: Val::Px(220.0), flex_shrink: 0.0, flex_direction: FlexDirection::Column, min_height: Val::Px(0.0), ..default() }))
                .with_children(|p| ui::scroll_area(p, Node::default(), 8.0, |l| {
                    if let Some(art) = &m.icon {
                        l.spawn((Node { width: Val::Px(170.0), height: Val::Px(170.0), flex_shrink: 0.0, ..default() }, ui::image(assets, texture(art), Color::WHITE)))
                            .with_children(|a| {
                                a.spawn((
                                    Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() },
                                    sliced(assets, "t_frame_gold_big02", 40.0, Color::WHITE),
                                ));
                            });
                    }
                    l.spawn((Text::new(m.name.clone()), title.at(26.0), TextColor(m.color().lighter(0.35))));
                    l.spawn((Text::new(m.slogan.clone()), text(14.0), TextColor(TEXT)));
                    let next = TIER_LEVEL.iter().find(|t| **t > level);
                    let progress = match next {
                        Some(n) => format!("Mastery level {level}  (next tier at {n})"),
                        None => format!("Mastery level {level}  (all tiers open)"),
                    };
                    l.spawn((Text::new(progress), text(14.0), TextColor(GOLD)));
                    l.spawn((Text::new("Learned skills"), title.at(16.0), Node { margin: UiRect::top(Val::Px(8.0)), ..default() }));
                    l.spawn((Text::new(format!("Level-ups in runs offer them, up to {MAX_EQUIPPED} per run.")), text(13.0), TextColor(TEXT)));
                    for id in profile.actives.keys() {
                        if let Some((_, _, s)) = book.active(id) {
                            l.spawn(Node { align_items: AlignItems::Center, column_gap: Val::Px(8.0), ..default() }).with_children(|r| {
                                if let Some(icon) = &s.icon {
                                    r.spawn((Node { width: Val::Px(40.0), height: Val::Px(40.0), ..default() }, ui::image(assets, texture(icon), Color::WHITE)));
                                }
                                r.spawn((Text::new(format!("{}  {}", s.name, profile.active_level(id))), text(14.0)));
                            });
                        }
                    }
                    if profile.actives.is_empty() {
                        l.spawn((Text::new("Learn an active skill to use it in runs."), text(16.0), TextColor(TEXT)));
                    }
                }));

            // Center: the five tiers, over the mastery's emblem in its colour.
            body.spawn(panel(assets, Node { flex_grow: 1.0, flex_basis: Val::Px(0.0), min_width: Val::Px(0.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(4.0), overflow: Overflow::clip(), ..default() }))
                .with_children(|c| {
                    if let Some(bg) = &m.background {
                        c.spawn((
                            Node { position_type: PositionType::Absolute, left: Val::Percent(20.0), top: Val::Percent(5.0), width: Val::Percent(60.0), aspect_ratio: Some(1.0), ..default() },
                            ui::image(assets, texture(bg), m.color().with_alpha(0.18)),
                        ));
                    }
                    for (t, tier) in m.tiers.iter().enumerate() {
                        let open = skills::tier_open(profile, book, &m.id, t);
                        c.spawn(Node { align_items: AlignItems::Center, column_gap: Val::Px(2.0), flex_grow: 1.0, ..default() }).with_children(|row| {
                            row.spawn(Node { width: Val::Px(52.0), flex_shrink: 0.0, flex_direction: FlexDirection::Column, ..default() }).with_children(|lbl| {
                                lbl.spawn((Text::new(format!("Tier {}", roman(t))), title.at(14.0), TextColor(if open { GOLD } else { Color::srgb(0.45, 0.43, 0.4) })));
                                lbl.spawn((Text::new(format!("lvl {}", TIER_LEVEL[t])), text(11.0), TextColor(TEXT)));
                            });
                            for (i, a) in tier.actives.iter().enumerate().filter(|(_, a)| !a.wip) {
                                let n = NodeRef { mastery: tab, tier: t, passive: false, index: i };
                                let lvl = profile.active_level(&a.id);
                                let playable = abilities::spec(&a.id).is_some();
                                let equipped = lvl > 0;
                                node(row, assets, n, a.icon.as_deref(), &format!("{lvl}/{ACTIVE_MAX_LEVEL}"), open && playable, lvl > 0, equipped, view.selected == Some(n));
                            }
                            row.spawn(Node { width: Val::Px(2.0), height: Val::Px(46.0), margin: UiRect::horizontal(Val::Px(4.0)), ..default() })
                                .insert(BackgroundColor(GOLD.with_alpha(0.4)));
                            for (i, p) in tier.passives.iter().enumerate() {
                                let n = NodeRef { mastery: tab, tier: t, passive: true, index: i };
                                let lvl = profile.passive_level(&p.id);
                                let playable = !skills::lines(p).is_empty();
                                node(row, assets, n, p.icon.as_deref(), &format!("{lvl}/{}", p.max_level.max(1)), open && playable, lvl > 0, false, view.selected == Some(n));
                            }
                        });
                    }
                });

            // Right: details of the selected skill.
            body.spawn(panel(assets, Node { width: Val::Px(330.0), flex_shrink: 0.0, flex_direction: FlexDirection::Column, min_height: Val::Px(0.0), ..default() }))
                .with_children(|p| ui::scroll_area(p, Node::default(), 8.0, |d| details(d, assets, title, profile, book, view)));
        });

        root.spawn(Node { column_gap: Val::Px(12.0), align_items: AlignItems::Center, ..default() }).with_children(|f| {
            f.spawn(button(assets, "Refund all points", 240.0, true, false)).observe(click(TreeAction::Refund));
            f.spawn(button(assets, "Change masteries", 240.0, true, false)).observe(click(TreeAction::ResetMasteries));
            f.spawn(button(assets, "Back (Esc)", 200.0, true, false)).observe(click(TreeAction::Back));
            f.spawn((Text::new(view.message.clone()), text(19.0), TextColor(Color::srgb(1.0, 0.55, 0.45))));
        });
    });
}

/// One skill/passive icon in its TQ2 gold frame, with its level below.
#[allow(clippy::too_many_arguments)]
fn node(row: &mut ChildSpawnerCommands, assets: &AssetServer, n: NodeRef, icon: Option<&str>, level: &str, available: bool, learned: bool, equipped: bool, selected: bool) {
    let (frame, w, h) = if n.passive { ("t_frame_gold_passivespell", 42.0, 50.0) } else { ("t_frame_gold_activespell", 46.0, 54.0) };
    let tint = if learned {
        Color::WHITE
    } else if available {
        Color::srgb(0.72, 0.72, 0.72)
    } else {
        Color::srgb(0.28, 0.28, 0.3)
    };
    row.spawn(Node { flex_direction: FlexDirection::Column, align_items: AlignItems::Center, width: Val::Px(w + 4.0), flex_shrink: 0.0, ..default() })
        .with_children(|col| {
            col.spawn((
                Node { width: Val::Px(w), height: Val::Px(h), justify_content: JustifyContent::Center, ..default() },
                ui::stretched(assets, frame, if selected { Color::srgb(1.4, 1.25, 0.8) } else if equipped { Color::srgb(1.2, 1.1, 0.7) } else { Color::WHITE }),
            ))
            .with_children(|f| {
                if let Some(icon) = icon {
                    let (top, size) = if n.passive { (8.0, 26.0) } else { (5.0, 32.0) };
                    f.spawn((
                        Node { position_type: PositionType::Absolute, top: Val::Px(top), width: Val::Px(size), height: Val::Px(size), ..default() },
                        ui::image(assets, texture(icon), tint),
                    ));
                }
            })
            .observe(click(TreeAction::Select(n)));
            col.spawn((Text::new(level.to_string()), text(11.0), TextColor(if learned { GOLD } else { TEXT })));
        });
}

fn details(d: &mut ChildSpawnerCommands, assets: &AssetServer, title: &TitleFont, profile: &Profile, book: &MasteryBook, view: &TreeView) {
    let Some(n) = view.selected else {
        d.spawn((Text::new("Select a skill or passive."), text(20.0), TextColor(TEXT)));
        d.spawn((
            Text::new("Learned active skills are offered on level-ups in runs (up to four per run) and then cast themselves; later level-ups raise them and add TQ2 modifiers. Passives raise your stats permanently. Points come from runs."),
            text(17.0),
            TextColor(TEXT),
        ));
        return;
    };
    let m = &book.0[n.mastery];
    let tier = &m.tiers[n.tier];
    if n.passive {
        let p = &tier.passives[n.index];
        let level = profile.passive_level(&p.id);
        header(d, assets, title, p.icon.as_deref(), &p.name, &format!("Passive · Tier {} · Level {level}/{}", roman(n.tier), p.max_level.max(1)));
        rich(d, render(&p.description, &skills::passive_values(p, level)), 14.0, Val::Percent(100.0));
        if skills::lines(p).is_empty() {
            d.spawn((Text::new("This passive's special effect is not in the game yet."), text(16.0), TextColor(Color::srgb(1.0, 0.6, 0.4))));
        }
        if !p.feats.is_empty() {
            d.spawn((Text::new("Feats (not in this game yet)"), title.at(18.0)));
            for f in &p.feats {
                let mut spans = vec![(format!("{}: ", f.name), GOLD)];
                spans.extend(render(&f.description, &|_| None));
                rich(d, spans, 13.0, Val::Percent(100.0));
            }
        }
        let blocked = skills::passive_blocked(profile, book, &p.id);
        let label = if level == 0 { "Learn (1 Passive Point)" } else { "Level up (1 Passive Point)" };
        d.spawn(button(assets, label, 300.0, blocked.is_none(), true)).observe(click(TreeAction::Learn));
        if let Some(why) = blocked {
            d.spawn((Text::new(why), text(16.0), TextColor(TEXT)));
        }
    } else {
        let a = &tier.actives[n.index];
        let level = profile.active_level(&a.id);
        header(d, assets, title, a.icon.as_deref(), &a.name, &format!("Active skill · Tier {} · Level {level}/{ACTIVE_MAX_LEVEL}", roman(n.tier)));
        match abilities::spec(&a.id) {
            Some(spec) => {
                let slot = Slot::new(spec, a.name.clone(), level.max(1));
                let values = slot.values(&HeroStats::default(), &super::equipment::Equipment::default());
                rich(d, render(&a.description, &values), 14.0, Val::Percent(100.0));
                d.spawn((Text::new(format!("Cooldown {:.1} s", spec.cooldown)), text(16.0), TextColor(GOLD)));
                let mods: Vec<String> = book.modifiers(a).filter(|m| !modifier_effects(&m.description).is_empty()).map(|m| m.name.clone()).collect();
                if !mods.is_empty() {
                    d.spawn((Text::new("Modifiers found in runs"), title.at(18.0)));
                    d.spawn((Text::new(mods.join(" · ")), text(15.0), TextColor(TEXT)));
                }
            }
            None => {
                rich(d, render(&a.description, &|_| None), 14.0, Val::Percent(100.0));
                d.spawn((Text::new("This skill is not in the game yet."), text(16.0), TextColor(Color::srgb(1.0, 0.6, 0.4))));
            }
        }
        let blocked = skills::active_blocked(profile, book, &a.id);
        let label = if level == 0 { "Learn (1 Active Point)" } else { "Level up (1 Active Point)" };
        d.spawn(Node { column_gap: Val::Px(10.0), ..default() }).with_children(|r| {
            r.spawn(button(assets, label, 280.0, blocked.is_none(), true)).observe(click(TreeAction::Learn));
        });
        if let Some(why) = blocked {
            d.spawn((Text::new(why), text(16.0), TextColor(TEXT)));
        }
    }
}

fn header(d: &mut ChildSpawnerCommands, assets: &AssetServer, title: &TitleFont, icon: Option<&str>, name: &str, sub: &str) {
    d.spawn(Node { align_items: AlignItems::Center, column_gap: Val::Px(12.0), ..default() }).with_children(|h| {
        if let Some(icon) = icon {
            h.spawn((Node { width: Val::Px(56.0), height: Val::Px(56.0), ..default() }, BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)), ui::image(assets, texture(icon), Color::WHITE)));
        }
        h.spawn(Node { flex_direction: FlexDirection::Column, ..default() }).with_children(|t| {
            t.spawn((Text::new(name.to_string()), title.at(22.0)));
            t.spawn((Text::new(sub.to_string()), text(16.0), TextColor(GOLD)));
        });
    });
}

/// Mastery selection: the six TQ2 masteries with their artwork.
fn chooser(commands: &mut Commands, assets: &AssetServer, title: &TitleFont, profile: &Profile, book: &MasteryBook, view: &TreeView) {
    commands.spawn(backdrop(assets, MainMenuUi)).with_children(|root| {
        let heading = if profile.masteries.is_empty() { "Choose your first Mastery" } else { "Choose your second Mastery" };
        root.spawn((Text::new(heading), title.at(42.0)));
        root.spawn((
            Text::new("Two of six masteries can be combined. The second one opens at mastery level 8 of the first."),
            text(18.0),
            TextColor(TEXT),
        ));
        root.spawn(Node { flex_grow: 1.0, column_gap: Val::Px(16.0), min_height: Val::Px(0.0), ..default() }).with_children(|body| {
            body.spawn(Node { flex_wrap: FlexWrap::Wrap, column_gap: Val::Px(10.0), row_gap: Val::Px(10.0), width: Val::Px(700.0), flex_shrink: 0.0, align_content: AlignContent::FlexStart, ..default() })
                .with_children(|grid| {
                    for (i, m) in book.0.iter().enumerate() {
                        let chosen = profile.masteries.contains(&m.id);
                        let selected = view.preview == Some(i);
                        grid.spawn((
                            Node { width: Val::Px(220.0), flex_direction: FlexDirection::Column, align_items: AlignItems::Center, padding: UiRect::all(Val::Px(8.0)), border: UiRect::all(Val::Px(if selected { 3.0 } else { 0.0 })), ..default() },
                            BackgroundColor(m.color().with_alpha(0.35)),
                            BorderColor::all(GOLD),
                        ))
                        .with_children(|card| {
                            if let Some(art) = &m.icon {
                                card.spawn((Node { width: Val::Px(180.0), height: Val::Px(180.0), ..default() }, ui::image(assets, texture(art), if chosen { Color::srgb(0.5, 0.5, 0.5) } else { Color::WHITE })))
                                    .with_children(|a| {
                                        a.spawn((
                                            Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() },
                                            sliced(assets, "t_frame_gold_big02", 36.0, Color::WHITE),
                                        ));
                                    });
                            }
                            card.spawn((Text::new(if chosen { format!("{} (chosen)", m.name) } else { m.name.clone() }), title.at(22.0), TextColor(m.color().lighter(0.35))));
                            card.spawn((Text::new(m.slogan.clone()), text(14.0), TextColor(TEXT)));
                        })
                        .observe(click(TreeAction::Preview(i)));
                    }
                });
            body.spawn(panel(assets, Node { flex_grow: 1.0, flex_direction: FlexDirection::Column, min_height: Val::Px(0.0), ..default() }))
                .with_children(|p| ui::scroll_area(p, Node::default(), 10.0, |d| match view.preview.map(|i| &book.0[i]) {
                    Some(m) => {
                        d.spawn((Text::new(m.name.clone()), title.at(36.0), TextColor(m.color().lighter(0.35))));
                        d.spawn((Text::new(m.slogan.clone()), text(19.0), TextColor(GOLD)));
                        rich(d, render(&m.description, &|_| None), 18.0, Val::Percent(100.0));
                        let skills: Vec<&str> = m.tiers.iter().flat_map(|t| t.actives.iter()).filter(|a| !a.wip).map(|a| a.name.as_str()).collect();
                        d.spawn((Text::new("Skills"), title.at(20.0)));
                        d.spawn((Text::new(skills.join(" · ")), text(16.0), TextColor(TEXT)));
                        let chosen = profile.masteries.contains(&m.id);
                        let can = !chosen && skills::can_choose_mastery(profile, book);
                        d.spawn(button(assets, format!("Choose {}", m.name), 300.0, can, true)).observe(click(TreeAction::Choose));
                    }
                    None => {
                        d.spawn((Text::new("Select a mastery to see what it offers."), text(20.0), TextColor(TEXT)));
                    }
                }));
        });
        root.spawn(Node { column_gap: Val::Px(12.0), align_items: AlignItems::Center, ..default() }).with_children(|f| {
            f.spawn(button(assets, "Back (Esc)", 200.0, true, false)).observe(click(TreeAction::Back));
            f.spawn((Text::new(view.message.clone()), text(19.0), TextColor(Color::srgb(1.0, 0.55, 0.45))));
        });
    });
}

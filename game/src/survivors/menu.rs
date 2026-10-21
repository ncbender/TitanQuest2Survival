//! Main menu with character creation, armoury (equip gear from the stash) and skill tree,
//! plus the pause screen. Gear and skills only change here, between runs.
use bevy::prelude::*;

use super::equipment::{AttackKind, Item, OffHand, ITEMS, WEAPONS};
use super::profile::{Profile, Slot};
use super::skills::{self, BRANCHES, NODES};
use super::{GameState, StartRun};
use crate::fonts::TitleFont;

#[derive(Component)]
struct MainMenuUi;
#[derive(Component)]
struct PauseUi;

/// Which page of the main menu is shown.
#[derive(Resource, Default, Clone, Copy, PartialEq)]
enum Page {
    #[default]
    Title,
    /// Character creation: choose the starting weapon (replaces the current hero).
    Hero,
    Armory,
    Skills,
}

/// The armoury slot whose stash items are listed.
#[derive(Resource, Default)]
struct ArmorySlot(Slot);

/// Menu entry picked with the mouse this frame (keys are read directly).
#[derive(Resource, Default)]
struct Clicked(Option<MenuAction>);

#[derive(Clone, Copy)]
enum MenuAction {
    Start,
    Open(Page),
    Quit,
    /// Create a new hero with `WEAPONS[i]`.
    Pick(usize),
    Back,
    SelectSlot(Slot),
    Equip(Item),
    EmptyOffHand,
    Buy(&'static str),
    ResetSkills,
}

const GOLD: Color = Color::srgb(0.95, 0.78, 0.35);
const CARD: Color = Color::srgb(0.22, 0.18, 0.12);
const MUTED: Color = Color::srgb(0.78, 0.74, 0.66);

pub fn plugin(app: &mut App) {
    app.init_resource::<Clicked>()
        .init_resource::<Page>()
        .init_resource::<ArmorySlot>()
        .add_systems(OnEnter(GameState::MainMenu), (open_menu, show_page).chain())
        .add_systems(OnExit(GameState::MainMenu), despawn::<MainMenuUi>)
        .add_systems(
            Update,
            (
                main_menu_input,
                show_page.run_if(resource_changed::<Page>.or_eager(resource_changed::<Profile>).or_eager(resource_changed::<ArmorySlot>)),
            )
                .chain()
                .run_if(in_state(GameState::MainMenu)),
        )
        .add_systems(OnEnter(GameState::Paused), show_pause)
        .add_systems(OnExit(GameState::Paused), despawn::<PauseUi>)
        .add_systems(Update, pause_input);
}

/// The menu opens on the title page; TQ2_MENU=hero/armory/skills opens a page (screenshots).
fn open_menu(mut page: ResMut<Page>) {
    *page = match std::env::var("TQ2_MENU").as_deref() {
        Ok("hero") => Page::Hero,
        Ok("armory") => Page::Armory,
        Ok("skills") => Page::Skills,
        _ => Page::Title,
    };
}

fn overlay(marker: impl Component) -> impl Bundle {
    (
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            row_gap: Val::Px(12.0),
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
        marker,
    )
}

fn text(size: f32) -> TextFont {
    TextFont { font_size: FontSize::Px(size), ..default() }
}

fn entry(label: impl Into<String>) -> impl Bundle {
    (
        Node { width: Val::Px(360.0), padding: UiRect::all(Val::Px(10.0)), justify_content: JustifyContent::Center, ..default() },
        BackgroundColor(CARD),
        children![(Text::new(label), text(24.0))],
    )
}

/// A clickable card; `selected` draws a gold border, `enabled = false` greys it out.
fn card(width: f32, selected: bool, enabled: bool) -> impl Bundle {
    (
        Node {
            width: Val::Px(width),
            padding: UiRect::axes(Val::Px(12.0), Val::Px(8.0)),
            flex_direction: FlexDirection::Column,
            border: UiRect::all(Val::Px(if selected { 2.0 } else { 0.0 })),
            ..default()
        },
        BackgroundColor(if enabled { CARD } else { Color::srgb(0.14, 0.13, 0.12) }),
        BorderColor::all(GOLD),
    )
}

fn click(action: MenuAction) -> impl Fn(On<PointerClick>, ResMut<Clicked>) {
    move |_, mut c| c.0 = Some(action)
}

fn show_page(
    mut commands: Commands,
    page: Res<Page>,
    title: Res<TitleFont>,
    profile: Res<Profile>,
    slot: Res<ArmorySlot>,
    old: Query<Entity, With<MainMenuUi>>,
) {
    for e in &old {
        commands.entity(e).despawn();
    }
    let root = commands.spawn(overlay(MainMenuUi)).id();
    let mut ui = commands.entity(root);
    match *page {
        Page::Title => {
            ui.with_children(|p| {
                p.spawn((Text::new("Titan Quest II Survivors"), title.at(60.0)));
                p.spawn((Text::new("Survive 10 minutes against the hordes of Greece"), text(20.0)));
                if profile.created {
                    p.spawn((
                        Text::new(format!(
                            "Runs: {}   Best: {:02}:{:02}   Favour: {}   Stash: {} items",
                            profile.runs,
                            profile.best_time as u32 / 60,
                            profile.best_time as u32 % 60,
                            profile.favor,
                            profile.stash.len()
                        )),
                        text(18.0),
                        TextColor(MUTED),
                    ));
                    p.spawn(entry("Start Run  (Enter)")).observe(click(MenuAction::Start));
                    p.spawn(entry("Armoury  (A)")).observe(click(MenuAction::Open(Page::Armory)));
                    p.spawn(entry("Skill Tree  (S)")).observe(click(MenuAction::Open(Page::Skills)));
                }
                p.spawn(entry(if profile.created { "New Hero  (N)" } else { "New Hero  (Enter)" }))
                    .observe(click(MenuAction::Open(Page::Hero)));
                p.spawn(entry("Quit  (Esc)")).observe(click(MenuAction::Quit));
                p.spawn((Text::new("WASD move · Q/E rotate camera · mouse wheel zoom · 1–3 choose upgrade · Esc pause"), text(16.0)));
            });
        }
        Page::Hero => {
            ui.with_children(|p| {
                p.spawn((Text::new("Create Your Hero"), title.at(48.0)));
                p.spawn((Text::new("Choose your starting weapon – it decides your auto-attack"), text(20.0)));
                if profile.created {
                    p.spawn((Text::new("This replaces your current hero, stash and skills!"), text(18.0), TextColor(Color::srgb(1.0, 0.45, 0.35))));
                }
                for (i, w) in WEAPONS.iter().enumerate() {
                    let reach = if w.kind == AttackKind::Sweep { format!("{:.1} m reach", w.speed_or_radius) } else { format!("{:.0} m range", w.range) };
                    let hands = if w.two_handed { "two-handed" } else { "one-handed" };
                    p.spawn(card(600.0, false, true))
                        .with_children(|c| {
                            c.spawn((Text::new(format!("{}  {}", i + 1, w.name)), title.at(26.0)));
                            c.spawn((Text::new(format!("{hands} – {}", w.attack_text())), text(19.0)));
                            c.spawn((Text::new(format!("{:.0} damage · every {:.2} s · {reach}", w.damage, w.cooldown)), text(16.0), TextColor(MUTED)));
                        })
                        .observe(click(MenuAction::Pick(i)));
                }
                p.spawn((Text::new("1–5 or click to choose · Esc back"), text(16.0)));
            });
        }
        Page::Armory => armory_page(&mut ui, &title, &profile, slot.0),
        Page::Skills => skills_page(&mut ui, &title, &profile),
    }
}

fn armory_page(ui: &mut EntityCommands, title: &TitleFont, profile: &Profile, selected: Slot) {
    let eq = profile.equipment();
    let stance = match eq.off {
        _ if eq.main.two_handed => "two-handed",
        None => "one-handed, off hand empty",
        Some(OffHand::Weapon(_)) => "dual wielding: 30 % faster attacks",
        Some(OffHand::Shield(_)) => "sword and shield",
        Some(OffHand::Talisman(_)) => "weapon and talisman",
    };
    let off_text = match eq.off {
        Some(o) => o.item().name().to_string(),
        None if eq.main.two_handed => "— (two-handed weapon)".into(),
        None => "— empty".into(),
    };
    let slots = [
        (Slot::Main, "Main hand", eq.main.name.to_string()),
        (Slot::Off, "Off hand", off_text),
        (Slot::Armor, "Armour", eq.armor.name.to_string()),
    ];
    // Stash items for the selected slot, each listed once with the copies still free.
    let mut listed: Vec<(Item, usize)> = vec![];
    for item in ITEMS.iter().copied().filter(|i| selected.fits(*i) && profile.copies(*i) > 0) {
        listed.push((item, profile.free_copies(item, &eq, selected)));
    }
    ui.with_children(|p| {
        p.spawn((Text::new("Armoury"), title.at(48.0)));
        p.spawn((
            Text::new(format!(
                "{stance} · attack every {:.2} s · {:.0} % less damage taken · skills x{:.1}",
                eq.cooldown(),
                eq.damage_reduction() * 100.0,
                eq.skill_damage()
            )),
            text(18.0),
            TextColor(MUTED),
        ));
        p.spawn(Node { column_gap: Val::Px(12.0), ..default() }).with_children(|row| {
            for (slot, label, item) in slots {
                row.spawn(card(260.0, slot == selected, true))
                    .with_children(|c| {
                        c.spawn((Text::new(label), text(16.0), TextColor(MUTED)));
                        c.spawn((Text::new(item), text(21.0)));
                    })
                    .observe(click(MenuAction::SelectSlot(slot)));
            }
        });
        p.spawn((Text::new("Stash – click to equip"), title.at(24.0)));
        if selected == Slot::Off {
            let enabled = !eq.main.two_handed;
            p.spawn(card(700.0, false, enabled))
                .with_children(|c| {
                    c.spawn((Text::new("Empty off hand"), text(19.0)));
                })
                .observe(click(MenuAction::EmptyOffHand));
            if !enabled {
                p.spawn((Text::new("A two-handed weapon needs both hands – equip a one-handed weapon first."), text(17.0), TextColor(MUTED)));
            }
        }
        if listed.is_empty() {
            p.spawn((Text::new("Nothing for this slot yet – open chests in runs."), text(18.0), TextColor(MUTED)));
        }
        for (item, free) in listed {
            let usable = free > 0 && (selected != Slot::Off || !eq.main.two_handed);
            p.spawn(card(700.0, false, usable))
                .with_children(|c| {
                    let copies = profile.copies(item);
                    let count = if copies > 1 { format!("  ×{copies}") } else { String::new() };
                    c.spawn((Text::new(format!("{}{count}", item.title())), text(19.0), TextColor(if usable { Color::WHITE } else { MUTED })));
                })
                .observe(click(MenuAction::Equip(item)));
        }
        p.spawn((Text::new("Esc back"), text(16.0)));
    });
}

fn skills_page(ui: &mut EntityCommands, title: &TitleFont, profile: &Profile) {
    ui.with_children(|p| {
        p.spawn((Text::new("Skill Tree"), title.at(48.0)));
        p.spawn((Text::new(format!("Favour to spend: {}   (earned in runs: levels, minutes survived, victory)", profile.favor)), text(19.0)));
        p.spawn(Node { column_gap: Val::Px(12.0), align_items: AlignItems::FlexStart, ..default() }).with_children(|row| {
            for (b, branch) in BRANCHES.iter().enumerate() {
                row.spawn(Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(8.0), width: Val::Px(290.0), ..default() })
                    .with_children(|col| {
                        col.spawn((Text::new(*branch), title.at(24.0)));
                        for n in NODES.iter().filter(|n| n.branch == b) {
                            let rank = profile.rank(n.id);
                            let blocked = skills::blocked(profile, n.id);
                            col.spawn(card(290.0, rank > 0, blocked.is_none()))
                                .with_children(|c| {
                                    c.spawn((Text::new(format!("{}  {}/{}", n.name, rank, n.max_rank)), text(20.0)));
                                    c.spawn((Text::new(n.effect), text(16.0)));
                                    let state = blocked.unwrap_or_else(|| format!("click to learn · {} favour", n.cost));
                                    c.spawn((Text::new(state), text(15.0), TextColor(MUTED)));
                                })
                                .observe(click(MenuAction::Buy(n.id)));
                        }
                    });
            }
        });
        p.spawn(entry("Reset skills (refund all)")).observe(click(MenuAction::ResetSkills));
        p.spawn((Text::new("Esc back"), text(16.0)));
    });
}

fn main_menu_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut clicked: ResMut<Clicked>,
    mut page: ResMut<Page>,
    mut slot: ResMut<ArmorySlot>,
    mut profile: ResMut<Profile>,
    mut start: MessageWriter<StartRun>,
    mut exit: MessageWriter<AppExit>,
) {
    let pressed = |k| keys.just_pressed(k);
    let key_action = match *page {
        Page::Title if pressed(KeyCode::Enter) => Some(if profile.created { MenuAction::Start } else { MenuAction::Open(Page::Hero) }),
        Page::Title if pressed(KeyCode::KeyA) && profile.created => Some(MenuAction::Open(Page::Armory)),
        Page::Title if pressed(KeyCode::KeyS) && profile.created => Some(MenuAction::Open(Page::Skills)),
        Page::Title if pressed(KeyCode::KeyN) => Some(MenuAction::Open(Page::Hero)),
        Page::Title if pressed(KeyCode::Escape) => Some(MenuAction::Quit),
        _ if pressed(KeyCode::Escape) => Some(MenuAction::Back),
        Page::Hero => [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3, KeyCode::Digit4, KeyCode::Digit5]
            .iter()
            .position(|k| pressed(*k))
            .filter(|i| *i < WEAPONS.len())
            .map(MenuAction::Pick),
        _ => None,
    };
    let Some(action) = clicked.0.take().or(key_action) else { return };
    match action {
        MenuAction::Start => {
            start.write(StartRun);
        }
        MenuAction::Open(p) => *page = p,
        MenuAction::Back => *page = Page::Title,
        MenuAction::Quit => {
            exit.write(AppExit::Success);
        }
        MenuAction::Pick(i) => {
            *profile = Profile::new_hero(WEAPONS[i]);
            *page = Page::Title;
        }
        MenuAction::SelectSlot(s) => slot.0 = s,
        MenuAction::Equip(item) => {
            let mut eq = profile.equipment();
            if profile.free_copies(item, &eq, slot.0) == 0 {
                return;
            }
            let ok = match (slot.0, item) {
                (Slot::Main, Item::Weapon(w)) => {
                    eq.equip_main(w);
                    // The off hand may hold the only copy of the new main weapon.
                    if eq.off.is_some_and(|o| o.item().same(item)) && profile.copies(item) < 2 {
                        eq.off = None;
                    }
                    true
                }
                (Slot::Off, _) => eq.equip_off(OffHand::from_item(item)),
                (Slot::Armor, Item::Armor(a)) => {
                    eq.armor = a;
                    true
                }
                _ => false,
            };
            if ok {
                profile.set_equipment(&eq);
            }
        }
        MenuAction::EmptyOffHand => {
            let mut eq = profile.equipment();
            eq.off = None;
            profile.set_equipment(&eq);
        }
        MenuAction::Buy(id) => {
            skills::buy(&mut profile, id);
        }
        MenuAction::ResetSkills => skills::reset(&mut profile),
    }
}

fn show_pause(mut commands: Commands, title: Res<TitleFont>) {
    commands.spawn(overlay(PauseUi)).with_children(|p| {
        p.spawn((Text::new("Paused"), title.at(48.0)));
        p.spawn((Text::new("Esc – resume     M – main menu (loot and favour are kept)"), text(22.0)));
    });
}

fn pause_input(keys: Res<ButtonInput<KeyCode>>, state: Res<State<GameState>>, mut next: ResMut<NextState<GameState>>) {
    match state.get() {
        GameState::Playing if keys.just_pressed(KeyCode::Escape) => next.set(GameState::Paused),
        GameState::Paused if keys.just_pressed(KeyCode::Escape) => next.set(GameState::Playing),
        GameState::Paused | GameState::GameOver if keys.just_pressed(KeyCode::KeyM) => next.set(GameState::MainMenu),
        _ => {}
    }
}

fn despawn<T: Component>(mut commands: Commands, q: Query<Entity, With<T>>) {
    for e in &q {
        commands.entity(e).despawn();
    }
}

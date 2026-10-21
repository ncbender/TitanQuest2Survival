//! Main menu: characters (choose, create, delete), armoury (equip rolled items from the shared
//! stash) and the mastery screen (skilltree.rs), plus the pause screen. Gear and skills only
//! change here, between runs.
use bevy::prelude::*;

use super::arenas::{self, ARENAS};
use super::inventory;
use super::equipment::{AttackKind, Item, WEAPONS};
use super::items::AffixBook;
use super::mastery_data::{MasteryBook, GOLD, TEXT};
use super::profile::Profile;
use super::settings::SettingsOpen;
use super::skilltree::{self, TreeView};
use super::ui::{backdrop, button, panel, text};
use super::{GameState, StartRun};
use crate::fonts::TitleFont;

#[derive(Component)]
pub struct MainMenuUi;
#[derive(Component)]
struct PauseUi;

/// Which page of the main menu is shown.
#[derive(Resource, Default, Clone, Copy, PartialEq)]
pub enum Page {
    #[default]
    Title,
    /// Character list: choose, create, delete.
    Characters,
    /// Character creation: choose the starting weapon.
    Hero,
    Armory,
    /// TQ2 mastery screen (skilltree.rs).
    Skills,
    /// Arena selection before a run.
    Arenas,
}

/// A character whose delete button was pressed once (press again to confirm).
#[derive(Resource, Default)]
struct DeleteArmed(Option<usize>);

/// Menu entry picked with the mouse this frame (keys are read directly).
#[derive(Resource, Default)]
struct Clicked(Option<MenuAction>);

#[derive(Clone, Copy)]
enum MenuAction {
    /// Start a run in `ARENAS[i]`.
    Start(usize),
    Open(Page),
    Quit,
    /// Create a new hero with `WEAPONS[i]`.
    Pick(usize),
    Back,
    SelectCharacter(usize),
    DeleteCharacter(usize),
    Settings,
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Clicked>()
        .init_resource::<Page>()
        .init_resource::<DeleteArmed>()
        .add_systems(OnEnter(GameState::MainMenu), (open_menu, show_page).chain())
        .add_systems(OnExit(GameState::MainMenu), despawn::<MainMenuUi>)
        .add_systems(
            Update,
            (
                main_menu_input,
                show_page.run_if(
                    resource_changed::<Page>
                        .or_eager(resource_changed::<Profile>)
                        .or_eager(resource_changed::<DeleteArmed>)
                        .or_eager(resource_changed::<TreeView>),
                ),
            )
                .chain()
                .run_if(in_state(GameState::MainMenu)),
        )
        .add_systems(OnEnter(GameState::Paused), show_pause)
        .add_systems(OnExit(GameState::Paused), despawn::<PauseUi>)
        .add_systems(Update, pause_input);
}

/// The menu opens on the title page; TQ2_MENU=hero/armory/skills/choose/characters opens a page
/// (screenshots).
fn open_menu(mut page: ResMut<Page>, mut view: ResMut<TreeView>, mut settings_open: ResMut<SettingsOpen>) {
    *page = match std::env::var("TQ2_MENU").as_deref() {
        Ok("hero") => Page::Hero,
        Ok("armory") => Page::Armory,
        Ok("skills") => Page::Skills,
        Ok("characters") => Page::Characters,
        Ok("arenas") => Page::Arenas,
        Ok("settings") => {
            settings_open.0 = true;
            Page::Title
        }
        Ok("choose") => {
            view.chooser = true;
            view.preview = Some(0);
            Page::Skills
        }
        _ => Page::Title,
    };
    // Test hook: TQ2_MENU_SELECT=<tier>,<index>[,p] selects a node on the mastery screen.
    if let Ok(sel) = std::env::var("TQ2_MENU_SELECT") {
        let parts: Vec<&str> = sel.split(',').collect();
        if let (Some(t), Some(i)) = (parts.first().and_then(|s| s.parse().ok()), parts.get(1).and_then(|s| s.parse().ok())) {
            view.selected = Some(skilltree::NodeRef { mastery: view.tab.unwrap_or(0), tier: t, passive: parts.get(2) == Some(&"p"), index: i });
        }
    }
}

/// The translucent overlay of the title and hero pages (the 3D menu scene stays visible).
fn overlay() -> impl Bundle {
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
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.5)),
        MainMenuUi,
    )
}

fn click(action: MenuAction) -> impl Fn(On<PointerClick>, ResMut<Clicked>) {
    move |_, mut c| c.0 = Some(action)
}

/// "Storm + Earth" or a hint.
fn mastery_names(masteries: &[String], book: &MasteryBook) -> String {
    let names: Vec<String> = masteries.iter().filter_map(|m| book.mastery(m)).map(|m| m.name.clone()).collect();
    if names.is_empty() { "no mastery yet".into() } else { names.join(" + ") }
}

#[allow(clippy::too_many_arguments)]
fn show_page(
    mut commands: Commands,
    assets: Res<AssetServer>,
    page: Res<Page>,
    title: Res<TitleFont>,
    profile: Res<Profile>,
    book: Res<MasteryBook>,
    affixes: Res<AffixBook>,
    view: Res<TreeView>,
    armed: Res<DeleteArmed>,
    inv: Res<inventory::InvState>,
    preview: Res<inventory::Preview>,
    old: Query<Entity, With<MainMenuUi>>,
) {
    for e in &old {
        commands.entity(e).despawn();
    }
    match *page {
        Page::Title => {
            commands.spawn(overlay()).with_children(|p| {
                p.spawn((Text::new("Titan Quest II Survivors"), title.at(60.0)));
                p.spawn((Text::new("Survive 10 minutes against the hordes of Greece"), text(20.0)));
                if profile.created {
                    p.spawn((Text::new(profile.name.clone()), title.at(30.0), TextColor(GOLD)));
                    p.spawn((
                        Text::new(format!(
                            "{}   ·   Runs {}   ·   Best {:02}:{:02}   ·   Points {} active / {} passive",
                            mastery_names(&profile.masteries, &book),
                            profile.runs,
                            profile.best_time as u32 / 60,
                            profile.best_time as u32 % 60,
                            profile.active_points,
                            profile.passive_points
                        )),
                        text(18.0),
                        TextColor(TEXT),
                    ));
                    p.spawn(button(&assets, "Start Run  (Enter)", 380.0, true, true)).observe(click(MenuAction::Open(Page::Arenas)));
                    p.spawn(button(&assets, "Masteries  (S)", 380.0, true, false)).observe(click(MenuAction::Open(Page::Skills)));
                    p.spawn(button(&assets, "Inventory  (I)", 380.0, true, false)).observe(click(MenuAction::Open(Page::Armory)));
                    p.spawn(button(&assets, "Characters  (C)", 380.0, true, false)).observe(click(MenuAction::Open(Page::Characters)));
                } else {
                    p.spawn(button(&assets, "New Hero  (Enter)", 380.0, true, true)).observe(click(MenuAction::Open(Page::Hero)));
                }
                p.spawn(button(&assets, "Settings  (O)", 380.0, true, false)).observe(click(MenuAction::Settings));
                p.spawn(button(&assets, "Quit  (Esc)", 380.0, true, false)).observe(click(MenuAction::Quit));
                p.spawn((Text::new("WASD move · Q/E rotate camera · mouse wheel zoom · 1–3 choose upgrade · Esc pause"), text(16.0)));
            });
        }
        Page::Arenas => {
            commands.spawn(backdrop(&assets, MainMenuUi)).with_children(|root| {
                root.spawn((Text::new("Choose an Arena"), title.at(46.0)));
                root.spawn((Text::new("Survive ten minutes to open the next arena. Later arenas are deadlier and drop better gear."), text(18.0), TextColor(TEXT)));
                root.spawn(Node { column_gap: Val::Px(14.0), flex_grow: 1.0, align_items: AlignItems::FlexStart, ..default() }).with_children(|row| {
                    for (i, a) in ARENAS.iter().enumerate() {
                        let open = arenas::unlocked(a, &profile.cleared);
                        let present = arenas::available(a);
                        let chosen = a.id == profile.arena || (profile.arena.is_empty() && i == 0);
                        row.spawn(panel(&assets, Node { width: Val::Px(296.0), min_height: Val::Px(440.0), ..default() }))
                            .with_children(|frame| {
                            // Extra inset so text clears the gold frame.
                            frame.spawn(Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(8.0), padding: UiRect::all(Val::Px(12.0)), flex_grow: 1.0, ..default() }).with_children(|c| {
                                c.spawn((Text::new(a.name), title.at(26.0), TextColor(if open { GOLD } else { Color::srgb(0.55, 0.55, 0.55) })));
                                c.spawn((Text::new(a.blurb), text(16.0), TextColor(TEXT)));
                                let mut names: Vec<&str> = vec![];
                                for sp in a.roster {
                                    if !names.contains(&sp.def.name) {
                                        names.push(sp.def.name);
                                    }
                                }
                                c.spawn((Text::new(format!("Enemies: {}", names.join(", "))), text(15.0)));
                                c.spawn((Text::new(format!("Boss: {}", a.boss.name)), text(15.0), TextColor(Color::srgb(0.95, 0.45, 0.35))));
                                c.spawn((Text::new(format!("Enemy strength x{:.1}   ·   Loot x{:.1}", a.tier, a.luck)), text(15.0), TextColor(TEXT)));
                                if profile.cleared.iter().any(|id| id == a.id) {
                                    c.spawn((Text::new("Survived"), text(16.0), TextColor(Color::srgb(0.5, 0.85, 0.45))));
                                }
                                c.spawn(Node { flex_grow: 1.0, ..default() });
                                if !present {
                                    c.spawn((Text::new("Map not extracted yet (run the asset update)"), text(15.0), TextColor(Color::srgb(0.85, 0.5, 0.4))));
                                } else if !open {
                                    c.spawn((Text::new(format!("Locked: survive {}", ARENAS[i - 1].name)), text(15.0), TextColor(Color::srgb(0.6, 0.6, 0.6))));
                                } else {
                                    c.spawn(button(&assets, if chosen { "Enter  (Enter)" } else { "Enter" }, 236.0, true, true)).observe(click(MenuAction::Start(i)));
                                }
                            });
                            });
                    }
                });
                root.spawn(button(&assets, "Back (Esc)", 200.0, true, false)).observe(click(MenuAction::Back));
            });
        }
        Page::Characters => {
            commands.spawn(backdrop(&assets, MainMenuUi)).with_children(|root| {
                root.spawn((Text::new("Characters"), title.at(46.0)));
                root.spawn((Text::new("Each hero has its own masteries and skills. The stash is shared by all heroes."), text(18.0), TextColor(TEXT)));
                root.spawn(Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(10.0), flex_grow: 1.0, ..default() }).with_children(|list| {
                    for (i, c) in profile.characters.iter().enumerate() {
                        let active = i == profile.active;
                        list.spawn(panel(&assets, Node { width: Val::Px(900.0), column_gap: Val::Px(18.0), align_items: AlignItems::Center, ..default() }))
                            .with_children(|row| {
                                row.spawn(Node { flex_direction: FlexDirection::Column, flex_grow: 1.0, ..default() }).with_children(|t| {
                                    t.spawn((Text::new(if active { format!("{}  (playing)", c.name) } else { c.name.clone() }), title.at(26.0), TextColor(if active { GOLD } else { Color::WHITE })));
                                    t.spawn((
                                        Text::new(format!(
                                            "{}   ·   Runs {}   ·   Best {:02}:{:02}   ·   {} skills learned",
                                            mastery_names(&c.masteries, &book),
                                            c.runs,
                                            c.best_time as u32 / 60,
                                            c.best_time as u32 % 60,
                                            c.actives.len()
                                        )),
                                        text(17.0),
                                        TextColor(TEXT),
                                    ));
                                });
                                if !active {
                                    row.spawn(button(&assets, "Play", 140.0, true, true)).observe(click(MenuAction::SelectCharacter(i)));
                                }
                                let confirm = armed.0 == Some(i);
                                row.spawn(button(&assets, if confirm { "Really delete?" } else { "Delete" }, 180.0, true, false)).observe(click(MenuAction::DeleteCharacter(i)));
                            });
                    }
                    list.spawn(button(&assets, "New Hero  (N)", 300.0, true, true)).observe(click(MenuAction::Open(Page::Hero)));
                });
                root.spawn(button(&assets, "Back (Esc)", 200.0, true, false)).observe(click(MenuAction::Back));
            });
        }
        Page::Hero => {
            commands.spawn(backdrop(&assets, MainMenuUi)).with_children(|p| {
                p.spawn((Text::new("Create Your Hero"), title.at(44.0)));
                p.spawn((Text::new("Choose your starting weapon – it decides your auto-attack. Then choose a mastery."), text(18.0), TextColor(TEXT)));
                if profile.created {
                    p.spawn((Text::new("Your other heroes and the shared stash are kept."), text(16.0), TextColor(TEXT)));
                }
                p.spawn(Node { column_gap: Val::Px(12.0), margin: UiRect::top(Val::Px(16.0)), flex_grow: 1.0, align_items: AlignItems::FlexStart, height: Val::Px(360.0), ..default() }).with_children(|row| {
                    for (i, w) in WEAPONS.iter().enumerate() {
                        let reach = if w.kind == AttackKind::Sweep { format!("{:.1} m reach", w.speed_or_radius) } else { format!("{:.0} m range", w.range) };
                        let hands = if w.two_handed { "Two-handed" } else { "One-handed" };
                        row.spawn(panel(&assets, Node { width: Val::Px(234.0), height: Val::Px(350.0), flex_direction: FlexDirection::Column, align_items: AlignItems::Center, row_gap: Val::Px(8.0), ..default() }))
                            .with_children(|c| {
                                c.spawn((Text::new(format!("{}", i + 1)), title.at(20.0), TextColor(GOLD)));
                                c.spawn((Node { width: Val::Px(70.0), height: Val::Px(124.0), flex_shrink: 0.0, ..default() }, ImageNode { image_mode: NodeImageMode::Auto, ..ImageNode::new(assets.load(Item::Weapon(w).icon())) }));
                                c.spawn((Text::new(w.name), title.at(22.0), TextLayout::justify(Justify::Center)));
                                c.spawn((Text::new(format!("{hands} – {}", w.attack_text())), text(15.0), TextColor(TEXT), TextLayout::justify(Justify::Center)));
                                c.spawn((Text::new(format!("{:.0} damage · every {:.2} s\n{reach}", w.damage, w.cooldown)), text(14.0), TextColor(GOLD), TextLayout::justify(Justify::Center)));
                            })
                            .observe(click(MenuAction::Pick(i)));
                    }
                });
                p.spawn(Node { align_items: AlignItems::Center, column_gap: Val::Px(18.0), ..default() }).with_children(|f| {
                    f.spawn(button(&assets, "Back (Esc)", 200.0, true, false)).observe(click(MenuAction::Back));
                    f.spawn((Text::new("1–5 or click to choose"), text(16.0), TextColor(TEXT)));
                });
            });
        }
        Page::Armory => inventory::build(&mut commands, &inventory::Ctx { assets: &assets, title: &title, profile: &profile, affixes: &affixes, state: &inv, preview: &preview }),
        Page::Skills => skilltree::build(&mut commands, &assets, &title, &profile, &book, &view),
    }
}

#[allow(clippy::too_many_arguments)]
fn main_menu_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut clicked: ResMut<Clicked>,
    mut page: ResMut<Page>,
    mut armed: ResMut<DeleteArmed>,
    mut profile: ResMut<Profile>,
    mut view: ResMut<TreeView>,
    mut settings_open: ResMut<SettingsOpen>,
    mut map: ResMut<crate::map::SelectedMap>,
    mut start: MessageWriter<StartRun>,
    mut exit: MessageWriter<AppExit>,
) {
    let pressed = |k| keys.just_pressed(k);
    // The settings window takes Esc and blocks the menu behind it.
    if settings_open.0 {
        if pressed(KeyCode::Escape) {
            settings_open.0 = false;
        }
        clicked.0 = None;
        return;
    }
    let key_action = match *page {
        Page::Title if pressed(KeyCode::Enter) => Some(if profile.created { MenuAction::Open(Page::Arenas) } else { MenuAction::Open(Page::Hero) }),
        Page::Arenas if pressed(KeyCode::Enter) => Some(MenuAction::Start(ARENAS.iter().position(|a| a.id == profile.arena).unwrap_or(0))),
        Page::Title if (pressed(KeyCode::KeyI) || pressed(KeyCode::KeyA)) && profile.created => Some(MenuAction::Open(Page::Armory)),
        Page::Title if pressed(KeyCode::KeyS) && profile.created => Some(MenuAction::Open(Page::Skills)),
        Page::Title if pressed(KeyCode::KeyC) && profile.created => Some(MenuAction::Open(Page::Characters)),
        Page::Title | Page::Characters if pressed(KeyCode::KeyN) => Some(MenuAction::Open(Page::Hero)),
        Page::Title if pressed(KeyCode::KeyO) => Some(MenuAction::Settings),
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
    if !matches!(action, MenuAction::DeleteCharacter(_)) && armed.0.is_some() {
        armed.0 = None;
    }
    match action {
        MenuAction::Start(i) => {
            let a = &ARENAS[i];
            if arenas::unlocked(a, &profile.cleared) && arenas::available(a) {
                if profile.arena != a.id {
                    profile.arena = a.id.to_string();
                }
                map.0 = arenas::map_for_run(a);
                start.write(StartRun);
            }
        }
        MenuAction::Open(p) => *page = p,
        MenuAction::Back => {
            // The mastery chooser closes first (once a mastery exists).
            if *page == Page::Skills && view.chooser && !profile.masteries.is_empty() {
                view.chooser = false;
            } else {
                *page = Page::Title;
            }
        }
        MenuAction::Quit => {
            exit.write(AppExit::Success);
        }
        MenuAction::Pick(i) => {
            // A new hero starts by choosing a mastery.
            profile.new_hero(WEAPONS[i]);
            *view = TreeView { chooser: true, ..default() };
            *page = Page::Skills;
        }
        MenuAction::SelectCharacter(i) => {
            profile.active = i;
            *view = TreeView::default();
            *page = Page::Title;
        }
        MenuAction::DeleteCharacter(i) => {
            if armed.0 == Some(i) {
                profile.delete_character(i);
                armed.0 = None;
                if profile.characters.is_empty() {
                    *page = Page::Title;
                }
            } else {
                armed.0 = Some(i);
            }
        }
        MenuAction::Settings => settings_open.0 = true,
    }
}

fn show_pause(mut commands: Commands, title: Res<TitleFont>) {
    commands
        .spawn((
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
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.5)),
            PauseUi,
        ))
        .with_children(|p| {
            p.spawn((Text::new("Paused"), title.at(48.0)));
            p.spawn((Text::new("Esc – resume     O – settings     M – main menu (loot and points are kept)"), text(22.0)));
        });
}

fn pause_input(keys: Res<ButtonInput<KeyCode>>, state: Res<State<GameState>>, mut next: ResMut<NextState<GameState>>, mut settings_open: ResMut<SettingsOpen>) {
    if settings_open.0 && *state.get() == GameState::Paused {
        if keys.just_pressed(KeyCode::Escape) {
            settings_open.0 = false;
        }
        return;
    }
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

//! Main menu with character creation, armoury (equip gear from the stash) and the mastery
//! screen (skilltree.rs), plus the pause screen. Gear and skills only change here, between runs.
use bevy::prelude::*;

use super::equipment::{AttackKind, Item, OffHand, ITEMS, WEAPONS};
use super::mastery_data::{MasteryBook, GOLD, TEXT};
use super::profile::{Profile, Slot};
use super::skilltree::{self, TreeView};
use super::ui::{self, backdrop, button, panel, sliced, text};
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
    /// Character creation: choose the starting weapon (replaces the current hero).
    Hero,
    Armory,
    /// TQ2 mastery screen (skilltree.rs).
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
}

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
                show_page.run_if(
                    resource_changed::<Page>
                        .or_eager(resource_changed::<Profile>)
                        .or_eager(resource_changed::<ArmorySlot>)
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

/// The menu opens on the title page; TQ2_MENU=hero/armory/skills/choose opens a page (screenshots).
fn open_menu(mut page: ResMut<Page>, mut view: ResMut<TreeView>) {
    *page = match std::env::var("TQ2_MENU").as_deref() {
        Ok("hero") => Page::Hero,
        Ok("armory") => Page::Armory,
        Ok("skills") => Page::Skills,
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


#[allow(clippy::too_many_arguments)]
fn show_page(
    mut commands: Commands,
    assets: Res<AssetServer>,
    page: Res<Page>,
    title: Res<TitleFont>,
    profile: Res<Profile>,
    book: Res<MasteryBook>,
    view: Res<TreeView>,
    slot: Res<ArmorySlot>,
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
                    let masteries: Vec<String> = profile.masteries.iter().filter_map(|m| book.mastery(m)).map(|m| m.name.clone()).collect();
                    p.spawn((
                        Text::new(format!(
                            "{}   Runs: {}   Best: {:02}:{:02}   Points: {} active / {} passive",
                            if masteries.is_empty() { "No mastery yet".to_string() } else { masteries.join(" + ") },
                            profile.runs,
                            profile.best_time as u32 / 60,
                            profile.best_time as u32 % 60,
                            profile.active_points,
                            profile.passive_points
                        )),
                        text(18.0),
                        TextColor(GOLD),
                    ));
                    p.spawn(button(&assets, "Start Run  (Enter)", 380.0, true, true)).observe(click(MenuAction::Start));
                    p.spawn(button(&assets, "Masteries  (S)", 380.0, true, false)).observe(click(MenuAction::Open(Page::Skills)));
                    p.spawn(button(&assets, "Armoury  (A)", 380.0, true, false)).observe(click(MenuAction::Open(Page::Armory)));
                }
                let new = if profile.created { "New Hero  (N)" } else { "New Hero  (Enter)" };
                p.spawn(button(&assets, new, 380.0, true, !profile.created)).observe(click(MenuAction::Open(Page::Hero)));
                p.spawn(button(&assets, "Quit  (Esc)", 380.0, true, false)).observe(click(MenuAction::Quit));
                p.spawn((Text::new("WASD move · Q/E rotate camera · mouse wheel zoom · 1–3 choose upgrade · Esc pause"), text(16.0)));
            });
        }
        Page::Hero => {
            commands.spawn(overlay()).with_children(|p| {
                p.spawn((Text::new("Create Your Hero"), title.at(48.0)));
                p.spawn((Text::new("Choose your starting weapon – it decides your auto-attack. Then choose a mastery."), text(20.0)));
                if profile.created {
                    p.spawn((Text::new("This replaces your current hero, stash and masteries!"), text(18.0), TextColor(Color::srgb(1.0, 0.45, 0.35))));
                }
                for (i, w) in WEAPONS.iter().enumerate() {
                    let reach = if w.kind == AttackKind::Sweep { format!("{:.1} m reach", w.speed_or_radius) } else { format!("{:.0} m range", w.range) };
                    let hands = if w.two_handed { "two-handed" } else { "one-handed" };
                    p.spawn(panel(&assets, Node { width: Val::Px(640.0), column_gap: Val::Px(14.0), align_items: AlignItems::Center, ..default() }))
                        .with_children(|c| {
                            c.spawn((Node { width: Val::Px(44.0), height: Val::Px(78.0), ..default() }, ui::image(&assets, Item::Weapon(w).icon(), Color::WHITE)));
                            c.spawn(Node { flex_direction: FlexDirection::Column, ..default() }).with_children(|t| {
                                t.spawn((Text::new(format!("{}  {}", i + 1, w.name)), title.at(24.0)));
                                t.spawn((Text::new(format!("{hands} – {}", w.attack_text())), text(18.0)));
                                t.spawn((Text::new(format!("{:.0} damage · every {:.2} s · {reach}", w.damage, w.cooldown)), text(15.0), TextColor(TEXT)));
                            });
                        })
                        .observe(click(MenuAction::Pick(i)));
                }
                p.spawn((Text::new("1–5 or click to choose · Esc back"), text(16.0)));
            });
        }
        Page::Armory => armory_page(&mut commands, &assets, &title, &profile, slot.0),
        Page::Skills => skilltree::build(&mut commands, &assets, &title, &profile, &book, &view),
    }
}

/// An item's TQ2 icon, scaled to fit without stretching.
fn item_image(assets: &AssetServer, item: Item) -> ImageNode {
    ImageNode { image_mode: NodeImageMode::Auto, ..ImageNode::new(assets.load(item.icon())) }
}

fn armory_page(commands: &mut Commands, assets: &AssetServer, title: &TitleFont, profile: &Profile, selected: Slot) {
    let eq = profile.equipment();
    let stance = match eq.off {
        _ if eq.main.two_handed => "two-handed",
        None => "one-handed, off hand empty",
        Some(OffHand::Weapon(_)) => "dual wielding: 30 % faster attacks",
        Some(OffHand::Shield(_)) => "weapon and shield",
        Some(OffHand::Talisman(_)) => "weapon and talisman",
    };
    let slots = [
        (Slot::Main, "Main hand", Some(Item::Weapon(eq.main))),
        (Slot::Off, "Off hand", eq.off.map(OffHand::item)),
        (Slot::Armor, "Armour", Some(Item::Armor(eq.armor))),
    ];
    let listed: Vec<(Item, usize)> = ITEMS.iter().copied().filter(|i| selected.fits(*i) && profile.copies(*i) > 0).map(|i| (i, profile.free_copies(i, &eq, selected))).collect();
    commands.spawn(backdrop(assets, MainMenuUi)).with_children(|root| {
        root.spawn((Text::new("Armoury"), title.at(46.0)));
        root.spawn((
            Text::new(format!(
                "{stance} · attack every {:.2} s · {:.0} % less damage taken · skills ×{:.1}",
                eq.cooldown(),
                eq.damage_reduction() * 100.0,
                eq.skill_damage()
            )),
            text(19.0),
            TextColor(GOLD),
        ));
        root.spawn(Node { column_gap: Val::Px(18.0), flex_grow: 1.0, min_height: Val::Px(0.0), ..default() }).with_children(|body| {
            // Equipped slots.
            body.spawn(panel(assets, Node { width: Val::Px(420.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(14.0), ..default() }))
                .with_children(|l| {
                    l.spawn((Text::new("Equipped"), title.at(26.0)));
                    for (slot, label, item) in slots {
                        let border = if slot == selected { 3.0 } else { 0.0 };
                        l.spawn((
                            Node { column_gap: Val::Px(14.0), align_items: AlignItems::Center, padding: UiRect::all(Val::Px(8.0)), border: UiRect::all(Val::Px(border)), ..default() },
                            BorderColor::all(GOLD),
                            BackgroundColor(Color::srgba(0.1, 0.09, 0.08, 0.8)),
                        ))
                        .with_children(|r| {
                            r.spawn((Node { width: Val::Px(78.0), height: Val::Px(78.0), padding: UiRect::all(Val::Px(10.0)), ..default() }, sliced(assets, "t_frame_gold_small05", 10.0, Color::WHITE)))
                                .with_children(|f| {
                                    if let Some(item) = item {
                                        f.spawn((Node { width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() }, item_image(assets, item)));
                                    }
                                });
                            r.spawn(Node { flex_direction: FlexDirection::Column, ..default() }).with_children(|t| {
                                t.spawn((Text::new(label), text(16.0), TextColor(TEXT)));
                                let name = match item {
                                    Some(i) => i.name().to_string(),
                                    None if eq.main.two_handed => "— (two-handed weapon)".into(),
                                    None => "— empty".into(),
                                };
                                t.spawn((Text::new(name), title.at(22.0)));
                            });
                        })
                        .observe(click(MenuAction::SelectSlot(slot)));
                    }
                    l.spawn((Text::new("Click a slot, then an item from the stash."), text(16.0), TextColor(TEXT)));
                });
            // Stash for the selected slot.
            body.spawn(panel(assets, Node { flex_grow: 1.0, flex_direction: FlexDirection::Column, row_gap: Val::Px(8.0), ..default() }))
                .with_children(|s| {
                    s.spawn((Text::new("Stash"), title.at(26.0)));
                    if selected == Slot::Off {
                        let enabled = !eq.main.two_handed;
                        s.spawn(button(assets, "Empty off hand", 260.0, enabled, false)).observe(click(MenuAction::EmptyOffHand));
                        if !enabled {
                            s.spawn((Text::new("A two-handed weapon needs both hands – equip a one-handed weapon first."), text(16.0), TextColor(TEXT)));
                        }
                    }
                    if listed.is_empty() {
                        s.spawn((Text::new("Nothing for this slot yet – open chests in runs."), text(18.0), TextColor(TEXT)));
                    }
                    for (item, free) in listed {
                        let usable = free > 0 && (selected != Slot::Off || !eq.main.two_handed);
                        let copies = profile.copies(item);
                        s.spawn((
                            Node { column_gap: Val::Px(12.0), align_items: AlignItems::Center, padding: UiRect::all(Val::Px(6.0)), ..default() },
                            BackgroundColor(if usable { Color::srgba(0.16, 0.13, 0.1, 0.9) } else { Color::srgba(0.08, 0.08, 0.08, 0.9) }),
                        ))
                        .with_children(|r| {
                            r.spawn((Node { width: Val::Px(56.0), height: Val::Px(56.0), padding: UiRect::all(Val::Px(7.0)), ..default() }, sliced(assets, "t_frame_gold_small05", 8.0, Color::WHITE)))
                                .with_children(|f| {
                                    f.spawn((Node { width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() }, item_image(assets, item)));
                                });
                            let count = if copies > 1 { format!("  ×{copies}") } else { String::new() };
                            r.spawn((Text::new(format!("{}{count}", item.title())), text(19.0), TextColor(if usable { Color::WHITE } else { TEXT })));
                        })
                        .observe(click(MenuAction::Equip(item)));
                    }
                });
        });
        root.spawn(button(assets, "Back (Esc)", 200.0, true, false)).observe(click(MenuAction::Back));
    });
}

#[allow(clippy::too_many_arguments)]
fn main_menu_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut clicked: ResMut<Clicked>,
    mut page: ResMut<Page>,
    mut slot: ResMut<ArmorySlot>,
    mut profile: ResMut<Profile>,
    mut view: ResMut<TreeView>,
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
            *profile = Profile::new_hero(WEAPONS[i]);
            *view = TreeView { chooser: true, ..default() };
            *page = Page::Skills;
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
            p.spawn((Text::new("Esc – resume     M – main menu (loot and points are kept)"), text(22.0)));
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

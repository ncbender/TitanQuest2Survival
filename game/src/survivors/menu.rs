//! Main menu, character creation (pick the starting weapon) and pause screen.
use bevy::prelude::*;

use super::equipment::{StartingWeapon, WEAPONS};
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
    /// Character creation: choose the starting weapon.
    Hero,
}

/// Menu entry picked with the mouse this frame (keys are read directly).
#[derive(Resource, Default)]
struct Clicked(Option<MenuAction>);

#[derive(Clone, Copy, PartialEq)]
enum MenuAction {
    NewHero,
    Quit,
    /// Start a run with `WEAPONS[i]`.
    Pick(usize),
    Back,
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Clicked>()
        .init_resource::<Page>()
        .add_systems(OnEnter(GameState::MainMenu), (open_menu, show_page).chain())
        .add_systems(OnExit(GameState::MainMenu), despawn::<MainMenuUi>)
        .add_systems(Update, (main_menu_input, show_page.run_if(resource_changed::<Page>)).chain().run_if(in_state(GameState::MainMenu)))
        .add_systems(OnEnter(GameState::Paused), show_pause)
        .add_systems(OnExit(GameState::Paused), despawn::<PauseUi>)
        .add_systems(Update, pause_input);
}

/// The menu opens on the title page (TQ2_MENU=hero opens character creation, for screenshots).
fn open_menu(mut page: ResMut<Page>) {
    *page = if std::env::var("TQ2_MENU").is_ok_and(|v| v == "hero") { Page::Hero } else { Page::Title };
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
            row_gap: Val::Px(14.0),
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.35)),
        marker,
    )
}

fn entry(label: impl Into<String>) -> impl Bundle {
    (
        Node { width: Val::Px(360.0), padding: UiRect::all(Val::Px(12.0)), justify_content: JustifyContent::Center, ..default() },
        BackgroundColor(Color::srgb(0.22, 0.18, 0.12)),
        BorderColor::all(Color::srgb(0.8, 0.65, 0.3)),
        children![(Text::new(label), TextFont { font_size: FontSize::Px(24.0), ..default() })],
    )
}

fn click(action: MenuAction) -> impl Fn(On<PointerClick>, ResMut<Clicked>) {
    move |_, mut c| c.0 = Some(action)
}

fn show_page(mut commands: Commands, page: Res<Page>, title: Res<TitleFont>, starting: Res<StartingWeapon>, old: Query<Entity, With<MainMenuUi>>) {
    for e in &old {
        commands.entity(e).despawn();
    }
    let text = |size: f32| TextFont { font_size: FontSize::Px(size), ..default() };
    match *page {
        Page::Title => {
            commands.spawn(overlay(MainMenuUi)).with_children(|p| {
                p.spawn((Text::new("Titan Quest II Survivors"), title.at(60.0)));
                p.spawn((Text::new("Survive 10 minutes against the hordes of Greece"), text(20.0)));
                p.spawn(entry("New Hero  (Enter)")).observe(click(MenuAction::NewHero));
                p.spawn(entry("Quit  (Esc)")).observe(click(MenuAction::Quit));
                p.spawn((Text::new("WASD move · Q/E rotate camera · mouse wheel zoom · 1–3 choose upgrade · Esc pause"), text(16.0)));
            });
        }
        Page::Hero => {
            commands.spawn(overlay(MainMenuUi)).with_children(|p| {
                p.spawn((Text::new("Create Your Hero"), title.at(48.0)));
                p.spawn((Text::new("Choose your starting weapon – it decides your auto-attack"), text(20.0)));
                for (i, w) in WEAPONS.iter().enumerate() {
                    let selected = std::ptr::eq(*w, starting.0);
                    let stats = if w.kind == super::equipment::AttackKind::Sweep {
                        format!("{:.0} damage · every {:.2} s · {:.1} m reach", w.damage, w.cooldown, w.speed_or_radius)
                    } else {
                        format!("{:.0} damage · every {:.2} s · {:.0} m range", w.damage, w.cooldown, w.range)
                    };
                    p.spawn((
                        Node {
                            width: Val::Px(560.0),
                            padding: UiRect::all(Val::Px(12.0)),
                            flex_direction: FlexDirection::Column,
                            align_items: AlignItems::Center,
                            border: UiRect::all(Val::Px(if selected { 2.0 } else { 0.0 })),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.22, 0.18, 0.12)),
                        BorderColor::all(Color::srgb(0.95, 0.78, 0.35)),
                        children![
                            (Text::new(format!("{}  {}", i + 1, w.name)), title.at(28.0)),
                            (Text::new(w.attack_text()), text(20.0)),
                            (Text::new(stats), text(17.0), TextColor(Color::srgb(0.8, 0.75, 0.65))),
                        ],
                    ))
                    .observe(click(MenuAction::Pick(i)));
                }
                p.spawn((Text::new("1–3 or click to begin · Esc back"), text(16.0)));
            });
        }
    }
}

fn main_menu_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut clicked: ResMut<Clicked>,
    mut page: ResMut<Page>,
    mut starting: ResMut<StartingWeapon>,
    mut start: MessageWriter<StartRun>,
    mut exit: MessageWriter<AppExit>,
) {
    let key_action = match *page {
        Page::Title if keys.just_pressed(KeyCode::Enter) => Some(MenuAction::NewHero),
        Page::Title if keys.just_pressed(KeyCode::Escape) => Some(MenuAction::Quit),
        Page::Hero if keys.just_pressed(KeyCode::Escape) => Some(MenuAction::Back),
        Page::Hero => [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3]
            .iter()
            .position(|k| keys.just_pressed(*k))
            .filter(|i| *i < WEAPONS.len())
            .map(MenuAction::Pick),
        _ => None,
    };
    match clicked.0.take().or(key_action) {
        Some(MenuAction::NewHero) => *page = Page::Hero,
        Some(MenuAction::Back) => *page = Page::Title,
        Some(MenuAction::Pick(i)) => {
            starting.0 = WEAPONS[i];
            start.write(StartRun);
        }
        Some(MenuAction::Quit) => {
            exit.write(AppExit::Success);
        }
        None => {}
    }
}

fn show_pause(mut commands: Commands, title: Res<TitleFont>) {
    commands.spawn(overlay(PauseUi)).with_children(|p| {
        p.spawn((Text::new("Paused"), title.at(48.0)));
        p.spawn((Text::new("Esc – resume     M – main menu"), TextFont { font_size: FontSize::Px(22.0), ..default() }));
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

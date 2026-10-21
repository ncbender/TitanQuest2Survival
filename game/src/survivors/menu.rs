//! Main menu and pause screen.
use bevy::prelude::*;

use super::{GameState, StartRun};
use crate::fonts::TitleFont;

#[derive(Component)]
struct MainMenuUi;
#[derive(Component)]
struct PauseUi;

/// Menu entry picked with the mouse this frame (keys are read directly).
#[derive(Resource, Default)]
struct Clicked(Option<MenuAction>);

#[derive(Clone, Copy, PartialEq)]
enum MenuAction {
    Start,
    Quit,
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Clicked>()
        .add_systems(OnEnter(GameState::MainMenu), show_main_menu)
        .add_systems(OnExit(GameState::MainMenu), despawn::<MainMenuUi>)
        .add_systems(Update, main_menu_input.run_if(in_state(GameState::MainMenu)))
        .add_systems(OnEnter(GameState::Paused), show_pause)
        .add_systems(OnExit(GameState::Paused), despawn::<PauseUi>)
        .add_systems(Update, pause_input);
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

fn entry(label: &str) -> impl Bundle {
    (
        Node { width: Val::Px(360.0), padding: UiRect::all(Val::Px(12.0)), justify_content: JustifyContent::Center, ..default() },
        BackgroundColor(Color::srgb(0.22, 0.18, 0.12)),
        BorderColor::all(Color::srgb(0.8, 0.65, 0.3)),
        children![(Text::new(label), TextFont { font_size: FontSize::Px(24.0), ..default() })],
    )
}

fn show_main_menu(mut commands: Commands, title: Res<TitleFont>) {
    commands.spawn(overlay(MainMenuUi)).with_children(|p| {
        p.spawn((Text::new("Titan Quest II Survivors"), title.at(60.0)));
        p.spawn((Text::new("Überlebe 10 Minuten gegen die Horden Griechenlands"), TextFont { font_size: FontSize::Px(20.0), ..default() }));
        p.spawn(entry("Spiel starten  (Enter)"))
            .observe(|_: On<PointerClick>, mut c: ResMut<Clicked>| c.0 = Some(MenuAction::Start));
        p.spawn(entry("Beenden  (Esc)")).observe(|_: On<PointerClick>, mut c: ResMut<Clicked>| c.0 = Some(MenuAction::Quit));
        p.spawn((
            Text::new("WASD laufen · Q/E Kamera drehen · Mausrad Zoom · 1–3 Upgrade wählen · Esc Pause"),
            TextFont { font_size: FontSize::Px(16.0), ..default() },
        ));
    });
}

fn main_menu_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut clicked: ResMut<Clicked>,
    mut start: MessageWriter<StartRun>,
    mut exit: MessageWriter<AppExit>,
) {
    let action = clicked.0.take().or(if keys.just_pressed(KeyCode::Enter) {
        Some(MenuAction::Start)
    } else if keys.just_pressed(KeyCode::Escape) {
        Some(MenuAction::Quit)
    } else {
        None
    });
    match action {
        Some(MenuAction::Start) => {
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
        p.spawn((Text::new("Pause"), title.at(48.0)));
        p.spawn((Text::new("Esc – weiter     M – Hauptmenü"), TextFont { font_size: FontSize::Px(22.0), ..default() }));
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

//! Player settings (volumes, fullscreen, damage numbers), saved in `save/settings.json`. The
//! settings window opens from the main menu and from the pause screen (key O).
use bevy::prelude::*;
use bevy::window::{MonitorSelection, WindowMode};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use super::mastery_data::{GOLD, TEXT};
use super::ui::{backdrop, button, panel, text};
use super::GameState;
use crate::audio::Audio;
use crate::fonts::TitleFont;

#[derive(Resource, Serialize, Deserialize, Clone, PartialEq)]
#[serde(default)]
pub struct Settings {
    pub master: f32,
    pub music: f32,
    pub sfx: f32,
    pub fullscreen: bool,
    pub damage_numbers: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self { master: 0.8, music: 0.6, sfx: 0.9, fullscreen: false, damage_numbers: true }
    }
}

/// Whether the settings window is shown.
#[derive(Resource, Default)]
pub struct SettingsOpen(pub bool);

#[derive(Component)]
struct SettingsUi;

#[derive(Clone, Copy)]
enum Action {
    Change(Field, f32),
    Toggle(Field),
    Close,
}

#[derive(Clone, Copy)]
enum Field {
    Master,
    Music,
    Sfx,
    Fullscreen,
    DamageNumbers,
}

#[derive(Resource, Default)]
struct Clicked(Option<Action>);

fn path() -> PathBuf {
    let export = PathBuf::from(crate::export_root());
    export.parent().and_then(|p| p.parent()).map(|p| p.join("save")).unwrap_or_else(|| "save".into()).join("settings.json")
}

pub fn plugin(app: &mut App) {
    let settings: Settings = std::fs::read_to_string(path()).ok().and_then(|s| serde_json::from_str(s.trim_start_matches('\u{feff}')).ok()).unwrap_or_default();
    app.insert_resource(settings)
        .init_resource::<SettingsOpen>()
        .init_resource::<Clicked>()
        .add_systems(Update, (input, apply.run_if(resource_changed::<Settings>), show.run_if(resource_changed::<SettingsOpen>.or_eager(resource_changed::<Settings>))).chain())
        .add_systems(OnExit(GameState::Paused), close)
        .add_systems(OnExit(GameState::MainMenu), close);
}

fn close(mut open: ResMut<SettingsOpen>) {
    open.0 = false;
}

/// Applies volumes and window mode, and saves (not in automated tests).
fn apply(settings: Res<Settings>, audio: Res<Audio>, mut windows: Query<&mut Window>) {
    audio.set_volumes(settings.master * settings.sfx, settings.master * settings.music);
    if let Ok(mut w) = windows.single_mut() {
        let mode = if settings.fullscreen { WindowMode::BorderlessFullscreen(MonitorSelection::Current) } else { WindowMode::Windowed };
        if w.mode != mode {
            w.mode = mode;
        }
    }
    if !super::is_test_run() {
        let p = path();
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Err(e) = std::fs::write(&p, serde_json::to_string_pretty(&*settings).unwrap_or_default()) {
            warn!("saving settings: {e}");
        }
    }
}

fn input(keys: Res<ButtonInput<KeyCode>>, state: Res<State<GameState>>, mut open: ResMut<SettingsOpen>, mut clicked: ResMut<Clicked>, mut settings: ResMut<Settings>) {
    if *state.get() == GameState::Paused && keys.just_pressed(KeyCode::KeyO) {
        open.0 = !open.0;
    }
    let Some(action) = clicked.0.take() else { return };
    let s = &mut *settings;
    match action {
        Action::Change(f, d) => {
            let v = match f {
                Field::Master => &mut s.master,
                Field::Music => &mut s.music,
                Field::Sfx => &mut s.sfx,
                _ => return,
            };
            *v = ((*v + d) * 10.0).round().clamp(0.0, 10.0) / 10.0;
        }
        Action::Toggle(Field::Fullscreen) => s.fullscreen = !s.fullscreen,
        Action::Toggle(Field::DamageNumbers) => s.damage_numbers = !s.damage_numbers,
        Action::Toggle(_) => {}
        Action::Close => open.0 = false,
    }
}

fn click(action: Action) -> impl Fn(On<PointerClick>, ResMut<Clicked>) {
    move |_, mut c| c.0 = Some(action)
}

fn show(mut commands: Commands, assets: Res<AssetServer>, title: Res<TitleFont>, open: Res<SettingsOpen>, settings: Res<Settings>, old: Query<Entity, With<SettingsUi>>) {
    for e in &old {
        commands.entity(e).despawn();
    }
    if !open.0 {
        return;
    }
    commands.spawn((backdrop(&assets, SettingsUi), GlobalZIndex(20))).with_children(|root| {
        root.spawn((Text::new("Settings"), title.at(40.0), Node { align_self: AlignSelf::Center, margin: UiRect::top(Val::Px(90.0)), ..default() }));
        root.spawn(panel(&assets, Node { width: Val::Px(620.0), align_self: AlignSelf::Center, flex_direction: FlexDirection::Column, row_gap: Val::Px(12.0), ..default() })).with_children(|p| {
            for (label, field, value) in [("Master volume", Field::Master, settings.master), ("Music", Field::Music, settings.music), ("Effects", Field::Sfx, settings.sfx)] {
                p.spawn(Node { align_items: AlignItems::Center, column_gap: Val::Px(14.0), ..default() }).with_children(|r| {
                    r.spawn((Text::new(label), text(20.0), Node { width: Val::Px(220.0), ..default() }));
                    r.spawn(button(&assets, "−", 54.0, value > 0.0, false)).observe(click(Action::Change(field, -0.1)));
                    r.spawn((Text::new(format!("{:.0} %", value * 100.0)), text(20.0), TextColor(GOLD), Node { width: Val::Px(80.0), justify_content: JustifyContent::Center, ..default() }));
                    r.spawn(button(&assets, "+", 54.0, value < 1.0, false)).observe(click(Action::Change(field, 0.1)));
                });
            }
            for (label, field, on) in [("Fullscreen", Field::Fullscreen, settings.fullscreen), ("Damage numbers", Field::DamageNumbers, settings.damage_numbers)] {
                p.spawn(Node { align_items: AlignItems::Center, column_gap: Val::Px(14.0), ..default() }).with_children(|r| {
                    r.spawn((Text::new(label), text(20.0), Node { width: Val::Px(220.0), ..default() }));
                    r.spawn(button(&assets, if on { "On" } else { "Off" }, 120.0, true, on)).observe(click(Action::Toggle(field)));
                });
            }
            p.spawn((Text::new("Saved automatically."), text(15.0), TextColor(TEXT)));
        });
        root.spawn((button(&assets, "Close", 200.0, true, false), Node { align_self: AlignSelf::Center, ..default() })).observe(click(Action::Close));
    });
}

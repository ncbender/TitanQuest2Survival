//! HUD (life, XP, level, time, kills), level-up choice panel and end screen.
use bevy::prelude::*;

use super::player::{Hero, HeroStats};
use super::progression::Offer;
use super::{GameState, Health, Run};
use super::combat::Hit;
use super::equipment::{LootOffer, RunLoot};
use super::profile::LastReward;
use crate::fonts::TitleFont;
use crate::ground::Ground;
use crate::survivors::enemies::Enemy;

/// Root of the always-on HUD (hidden in the main menu).
#[derive(Component)]
struct HudRoot;
#[derive(Component)]
struct LifeFill;
#[derive(Component)]
struct XpFill;
#[derive(Component)]
struct StatusText;
#[derive(Component)]
struct LevelUpPanel;
/// Upgrade picked by mouse click this frame (keys 1-3 are read directly).
#[derive(Resource, Default)]
struct ClickedChoice(Option<usize>);
#[derive(Component)]
struct EndPanel;
#[derive(Component)]
struct LootPanel;

pub fn plugin(app: &mut App) {
    app.init_resource::<ClickedChoice>()
        .add_systems(Startup, spawn_hud)
        .add_systems(Update, (update_hud, hud_visibility, spawn_damage_numbers, float_damage_numbers))
        // The panel shows the offer, so it must run after the offer is rolled.
        .add_systems(OnEnter(GameState::LevelUp), show_level_up.after(super::progression::make_offer))
        .add_systems(OnExit(GameState::LevelUp), despawn::<LevelUpPanel>)
        .add_systems(Update, choose_upgrade.run_if(in_state(GameState::LevelUp)))
        .add_systems(OnEnter(GameState::Loot), show_loot)
        .add_systems(OnExit(GameState::Loot), despawn::<LootPanel>)
        .add_systems(Update, choose_loot.run_if(in_state(GameState::Loot)))
        .add_systems(OnEnter(GameState::GameOver), show_end.after(super::profile::bank_run))
        .add_systems(OnExit(GameState::GameOver), despawn::<EndPanel>);
}

fn bar(color: Color, marker: impl Component) -> impl Bundle {
    (
        Node { width: Val::Px(320.0), height: Val::Px(14.0), margin: UiRect::bottom(Val::Px(6.0)), ..default() },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
        children![(Node { width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() }, BackgroundColor(color), marker)],
    )
}

fn spawn_hud(mut commands: Commands) {
    commands.spawn((
        Node { position_type: PositionType::Absolute, left: Val::Px(16.0), top: Val::Px(16.0), flex_direction: FlexDirection::Column, ..default() },
        HudRoot,
        children![
            bar(Color::srgb(0.75, 0.12, 0.1), LifeFill),
            bar(Color::srgb(0.35, 0.7, 1.0), XpFill),
            (Text::new(""), TextFont { font_size: FontSize::Px(20.0), ..default() }, TextColor(Color::WHITE), StatusText),
        ],
    ));
}

fn update_hud(
    run: Res<Run>,
    loot: Res<RunLoot>,
    hero: Query<&Health, With<Hero>>,
    enemies: Query<(&Enemy, &Health)>,
    mut life: Query<&mut Node, (With<LifeFill>, Without<XpFill>)>,
    mut xp: Query<&mut Node, (With<XpFill>, Without<LifeFill>)>,
    mut text: Query<&mut Text, With<StatusText>>,
) {
    if let (Ok(hp), Ok(mut node)) = (hero.single(), life.single_mut()) {
        node.width = Val::Percent((hp.hp / hp.max).clamp(0.0, 1.0) * 100.0);
    }
    if let Ok(mut node) = xp.single_mut() {
        node.width = Val::Percent(run.xp as f32 / run.xp_next.max(1) as f32 * 100.0);
    }
    if let Ok(mut text) = text.single_mut() {
        let t = run.time as u32;
        let boss = enemies
            .iter()
            .find(|(e, _)| e.boss)
            .map(|(e, hp)| format!("
Boss: {} {:.0} %", e.def.name, hp.hp / hp.max * 100.0))
            .unwrap_or_default();
        let found = if loot.0.is_empty() { String::new() } else { format!("   Loot: {}", loot.0.len()) };
        text.0 = format!(
            "Level {}   {:02}:{:02} / 10:00   Kills: {}   Enemies: {}{found}{boss}",
            run.level,
            t / 60,
            t % 60,
            run.kills,
            enemies.iter().count()
        );
    }
}

/// Floating number for one hit, anchored at a world position.
#[derive(Component)]
struct DamageNumber {
    world: Vec3,
    age: f32,
}

const MAX_DAMAGE_NUMBERS: usize = 60;

fn spawn_damage_numbers(
    mut commands: Commands,
    mut hits: MessageReader<Hit>,
    ground: Res<Ground>,
    existing: Query<(), With<DamageNumber>>,
) {
    let mut count = existing.iter().count();
    for hit in hits.read() {
        if count >= MAX_DAMAGE_NUMBERS {
            continue;
        }
        count += 1;
        let jitter = Vec3::new(fastrand::f32() - 0.5, 0.0, fastrand::f32() - 0.5) * 0.6;
        commands.spawn((
            DamageNumber { world: Vec3::new(hit.at.x, ground.stand(hit.at) + 2.2, hit.at.y) + jitter, age: 0.0 },
            Text::new(format!("{:.0}", hit.amount)),
            TextFont { font_size: FontSize::Px(if hit.crit { 28.0 } else { 20.0 }), ..default() },
            TextColor(if hit.crit { Color::srgb(1.0, 0.8, 0.2) } else { Color::WHITE }),
            Node { position_type: PositionType::Absolute, ..default() },
        ));
    }
}

fn float_damage_numbers(
    mut commands: Commands,
    time: Res<Time>,
    camera: Query<(&Camera, &GlobalTransform)>,
    mut numbers: Query<(Entity, &mut DamageNumber, &mut Node, &mut TextColor)>,
) {
    let Ok((camera, cam_transform)) = camera.single() else { return };
    for (e, mut n, mut node, mut color) in &mut numbers {
        n.age += time.delta_secs();
        if n.age > 0.7 {
            commands.entity(e).despawn();
            continue;
        }
        let world = n.world + Vec3::Y * n.age * 1.5;
        if let Ok(screen) = camera.world_to_viewport(cam_transform, world) {
            node.left = Val::Px(screen.x - 10.0);
            node.top = Val::Px(screen.y - 10.0);
        }
        color.0.set_alpha(1.0 - (n.age / 0.7).powi(2));
    }
}

fn hud_visibility(state: Res<State<GameState>>, mut hud: Query<&mut Visibility, With<HudRoot>>) {
    let wanted = if *state.get() == GameState::MainMenu { Visibility::Hidden } else { Visibility::Inherited };
    for mut v in &mut hud {
        if *v != wanted {
            *v = wanted;
        }
    }
}

fn panel(marker: impl Component) -> impl Bundle {
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
        marker,
    )
}

fn show_level_up(mut commands: Commands, offer: Res<Offer>, loadout: Res<super::abilities::Loadout>, run: Res<Run>, title: Res<TitleFont>) {
    commands.spawn(panel(LevelUpPanel)).with_children(|p| {
        p.spawn((Text::new(format!("Level {} reached – choose (1–3)", run.level)), title.at(36.0)));
        for (i, upgrade) in offer.0.iter().enumerate() {
            p.spawn((
                Node { width: Val::Px(560.0), padding: UiRect::all(Val::Px(14.0)), ..default() },
                BackgroundColor(Color::srgb(0.22, 0.18, 0.12)),
                BorderColor::all(Color::srgb(0.8, 0.65, 0.3)),
                children![(Text::new(format!("{}  {}", i + 1, upgrade.title(&loadout))), TextFont { font_size: FontSize::Px(22.0), ..default() })],
            ))
            .observe(move |_: On<PointerClick>, mut clicked: ResMut<ClickedChoice>| clicked.0 = Some(i));
        }
    });
}

fn choose_upgrade(
    keys: Res<ButtonInput<KeyCode>>,
    mut clicked: ResMut<ClickedChoice>,
    offer: Res<Offer>,
    mut stats: ResMut<HeroStats>,
    mut hero: Query<&mut Health, With<Hero>>,
    mut next: ResMut<NextState<GameState>>,
    audio: Res<crate::audio::Audio>,
    mut loadout: ResMut<super::abilities::Loadout>,
) {
    let keys_pick = [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3].iter().position(|k| keys.just_pressed(*k));
    let click_pick = clicked.0.take();
    // Test drivers (TQ2_AUTOPLAY, TQ2_AUTOPICK) pick the first offer.
    let auto_pick = (std::env::var_os("TQ2_AUTOPLAY").is_some() || std::env::var_os("TQ2_AUTOPICK").is_some()).then_some(0);
    let Some(choice) = keys_pick.or(click_pick).or(auto_pick).and_then(|i| offer.0.get(i)) else { return };
    info!("picked {}", choice.title(&loadout));
    if let Ok(mut hp) = hero.single_mut() {
        choice.apply(&mut stats, &mut hp, &mut loadout);
    }
    audio.play(super::content::sound::PICK, None, 0.9, 0.0);
    next.set(GameState::Playing);
}

fn show_loot(mut commands: Commands, offer: Res<LootOffer>, title: Res<TitleFont>) {
    commands.spawn(panel(LootPanel)).with_children(|p| {
        p.spawn((Text::new("Chest opened – choose an item"), title.at(36.0)));
        p.spawn((Text::new("It goes to your stash at the end of the run; equip it in the armoury."), TextFont { font_size: FontSize::Px(18.0), ..default() }));
        for (i, gear) in offer.0.iter().enumerate() {
            p.spawn((
                Node { width: Val::Px(620.0), padding: UiRect::all(Val::Px(14.0)), ..default() },
                BackgroundColor(Color::srgb(0.18, 0.16, 0.22)),
                children![(Text::new(format!("{}  {}", i + 1, gear.title())), TextFont { font_size: FontSize::Px(22.0), ..default() })],
            ))
            .observe(move |_: On<PointerClick>, mut clicked: ResMut<ClickedChoice>| clicked.0 = Some(i));
        }
        p.spawn((Text::new("Esc – take nothing"), TextFont { font_size: FontSize::Px(18.0), ..default() }));
    });
}

fn choose_loot(
    keys: Res<ButtonInput<KeyCode>>,
    mut clicked: ResMut<ClickedChoice>,
    offer: Res<LootOffer>,
    audio: Res<crate::audio::Audio>,
    mut loot: ResMut<RunLoot>,
    mut next: ResMut<NextState<GameState>>,
) {
    let keys_pick = [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3].iter().position(|k| keys.just_pressed(*k));
    let auto_pick = (std::env::var_os("TQ2_AUTOPLAY").is_some() || std::env::var_os("TQ2_AUTOPICK").is_some()).then_some(0);
    if keys.just_pressed(KeyCode::Escape) {
        next.set(GameState::Playing);
        return;
    }
    let Some(item) = keys_pick.or(clicked.0.take()).or(auto_pick).and_then(|i| offer.0.get(i)) else { return };
    loot.0.push(*item);
    info!("looted {}", item.name());
    audio.play(super::content::sound::PICK, None, 0.9, 0.0);
    next.set(GameState::Playing);
}

fn show_end(mut commands: Commands, run: Res<Run>, reward: Res<LastReward>, title_font: Res<TitleFont>) {
    let title = if run.won { "You survived! The Titans are impressed." } else { "You have fallen." };
    let items = if reward.items.is_empty() { "none".to_string() } else { reward.items.join(", ") };
    commands.spawn(panel(EndPanel)).with_children(|p| {
        p.spawn((Text::new(title), title_font.at(44.0)));
        p.spawn((
            Text::new(format!(
                "Level {}   Kills: {}   Time {:02}:{:02}\n\nActive points: +{}   Passive points: +{}   Items to the stash: {items}\n\nR – restart     M – main menu",
                run.level,
                run.kills,
                run.time as u32 / 60,
                run.time as u32 % 60,
                reward.active,
                reward.passive
            )),
            TextFont { font_size: FontSize::Px(24.0), ..default() },
            TextLayout::justify(Justify::Center),
        ));
    });
}

fn despawn<T: Component>(mut commands: Commands, q: Query<Entity, With<T>>) {
    for e in &q {
        commands.entity(e).despawn();
    }
}

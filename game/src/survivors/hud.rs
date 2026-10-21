//! HUD (life, XP, level, time, kills), level-up choice panel and end screen.
use bevy::prelude::*;

use super::player::{Hero, HeroStats};
use super::progression::Offer;
use super::{GameState, Health, Run};
use super::combat::Hit;
use super::equipment::{LootOffer, RunLoot};
use super::items::AffixBook;
use super::mastery_data::{MasteryBook, GOLD, TEXT};
use super::profile::LastReward;
use super::ui::{self, sliced, text};
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
    settings: Res<super::settings::Settings>,
    existing: Query<(), With<DamageNumber>>,
) {
    if !settings.damage_numbers {
        hits.clear();
        return;
    }
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

/// A centred TQ2 window: gold frame over a dark fill, with a title in a gold nameplate.
fn window(p: &mut ChildSpawnerCommands, assets: &AssetServer, title_font: &TitleFont, title: String, width: f32, body: impl FnOnce(&mut ChildSpawnerCommands)) {
    p.spawn((
        Node { width: Val::Px(width), flex_direction: FlexDirection::Column, align_items: AlignItems::Center, row_gap: Val::Px(10.0), padding: UiRect::new(Val::Px(26.0), Val::Px(26.0), Val::Px(16.0), Val::Px(24.0)), ..default() },
        sliced(assets, "t_frame_gold_big03", 48.0, Color::WHITE),
    ))
    .with_children(|w| {
        // The frame's art is inset; the dark fill sits inside it.
        w.spawn((
            Node { position_type: PositionType::Absolute, left: Val::Px(14.0), right: Val::Px(14.0), top: Val::Px(14.0), bottom: Val::Px(14.0), ..default() },
            BackgroundColor(Color::srgba(0.03, 0.03, 0.035, 0.93)),
            ZIndex(-1),
        ));
        w.spawn((Node { padding: UiRect::axes(Val::Px(48.0), Val::Px(10.0)), margin: UiRect::bottom(Val::Px(6.0)), ..default() }, sliced(assets, "t_frame_gold_big02_nameplate01", 24.0, Color::WHITE)))
            .with_children(|n| {
                n.spawn((Text::new(title), title_font.at(30.0), TextColor(GOLD)));
            });
        body(w);
    });
}

fn show_level_up(
    mut commands: Commands,
    assets: Res<AssetServer>,
    offer: Res<Offer>,
    loadout: Res<super::abilities::Loadout>,
    book: Res<MasteryBook>,
    run: Res<Run>,
    title: Res<TitleFont>,
) {
    commands.spawn(panel(LevelUpPanel)).with_children(|p| {
        window(p, &assets, &title, format!("Level {}", run.level), 720.0, |w| {
            w.spawn((Text::new("Choose a blessing (1–3 or click)"), text(18.0), TextColor(TEXT)));
            for (i, choice) in offer.0.iter().enumerate() {
                let (frame, fw, fh, icon_top, icon) = if choice.is_skill() {
                    ("t_frame_gold_activespell", 60.0, 70.0, 7.0, 42.0)
                } else {
                    ("t_frame_gold_passivespell", 56.0, 66.0, 11.0, 34.0)
                };
                let title_text = choice.title(&loadout);
                let (head, tail) = title_text.split_once(": ").map_or((title_text.as_str(), ""), |(a, b)| (a, b));
                w.spawn((
                    Node { width: Val::Percent(100.0), padding: UiRect::axes(Val::Px(14.0), Val::Px(8.0)), column_gap: Val::Px(14.0), align_items: AlignItems::Center, ..default() },
                    BackgroundColor(Color::srgba(0.09, 0.08, 0.07, 0.95)),
                    sliced(&assets, "t_frame_gold_small05", 10.0, Color::WHITE),
                ))
                .with_children(|row| {
                    row.spawn((Text::new(format!("{}", i + 1)), title.at(26.0), TextColor(GOLD), Node { width: Val::Px(22.0), ..default() }));
                    row.spawn((Node { width: Val::Px(fw), height: Val::Px(fh), justify_content: JustifyContent::Center, flex_shrink: 0.0, ..default() }, ui::stretched(&assets, frame, Color::WHITE)))
                        .with_children(|f| {
                            if let Some(path) = choice.icon(&loadout, &book) {
                                f.spawn((Node { position_type: PositionType::Absolute, top: Val::Px(icon_top), width: Val::Px(icon), height: Val::Px(icon), ..default() }, ui::image(&assets, path, Color::WHITE)));
                            }
                        });
                    row.spawn(Node { flex_direction: FlexDirection::Column, ..default() }).with_children(|t| {
                        t.spawn((Text::new(head.to_string()), title.at(21.0), TextColor(if choice.is_skill() { Color::srgb(0.6, 0.85, 1.0) } else { Color::WHITE })));
                        if !tail.is_empty() {
                            t.spawn((Text::new(tail.to_string()), text(17.0), TextColor(TEXT)));
                        }
                    });
                })
                .observe(move |_: On<PointerClick>, mut clicked: ResMut<ClickedChoice>| clicked.0 = Some(i));
            }
        });
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

fn show_loot(mut commands: Commands, assets: Res<AssetServer>, offer: Res<LootOffer>, book: Res<AffixBook>, title: Res<TitleFont>) {
    commands.spawn(panel(LootPanel)).with_children(|p| {
        window(p, &assets, &title, "Treasure".into(), 720.0, |w| {
            w.spawn((Text::new("Choose one item (1–3 or click). It goes to the shared stash when the run ends."), text(17.0), TextColor(TEXT)));
            for (i, item) in offer.0.iter().enumerate() {
                ui::item_card(w, &assets, &title, item, &book, 660.0, Some(format!("{}  – take", i + 1)), |e| {
                    e.observe(move |_: On<PointerClick>, mut clicked: ResMut<ClickedChoice>| clicked.0 = Some(i));
                });
            }
            w.spawn((Text::new("Esc – take nothing"), text(16.0), TextColor(TEXT)));
        });
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
    loot.0.push(item.clone());
    info!("looted {} ({:?})", item.base, item.rarity);
    audio.play(super::content::sound::PICK, None, 0.9, 0.0);
    next.set(GameState::Playing);
}

fn show_end(mut commands: Commands, assets: Res<AssetServer>, run: Res<Run>, reward: Res<LastReward>, title_font: Res<TitleFont>) {
    let title = if run.won { "Victory" } else { "You have fallen" };
    commands.spawn(panel(EndPanel)).with_children(|p| {
        window(p, &assets, &title_font, title.into(), 660.0, |w| {
            if run.won {
                w.spawn((Text::new("The Titans are impressed."), text(19.0), TextColor(TEXT)));
            }
            w.spawn((
                Text::new(format!("Level {}   ·   Kills {}   ·   Time {:02}:{:02}", run.level, run.kills, run.time as u32 / 60, run.time as u32 % 60)),
                text(22.0),
            ));
            w.spawn((Text::new(format!("Active points +{}   ·   Passive points +{}", reward.active, reward.passive)), text(21.0), TextColor(GOLD)));
            if !reward.items.is_empty() {
                w.spawn((Text::new("Items to the stash"), title_font.at(20.0)));
                for (name, rarity) in &reward.items {
                    w.spawn((Text::new(name.clone()), text(19.0), TextColor(rarity.color())));
                }
            }
            w.spawn((Text::new("R – restart     M – main menu"), text(18.0), TextColor(TEXT), Node { margin: UiRect::top(Val::Px(8.0)), ..default() }));
        });
    });
}

fn despawn<T: Component>(mut commands: Commands, q: Query<Entity, With<T>>) {
    for e in &q {
        commands.entity(e).despawn();
    }
}

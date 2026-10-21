//! Inventory (main menu): the hero in 3D with the equipment slots around it like TQ2's
//! character screen, the shared bag (more slots for gold), item upgrades at the anvil (+5 % to
//! every stat per level) and recycling into gold. Items move by right-click or drag and drop:
//! bag → slot equips, slot → bag takes off, bag → anvil recycles.
use bevy::camera::RenderTarget;
use bevy::color::Mix;
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;

use super::equipment::Item;
use super::items::{AffixBook, ItemInstance};
use super::mastery_data::{GOLD, TEXT};
use super::menu::{MainMenuUi, Page};
use super::profile::{Profile, Slot};
use super::ui::{self, button, headline, panel, text};
use super::GameState;
use crate::character::CharacterAnim;
use crate::fonts::TitleFont;

/// Bag cell size and columns.
const CELL: f32 = 52.0;
const COLUMNS: usize = 8;
/// Where the preview hero stands: far above every map, outside the game camera's view.
const STAGE: Vec3 = Vec3::new(0.0, 3000.0, 0.0);

/// Selected and hovered item and the last result message.
#[derive(Resource, Default)]
pub struct InvState {
    selected: Option<u64>,
    hover: Option<u64>,
    message: String,
}

/// The render target the preview hero is drawn into.
#[derive(Resource)]
pub struct Preview(Handle<Image>);

/// The item being dragged and the slot it was taken from (None: from the bag).
#[derive(Resource, Default)]
struct Dragging(Option<(u64, Option<Slot>)>);

#[derive(Component)]
struct PreviewHero;
#[derive(Component)]
struct PreviewStage;
#[derive(Component)]
struct DragGhost;
/// The details column (rebuilt when the selection or hover changes).
#[derive(Component)]
struct Details;

#[derive(Clone, Copy, Debug)]
enum Act {
    Select(u64),
    Equip(u64),
    Unequip(Slot),
    Put(u64, Slot),
    Upgrade(u64),
    Salvage(u64),
    UpgradeBag,
}

#[derive(Resource, Default)]
struct Pending(Vec<Act>);

pub fn plugin(app: &mut App) {
    // Created right away: the first menu page is built before Startup systems run.
    let image = Image::new_target_texture(480, 640, TextureFormat::Rgba8Unorm, Some(TextureFormat::Rgba8UnormSrgb));
    let handle = app.world_mut().resource_mut::<Assets<Image>>().add(image);
    app.insert_resource(Preview(handle))
        .init_resource::<InvState>()
        .init_resource::<Dragging>()
        .init_resource::<Pending>()
        .add_systems(
            Update,
            (apply_actions, sync_preview, move_ghost, refresh_details.run_if(resource_changed::<InvState>)).chain().run_if(in_state(GameState::MainMenu)),
        )
        .add_systems(OnExit(GameState::MainMenu), despawn_preview);
}

/// The slot an item goes to on right-click.
fn slot_for(item: Item) -> Slot {
    match item {
        Item::Weapon(_) => Slot::Main,
        Item::Shield(_) | Item::Talisman(_) => Slot::Off,
        Item::Armor(_) => Slot::Armor,
    }
}

fn worn_slot(profile: &Profile, uid: u64) -> Option<Slot> {
    [(Slot::Main, profile.main), (Slot::Off, profile.off), (Slot::Armor, profile.armor)].into_iter().find(|(_, u)| *u == Some(uid)).map(|(s, _)| s)
}

fn apply_actions(mut pending: ResMut<Pending>, mut profile: ResMut<Profile>, mut state: ResMut<InvState>, audio: Res<crate::audio::Audio>) {
    if pending.0.is_empty() {
        return;
    }
    for act in std::mem::take(&mut pending.0) {
        let result: Result<String, String> = match act {
            Act::Select(uid) => {
                state.selected = Some(uid);
                continue;
            }
            Act::Equip(uid) => match profile.item(uid).and_then(|i| i.item()) {
                Some(item) => profile.equip(slot_for(item), Some(uid)).map(|_| "Equipped.".into()),
                None => Err("Unknown item.".into()),
            },
            Act::Unequip(slot) => profile.equip(slot, None).map(|_| "Taken off.".into()),
            Act::Put(uid, slot) => profile.equip(slot, Some(uid)).map(|_| "Equipped.".into()),
            Act::Upgrade(uid) => profile.upgrade_item(uid).map(|_| format!("Upgraded to +{}.", profile.item(uid).map_or(0, |i| i.level))),
            Act::Salvage(uid) => profile.salvage(uid).map(|gold| {
                if state.selected == Some(uid) {
                    state.selected = None;
                }
                state.hover = None;
                format!("Recycled for {gold} gold.")
            }),
            Act::UpgradeBag => profile.upgrade_bag().map(|_| format!("The bag holds {} items now.", profile.bag_capacity())),
        };
        if result.is_ok() {
            audio.play(super::content::sound::PICK, None, 0.8, 0.0);
        }
        state.message = result.unwrap_or_else(|e| format!("{}{}.", e[..1].to_uppercase(), &e[1..]));
    }
}

/// Spawns the preview hero and its camera on the inventory page and keeps its gear current.
#[allow(clippy::too_many_arguments)]
fn sync_preview(
    mut commands: Commands,
    page: Res<Page>,
    profile: Res<Profile>,
    affixes: Res<AffixBook>,
    preview: Res<Preview>,
    heroes: Query<Entity, With<PreviewHero>>,
    stage: Query<Entity, With<PreviewStage>>,
) {
    if *page != Page::Armory || !profile.created {
        for e in heroes.iter().chain(stage.iter()) {
            commands.entity(e).despawn();
        }
        return;
    }
    let eq = profile.equipment(&affixes);
    if heroes.is_empty() {
        let idle = eq.stance().idle;
        commands.spawn((PreviewHero, PreviewStage, eq, CharacterAnim::looping(idle), Transform::from_translation(STAGE).with_rotation(Quat::from_rotation_y(0.35)), Visibility::default()));
        commands.spawn((
            PreviewStage,
            Camera3d::default(),
            Camera { order: -1, clear_color: ClearColorConfig::Custom(Color::NONE), ..default() },
            RenderTarget::from(preview.0.clone()),
            Transform::from_translation(STAGE + Vec3::new(0.0, 1.05, 3.4)).looking_at(STAGE + Vec3::Y * 0.92, Vec3::Y),
        ));
        commands.spawn((PreviewStage, PointLight { intensity: 600_000.0, range: 30.0, ..default() }, Transform::from_translation(STAGE + Vec3::new(1.6, 2.6, 2.8))));
        commands.spawn((PreviewStage, PointLight { intensity: 250_000.0, range: 30.0, color: Color::srgb(1.0, 0.85, 0.65), ..default() }, Transform::from_translation(STAGE + Vec3::new(-2.0, 1.5, -1.5))));
    } else if profile.is_changed() {
        for e in &heroes {
            commands.entity(e).insert((CharacterAnim::looping(eq.stance().idle), eq.clone()));
        }
    }
}

fn despawn_preview(mut commands: Commands, stage: Query<Entity, With<PreviewStage>>) {
    for e in &stage {
        commands.entity(e).despawn();
    }
}

/// The dragged item's icon follows the cursor; releasing the button ends the drag (drops are
/// handled by the targets' observers before this runs).
fn move_ghost(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    scale: Res<UiScale>,
    profile: Res<Profile>,
    mut dragging: ResMut<Dragging>,
    mut ghosts: Query<(Entity, &mut Node), With<DragGhost>>,
) {
    if !mouse.pressed(MouseButton::Left) {
        if dragging.0.is_some() {
            dragging.0 = None;
        }
        for (e, _) in &ghosts {
            commands.entity(e).despawn();
        }
        return;
    }
    let Some((uid, _)) = dragging.0 else { return };
    let Some(cursor) = windows.single().ok().and_then(|w| w.cursor_position()) else { return };
    let at = cursor / scale.0 - Vec2::splat(CELL / 2.0);
    if let Ok((_, mut node)) = ghosts.single_mut() {
        node.left = Val::Px(at.x);
        node.top = Val::Px(at.y);
    } else if let Some(item) = profile.item(uid).and_then(|i| i.item()) {
        commands.spawn((
            DragGhost,
            Node { position_type: PositionType::Absolute, left: Val::Px(at.x), top: Val::Px(at.y), width: Val::Px(CELL), height: Val::Px(CELL), ..default() },
            ImageNode { image_mode: NodeImageMode::Auto, color: Color::srgba(1.0, 1.0, 1.0, 0.85), ..ImageNode::new(assets.load(item.icon())) },
            GlobalZIndex(40),
            Pickable::IGNORE,
        ));
    }
}

/// Click selects, right-click equips or takes off, dragging starts a move; hovering shows the
/// item in the details column.
fn item_events(e: &mut EntityCommands, uid: u64, from: Option<Slot>) {
    e.observe(move |ev: On<PointerClick>, mut pending: ResMut<Pending>| match ev.button {
        PointerButton::Primary => pending.0.push(Act::Select(uid)),
        PointerButton::Secondary => pending.0.push(match from {
            Some(slot) => Act::Unequip(slot),
            None => Act::Equip(uid),
        }),
        _ => {}
    })
    .observe(move |ev: On<PointerDragStart>, mut dragging: ResMut<Dragging>| {
        if ev.button == PointerButton::Primary {
            dragging.0 = Some((uid, from));
        }
    })
    .observe(move |_: On<PointerOver>, mut state: ResMut<InvState>| {
        if state.hover != Some(uid) {
            state.hover = Some(uid);
        }
    })
    .observe(move |_: On<PointerOut>, mut state: ResMut<InvState>| {
        if state.hover == Some(uid) {
            state.hover = None;
        }
    });
}

/// An item (or empty) cell: rarity-tinted fill in a gold frame, the TQ2 item icon, the
/// upgrade level in the corner.
fn cell(p: &mut ChildSpawnerCommands, assets: &AssetServer, inst: Option<&ItemInstance>, w: f32, h: f32, selected: bool, empty_icon: Option<&str>) -> Entity {
    let fill = inst.map_or(ui::ROW_FILL, |i| i.rarity.color().mix(&ui::ROW_FILL, 0.78));
    p.spawn((
        Node { width: Val::Px(w), height: Val::Px(h), padding: UiRect::all(Val::Px(5.0)), flex_shrink: 0.0, ..default() },
        BackgroundColor(fill),
    ))
    .with_children(|c| {
        c.spawn(ui::frame(assets, "t_frame_gold_small05", 8.0, if selected { Color::srgb(1.6, 1.4, 0.9) } else { Color::WHITE }));
        if let Some(item) = inst.and_then(|i| i.item()) {
            c.spawn((Node { width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() }, ImageNode { image_mode: NodeImageMode::Auto, ..ImageNode::new(assets.load(item.icon())) }, Pickable::IGNORE));
        } else if let Some(icon) = empty_icon {
            c.spawn((Node { width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() }, ImageNode { image_mode: NodeImageMode::Auto, color: Color::srgba(1.0, 1.0, 1.0, 0.35), ..ImageNode::new(assets.load(ui::ui_path(icon))) }, Pickable::IGNORE));
        }
        if let Some(i) = inst.filter(|i| i.level > 0) {
            c.spawn((Text::new(format!("+{}", i.level)), text(13.0), TextColor(GOLD), Node { position_type: PositionType::Absolute, right: Val::Px(4.0), bottom: Val::Px(1.0), ..default() }, Pickable::IGNORE));
        }
    })
    .id()
}

pub struct Ctx<'a> {
    pub assets: &'a AssetServer,
    pub title: &'a TitleFont,
    pub profile: &'a Profile,
    pub affixes: &'a AffixBook,
    pub state: &'a InvState,
    pub preview: &'a Preview,
}

/// The inventory page.
pub fn build(commands: &mut Commands, cx: &Ctx) {
    let Ctx { assets, title, profile, affixes, state, preview } = *cx;
    commands.spawn(ui::backdrop(assets, MainMenuUi)).with_children(|root| {
        // Header: title, hero, gold.
        root.spawn(Node { align_items: AlignItems::Center, column_gap: Val::Px(20.0), flex_shrink: 0.0, ..default() }).with_children(|h| {
            h.spawn((Text::new("Inventory"), title.at(40.0)));
            h.spawn((Text::new(profile.name.clone()), title.at(24.0), TextColor(GOLD)));
            h.spawn(Node { flex_grow: 1.0, ..default() });
            h.spawn((Node { width: Val::Px(34.0), height: Val::Px(34.0), ..default() }, ui::stretched(assets, "t_icon_embossed_coin02", Color::WHITE)));
            h.spawn((Text::new(format!("{} gold", profile.gold)), title.at(24.0), TextColor(GOLD)));
        });
        root.spawn(Node { column_gap: Val::Px(14.0), flex_grow: 1.0, min_height: Val::Px(0.0), ..default() }).with_children(|body| {
            character_column(body, cx);
            bag_column(body, cx);
            body.spawn(panel(assets, Node { width: Val::Px(300.0), flex_shrink: 0.0, flex_direction: FlexDirection::Column, ..default() })).with_children(|p| {
                // A separate column, so rebuilding it keeps the panel's frame.
                p.spawn((Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(8.0), flex_grow: 1.0, ..default() }, Details))
                    .with_children(|d| details(d, assets, title, profile, affixes, state));
            });
        });
        root.spawn(Node { align_items: AlignItems::Center, column_gap: Val::Px(18.0), flex_shrink: 0.0, ..default() }).with_children(|f| {
            f.spawn(button(assets, "Back (Esc)", 200.0, true, false)).observe(|_: On<PointerClick>, mut page: ResMut<Page>| *page = Page::Title);
            f.spawn((Text::new("Right-click: equip / take off   ·   Drag items onto the slots, back into the bag or onto the anvil"), text(15.0), TextColor(TEXT)));
        });
    });
    let _ = preview;
}

fn character_column(body: &mut ChildSpawnerCommands, cx: &Ctx) {
    let Ctx { assets, title, profile, affixes, state, preview } = *cx;
    let eq = profile.equipment(affixes);
    body.spawn(panel(assets, Node { width: Val::Px(400.0), flex_shrink: 0.0, flex_direction: FlexDirection::Column, align_items: AlignItems::Center, ..default() }))
        .with_children(|c| {
            headline(c, assets, title, "Equipment", 22.0);
            c.spawn(Node { width: Val::Px(352.0), height: Val::Px(400.0), flex_shrink: 0.0, ..default() }).with_children(|stage| {
                // The hero, rendered by the preview camera.
                stage.spawn((
                    Node { position_type: PositionType::Absolute, left: Val::Px(26.0), top: Val::Px(0.0), width: Val::Px(300.0), height: Val::Px(400.0), ..default() },
                    ImageNode::new(preview.0.clone()),
                    Pickable::IGNORE,
                ));
                // Slots around it: weapons left and right of the hands, armour at the chest.
                let worn = |uid: Option<u64>| uid.and_then(|u| profile.item(u));
                let two_handed = eq.main.two_handed;
                let main_icon = if two_handed { "t_2-4_slot_2h_staff_" } else { "t_2-3_slot_1h_sword" };
                for (slot, label, uid, left, top, w, h, empty) in [
                    (Slot::Main, "Main hand", profile.main, 0.0, 170.0, 76.0, 132.0, main_icon),
                    (Slot::Off, "Off hand", profile.off, 276.0, 170.0, 76.0, 132.0, "t_2-3_slot_1h_shield"),
                    (Slot::Armor, "Armour", profile.armor, 276.0, 30.0, 76.0, 96.0, "t_icon_equipment_2-3_chest"),
                ] {
                    stage.spawn(Node { position_type: PositionType::Absolute, left: Val::Px(left), top: Val::Px(top - 18.0), flex_direction: FlexDirection::Column, align_items: AlignItems::Center, ..default() }).with_children(|s| {
                        s.spawn((Text::new(label), text(13.0), TextColor(TEXT), Pickable::IGNORE));
                        let inst = worn(uid);
                        let blocked = slot == Slot::Off && two_handed;
                        let id = cell(s, assets, inst, w, h, inst.is_some_and(|i| state.selected == Some(i.uid)), (!blocked).then_some(empty));
                        let mut commands = s.commands();
                        let mut e = commands.entity(id);
                        if let Some(i) = inst {
                            item_events(&mut e, i.uid, Some(slot));
                        }
                        e.observe(move |_: On<PointerDragDrop>, dragging: Res<Dragging>, mut pending: ResMut<Pending>| {
                            if let Some((uid, from)) = dragging.0 {
                                if from != Some(slot) {
                                    pending.0.push(Act::Put(uid, slot));
                                }
                            }
                        });
                    });
                }
            });
            // What the gear adds up to.
            let b = eq.bonus;
            let mut lines = vec![
                format!("{:.0} weapon damage every {:.2} s", eq.weapon_damage() * (1.0 + b.weapon_pct), eq.cooldown() / (1.0 + b.attack_speed)),
                format!("{:.0} % less damage taken   ·   skills ×{:.2}", eq.damage_reduction() * 100.0, eq.skill_damage() * (1.0 + b.skill_pct)),
            ];
            let mut extra = vec![];
            if b.health > 0.0 {
                extra.push(format!("+{:.0} health", b.health));
            }
            if b.crit > 0.0 {
                extra.push(format!("+{:.0} % crit", b.crit * 100.0));
            }
            if b.attack_speed > 0.0 {
                extra.push(format!("+{:.0} % attack speed", b.attack_speed * 100.0));
            }
            if !extra.is_empty() {
                lines.push(extra.join("   ·   "));
            }
            for l in lines {
                c.spawn((Text::new(l), text(15.0), TextColor(TEXT)));
            }
        });
}

fn bag_column(body: &mut ChildSpawnerCommands, cx: &Ctx) {
    let Ctx { assets, title, profile, state, .. } = *cx;
    let mut bag = profile.bag();
    bag.sort_by(|a, b| b.rarity.cmp(&a.rarity).then(a.base.cmp(&b.base)));
    let capacity = profile.bag_capacity().max(bag.len());
    let mut panel_cmd = body.spawn(panel(assets, Node { flex_grow: 1.0, flex_direction: FlexDirection::Column, row_gap: Val::Px(8.0), ..default() }));
    panel_cmd.with_children(|b| {
        b.spawn(Node { align_items: AlignItems::Center, column_gap: Val::Px(10.0), flex_shrink: 0.0, ..default() }).with_children(|h| {
            h.spawn((Node { width: Val::Px(30.0), height: Val::Px(30.0), ..default() }, ui::stretched(assets, "t_icon_embossed_bag", Color::WHITE)));
            h.spawn((Text::new(format!("Bag  {}/{}", bag.len(), profile.bag_capacity())), title.at(22.0), TextColor(GOLD)));
        });
        b.spawn(Node {
            display: Display::Grid,
            grid_template_columns: RepeatedGridTrack::px(COLUMNS as u16, CELL),
            column_gap: Val::Px(4.0),
            row_gap: Val::Px(4.0),
            flex_grow: 1.0,
            min_height: Val::Px(0.0),
            overflow: Overflow::scroll_y(),
            align_content: AlignContent::Start,
            ..default()
        })
        .with_children(|grid| {
            for i in 0..capacity {
                let inst = bag.get(i).copied();
                let id = cell(grid, assets, inst, CELL, CELL, inst.is_some_and(|x| state.selected == Some(x.uid)), None);
                if let Some(x) = inst {
                    let mut commands = grid.commands();
                    item_events(&mut commands.entity(id), x.uid, None);
                }
            }
        });
        match profile.bag_upgrade_cost() {
            Some(cost) => {
                b.spawn(button(assets, format!("Larger bag (+{})  ·  {cost} gold", super::profile::BAG_STEP), 340.0, profile.gold >= cost, false))
                    .observe(|_: On<PointerClick>, mut pending: ResMut<Pending>| pending.0.push(Act::UpgradeBag));
            }
            None => {
                b.spawn((Text::new("The bag has its full size."), text(15.0), TextColor(TEXT)));
            }
        }
    });
    // Dropping a worn item anywhere on the bag takes it off.
    panel_cmd.observe(|_: On<PointerDragDrop>, dragging: Res<Dragging>, mut pending: ResMut<Pending>| {
        if let Some((_, Some(slot))) = dragging.0 {
            pending.0.push(Act::Unequip(slot));
        }
    });
}

/// The selected (or hovered) item with what can be done to it, and the anvil drop zone.
fn details(d: &mut ChildSpawnerCommands, assets: &AssetServer, title: &TitleFont, profile: &Profile, affixes: &AffixBook, state: &InvState) {
    headline(d, assets, title, "Anvil", 22.0);
    let shown = state.hover.or(state.selected).and_then(|u| profile.item(u));
    match shown {
        Some(inst) => {
            ui::item_card(d, assets, title, inst, affixes, 252.0, None, |_| {});
            if Some(inst.uid) == state.selected && state.hover.is_none_or(|h| h == inst.uid) {
                let uid = inst.uid;
                let worn = worn_slot(profile, uid);
                match worn {
                    Some(slot) => {
                        d.spawn(button(assets, "Take off", 252.0, slot != Slot::Main, false)).observe(move |_: On<PointerClick>, mut p: ResMut<Pending>| p.0.push(Act::Unequip(slot)));
                    }
                    None => {
                        d.spawn(button(assets, "Equip", 252.0, true, true)).observe(move |_: On<PointerClick>, mut p: ResMut<Pending>| p.0.push(Act::Equip(uid)));
                    }
                }
                match inst.upgrade_cost() {
                    Some(cost) => {
                        d.spawn(button(assets, format!("Upgrade to +{}  ·  {cost} gold", inst.level + 1), 252.0, profile.gold >= cost, false))
                            .observe(move |_: On<PointerClick>, mut p: ResMut<Pending>| p.0.push(Act::Upgrade(uid)));
                    }
                    None => {
                        d.spawn((Text::new("Fully upgraded."), text(15.0), TextColor(TEXT)));
                    }
                }
                if worn.is_none() {
                    d.spawn(button(assets, format!("Recycle  ·  +{} gold", inst.salvage_value()), 252.0, true, false))
                        .observe(move |_: On<PointerClick>, mut p: ResMut<Pending>| p.0.push(Act::Salvage(uid)));
                }
            }
        }
        None => {
            d.spawn((Text::new("Click an item to select it. Each upgrade adds 5 % to all its stats."), text(15.0), TextColor(TEXT)));
        }
    }
    d.spawn(Node { flex_grow: 1.0, ..default() });
    // Drop zone: recycle into gold.
    d.spawn((
        Node { height: Val::Px(84.0), flex_shrink: 0.0, column_gap: Val::Px(10.0), align_items: AlignItems::Center, justify_content: JustifyContent::Center, padding: UiRect::all(Val::Px(8.0)), ..default() },
        BackgroundColor(ui::ROW_FILL),
    ))
    .with_children(|z| {
        z.spawn(ui::frame(assets, "t_button_salvage_normal01", 30.0, Color::WHITE));
        z.spawn((Node { width: Val::Px(50.0), height: Val::Px(36.0), ..default() }, ui::stretched(assets, "t_icon_button_salvage", Color::WHITE), Pickable::IGNORE));
        z.spawn((Text::new("Drop an item here\nto recycle it"), text(15.0), TextColor(TEXT), Pickable::IGNORE));
    })
    .observe(|_: On<PointerDragDrop>, dragging: Res<Dragging>, mut pending: ResMut<Pending>| {
        if let Some((uid, None)) = dragging.0 {
            pending.0.push(Act::Salvage(uid));
        }
    });
    if !state.message.is_empty() {
        d.spawn((Text::new(state.message.clone()), text(15.0), TextColor(GOLD)));
    }
}

fn refresh_details(
    mut commands: Commands,
    assets: Res<AssetServer>,
    title: Res<TitleFont>,
    profile: Res<Profile>,
    affixes: Res<AffixBook>,
    state: Res<InvState>,
    columns: Query<Entity, With<Details>>,
) {
    for column in &columns {
        commands.entity(column).despawn_related::<Children>().with_children(|d| details(d, &assets, &title, &profile, &affixes, &state));
    }
}

//! Building blocks for menus in TQ2's look: its gold frames (9-sliced), gemstone buttons,
//! marble background and coloured text spans.
use bevy::prelude::*;

use super::mastery_data::{ui_texture, TEXT};

/// A TQ2 UI texture as a 9-sliced image (corners of `border` px stay sharp).
pub fn sliced(assets: &AssetServer, name: &str, border: f32, color: Color) -> ImageNode {
    ImageNode {
        color,
        image_mode: NodeImageMode::Sliced(TextureSlicer { border: BorderRect::all(border), max_corner_scale: 1.0, ..default() }),
        ..ImageNode::new(assets.load(ui_texture(name)))
    }
}

/// A TQ2 UI texture stretched over the node.
pub fn stretched(assets: &AssetServer, name: &str, color: Color) -> ImageNode {
    ImageNode { color, image_mode: NodeImageMode::Stretch, ..ImageNode::new(assets.load(ui_texture(name))) }
}

/// Any texture below the asset root, stretched.
pub fn image(assets: &AssetServer, path: String, color: Color) -> ImageNode {
    ImageNode { color, image_mode: NodeImageMode::Stretch, ..ImageNode::new(assets.load(path)) }
}

/// Full-screen dark marble backdrop (TQ2 menu background).
pub fn backdrop(assets: &AssetServer, marker: impl Component) -> impl Bundle {
    (
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            padding: UiRect::axes(Val::Px(28.0), Val::Px(18.0)),
            row_gap: Val::Px(10.0),
            ..default()
        },
        // The marble has soft transparent edges; a dark fill keeps the scene from showing through.
        BackgroundColor(Color::srgb(0.035, 0.033, 0.04)),
        stretched(assets, "t_backdrop_marble_dark", Color::srgb(0.55, 0.52, 0.5)),
        marker,
    )
}

/// A panel in a gold frame with a dark fill.
pub fn panel(assets: &AssetServer, node: Node) -> impl Bundle {
    (
        Node { padding: UiRect::all(Val::Px(18.0)), ..node },
        BackgroundColor(Color::srgba(0.03, 0.03, 0.04, 0.82)),
        sliced(assets, "t_frame_gold_medium02", 22.0, Color::WHITE),
    )
}

/// A TQ2 gemstone button with a label; `enabled = false` uses the grey deactivated stone.
pub fn button(assets: &AssetServer, label: impl Into<String>, width: f32, enabled: bool, accent: bool) -> impl Bundle {
    let texture = match (enabled, accent) {
        (false, _) => "t_button_gemstone_medium01_deactivated01",
        (true, true) => "t_button_gemstone_medium01_normal01_green01",
        (true, false) => "t_button_gemstone_medium01_normal01_grey01",
    };
    (
        Node {
            width: Val::Px(width),
            height: Val::Px(44.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        sliced(assets, texture, 12.0, Color::WHITE),
        children![(Text::new(label), text(20.0), TextColor(if enabled { Color::WHITE } else { Color::srgb(0.6, 0.6, 0.6) }))],
    )
}

pub fn text(size: f32) -> TextFont {
    TextFont { font_size: FontSize::Px(size), ..default() }
}

/// A text block made of coloured spans (from `mastery_data::render`).
pub fn rich(parent: &mut ChildSpawnerCommands, spans: Vec<(String, Color)>, size: f32, width: Val) {
    parent
        .spawn((Text::new(""), text(size), TextColor(TEXT), Node { width, ..default() }))
        .with_children(|t| {
            for (s, color) in spans {
                t.spawn((TextSpan::new(s), text(size), TextColor(color)));
            }
        });
}

pub fn roman(n: usize) -> &'static str {
    ["I", "II", "III", "IV", "V", "VI"].get(n).copied().unwrap_or("?")
}

/// An item in TQ2 style: icon in a gold frame on a rarity-tinted backdrop, name in the rarity
/// colour, rarity and base value, affix lines. `setup` can attach observers to the card.
pub fn item_card(
    parent: &mut ChildSpawnerCommands,
    assets: &AssetServer,
    title: &crate::fonts::TitleFont,
    inst: &super::items::ItemInstance,
    book: &super::items::AffixBook,
    width: f32,
    note: Option<String>,
    setup: impl FnOnce(&mut EntityCommands),
) {
    let color = inst.rarity.color();
    let kind = inst.item().map_or("", |i| match i {
        super::equipment::Item::Weapon(w) if w.two_handed => "Two-handed weapon",
        super::equipment::Item::Weapon(_) => "One-handed weapon",
        super::equipment::Item::Shield(_) => "Shield",
        super::equipment::Item::Talisman(_) => "Talisman",
        super::equipment::Item::Armor(_) => "Armour",
    });
    let mut card = parent.spawn((
        Node {
            width: Val::Px(width),
            padding: UiRect::all(Val::Px(8.0)),
            column_gap: Val::Px(12.0),
            align_items: AlignItems::FlexStart,
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.07, 0.065, 0.06, 0.92)),
        BorderColor::all(color.with_alpha(0.6)),
    ));
    card.with_children(|c| {
        c.spawn((
            Node { width: Val::Px(64.0), height: Val::Px(64.0), padding: UiRect::all(Val::Px(6.0)), flex_shrink: 0.0, ..default() },
            BackgroundColor(color.with_alpha(0.18)),
            sliced(assets, "t_frame_gold_small05", 8.0, Color::WHITE),
        ))
        .with_children(|f| {
            if let Some(item) = inst.item() {
                f.spawn((Node { width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() }, ImageNode { image_mode: NodeImageMode::Auto, ..ImageNode::new(assets.load(item.icon())) }));
            }
        });
        c.spawn(Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(2.0), flex_grow: 1.0, ..default() }).with_children(|t| {
            t.spawn((Text::new(inst.name(book)), title.at(20.0), TextColor(color)));
            t.spawn((Text::new(format!("{} {kind} · {}", inst.rarity.name(), inst.base_line())), text(15.0), TextColor(TEXT)));
            for line in inst.lines(book) {
                rich(t, line, 15.0, Val::Auto);
            }
            if let Some(note) = note {
                t.spawn((Text::new(note), text(14.0), TextColor(super::mastery_data::GOLD)));
            }
        });
    });
    setup(&mut card);
}

/// Menus and HUD are laid out on a 1280×720 canvas; scale the whole UI to the actual window so
/// it looks the same at any resolution and Windows display scaling.
pub fn scale_to_window(windows: Query<&Window, Changed<Window>>, mut scale: ResMut<UiScale>) {
    if let Ok(window) = windows.single() {
        let s = (window.height() / 720.0).clamp(0.5, 4.0);
        if (scale.0 - s).abs() > 0.001 {
            scale.0 = s;
        }
    }
}

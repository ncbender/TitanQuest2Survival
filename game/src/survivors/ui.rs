//! Building blocks for menus in TQ2's look: warm marble backdrop with a vignette, brown
//! stone panels in gold frames (9-sliced), gemstone buttons and coloured text spans.
//!
//! Colours: panels are warm dark brown (never black) so cream text and TQ2's dark icon art
//! stay readable on them.
use bevy::prelude::*;
use bevy::ui_widgets::{ControlOrientation, ScrollArea, Scrollbar, ScrollbarThumb};

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

/// Asset path of a TQ2 UI texture.
pub fn ui_path(name: &str) -> String {
    ui_texture(name)
}

/// Any texture below the asset root, stretched.
pub fn image(assets: &AssetServer, path: String, color: Color) -> ImageNode {
    ImageNode { color, image_mode: NodeImageMode::Stretch, ..ImageNode::new(assets.load(path)) }
}

/// Fill of panels and cards: warm dark brown stone.
pub const PANEL_FILL: Color = Color::srgba(0.15, 0.12, 0.095, 0.94);
/// Fill of rows and slots inside panels (a little lighter).
pub const ROW_FILL: Color = Color::srgba(0.21, 0.17, 0.13, 0.95);

/// A 9-sliced frame over its parent's whole box. Bevy draws a node's own image only inside
/// its padding, so frames around padded content must be a child like this one (spawned
/// first, it stays behind the content).
pub fn frame(assets: &AssetServer, name: &str, border: f32, color: Color) -> impl Bundle {
    (cover(0.0), sliced(assets, name, border, color), ZIndex(-1), Pickable::IGNORE)
}

/// A colour fill over the parent's box, `inset` from its edges (inside a frame's art).
pub fn fill(color: Color, inset: f32) -> impl Bundle {
    (cover(inset), BackgroundColor(color), ZIndex(-1), Pickable::IGNORE)
}

fn cover(inset: f32) -> Node {
    Node { position_type: PositionType::Absolute, left: Val::Px(inset), right: Val::Px(inset), top: Val::Px(inset), bottom: Val::Px(inset), ..default() }
}

/// Full-screen TQ2 menu background: warm marble with a darkened rim.
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
        BackgroundColor(Color::srgb(0.16, 0.13, 0.105)),
        children![
            (cover(0.0), stretched(assets, "t_marble_bg_ui", Color::srgb(0.42, 0.355, 0.29)), ZIndex(-1), Pickable::IGNORE),
            (cover(0.0), stretched(assets, "t_overlay_vignette", Color::srgba(0.0, 0.0, 0.0, 0.85)), ZIndex(-1), Pickable::IGNORE),
        ],
        marker,
    )
}

/// A column whose content scrolls (mouse wheel or dragging the gold bar) when it is taller than
/// the space the column gets; the bar only shows then. `node` sizes the whole area (give it a
/// width or flex_grow); the content is laid out top to bottom with `gap` between items.
pub fn scroll_area(parent: &mut ChildSpawnerCommands, node: Node, gap: f32, content: impl FnOnce(&mut ChildSpawnerCommands)) {
    parent.spawn(Node { column_gap: Val::Px(6.0), min_height: Val::Px(0.0), flex_basis: Val::Px(0.0), flex_grow: 1.0, ..node }).with_children(|area| {
        let list = area
            .spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(gap),
                    flex_grow: 1.0,
                    min_width: Val::Px(0.0),
                    min_height: Val::Px(0.0),
                    overflow: Overflow::scroll_y(),
                    ..default()
                },
                ScrollArea,
            ))
            .with_children(content)
            .id();
        area.spawn((
            Node { width: Val::Px(7.0), flex_shrink: 0.0, ..default() },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.35)),
            Scrollbar::new(list, ControlOrientation::Vertical, 24.0),
            Visibility::Hidden,
        ))
        .with_children(|bar| {
            bar.spawn((ScrollbarThumb { border_radius: BorderRadius::all(Val::Px(3.0)), ..default() }, BackgroundColor(super::mastery_data::GOLD.with_alpha(0.8))));
        });
    });
}

/// Shows a scroll area's bar only while its content overflows.
pub fn show_scrollbars(mut bars: Query<(&Scrollbar, &mut Visibility)>, nodes: Query<&ComputedNode>) {
    for (bar, mut vis) in &mut bars {
        let Ok(node) = nodes.get(bar.target) else { continue };
        let wanted = if node.content_size().y > node.size().y + 1.0 { Visibility::Inherited } else { Visibility::Hidden };
        if *vis != wanted {
            *vis = wanted;
        }
    }
}

/// A panel: brown stone in a gold frame. The padding keeps content clear of the frame art.
pub fn panel(assets: &AssetServer, node: Node) -> impl Bundle {
    (
        Node { padding: UiRect::all(Val::Px(26.0)), ..node },
        children![fill(PANEL_FILL, 5.0), frame(assets, "t_frame_gold_medium02", 26.0, Color::WHITE)],
    )
}

/// A section title with TQ2's thin gold ornament line under it.
pub fn headline(parent: &mut ChildSpawnerCommands, assets: &AssetServer, title: &crate::fonts::TitleFont, label: impl Into<String>, size: f32) {
    parent.spawn(Node { flex_direction: FlexDirection::Column, align_items: AlignItems::Center, margin: UiRect::bottom(Val::Px(6.0)), flex_shrink: 0.0, ..default() }).with_children(|h| {
        h.spawn((Text::new(label), title.at(size), TextColor(super::mastery_data::GOLD)));
        h.spawn((Node { width: Val::Px(size * 9.0), height: Val::Px(10.0), margin: UiRect::top(Val::Px(2.0)), ..default() }, stretched(assets, "t_frame_gold_headline_thin_top", Color::WHITE)));
    });
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
        super::equipment::Item::Armor(a) => a.slot.label(),
        super::equipment::Item::Jewelry(j) => match j.kind {
            super::equipment::JewelryKind::Ring => "Ring",
            super::equipment::JewelryKind::Amulet => "Amulet",
        },
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
        BackgroundColor(ROW_FILL),
        BorderColor::all(color.with_alpha(0.6)),
    ));
    card.with_children(|c| {
        c.spawn((
            Node { width: Val::Px(64.0), height: Val::Px(64.0), padding: UiRect::all(Val::Px(6.0)), flex_shrink: 0.0, ..default() },
            BackgroundColor(color.with_alpha(0.18)),
            children![frame(assets, "t_frame_gold_small05", 8.0, Color::WHITE)],
        ))
        .with_children(|f| {
            if let Some(item) = inst.item() {
                f.spawn((Node { width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() }, ImageNode { image_mode: NodeImageMode::Auto, ..ImageNode::new(assets.load(item.icon())) }));
            }
        });
        c.spawn(Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(2.0), flex_grow: 1.0, ..default() }).with_children(|t| {
            t.spawn((Text::new(inst.name(book)), title.at(20.0), TextColor(color)));
            let upgrade = if inst.level > 0 { format!(" · Upgrade {}", inst.level) } else { String::new() };
            t.spawn((Text::new(format!("{} {kind} · {}{upgrade}", inst.rarity.name(), inst.base_line())), text(15.0), TextColor(TEXT)));
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

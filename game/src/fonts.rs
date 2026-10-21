//! TQ2's own UI fonts (exported by `Tq2Extract assets fonts`): EB Garamond replaces Bevy's
//! built-in default font (which lacks German umlauts), Cinzel is used for titles.
use bevy::asset::AssetId;
use bevy::prelude::*;

/// Font for headings (Cinzel); falls back to the default font if the export is missing.
#[derive(Resource, Default, Clone)]
pub struct TitleFont(pub Handle<Font>);

pub fn install(app: &mut App, export_root: &str) {
    let world = app.world_mut();
    match std::fs::read(format!("{export_root}/fonts/text.ttf")) {
        Ok(bytes) => {
            let _ = world.resource_mut::<Assets<Font>>().insert(AssetId::default(), Font::from_bytes(bytes));
        }
        Err(e) => warn!("TQ2 text font not exported ({e}); using Bevy's default font"),
    }
    let title = if std::path::Path::new(&format!("{export_root}/fonts/title.ttf")).exists() {
        world.resource::<AssetServer>().load("fonts/title.ttf")
    } else {
        Handle::default()
    };
    world.insert_resource(TitleFont(title));
}

impl TitleFont {
    pub fn at(&self, size: f32) -> TextFont {
        TextFont { font: self.0.clone().into(), font_size: FontSize::Px(size), ..default() }
    }
}

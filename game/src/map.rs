//! The current map: a TQ2 scene export with its ground heights, obstacles and blended terrain.
//! Selecting another map (an arena) swaps it before the next run starts.
use bevy::prelude::*;

use crate::ground::Ground;
use crate::survivors::Arena;
use crate::tq2_materials::MaterialLibrary;
use crate::tq2_scene::{self, MapProp};

/// Export folder of the map that should be shown.
#[derive(Resource, Clone, PartialEq)]
pub struct SelectedMap(pub String);

/// Export folder of the map that is loaded.
#[derive(Resource)]
pub struct LoadedMap(pub String);

impl LoadedMap {
    /// The selected map is on screen (runs wait for this).
    pub fn ready(&self, selected: &SelectedMap) -> bool {
        self.0 == selected.0
    }
}

/// Triggered after a map was loaded (`Ground` and `Obstacles` are new).
#[derive(Event)]
pub struct MapLoaded;

/// Replaces the current map with `<export root>/<scene>`.
pub fn load(world: &mut World, scene: &str) {
    let root = crate::export_root();
    let old: Vec<Entity> = world.query_filtered::<Entity, With<MapProp>>().iter(world).collect();
    for e in old {
        world.despawn(e);
    }
    let assets = world.resource::<AssetServer>().clone();
    world.resource_mut::<MaterialLibrary>().load_dir(&root, scene);
    let mut info = {
        let mut commands = world.commands();
        tq2_scene::spawn_scene(&mut commands, &assets, &root, scene)
    };
    world.flush();
    crate::terrain::load(world, &format!("{root}/{scene}"));

    let water = info.as_ref().and_then(|i| i.water);
    let obstacles = info.as_mut().map(|i| std::mem::take(&mut i.obstacles)).unwrap_or_default();
    let mut ground = Ground::load(&format!("{root}/{scene}/ground.json"), water).unwrap_or_else(Ground::flat);
    // Keep units where the map has props; the ground past them is only scenery.
    ground.limit = info.as_ref().map(|i| i.props).filter(|r| !r.is_empty());
    let center = info.map(|i| Vec2::new(i.center.x, i.center.z)).unwrap_or_default();
    // Spiral outwards from the map centre until we stand on land.
    let start = (0..400)
        .map(|i| center + Vec2::from_angle(i as f32 * 0.7) * (i as f32 * 0.25))
        .find(|p| ground.is_land(*p) && obstacles.resolve(*p, 0.6, ground.stand(*p)) == *p)
        .unwrap_or(center);
    info!("map {scene}: start at {start}");
    spawn_pit_floor(world, &ground);
    world.insert_resource(ground);
    world.insert_resource(obstacles);
    world.insert_resource(Arena { start });
    world.insert_resource(LoadedMap(scene.into()));
    if world.get_resource::<SelectedMap>().is_none() {
        world.insert_resource(SelectedMap(scene.into()));
    }
    world.trigger(MapLoaded);
}

/// A dark floor below the ground, seen through landscape holes (TQ2 has cellars or caves
/// there, which the arenas do not include).
fn spawn_pit_floor(world: &mut World, ground: &Ground) {
    let b = ground.bounds();
    let size = b.size() + Vec2::splat(400.0);
    let mesh = world.resource_mut::<Assets<Mesh>>().add(Plane3d::new(Vec3::Y, size / 2.0));
    let material = world
        .resource_mut::<Assets<StandardMaterial>>()
        .add(StandardMaterial { base_color: Color::srgb(0.07, 0.06, 0.05), perceptual_roughness: 1.0, ..default() });
    let c = b.center();
    world.spawn((MapProp, Mesh3d(mesh), MeshMaterial3d(material), Transform::from_xyz(c.x, ground.lowest() - 1.0, c.y)));
}

/// Loads the selected map when it changed.
pub fn sync(world: &mut World) {
    let Some(want) = world.get_resource::<SelectedMap>().map(|s| s.0.clone()) else { return };
    if world.get_resource::<LoadedMap>().is_some_and(|l| l.0 == want) {
        return;
    }
    load(world, &want);
}

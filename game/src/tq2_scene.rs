//! Spawns a map exported by `Tq2Extract scene`: one glTF per unique mesh, placed
//! by `scene.json` (transforms already in Bevy space).
use bevy::prelude::*;

use crate::obstacles::Obstacles;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize)]
struct SceneFile {
    instances: Vec<Instance>,
}

#[derive(Deserialize)]
struct Instance {
    mesh: String,
    t: [f32; 3],
    r: [f32; 4],
    s: [f32; 3],
    /// False when the placed component switched its collision off.
    #[serde(default = "yes")]
    c: bool,
    /// False for invisible blockers (collision only).
    #[serde(default = "yes")]
    v: bool,
}

fn yes() -> bool {
    true
}

/// One simple-collision shape of a mesh, in mesh space (see `Tq2Extract` SceneExport).
#[derive(Deserialize)]
pub struct Shape {
    points: Vec<[f32; 3]>,
    radius: f32,
}

/// Root of a placed map mesh (as opposed to characters, projectiles, …).
#[derive(Component)]
pub struct MapProp;

pub struct SceneInfo {
    /// Median placement origin (robust against far-away outliers).
    pub center: Vec3,
    /// Height of the map's water surface mesh, if any.
    pub water: Option<f32>,
    /// Ground-plane (x, z) extent of the placed props (landscape excluded).
    pub props: Rect,
    pub obstacles: Obstacles,
}

/// Loads `<export_root>/<dir>/scene.json` and spawns every placement.
pub fn spawn_scene(commands: &mut Commands, assets: &AssetServer, export_root: &str, dir: &str) -> Option<SceneInfo> {
    let path = format!("{export_root}/{dir}/scene.json");
    let file: SceneFile = match std::fs::read_to_string(&path).map(|s| serde_json::from_str(&s)) {
        Ok(Ok(f)) => f,
        Ok(Err(e)) => { error!("{path}: {e}"); return None; }
        Err(e) => { error!("{path}: {e}"); return None; }
    };
    let mut handles = HashMap::new();
    for inst in file.instances.iter().filter(|i| i.v) {
        let scene = handles
            .entry(inst.mesh.clone())
            .or_insert_with(|| assets.load(GltfAssetLabel::Scene(0).from_asset(format!("{dir}/{}", inst.mesh))))
            .clone();
        let t = Vec3::from_array(inst.t);
        commands.spawn((
            WorldAssetRoot(scene),
            MapProp,
            Transform {
                translation: t,
                rotation: Quat::from_array(inst.r).normalize(),
                scale: Vec3::from_array(inst.s),
            },
        ));
    }
    info!("{dir}: {} instances of {} meshes", file.instances.len(), handles.len());
    let median = |axis: usize| {
        let mut v: Vec<f32> = file.instances.iter().filter(|i| i.v).map(|i| i.t[axis]).collect();
        v.sort_by(f32::total_cmp);
        v.get(v.len() / 2).copied().unwrap_or(0.0)
    };
    let water = file.instances.iter().find(|i| i.mesh.contains("watersurface")).map(|i| i.t[1]);
    let obstacles = load_obstacles(&file, export_root, dir);
    let mut props = Rect::EMPTY;
    for i in file.instances.iter().filter(|i| i.v && !i.mesh.contains("Landscape")) {
        props = props.union_point(Vec2::new(i.t[0], i.t[2]));
    }
    Some(SceneInfo { center: Vec3::new(median(0), median(1), median(2)), water, props, obstacles })
}

/// Collision shapes per mesh (keys like "meshes/sm_x.glb") from `<dir>/colliders.json`.
pub fn colliders(export_root: &str, dir: &str) -> HashMap<String, Vec<Shape>> {
    let path = format!("{export_root}/{dir}/colliders.json");
    match std::fs::read_to_string(&path).map(|s| serde_json::from_str(&s)) {
        Ok(Ok(s)) => s,
        _ => {
            warn!("{path}: no colliders");
            HashMap::new()
        }
    }
}

/// Adds a placed mesh's collision shapes to the obstacles.
pub fn add_shapes(obstacles: &mut Obstacles, shapes: &[Shape], transform: &Transform) {
    let scale = transform.scale.x.abs().max(transform.scale.z.abs());
    for shape in shapes {
        let points: Vec<Vec3> = shape.points.iter().map(|p| transform.transform_point(Vec3::from_array(*p))).collect();
        obstacles.add(&points, shape.radius * scale);
    }
}

/// Places every collision shape of every colliding instance into the world.
fn load_obstacles(file: &SceneFile, export_root: &str, dir: &str) -> Obstacles {
    let mut obstacles = Obstacles::default();
    let shapes = colliders(export_root, dir);
    for inst in file.instances.iter().filter(|i| i.c) {
        let Some(mesh_shapes) = shapes.get(&inst.mesh) else { continue };
        let transform = Transform {
            translation: Vec3::from_array(inst.t),
            rotation: Quat::from_array(inst.r).normalize(),
            scale: Vec3::from_array(inst.s),
        };
        add_shapes(&mut obstacles, mesh_shapes, &transform);
    }
    info!("{export_root}/{dir}: {} obstacle shapes", obstacles.len());
    obstacles
}

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
}

fn yes() -> bool {
    true
}

/// One simple-collision shape of a mesh, in mesh space (see `Tq2Extract` SceneExport).
#[derive(Deserialize)]
struct Shape {
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
    for inst in &file.instances {
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
        let mut v: Vec<f32> = file.instances.iter().map(|i| i.t[axis]).collect();
        v.sort_by(f32::total_cmp);
        v.get(v.len() / 2).copied().unwrap_or(0.0)
    };
    let water = file.instances.iter().find(|i| i.mesh.contains("watersurface")).map(|i| i.t[1]);
    let obstacles = load_obstacles(&file, export_root, dir);
    Some(SceneInfo { center: Vec3::new(median(0), median(1), median(2)), water, obstacles })
}

/// Places every collision shape of every colliding instance into the world.
fn load_obstacles(file: &SceneFile, export_root: &str, dir: &str) -> Obstacles {
    let mut obstacles = Obstacles::default();
    let path = format!("{export_root}/{dir}/colliders.json");
    let shapes: HashMap<String, Vec<Shape>> = match std::fs::read_to_string(&path).map(|s| serde_json::from_str(&s)) {
        Ok(Ok(s)) => s,
        _ => {
            warn!("{path}: no colliders");
            return obstacles;
        }
    };
    for inst in file.instances.iter().filter(|i| i.c) {
        let Some(mesh_shapes) = shapes.get(&inst.mesh) else { continue };
        let transform = Transform {
            translation: Vec3::from_array(inst.t),
            rotation: Quat::from_array(inst.r).normalize(),
            scale: Vec3::from_array(inst.s),
        };
        let scale = inst.s[0].abs().max(inst.s[2].abs());
        for shape in mesh_shapes {
            let points: Vec<Vec3> = shape.points.iter().map(|p| transform.transform_point(Vec3::from_array(*p))).collect();
            obstacles.add(&points, shape.radius * scale);
        }
    }
    info!("{path}: {} obstacle shapes", obstacles.len());
    obstacles
}

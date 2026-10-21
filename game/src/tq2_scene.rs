//! Spawns a map exported by `Tq2Extract scene`: one glTF per unique mesh, placed
//! by `scene.json` (transforms already in Bevy space).
use bevy::prelude::*;
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
}

/// Loads `<export_root>/<dir>/scene.json` and spawns every placement.
/// Returns the median placement origin (robust against far-away outliers).
pub fn spawn_scene(commands: &mut Commands, assets: &AssetServer, export_root: &str, dir: &str) -> Option<Vec3> {
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
    Some(Vec3::new(median(0), median(1), median(2)))
}

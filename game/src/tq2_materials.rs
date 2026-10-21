//! Replaces the placeholder glTF materials of exported TQ2 meshes with one shared
//! `StandardMaterial` per TQ2 material, built from `materials.json` (written by
//! `Tq2Extract`'s MaterialLibrary) and the original texture maps.
use bevy::gltf::GltfMaterialName;
use bevy::image::{ImageAddressMode, ImageLoaderSettings, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use serde::Deserialize;
use std::collections::HashMap;

pub struct Tq2MaterialsPlugin;

impl Plugin for Tq2MaterialsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MaterialLibrary>()
            .add_systems(Update, (apply_materials, strip_vertex_colors));
    }
}

#[derive(Deserialize, Clone)]
struct Def {
    albedo: Option<String>,
    normal: Option<String>,
    orm: Option<String>,
    color: [f32; 4],
    alpha: String,
    cutoff: f32,
    two_sided: bool,
}

#[derive(Resource, Default)]
pub struct MaterialLibrary {
    /// material name -> (export dir, definition)
    defs: HashMap<String, (String, Def)>,
    built: HashMap<String, Handle<StandardMaterial>>,
}

impl MaterialLibrary {
    /// Registers `<export_root>/<dir>/materials.json`; paths in it are relative to `dir`.
    pub fn load_dir(&mut self, export_root: &str, dir: &str) {
        let path = format!("{export_root}/{dir}/materials.json");
        match std::fs::read_to_string(&path).map(|s| serde_json::from_str::<HashMap<String, Def>>(&s)) {
            Ok(Ok(defs)) => {
                info!("{path}: {} materials", defs.len());
                for (name, def) in defs {
                    self.defs.insert(name.to_lowercase(), (dir.to_string(), def));
                }
            }
            Ok(Err(e)) => error!("{path}: {e}"),
            Err(e) => warn!("{path}: {e}"),
        }
    }

    fn get(&mut self, name: &str, assets: &AssetServer, materials: &mut Assets<StandardMaterial>) -> Option<Handle<StandardMaterial>> {
        let key = name.to_lowercase();
        if let Some(h) = self.built.get(&key) {
            return Some(h.clone());
        }
        let (dir, def) = self.defs.get(&key)?.clone();
        let load = |file: &Option<String>, srgb: bool| {
            file.as_ref().map(|f| {
                assets.load_builder().with_settings(move |s: &mut ImageLoaderSettings| {
                    s.is_srgb = srgb;
                    // TQ2 environment textures tile; Bevy defaults to clamp-to-edge.
                    s.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
                        address_mode_u: ImageAddressMode::Repeat,
                        address_mode_v: ImageAddressMode::Repeat,
                        ..ImageSamplerDescriptor::linear()
                    });
                }).load(format!("{dir}/{f}"))
            })
        };
        let [r, g, b, a] = def.color;
        let mut mat = StandardMaterial {
            base_color: Color::linear_rgba(r, g, b, a),
            base_color_texture: load(&def.albedo, true),
            normal_map_texture: load(&def.normal, false),
            // UE normal maps use the DirectX convention (green down).
            flip_normal_map_y: true,
            perceptual_roughness: 0.85,
            metallic: 0.0,
            double_sided: def.two_sided,
            cull_mode: if def.two_sided { None } else { Some(bevy::render::render_resource::Face::Back) },
            alpha_mode: match def.alpha.as_str() {
                "mask" => AlphaMode::Mask(def.cutoff),
                "blend" => AlphaMode::Blend,
                _ => AlphaMode::Opaque,
            },
            ..default()
        };
        if let Some(orm) = load(&def.orm, false) {
            // ORM packing (R=AO, G=roughness, B=metal) matches Bevy's channels.
            mat.metallic_roughness_texture = Some(orm.clone());
            mat.occlusion_texture = Some(orm);
            mat.perceptual_roughness = 1.0;
            mat.metallic = 1.0;
        }
        if def.albedo.is_none() {
            // Engine grid / water placeholders: render as calm translucent water.
            mat.base_color = Color::linear_rgba(0.05, 0.12, 0.16, 0.75);
            mat.alpha_mode = AlphaMode::Blend;
            mat.perceptual_roughness = 0.1;
        }
        let handle = materials.add(mat);
        self.built.insert(key, handle.clone());
        Some(handle)
    }
}

fn apply_materials(
    mut library: ResMut<MaterialLibrary>,
    assets: Res<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut query: Query<(&GltfMaterialName, &mut MeshMaterial3d<StandardMaterial>), Added<GltfMaterialName>>,
) {
    for (name, mut material) in &mut query {
        if let Some(handle) = library.get(&name.0, &assets, &mut materials) {
            material.0 = handle;
        }
    }
}

/// UE stores masks/blend weights in vertex colors; Bevy would multiply them into the base color.
fn strip_vertex_colors(mut events: MessageReader<AssetEvent<Mesh>>, mut meshes: ResMut<Assets<Mesh>>) {
    for event in events.read() {
        if let AssetEvent::Added { id } = event {
            if let Some(mut mesh) = meshes.get_mut(*id) {
                if mesh.contains_attribute(Mesh::ATTRIBUTE_COLOR) {
                    mesh.remove_attribute(Mesh::ATTRIBUTE_COLOR);
                }
            }
        }
    }
}

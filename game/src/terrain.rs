//! Landscape layer blending: replaces the landscape's single-texture material with a shader
//! that mixes the original TQ2 ground textures by the landscape's own layer weight maps
//! (`terrain.json`, written by `Tq2Extract scene`).
use bevy::asset::{embedded_asset, RenderAssetUsages};
use bevy::gltf::GltfMaterialName;
use bevy::image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor};
use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, Extent3d, ShaderType, TextureDimension, TextureFormat};
use bevy::shader::ShaderRef;
use serde::Deserialize;

/// Ground textures are resampled to this size so they fit one texture array.
const LAYER_SIZE: u32 = 1024;

pub type TerrainMaterial = ExtendedMaterial<StandardMaterial, TerrainExtension>;

#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct TerrainExtension {
    #[uniform(100)]
    params: TerrainParams,
    #[texture(101, dimension = "2d_array")]
    #[sampler(102)]
    weights: Handle<Image>,
    #[texture(103, dimension = "2d_array")]
    #[sampler(104)]
    albedo: Handle<Image>,
    #[texture(105, dimension = "2d_array")]
    #[sampler(106)]
    normals: Handle<Image>,
}

#[derive(Clone, Copy, ShaderType)]
struct TerrainParams {
    layer_count: u32,
    tiling: f32,
    _pad0: f32,
    _pad1: f32,
    /// World XZ rectangle the weight maps cover (min x, min z, max x, max z); empty means the
    /// landscape's own UV_1 addresses them (older exports).
    bounds: Vec4,
}

impl MaterialExtension for TerrainExtension {
    fn fragment_shader() -> ShaderRef {
        "embedded://tq2_survivors/terrain.wesl".into()
    }
}

#[derive(Deserialize)]
struct TerrainFile {
    material: String,
    tiling: f32,
    #[serde(default)]
    bounds: Option<[f32; 4]>,
    layers: Vec<Layer>,
}

#[derive(Deserialize)]
struct Layer {
    name: String,
    weight: String,
    albedo: String,
    normal: Option<String>,
}

/// The blended material and the TQ2 material name of the landscape it replaces.
#[derive(Resource)]
struct Terrain {
    replaces: String,
    material: Handle<TerrainMaterial>,
}

pub struct TerrainPlugin;

impl Plugin for TerrainPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "terrain.wesl");
        app.add_plugins(MaterialPlugin::<TerrainMaterial>::default()).add_systems(Update, swap_landscape_material);
    }
}

/// Builds the blended material for the map in `dir` (or none when it has no terrain.json).
pub fn load(world: &mut World, dir: &str) {
    world.remove_resource::<Terrain>();
    match std::fs::read_to_string(format!("{dir}/terrain.json")).map(|s| serde_json::from_str::<TerrainFile>(&s)) {
        Ok(Ok(file)) => build_terrain(world, dir, file),
        Ok(Err(e)) => error!("{dir}/terrain.json: {e}"),
        Err(_) => info!("{dir}: no terrain.json, landscape keeps its single texture"),
    }
}

fn build_terrain(world: &mut World, dir: &str, file: TerrainFile) {
    let started = std::time::Instant::now();
    let n = file.layers.len();
    // Weights: 4 layers per RGBA slice, at the weight maps' own resolution.
    let weight_maps: Vec<_> = file.layers.iter().filter_map(|l| load_gray(&format!("{dir}/{}", l.weight))).collect();
    if weight_maps.len() != n || n == 0 {
        error!("terrain: could not read all {n} weight maps");
        return;
    }
    let (ww, wh) = weight_maps[0].dimensions();
    let slices = n.div_ceil(4);
    let mut weights = vec![0u8; (ww * wh * 4) as usize * slices];
    for (i, map) in weight_maps.iter().enumerate() {
        let slice = &mut weights[(ww * wh * 4) as usize * (i / 4)..];
        for (p, v) in map.pixels().enumerate() {
            slice[p * 4 + i % 4] = v.0[0];
        }
    }
    let albedo: Vec<_> = file.layers.iter().map(|l| load_rgba(&format!("{dir}/{}", l.albedo), [128, 128, 128, 255])).collect();
    let normal: Vec<_> = file
        .layers
        .iter()
        .map(|l| l.normal.as_ref().map_or_else(|| flat_normal(), |p| load_rgba(&format!("{dir}/{p}"), [128, 128, 255, 255])))
        .collect();

    let mut images = world.resource_mut::<Assets<Image>>();
    let weights = images.add(array_image(ww, wh, slices as u32, weights, 1, TextureFormat::Rgba8Unorm, false));
    let albedo = images.add(mipmapped_array(&albedo, TextureFormat::Rgba8UnormSrgb));
    let normals = images.add(mipmapped_array(&normal, TextureFormat::Rgba8Unorm));
    let material = world.resource_mut::<Assets<TerrainMaterial>>().add(TerrainMaterial {
        base: StandardMaterial { perceptual_roughness: 0.9, ..default() },
        extension: TerrainExtension {
            params: TerrainParams { layer_count: n as u32, tiling: file.tiling, _pad0: 0.0, _pad1: 0.0, bounds: file.bounds.map_or(Vec4::ZERO, Vec4::from_array) },
            weights,
            albedo,
            normals,
        },
    });
    info!(
        "terrain: {n} layers ({}) blended for {} in {:.1} s",
        file.layers.iter().map(|l| l.name.as_str()).collect::<Vec<_>>().join(", "),
        file.material,
        started.elapsed().as_secs_f32()
    );
    world.insert_resource(Terrain { replaces: file.material.to_lowercase(), material });
}

fn load_gray(path: &str) -> Option<image::GrayImage> {
    image::open(path).map_err(|e| warn!("{path}: {e}")).ok().map(|i| i.to_luma8())
}

fn load_rgba(path: &str, fallback: [u8; 4]) -> image::RgbaImage {
    match image::open(path) {
        Ok(i) => image::imageops::resize(&i.to_rgba8(), LAYER_SIZE, LAYER_SIZE, image::imageops::FilterType::Triangle),
        Err(e) => {
            warn!("{path}: {e}");
            image::RgbaImage::from_pixel(LAYER_SIZE, LAYER_SIZE, image::Rgba(fallback))
        }
    }
}

fn flat_normal() -> image::RgbaImage {
    image::RgbaImage::from_pixel(LAYER_SIZE, LAYER_SIZE, image::Rgba([128, 128, 255, 255]))
}

/// A texture array with a full mip chain (box-filtered on the CPU), layer-major data order.
fn mipmapped_array(layers: &[image::RgbaImage], format: TextureFormat) -> Image {
    let levels = LAYER_SIZE.ilog2() + 1;
    let mut data = vec![];
    for layer in layers {
        let mut level = layer.clone();
        for mip in 0..levels {
            data.extend_from_slice(level.as_raw());
            if mip + 1 < levels {
                let size = (LAYER_SIZE >> (mip + 1)).max(1);
                level = image::imageops::resize(&level, size, size, image::imageops::FilterType::Triangle);
            }
        }
    }
    array_image(LAYER_SIZE, LAYER_SIZE, layers.len() as u32, data, levels, format, true)
}

fn array_image(w: u32, h: u32, layers: u32, data: Vec<u8>, mips: u32, format: TextureFormat, repeat: bool) -> Image {
    let mut image = Image::new(
        Extent3d { width: w, height: h, depth_or_array_layers: layers },
        TextureDimension::D2,
        data,
        format,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.texture_descriptor.mip_level_count = mips;
    let address = if repeat { ImageAddressMode::Repeat } else { ImageAddressMode::ClampToEdge };
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: address,
        address_mode_v: address,
        anisotropy_clamp: if repeat { 8 } else { 1 },
        ..ImageSamplerDescriptor::linear()
    });
    image
}

/// Swaps the landscape's standard material for the blended terrain material.
fn swap_landscape_material(
    mut commands: Commands,
    terrain: Option<Res<Terrain>>,
    meshes: Query<(Entity, &GltfMaterialName), (With<MeshMaterial3d<StandardMaterial>>, Added<GltfMaterialName>)>,
) {
    let Some(terrain) = terrain else { return };
    for (e, name) in &meshes {
        if name.0.to_lowercase() == terrain.replaces {
            commands.entity(e).remove::<MeshMaterial3d<StandardMaterial>>().insert(MeshMaterial3d(terrain.material.clone()));
        }
    }
}

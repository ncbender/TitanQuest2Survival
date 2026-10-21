//! Arenas built from TQ2 pieces instead of cut from TQ2's maps: a themed kit of TQ2 meshes
//! (taken from exported maps, with their materials and collision) and TQ2 ground textures,
//! placed by a seeded generator. Layout: an open square field (gentle hills, scattered cover
//! with wide gaps, a free centre), ringed by a border of large pieces on rising ground.
use bevy::prelude::*;
use std::collections::HashMap;

use crate::ground::Ground;
use crate::obstacles::Obstacles;
use crate::tq2_materials::MaterialLibrary;
use crate::tq2_scene::{self, MapProp};

/// Half size of the playable square (m).
pub const PLAY: f32 = 58.0;
/// Half size of the generated ground (the border ring and hills lie between PLAY and this).
const EDGE: f32 = 88.0;

/// A TQ2 mesh of a kit: `dir/meshes/<mesh>.glb`, its scale range and spawn weight.
pub struct Piece {
    pub dir: &'static str,
    pub mesh: &'static str,
    pub scale: (f32, f32),
    pub weight: u32,
}

const fn p(dir: &'static str, mesh: &'static str, scale: (f32, f32), weight: u32) -> Piece {
    Piece { dir, mesh, scale, weight }
}

/// A ground layer: texture set name below `<dir>/textures/` (albedo `<set>_a.png`, normal
/// `<set>_n.png`).
pub struct Layer {
    pub dir: &'static str,
    pub set: &'static str,
}

const fn l(dir: &'static str, set: &'static str) -> Layer {
    Layer { dir, set }
}

pub struct Kit {
    pub id: &'static str,
    /// Large pieces forming the arena's edge.
    pub border: &'static [Piece],
    /// Obstacles scattered over the field (with collision).
    pub cover: &'static [Piece],
    /// Small decoration without collision (grass, flowers, pebbles).
    pub clutter: &'static [Piece],
    /// Ground layers: base, patches, edge.
    pub ground: [Layer; 3],
    pub cover_count: usize,
    pub clutter_count: usize,
}

pub const KITS: &[Kit] = &[
    Kit {
        id: "woods",
        border: &[
            p("menu2", "sm_tree_olive_large_01a", (0.9, 1.2), 3),
            p("menu2", "sm_tree_pine_03a", (1.0, 1.4), 3),
            p("menu2", "sm_tree_pine_05a", (1.0, 1.3), 2),
            p("menu2", "sm_bush_cypress_patch_01c", (0.9, 1.1), 2),
            p("menu2", "sm_rock_greek_chunk_06a", (1.0, 1.5), 2),
            p("menu2", "sm_rock_greek_shelf_01a", (0.8, 1.1), 1),
        ],
        cover: &[
            p("menu2", "sm_rock_greek_boulder_02a", (1.3, 2.2), 2),
            p("menu2", "sm_rock_greek_boulder_03a", (1.3, 2.2), 2),
            p("menu2", "sm_rock_greek_chunk_04a", (0.5, 0.7), 1),
            p("forest", "sm_bush_thorn_02a", (1.0, 1.3), 3),
            p("menu2", "sm_bush_olive_01a", (1.0, 1.3), 3),
            p("menu2", "sm_tree_pine_03a", (0.8, 1.0), 2),
            p("menu2", "sm_rock_greek_boulder_01a", (1.2, 2.0), 3),
        ],
        clutter: &[
            p("forest", "sm_grass_short_new_01a", (0.9, 1.4), 8),
            p("forest", "sm_grass_short_new_03a", (1.0, 1.5), 4),
            p("forest", "sm_boston_fern_03a", (0.8, 1.2), 3),
            p("forest", "sm_flower_cornflower_01a", (0.9, 1.2), 2),
            p("forest", "sm_flower_star_01b", (0.9, 1.2), 2),
            p("forest", "sm_zamia_palm_05a", (0.8, 1.1), 2),
            p("forest", "sm_spindle_16a", (0.9, 1.2), 2),
            p("forest", "sm_pebbles_03", (0.8, 1.2), 1),
        ],
        ground: [l("forest", "t_grass_mixed_01a"), l("forest", "t_mud_grass_01a"), l("forest", "t_rock_ground_cracked_01a")],
        cover_count: 34,
        clutter_count: 900,
    },
    Kit {
        id: "necropolis",
        border: &[
            p("graveyard", "sm_wall_stone_4m_01a", (1.0, 1.0), 4),
            p("graveyard", "sm_wall_13a", (1.0, 1.0), 2),
            p("graveyard", "sm_obelisk_01a", (0.9, 1.1), 1),
            p("graveyard", "sm_column_corner_02a", (1.0, 1.0), 1),
            p("menu2", "sm_tree_pine_03a", (1.0, 1.3), 2),
            p("menu2", "sm_rock_greek_chunk_06a", (1.0, 1.4), 2),
        ],
        cover: &[
            p("graveyard", "sm_sarcophagus_02a", (1.0, 1.2), 4),
            p("graveyard", "sm_pedestal_04a", (1.0, 1.1), 2),
            p("graveyard", "sm_column_02a", (0.8, 1.0), 2),
            p("graveyard", "sm_wall_stone_2m_broken_01a", (1.0, 1.0), 3),
            p("graveyard", "sm_wall_stone_4m_broken_01a", (1.0, 1.0), 2),
            p("graveyard", "sm_brazier_01a", (1.0, 1.2), 2),
            p("graveyard", "sm_obelisk_01a", (0.6, 0.8), 1),
        ],
        clutter: &[
            p("graveyard", "sm_grass_short_new_01a", (0.9, 1.4), 8),
            p("graveyard", "sm_grass_directional_02", (0.9, 1.3), 4),
            p("graveyard", "sm_skeleton_01a", (1.0, 1.0), 1),
            p("graveyard", "sm_candle_01a_assembly", (1.0, 1.4), 2),
            p("graveyard", "sm_centaurvessel_cluster_05_mat01", (0.9, 1.1), 1),
            p("menu2", "sm_wall_stone_pile_01a", (0.8, 1.2), 3),
        ],
        ground: [l("graveyard", "t_soil_gravel_01"), l("graveyard", "t_grass_green_01a"), l("graveyard", "t_rock_ground_cave_02a")],
        cover_count: 40,
        clutter_count: 700,
    },
    Kit {
        id: "village",
        border: &[
            p("village", "sm_wall_low_tegea_01b", (1.0, 1.0), 4),
            p("village", "sm_wall_low_pylon_tegea_01a", (1.0, 1.0), 2),
            p("village", "sm_wall_pylon_tegea_01a", (1.0, 1.0), 1),
            p("menu2", "sm_tree_olive_large_01a", (0.9, 1.1), 2),
            p("menu2", "sm_rock_greek_chunk_06a", (1.0, 1.4), 2),
        ],
        cover: &[
            p("village", "sm_column_02_small_01a", (0.9, 1.0), 3),
            p("menu2", "sm_rock_greek_boulder_02a", (1.3, 2.0), 2),
            p("village", "sm_centaurvessel_cluster_02_mat01", (1.0, 1.3), 2),
            p("village", "sm_shrine_demeter_brazier_01a", (1.0, 1.1), 1),
            p("village", "sm_statue_moirai_clotho_rod_01", (0.9, 1.0), 1),
            p("village", "sm_dirtyMesh_wall_low_pylon_tegea_01a2", (1.0, 1.0), 2),
            p("menu2", "sm_bush_olive_01a", (1.0, 1.3), 2),
        ],
        clutter: &[
            p("village", "sm_pot_piece_03a", (1.0, 1.5), 3),
            p("village", "sm_pebbles_02", (0.8, 1.3), 4),
            p("village", "sm_vase_clay_13a_painted_01", (0.9, 1.1), 2),
            p("village", "sm_pot_broken_02b", (0.9, 1.1), 2),
            p("menu2", "sm_grass_short_new_01a", (0.9, 1.3), 5),
            p("menu2", "sm_shrub_rosemary_large_01a", (0.8, 1.2), 2),
        ],
        ground: [l("village", "t_sand_beach_fine_01a"), l("village", "t_floor_stones_01a"), l("village", "t_soil_gravel_01")],
        cover_count: 36,
        clutter_count: 500,
    },
];

/// Seeded value noise in [0, 1] with cells of `size` metres.
fn noise(seed: u64, p: Vec2, size: f32) -> f32 {
    let g = p / size;
    // fract() is negative for negative inputs; the interpolation needs 0..1.
    let (i, f) = (g.floor(), g - g.floor());
    let h = |x: f32, y: f32| {
        let mut v = (x as i64 as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (y as i64 as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F) ^ seed;
        v ^= v >> 29;
        v = v.wrapping_mul(0xBF58_476D_1CE4_E5B9);
        v ^= v >> 32;
        (v & 0xFFFF) as f32 / 65535.0
    };
    let s = f * f * (Vec2::splat(3.0) - 2.0 * f);
    let a = h(i.x, i.y) + (h(i.x + 1.0, i.y) - h(i.x, i.y)) * s.x;
    let b = h(i.x, i.y + 1.0) + (h(i.x + 1.0, i.y + 1.0) - h(i.x, i.y + 1.0)) * s.x;
    a + (b - a) * s.y
}

/// Distance from the centre in the max norm (the arena is square).
fn square_dist(p: Vec2) -> f32 {
    p.x.abs().max(p.y.abs())
}

/// A rounded-square distance (4-norm): like `square_dist` without creases on the diagonals.
fn round_dist(p: Vec2) -> f32 {
    (p.x.powi(4) + p.y.powi(4)).powf(0.25)
}

fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn height(seed: u64, p: Vec2) -> f32 {
    let field = (noise(seed, p, 23.0) - 0.5) * 1.6 + (noise(seed ^ 1, p, 9.0) - 0.5) * 0.4;
    let rise = smoothstep(PLAY, EDGE - 4.0, round_dist(p));
    field + rise * (3.0 + noise(seed ^ 2, p, 21.0) * 3.5)
}

/// Bounds (min, max) of a glb's meshes in mesh space, from the POSITION accessors.
fn glb_bounds(path: &str) -> Option<(Vec3, Vec3)> {
    let bytes = std::fs::read(path).ok()?;
    let len = u32::from_le_bytes(bytes.get(12..16)?.try_into().ok()?) as usize;
    let json: serde_json::Value = serde_json::from_slice(bytes.get(20..20 + len)?).ok()?;
    let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
    for mesh in json["meshes"].as_array()? {
        for prim in mesh["primitives"].as_array()? {
            let acc = &json["accessors"][prim["attributes"]["POSITION"].as_u64()? as usize];
            let v = |k: &str| -> Option<Vec3> {
                let a = acc[k].as_array()?;
                Some(Vec3::new(a[0].as_f64()? as f32, a[1].as_f64()? as f32, a[2].as_f64()? as f32))
            };
            lo = lo.min(v("min")?);
            hi = hi.max(v("max")?);
        }
    }
    (lo.x <= hi.x).then_some((lo, hi))
}

/// A kit piece with its measured footprint.
struct Measured<'a> {
    piece: &'a Piece,
    path: String,
    lo: Vec3,
    hi: Vec3,
}

impl Measured<'_> {
    fn radius(&self) -> f32 {
        Vec2::new(self.hi.x - self.lo.x, self.hi.z - self.lo.z).length() * 0.5
    }
}

fn measure<'a>(root: &str, pieces: &'a [Piece]) -> Vec<Measured<'a>> {
    pieces
        .iter()
        .filter_map(|piece| {
            let rel = format!("{}/meshes/{}.glb", piece.dir, piece.mesh);
            match glb_bounds(&format!("{root}/{rel}")) {
                Some((lo, hi)) => Some(Measured { piece, path: rel, lo, hi }),
                None => {
                    warn!("mapgen: {rel} missing");
                    None
                }
            }
        })
        .collect()
}

fn pick<'a>(rng: &mut fastrand::Rng, pieces: &'a [Measured<'a>]) -> Option<&'a Measured<'a>> {
    let total: u32 = pieces.iter().map(|m| m.piece.weight).sum();
    let mut roll = rng.u32(0..total.max(1));
    for m in pieces {
        if roll < m.piece.weight {
            return Some(m);
        }
        roll -= m.piece.weight;
    }
    pieces.first()
}

/// What `map::load` needs from a generated arena.
pub struct Generated {
    pub ground: Ground,
    pub obstacles: Obstacles,
    pub start: Vec2,
}

/// Builds the arena of `kit` with `seed` into the world (props, ground mesh, terrain material).
pub fn build(world: &mut World, kit: &Kit, seed: u64) -> Generated {
    let root = crate::export_root();
    let started = std::time::Instant::now();
    let mut rng = fastrand::Rng::with_seed(seed);

    // Ground: a 1 m height grid.
    let n = (EDGE * 2.0) as usize + 1;
    let origin = Vec2::splat(-EDGE);
    let heights: Vec<f32> = (0..n * n).map(|i| height(seed, origin + Vec2::new((i % n) as f32, (i / n) as f32))).collect();
    let mut ground = Ground::from_heights(origin, 1.0, n, n, heights.clone());
    ground.limit = Some(Rect::from_center_half_size(Vec2::ZERO, Vec2::splat(PLAY)));

    // Materials and collision of every source folder.
    let mut dirs: Vec<&str> = kit.border.iter().chain(kit.cover).chain(kit.clutter).map(|p| p.dir).chain(kit.ground.iter().map(|l| l.dir)).collect();
    dirs.sort();
    dirs.dedup();
    let mut shapes = HashMap::new();
    for dir in &dirs {
        world.resource_mut::<MaterialLibrary>().load_dir(&root, dir);
        for (mesh, s) in tq2_scene::colliders(&root, dir) {
            shapes.insert(format!("{dir}/{mesh}"), s);
        }
    }

    let border = measure(&root, kit.border);
    let cover = measure(&root, kit.cover);
    let clutter = measure(&root, kit.clutter);
    let mut obstacles = Obstacles::default();
    let mut placed: Vec<(Vec2, f32)> = vec![];
    let mut props: Vec<(String, Transform, bool)> = vec![];

    let place = |m: &Measured, at: Vec2, yaw: f32, scale: f32, collide: bool, props: &mut Vec<(String, Transform, bool)>| {
        // Sit on the lowest ground under the footprint, slightly sunk in.
        let r = m.radius() * scale * 0.6;
        let low = [Vec2::ZERO, Vec2::X, Vec2::NEG_X, Vec2::Y, Vec2::NEG_Y].iter().map(|d| ground.terrain(at + *d * r)).fold(f32::MAX, f32::min);
        let y = low - m.lo.y * scale - 0.05 * (m.hi.y - m.lo.y) * scale;
        let t = Transform { translation: Vec3::new(at.x, y, at.y), rotation: Quat::from_rotation_y(yaw), scale: Vec3::splat(scale) };
        props.push((m.path.clone(), t, collide));
    };

    // Border: two staggered rings of large pieces just outside the playable square.
    for (ring, offset) in [(0, 3.0), (1, 12.0)] {
        let side = PLAY + offset;
        let mut s = -side;
        while s < side * 7.0 {
            // Walk the square's perimeter (4 sides of length 2*side).
            let (k, t) = ((s + side) / (2.0 * side), (s + side) % (2.0 * side) - side);
            let at = match k as i32 {
                0 => Vec2::new(t, -side),
                1 => Vec2::new(side, t),
                2 => Vec2::new(-t, side),
                _ => Vec2::new(-side, -t),
            } + Vec2::new(rng.f32() - 0.5, rng.f32() - 0.5) * 3.0;
            let Some(m) = pick(&mut rng, &border) else { break };
            let scale = m.piece.scale.0 + rng.f32() * (m.piece.scale.1 - m.piece.scale.0);
            // Walls face along the edge; everything else turns freely.
            let along = [0.0, std::f32::consts::FRAC_PI_2, 0.0, std::f32::consts::FRAC_PI_2][(k as usize).min(3)];
            let yaw = if m.path.contains("wall") { along } else { rng.f32() * std::f32::consts::TAU };
            place(m, at, yaw, scale, ring == 0, &mut props);
            s += (m.radius() * scale * 1.4).max(2.5);
        }
    }

    // Cover: clusters with wide gaps, none in the free centre.
    let mut tries = 0;
    while placed.len() < kit.cover_count && tries < 4000 {
        tries += 1;
        let at = Vec2::new(rng.f32() * 2.0 - 1.0, rng.f32() * 2.0 - 1.0) * (PLAY - 6.0);
        if at.length() < 16.0 {
            continue;
        }
        let Some(m) = pick(&mut rng, &cover) else { break };
        let scale = m.piece.scale.0 + rng.f32() * (m.piece.scale.1 - m.piece.scale.0);
        let r = m.radius() * scale;
        // At least 7 m of open ground between pieces keeps the field wide.
        if placed.iter().any(|(q, qr)| q.distance(at) < r + qr + 7.0) {
            continue;
        }
        placed.push((at, r));
        place(m, at, rng.f32() * std::f32::consts::TAU, scale, true, &mut props);
    }

    // Clutter everywhere except on cover.
    for _ in 0..kit.clutter_count {
        let at = Vec2::new(rng.f32() * 2.0 - 1.0, rng.f32() * 2.0 - 1.0) * (EDGE - 4.0);
        if placed.iter().any(|(q, qr)| q.distance(at) < *qr) {
            continue;
        }
        let Some(m) = pick(&mut rng, &clutter) else { break };
        let scale = m.piece.scale.0 + rng.f32() * (m.piece.scale.1 - m.piece.scale.0);
        place(m, at, rng.f32() * std::f32::consts::TAU, scale, false, &mut props);
    }

    let assets = world.resource::<AssetServer>().clone();
    for (path, t, collide) in &props {
        world.spawn((WorldAssetRoot(assets.load(GltfAssetLabel::Scene(0).from_asset(path.clone()))), MapProp, *t));
        if *collide {
            if let Some(s) = shapes.get(path.as_str()) {
                tq2_scene::add_shapes(&mut obstacles, s, t);
            }
        }
    }

    spawn_ground(world, kit, seed, &ground, &heights, n, origin, &root);
    info!("mapgen {} (seed {seed}): {} props, {} obstacle shapes in {:.1} s", kit.id, props.len(), obstacles.len(), started.elapsed().as_secs_f32());
    Generated { ground, obstacles, start: Vec2::ZERO }
}

/// The ground mesh with the blended TQ2 ground textures: base everywhere, patches by noise,
/// the edge layer on the rising border.
#[allow(clippy::too_many_arguments)]
fn spawn_ground(world: &mut World, kit: &Kit, seed: u64, ground: &Ground, heights: &[f32], n: usize, origin: Vec2, root: &str) {
    use bevy::asset::RenderAssetUsages;
    use bevy::mesh::{Indices, PrimitiveTopology};

    let at = |x: usize, z: usize| heights[z * n + x];
    let mut positions = Vec::with_capacity(n * n);
    let mut normals = Vec::with_capacity(n * n);
    let mut uvs = Vec::with_capacity(n * n);
    for z in 0..n {
        for x in 0..n {
            let p = origin + Vec2::new(x as f32, z as f32);
            positions.push([p.x, at(x, z), p.y]);
            let dx = at((x + 1).min(n - 1), z) - at(x.saturating_sub(1), z);
            let dz = at(x, (z + 1).min(n - 1)) - at(x, z.saturating_sub(1));
            normals.push(Vec3::new(-dx, 2.0, -dz).normalize().to_array());
            uvs.push([x as f32 / (n - 1) as f32, z as f32 / (n - 1) as f32]);
        }
    }
    let mut indices = Vec::with_capacity((n - 1) * (n - 1) * 6);
    for z in 0..n - 1 {
        for x in 0..n - 1 {
            let i = (z * n + x) as u32;
            let n = n as u32;
            indices.extend_from_slice(&[i, i + n, i + 1, i + 1, i + n, i + n + 1]);
        }
    }
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs.clone())
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_1, uvs)
        .with_inserted_indices(Indices::U32(indices));
    if let Err(e) = mesh.generate_tangents() {
        warn!("mapgen: tangents: {e}");
    }

    // Weight maps at 1 px per metre.
    let mut maps = vec![image::GrayImage::new(n as u32, n as u32); 3];
    for z in 0..n {
        for x in 0..n {
            let p = origin + Vec2::new(x as f32, z as f32);
            let edge = smoothstep(PLAY - 4.0, PLAY + 10.0, square_dist(p)).max(if ground.steep(p) { 1.0 } else { 0.0 });
            let patch = smoothstep(0.5, 0.68, noise(seed ^ 7, p, 11.0)) * (1.0 - edge);
            let base = (1.0 - patch - edge).max(0.0);
            for (i, w) in [base, patch, edge].into_iter().enumerate() {
                maps[i].put_pixel(x as u32, z as u32, image::Luma([(w * 255.0) as u8]));
            }
        }
    }
    let layers = maps
        .into_iter()
        .zip(&kit.ground)
        .map(|(w, l)| (w, format!("{root}/{}/textures/{}_a.png", l.dir, l.set), Some(format!("{root}/{}/textures/{}_n.png", l.dir, l.set))))
        .collect();
    let end = origin + Vec2::splat((n - 1) as f32);
    let material = crate::terrain::build_material(world, layers, 0.25, Vec4::new(origin.x, origin.y, end.x, end.y));
    let mesh = world.resource_mut::<Assets<Mesh>>().add(mesh);
    world.spawn((MapProp, Mesh3d(mesh), MeshMaterial3d(material), Transform::default()));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noise_is_continuous_across_cells() {
        for x in -60..60 {
            let p = Vec2::new(x as f32 * 0.5, -17.3);
            let step = (noise(5, p + Vec2::X * 0.05, 9.0) - noise(5, p, 9.0)).abs();
            assert!(step < 0.05, "jump at {p}: {step}");
        }
    }
}

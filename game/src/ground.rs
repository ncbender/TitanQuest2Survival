//! Terrain height lookup from `ground.json` (written by `Tq2Extract scene` from the
//! landscape vertices), so units stand on the original TQ2 terrain without physics.
use bevy::prelude::*;
use serde::Deserialize;

#[derive(Deserialize)]
struct GroundFile {
    origin: [f32; 2],
    cell: f32,
    w: usize,
    h: usize,
    heights: Vec<f32>,
}

#[derive(Resource)]
pub struct Ground {
    origin: Vec2,
    cell: f32,
    w: usize,
    h: usize,
    heights: Vec<f32>,
    /// Water surface height, if the map has water. Units wade at this height.
    pub water: Option<f32>,
}

impl Ground {
    pub fn load(path: &str, water: Option<f32>) -> Option<Self> {
        let text = std::fs::read_to_string(path).map_err(|e| warn!("{path}: {e}")).ok()?;
        let f: GroundFile = serde_json::from_str(&text).map_err(|e| error!("{path}: {e}")).ok()?;
        info!("{path}: {}x{} cells", f.w, f.h);
        Some(Self { origin: Vec2::from_array(f.origin), cell: f.cell, w: f.w, h: f.h, heights: f.heights, water })
    }

    /// A flat fallback when a map has no landscape.
    pub fn flat() -> Self {
        Self { origin: Vec2::splat(-500.0), cell: 1000.0, w: 2, h: 2, heights: vec![0.0; 4], water: None }
    }

    /// Bilinearly interpolated terrain height at world (x, z); clamped at the edges.
    pub fn terrain(&self, p: Vec2) -> f32 {
        let g = (p - self.origin) / self.cell;
        let x = g.x.clamp(0.0, (self.w - 1) as f32);
        let z = g.y.clamp(0.0, (self.h - 1) as f32);
        let (x0, z0) = (x.floor() as usize, z.floor() as usize);
        let (x1, z1) = ((x0 + 1).min(self.w - 1), (z0 + 1).min(self.h - 1));
        let (fx, fz) = (x - x0 as f32, z - z0 as f32);
        let at = |x: usize, z: usize| self.heights[z * self.w + x];
        let top = at(x0, z0) + (at(x1, z0) - at(x0, z0)) * fx;
        let bottom = at(x0, z1) + (at(x1, z1) - at(x0, z1)) * fx;
        top + (bottom - top) * fz
    }

    /// Height a unit stands at: terrain, or the water surface when submerged.
    pub fn stand(&self, p: Vec2) -> f32 {
        let t = self.terrain(p);
        self.water.map_or(t, |w| t.max(w))
    }

    pub fn is_land(&self, p: Vec2) -> bool {
        self.water.is_none_or(|w| self.terrain(p) > w + 0.15)
    }

    /// World-space (x, z) rectangle covered by the height grid.
    pub fn bounds(&self) -> Rect {
        Rect::from_corners(self.origin, self.origin + Vec2::new((self.w - 1) as f32, (self.h - 1) as f32) * self.cell)
    }
}

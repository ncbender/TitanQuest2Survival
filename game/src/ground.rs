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
    /// null marks a landscape hole.
    heights: Vec<Option<f32>>,
}

#[derive(Resource)]
pub struct Ground {
    origin: Vec2,
    cell: f32,
    w: usize,
    h: usize,
    heights: Vec<f32>,
    /// Landscape holes (no ground; units cannot enter).
    holes: Vec<bool>,
    /// Water surface height, if the map has water. Units wade at this height.
    pub water: Option<f32>,
    /// Playable area when smaller than the height grid (arenas cut from the open world).
    pub limit: Option<Rect>,
}

impl Ground {
    pub fn load(path: &str, water: Option<f32>) -> Option<Self> {
        let text = std::fs::read_to_string(path).map_err(|e| warn!("{path}: {e}")).ok()?;
        let f: GroundFile = serde_json::from_str(&text).map_err(|e| error!("{path}: {e}")).ok()?;
        info!("{path}: {}x{} cells", f.w, f.h);
        let holes: Vec<bool> = f.heights.iter().map(Option::is_none).collect();
        // Holes get a pit below the lowest ground so nothing stands in them.
        let pit = f.heights.iter().flatten().copied().fold(f32::MAX, f32::min) - 3.0;
        let heights = f.heights.iter().map(|h| h.unwrap_or(pit)).collect();
        info!("{path}: {} hole cells", holes.iter().filter(|h| **h).count());
        Some(Self { origin: Vec2::from_array(f.origin), cell: f.cell, w: f.w, h: f.h, heights, holes, water, limit: None })
    }

    /// A flat fallback when a map has no landscape.
    pub fn flat() -> Self {
        Self { origin: Vec2::splat(-500.0), cell: 1000.0, w: 2, h: 2, heights: vec![0.0; 4], holes: vec![false; 4], water: None, limit: None }
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

    /// Highest standing height over a small disc (centre + 4 points at `r`).
    pub fn stand_area(&self, p: Vec2, r: f32) -> f32 {
        [Vec2::ZERO, Vec2::X, Vec2::NEG_X, Vec2::Y, Vec2::NEG_Y]
            .iter()
            .map(|d| self.stand(p + *d * r))
            .fold(f32::MIN, f32::max)
    }

    /// Whether `p` lies in a landscape hole.
    pub fn hole(&self, p: Vec2) -> bool {
        let g = ((p - self.origin) / self.cell).round();
        let (x, z) = (g.x.clamp(0.0, (self.w - 1) as f32) as usize, g.y.clamp(0.0, (self.h - 1) as f32) as usize);
        self.holes[z * self.w + x]
    }

    /// Lowest ground height (the floor drawn under landscape holes sits below it).
    pub fn lowest(&self) -> f32 {
        self.heights.iter().copied().fold(f32::MAX, f32::min)
    }

    /// Shallow water is wadeable; deeper water and landscape holes block movement.
    pub fn walkable(&self, p: Vec2) -> bool {
        !self.hole(p) && self.water.is_none_or(|w| self.terrain(p) > w - 0.5)
    }

    /// Moves from `from` towards `to`, sliding along the shore instead of entering deep water.
    pub fn clamp_move(&self, from: Vec2, to: Vec2) -> Vec2 {
        [to, Vec2::new(to.x, from.y), Vec2::new(from.x, to.y)]
            .into_iter()
            .find(|p| self.walkable(*p))
            .unwrap_or(from)
    }

    pub fn is_land(&self, p: Vec2) -> bool {
        !self.hole(p) && self.water.is_none_or(|w| self.terrain(p) > w + 0.15)
    }

    /// World-space (x, z) rectangle units may use: the height grid, cut to `limit`.
    pub fn bounds(&self) -> Rect {
        let grid = Rect::from_corners(self.origin, self.origin + Vec2::new((self.w - 1) as f32, (self.h - 1) as f32) * self.cell);
        match self.limit {
            Some(l) if !grid.intersect(l).is_empty() => grid.intersect(l),
            _ => grid,
        }
    }
}

//! Flow field towards the hero, so the horde walks around rocks, walls and deep water
//! instead of pressing against them. Blocked cells are computed once per map; distances
//! are refreshed by a breadth-first search from the hero a few times per second.
use bevy::prelude::*;
use std::collections::VecDeque;

use super::player::Hero;
use super::{GameState, Pos};
use crate::ground::Ground;
use crate::obstacles::Obstacles;

const CELL: f32 = 1.0;
/// Search radius in cells; enemies further away walk straight at the hero.
const MAX_STEPS: u16 = 70;
const REFRESH: f32 = 0.25;
const UNREACHED: u16 = u16::MAX;

#[derive(Resource)]
pub struct FlowField {
    origin: Vec2,
    w: usize,
    h: usize,
    blocked: Vec<bool>,
    dist: Vec<u16>,
    timer: f32,
}

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, build).add_systems(FixedUpdate, refresh.run_if(in_state(GameState::Playing)));
}

/// Marks every cell a body cannot stand in: deep water or inside an obstacle.
fn build(mut commands: Commands, ground: Res<Ground>, obstacles: Res<Obstacles>) {
    let b = ground.bounds();
    let (w, h) = (((b.max.x - b.min.x) / CELL) as usize + 1, ((b.max.y - b.min.y) / CELL) as usize + 1);
    let mut blocked = vec![false; w * h];
    for z in 0..h {
        for x in 0..w {
            let p = b.min + Vec2::new(x as f32, z as f32) * CELL;
            blocked[z * w + x] = !ground.walkable(p) || obstacles.resolve(p, 0.45, ground.stand(p)) != p;
        }
    }
    info!("flow field {w}x{h}, {} blocked cells", blocked.iter().filter(|b| **b).count());
    commands.insert_resource(FlowField { origin: b.min, w, h, blocked, dist: vec![UNREACHED; w * h], timer: 0.0 });
}

fn refresh(time: Res<Time>, mut field: ResMut<FlowField>, hero: Query<&Pos, With<Hero>>) {
    field.timer -= time.delta_secs();
    if field.timer > 0.0 {
        return;
    }
    field.timer = REFRESH;
    if let Ok(hero) = hero.single() {
        field.spread_from(hero.0);
    }
}

impl FlowField {
    /// Breadth-first distances (in cells, up to MAX_STEPS) from `target` over free cells.
    fn spread_from(&mut self, target: Vec2) {
        let Some(start) = self.cell(target) else { return };
        let (w, h) = (self.w, self.h);
        self.dist.fill(UNREACHED);
        self.dist[start] = 0;
        let mut queue = VecDeque::from([start]);
        while let Some(i) = queue.pop_front() {
            let d = self.dist[i];
            if d >= MAX_STEPS {
                continue;
            }
            let (x, z) = (i % w, i / w);
            for (nx, nz) in [(x.wrapping_sub(1), z), (x + 1, z), (x, z.wrapping_sub(1)), (x, z + 1)] {
                if nx >= w || nz >= h {
                    continue;
                }
                let n = nz * w + nx;
                if self.dist[n] == UNREACHED && !self.blocked[n] {
                    self.dist[n] = d + 1;
                    queue.push_back(n);
                }
            }
        }
    }

    fn cell(&self, p: Vec2) -> Option<usize> {
        let g = ((p - self.origin) / CELL).round();
        (g.x >= 0.0 && g.y >= 0.0 && (g.x as usize) < self.w && (g.y as usize) < self.h).then(|| g.y as usize * self.w + g.x as usize)
    }

    /// Direction to walk from `p` towards the hero along the field, if `p` is inside it.
    pub fn direction(&self, p: Vec2) -> Option<Vec2> {
        let i = self.cell(p)?;
        let (x, z) = ((i % self.w) as i32, (i / self.w) as i32);
        let mut best = (self.dist[i], None);
        for dz in -1..=1 {
            for dx in -1..=1 {
                let (nx, nz) = (x + dx, z + dz);
                if (dx, dz) == (0, 0) || nx < 0 || nz < 0 || nx as usize >= self.w || nz as usize >= self.h {
                    continue;
                }
                let d = self.dist[nz as usize * self.w + nx as usize];
                if d < best.0 {
                    best = (d, Some(self.origin + Vec2::new(nx as f32, nz as f32) * CELL));
                }
            }
        }
        best.1.map(|target| (target - p).normalize_or_zero())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 20x20 field with a wall at z = 10 from x = 0..15; the only way through is at x >= 15.
    fn walled() -> FlowField {
        let (w, h) = (20, 20);
        let mut blocked = vec![false; w * h];
        for x in 0..15 {
            blocked[10 * w + x] = true;
        }
        FlowField { origin: Vec2::ZERO, w, h, blocked, dist: vec![UNREACHED; w * h], timer: 0.0 }
    }

    #[test]
    fn walks_around_a_wall() {
        let mut field = walled();
        field.spread_from(Vec2::new(5.0, 15.0)); // hero behind the wall
        // An enemy right in front of the wall must head for the gap (+x), not into the wall (+z).
        let dir = field.direction(Vec2::new(5.0, 8.0)).unwrap();
        assert!(dir.x > 0.5, "should move towards the gap, got {dir}");
        // Following the field always ends at the hero.
        let mut p = Vec2::new(2.0, 2.0);
        for _ in 0..200 {
            match field.direction(p) {
                Some(d) => p += d * 0.5,
                None => break,
            }
        }
        assert!(p.distance(Vec2::new(5.0, 15.0)) < 1.5, "ended at {p}");
    }
}

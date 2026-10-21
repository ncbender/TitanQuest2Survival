//! Static obstacles from the original UE simple collision (`colliders.json`), flattened to
//! convex footprints on the ground plane. Units are pushed out of them; no physics engine.
use bevy::prelude::*;
use std::collections::HashMap;

/// One collision shape of one placed mesh: the convex hull of its points on the ground
/// plane, inflated by `radius` (spheres/capsules), spanning `y_min..y_max` in height.
pub struct Obstacle {
    hull: Vec<Vec2>,
    radius: f32,
    y_min: f32,
    y_max: f32,
}

const CELL: f32 = 4.0;

#[derive(Resource, Default)]
pub struct Obstacles {
    list: Vec<Obstacle>,
    grid: HashMap<(i32, i32), Vec<u32>>,
}

impl Obstacles {
    /// Adds a shape given by world-space points.
    pub fn add(&mut self, points: &[Vec3], radius: f32) {
        if points.is_empty() {
            return;
        }
        let y_min = points.iter().map(|p| p.y).fold(f32::MAX, f32::min) - radius;
        let y_max = points.iter().map(|p| p.y).fold(f32::MIN, f32::max) + radius;
        let hull = convex_hull(points.iter().map(|p| p.xz()).collect());
        let (lo, hi) = hull.iter().fold((Vec2::MAX, Vec2::MIN), |(lo, hi), p| (lo.min(*p), hi.max(*p)));
        let (lo, hi) = (lo - radius, hi + radius);
        let index = self.list.len() as u32;
        for x in (lo.x / CELL).floor() as i32..=(hi.x / CELL).floor() as i32 {
            for z in (lo.y / CELL).floor() as i32..=(hi.y / CELL).floor() as i32 {
                self.grid.entry((x, z)).or_default().push(index);
            }
        }
        self.list.push(Obstacle { hull, radius, y_min, y_max });
    }

    pub fn len(&self) -> usize {
        self.list.len()
    }

    /// Shapes that block a body standing at height `ground` (between knee and head).
    fn blocking(&self, p: Vec2, ground: f32) -> impl Iterator<Item = &Obstacle> {
        let key = ((p.x / CELL).floor() as i32, (p.y / CELL).floor() as i32);
        self.grid
            .get(&key)
            .into_iter()
            .flatten()
            .map(|&i| &self.list[i as usize])
            .filter(move |o| o.y_min < ground + 1.6 && o.y_max > ground + 0.4)
    }

    /// Moves a circle of radius `r` at `p` out of every blocking obstacle.
    pub fn resolve(&self, mut p: Vec2, r: f32, ground: f32) -> Vec2 {
        for _ in 0..3 {
            let mut moved = false;
            for o in self.blocking(p, ground) {
                let (dist, normal) = o.signed_distance(p);
                if dist < r {
                    p += normal * (r - dist);
                    moved = true;
                }
            }
            if !moved {
                break;
            }
        }
        p
    }

    /// True if a point at height `y` is inside an obstacle (projectiles stop there).
    pub fn hits(&self, p: Vec2, y: f32) -> bool {
        self.blocking(p, y - 1.0).any(|o| o.signed_distance(p).0 < 0.0)
    }
}

impl Obstacle {
    /// Signed distance from `p` to the inflated hull and the outward direction.
    fn signed_distance(&self, p: Vec2) -> (f32, Vec2) {
        let h = &self.hull;
        if h.len() == 1 {
            let d = p - h[0];
            return (d.length() - self.radius, d.normalize_or(Vec2::X));
        }
        let mut best = (f32::MAX, Vec2::X);
        let mut inside = h.len() >= 3;
        for i in 0..h.len() {
            let (a, b) = (h[i], h[(i + 1) % h.len()]);
            let ab = b - a;
            let t = ((p - a).dot(ab) / ab.length_squared().max(1e-6)).clamp(0.0, 1.0);
            let closest = a + ab * t;
            let d = p.distance(closest);
            // Hull is counter-clockwise: outward normal is to the right of each edge.
            let outward = Vec2::new(ab.y, -ab.x).normalize_or(Vec2::X);
            if (p - a).dot(outward) > 0.0 {
                inside = false;
            }
            if d < best.0 {
                best = (d, if d > 1e-4 { (p - closest) / d } else { outward });
            }
        }
        if inside {
            // Push out through the nearest edge.
            (-best.0 - self.radius, -best.1)
        } else {
            (best.0 - self.radius, best.1)
        }
    }
}

/// Andrew's monotone chain; returns the hull counter-clockwise (in x/z with z "up").
fn convex_hull(mut pts: Vec<Vec2>) -> Vec<Vec2> {
    pts.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
    pts.dedup_by(|a, b| a.distance_squared(*b) < 1e-6);
    if pts.len() < 3 {
        return pts;
    }
    let cross = |o: Vec2, a: Vec2, b: Vec2| (a - o).perp_dot(b - o);
    let mut hull: Vec<Vec2> = Vec::with_capacity(pts.len() * 2);
    for pass in [false, true] {
        let start = hull.len();
        let iter: Box<dyn Iterator<Item = &Vec2>> = if pass { Box::new(pts.iter().rev()) } else { Box::new(pts.iter()) };
        for &p in iter {
            while hull.len() >= start + 2 && cross(hull[hull.len() - 2], hull[hull.len() - 1], p) <= 0.0 {
                hull.pop();
            }
            hull.push(p);
        }
        hull.pop();
    }
    hull
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pushes_out_of_box() {
        let mut o = Obstacles::default();
        let corners: Vec<Vec3> = [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)]
            .iter()
            .flat_map(|&(x, z)| [Vec3::new(x, 0.0, z), Vec3::new(x, 2.0, z)])
            .collect();
        o.add(&corners, 0.0);
        let p = o.resolve(Vec2::new(0.8, 0.0), 0.5, 0.0);
        assert!((p.x - 1.5).abs() < 1e-3, "{p}");
        let outside = o.resolve(Vec2::new(3.0, 0.0), 0.5, 0.0);
        assert_eq!(outside, Vec2::new(3.0, 0.0));
        // A shape below knee height does not block.
        let mut flat = Obstacles::default();
        flat.add(&[Vec3::new(-1.0, 0.0, -1.0), Vec3::new(1.0, 0.2, 1.0), Vec3::new(1.0, 0.0, -1.0)], 0.0);
        assert_eq!(flat.resolve(Vec2::ZERO, 0.5, 0.0), Vec2::ZERO);
    }

    #[test]
    fn circle_obstacle() {
        let mut o = Obstacles::default();
        o.add(&[Vec3::new(0.0, 1.0, 0.0)], 1.0);
        let p = o.resolve(Vec2::new(1.2, 0.0), 0.5, 0.0);
        assert!((p.x - 1.5).abs() < 1e-3, "{p}");
    }
}

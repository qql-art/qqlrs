use std::ops::RangeInclusive;

use crate::{
    config::Config,
    math::{dist, dist_lower_bound, dist_upper_bound},
};

const NUM_SECTORS: usize = 50;

struct Indexer {
    start: f64,
    length: f64,
}

impl Indexer {
    pub fn new(start: f64, stop: f64) -> Self {
        let length = (stop - start) / NUM_SECTORS as f64;
        Indexer { start, length }
    }

    pub fn index(&self, value: f64) -> usize {
        let index = f64::floor((value - self.start) / self.length) as usize;
        index.clamp(0, NUM_SECTORS - 1)
    }
}

pub struct Sectors {
    fast_collisions: bool,
    ix: Indexer,
    iy: Indexer,
    sectors: Box<[[Vec<Collider>; NUM_SECTORS]; NUM_SECTORS]>,
}

#[derive(Debug, Copy, Clone)]
pub struct Collider {
    pub position: (f64, f64),
    pub radius: f64,
}

impl Sectors {
    pub fn new(config: &Config, left: f64, right: f64, top: f64, bottom: f64) -> Self {
        let ix = Indexer::new(f64::min(left, right), f64::max(left, right));
        let iy = Indexer::new(f64::min(top, bottom), f64::max(top, bottom));
        let sectors = {
            const EMPTY_SECTOR: Vec<Collider> = Vec::new();
            let vec_of_arrays = vec![[EMPTY_SECTOR; NUM_SECTORS]; NUM_SECTORS];
            let slice_of_arrays = vec_of_arrays.into_boxed_slice();
            slice_of_arrays.try_into().unwrap()
        };
        Sectors {
            fast_collisions: config.fast_collisions,
            ix,
            iy,
            sectors,
        }
    }

    /// Tests whether the given collider can be included in this sector grid without any
    /// collisions, and adds it if it can. Returns whether the collider was added.
    pub fn test_and_add(&mut self, collider: Collider) -> bool {
        let Some(affected) = self.affected(&collider) else {
            // Colliders affecting no sectors never collide and don't need to be added to anything.
            // We can't actually iterate over this because colliders with very negative radii can
            // have `y_max > y_min` and thus panic in the slice dereference.
            return true;
        };
        let fast_collisions = self.fast_collisions;
        for sector in self.sectors(&affected) {
            for other in sector {
                if collides(fast_collisions, &collider, other) {
                    return false;
                }
            }
        }
        // OK: no collisions.
        for sector in self.sectors(&affected) {
            sector.push(collider);
        }
        true
    }

    /// Precondition: `collider.radius` must be non-negative.
    fn affected(&self, collider: &Collider) -> Option<Affected> {
        let x_min = self.ix.index(collider.position.0 - collider.radius);
        let x_max = self.ix.index(collider.position.0 + collider.radius);
        if x_min > x_max {
            return None;
        }

        let y_min = self.iy.index(collider.position.1 - collider.radius);
        let y_max = self.iy.index(collider.position.1 + collider.radius);
        if y_min > y_max {
            return None;
        }

        Some(Affected {
            xs: x_min..=x_max,
            ys: y_min..=y_max,
        })
    }

    fn sectors(&mut self, affected: &Affected) -> impl Iterator<Item = &mut Vec<Collider>> {
        let (x_min, x_max) = (*affected.xs.start(), *affected.xs.end());
        let (y_min, y_max) = (*affected.ys.start(), *affected.ys.end());
        self.sectors[x_min..=x_max]
            .iter_mut()
            .flat_map(move |row| row[y_min..=y_max].iter_mut())
    }
}

/// Invariant: only constructed with `xs` and `ys` as well-formed ranges (i.e., that won't panic on
/// slicing).
#[derive(Debug)]
struct Affected {
    xs: RangeInclusive<usize>,
    ys: RangeInclusive<usize>,
}

fn collides(fast_collisions: bool, c1: &Collider, c2: &Collider) -> bool {
    if fast_collisions {
        collides_fast(c1, c2)
    } else {
        collides_dist(c1, c2)
    }
}

fn collides_fast(c1: &Collider, c2: &Collider) -> bool {
    let (x1, y1) = c1.position;
    let (x2, y2) = c2.position;
    let radius = c1.radius + c2.radius;
    if radius < 0.0 {
        return false;
    }
    let dx = x1 - x2;
    let dy = y1 - y2;
    dx * dx + dy * dy <= radius * radius
}

fn collides_dist(c1: &Collider, c2: &Collider) -> bool {
    let p1 = c1.position;
    let p2 = c2.position;
    let radius = c1.radius + c2.radius;
    if dist_lower_bound(p1, p2) > radius {
        return false;
    }
    if dist_upper_bound(p1, p2) <= radius {
        return true;
    }
    dist(p1, p2) <= radius
}

use crate::math::{dist, dist_lower_bound, dist_upper_bound};

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
    pub fn new(left: f64, right: f64, top: f64, bottom: f64) -> Self {
        let ix = Indexer::new(f64::min(left, right), f64::max(left, right));
        let iy = Indexer::new(f64::min(top, bottom), f64::max(top, bottom));
        let sectors = Box::new(std::array::from_fn(|_| std::array::from_fn(|_| Vec::new())));
        Sectors { ix, iy, sectors }
    }

    /// Tests whether the given collider can be included in this sector grid without any
    /// collisions, and adds it if it can. Returns whether the collider was added.
    pub fn test_and_add(&mut self, collider: Collider) -> bool {
        for sector in self.affected(&collider) {
            for other in sector {
                if collides(&collider, other) {
                    return false;
                }
            }
        }
        // OK: no collisions.
        for sector in self.affected(&collider) {
            sector.push(collider);
        }
        true
    }

    fn affected(&mut self, collider: &Collider) -> impl Iterator<Item = &mut Vec<Collider>> {
        let x_min = self.ix.index(collider.position.0 - collider.radius);
        let x_max = self.ix.index(collider.position.0 + collider.radius);

        let y_min = self.iy.index(collider.position.1 - collider.radius);
        let y_max = self.iy.index(collider.position.1 + collider.radius);

        self.sectors[x_min..=x_max]
            .iter_mut()
            .flat_map(move |row| row[y_min..=y_max].iter_mut())
    }
}

fn collides(c1: &Collider, c2: &Collider) -> bool {
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

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

pub struct Sectors<T> {
    ix: Indexer,
    iy: Indexer,
    sectors: Box<[[Vec<T>; NUM_SECTORS]; NUM_SECTORS]>,
}

impl<T> Sectors<T> {
    pub fn new(left: f64, right: f64, top: f64, bottom: f64) -> Self {
        let ix = Indexer::new(f64::min(left, right), f64::max(left, right));
        let iy = Indexer::new(f64::min(top, bottom), f64::max(top, bottom));
        let sectors = Box::new(std::array::from_fn(|_| std::array::from_fn(|_| Vec::new())));
        Sectors { ix, iy, sectors }
    }

    pub fn affected(
        &mut self,
        (x, y): (f64, f64),
        margin: f64,
    ) -> impl Iterator<Item = &mut Vec<T>> {
        let x_min = self.ix.index(x - margin);
        let x_max = self.ix.index(x + margin);

        let y_min = self.iy.index(y - margin);
        let y_max = self.iy.index(y + margin);

        self.sectors[x_min..=x_max]
            .iter_mut()
            .flat_map(move |row| row[y_min..=y_max].iter_mut())
    }
}

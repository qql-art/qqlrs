use std::num::Wrapping;

// Linear congruential generator parameters
const MUL: u64 = 6364136223846793005; // Knuth section 3.3.4 (p.108)
const INC: u64 = 1442695040888963407;

#[derive(Clone, PartialEq)]
pub struct Rng {
    state: u64,
    next_gaussian: Option<f64>,
}

impl Rng {
    pub fn from_seed(seed: &[u8]) -> Rng {
        // NOTE(wchargin): There are endianness dragons here. The original JavaScript code uses
        // `DataView.setUint32` to portably write a big-endian integer into an `ArrayBuffer`.
        // However, this buffer is the backing storage of a `Uint16Array`, which later reads from
        // the data in platform-dependent order.
        //
        // This Rust port chooses to use the little-endian behavior everywhere for portability. The
        // original behavior can be recovered by changing `swap_bytes` to `to_be`.
        let lower = murmur2(seed, 1690382925).swap_bytes();
        let upper = murmur2(seed, 72970470).swap_bytes();
        let state = u64::from(lower) | (u64::from(upper) << 32);
        Rng {
            state,
            next_gaussian: None,
        }
    }

    /// Picks a random value uniformly distributed between `0.0` (inclusive) and `1.0` (exclusive).
    pub fn rnd(&mut self) -> f64 {
        let old_state = self.state;
        // Advance internal state.
        self.state = old_state.wrapping_mul(MUL).wrapping_add(INC);
        // Calculate output function (XSH RR) using the old state.
        // This is a standard PCG-XSH-RR generator (O'Neill 2014, section 6.3.1) but drops 3 bits
        // during the xorshift to be compatible with an anomaly in the JavaScript implementation.
        let xorshifted = ((((old_state >> 18) & !(3 << 30)) ^ old_state) >> 27) as u32;
        let fac = xorshifted.rotate_right((old_state >> 59) as u32);
        2.0f64.powi(-32) * f64::from(fac)
    }

    /// Picks a random value uniformly distributed between `min` (inclusive) and `max` (exclusive).
    pub fn uniform(&mut self, min: f64, max: f64) -> f64 {
        self.rnd() * (max - min) + min
    }

    /// Picks a random value according to a Gaussian (normal) distribution with the given mean and
    /// standard deviation.
    ///
    /// # Implementation-defined approximation behavior
    ///
    /// The canonical QQL JavaScript algorithm uses `Math.sqrt` and `Math.log` here, which both
    /// have implementation-defined approximation behavior per ECMA-262. The values returned by
    /// [`f64::ln`] may differ slightly, and testing on my machine shows that the results do differ
    /// for about 7% of values selected uniformly at random by `Math.random()`.
    ///
    /// Therefore, the results of this `gauss` method may also differ slightly across the two
    /// implementations. For example:
    ///
    /// ```rust
    /// use qql::rand::Rng;
    /// let mut rng = Rng::from_seed(b"\x08");
    /// assert_eq!(rng.gauss(0.0, 1.0), 1.0637608855800318);
    /// ```
    ///
    /// ```javascript
    /// > import("./src/art/safe-random.js").then((r) => console.log(r.makeSeededRng("0x08").gauss()))
    /// 1.063760885580032
    /// ```
    ///
    /// Here, the two normal deviates differ by one unit in the last place.
    pub fn gauss(&mut self, mean: f64, stdev: f64) -> f64 {
        if let Some(z) = self.next_gaussian.take() {
            return mean + stdev * z;
        }
        let (v1, v2, s) = loop {
            let v1 = self.rnd() * 2.0 - 1.0;
            let v2 = self.rnd() * 2.0 - 1.0;
            let s = v1 * v1 + v2 * v2;
            if s < 1.0 && s != 0.0 {
                break (v1, v2, s);
            }
        };
        let multiplier = (-2.0 * f64::ln(s) / s).sqrt();
        let (z1, z2) = (v1 * multiplier, v2 * multiplier);
        self.next_gaussian = Some(z2);
        mean + stdev * z1
    }

    /// Picks `true` with probability roughly `p`, or `false` otherwise.
    ///
    /// # Approximate correctness
    ///
    /// The probability that `true` is returned is actually roughly `p + 2^-32`. In particular,
    /// `true` may be returned even if `p` is exactly zero.
    ///
    /// ```
    /// use qql::rand::Rng;
    /// let mut rng = Rng::from_seed(b"\x2e\x7e\x19\x00");
    /// let odds: [bool; 76] = std::array::from_fn(|_| rng.odds(0.0));
    /// assert_eq!(&odds[71..], &[false, false, false, false, true]);
    /// ```
    pub fn odds(&mut self, p: f64) -> bool {
        self.rnd() <= p
    }

    /// Chooses an item from `items` at a uniformly random index.
    ///
    /// # Panics
    ///
    /// Panics if `items.is_empty()`.
    pub fn choice<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        items
            .get(self.uniform(0.0, items.len() as f64) as usize)
            .expect("no items")
    }

    /// Given a slice of `(item, weight)` pairs, chooses an `item` with probability proportional to
    /// its `weight`. (The name `wc` is short for *weighted choice*.)
    ///
    /// Weights can be `u32`, `f64`, or any other type that can be [converted into
    /// `f64`][std::convert::Into].
    ///
    /// # Panics
    ///
    /// Panics if `weighted_items.is_empty()`.
    pub fn wc<'a, T, Weight: Into<f64> + Copy>(
        &mut self,
        weighted_items: &'a [(T, Weight)],
    ) -> &'a T {
        let sum_weight: f64 = weighted_items.iter().map(|(_, w)| (*w).into()).sum();
        let bisection = sum_weight * self.rnd();

        let mut cum_weight: f64 = 0.0;
        for (value, weight) in weighted_items {
            cum_weight += (*weight).into();
            if cum_weight >= bisection {
                return value;
            }
        }
        &weighted_items.last().expect("no items").0
    }

    /// Constructs a new vector with a uniformly random permutation of the elements in `xs`.
    ///
    /// # Caveats
    ///
    /// If the next `n` [uniform deviates][Rng::rnd] from the current state would contain any
    /// duplicates (where `n` is the number of elements in `xs`), the sort order is
    /// implementation-defined. The chance of this happening (if the internal state is chosen
    /// uniformly at random) is `n * (n - 1) / (2 * 2^32)`.
    pub fn shuffle<T, I: IntoIterator<Item = T>>(&mut self, xs: I) -> Vec<T> {
        let mut result: Vec<(f64, T)> = xs.into_iter().map(|x| (self.rnd(), x)).collect();
        // Using an unstable sort here under the assumption that no keys collide. If any keys do
        // collide, then the callback defined in the JavaScript implementation is not a *consistent
        // comparator* (per ECMAScript spec section 23.1.3.30.1) and so the sort order is
        // implementation-defined, anyway.
        result.sort_unstable_by(|(k1, _), (k2, _)| k1.total_cmp(k2));
        result.into_iter().map(|(_, x)| x).collect()
    }

    /// Mutates `xs` in place to "winnow it down" to contain at most `num` elements, preserving the
    /// original order.
    ///
    /// # Performance
    ///
    /// The current implementation takes time linear in the product of `xs.len()` and the number of
    /// elements removed.
    pub fn winnow<T>(&mut self, xs: &mut Vec<T>, num: usize) {
        // Inefficient quadratic implementation, but this function is only called twice per QQL on
        // a fairly small sequence, so we just go with it to be entropy-consistent with the
        // JavaScript implementation.
        while xs.len() > num {
            let index = (self.rnd() * (xs.len() as f64)) as usize;
            xs.remove(index);
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use hex_literal::hex;

    #[test]
    fn test_seed_state() {
        assert_eq!(Rng::from_seed(b"").state, 0x381a85e943aeeb00);
        assert_eq!(
            Rng::from_seed(&hex!(
                "efa7bdd92b5e9cd9de9b54ac0e3dc60623f1c989a80ed9c5157fffff10c2a148"
            ))
            .state,
            0x506997572177a894
        );
    }

    #[test]
    fn test_rnd_sequence() {
        let mut rng = Rng::from_seed(b"");
        let us: [f64; 8] = std::array::from_fn(|_| rng.rnd());
        assert_eq!(
            us,
            [
                0.8438512671273202,
                0.43491613143123686,
                0.26782758394256234,
                0.9794597257860005,
                0.8957886048592627,
                0.5943453973159194,
                0.07430003909394145,
                0.37728449678979814
            ]
        );

        let mut rng = Rng::from_seed(&hex!(
            "efa7bdd92b5e9cd9de9b54ac0e3dc60623f1c989a80ed9c5157fffff10c2a148"
        ));
        let us: [f64; 8] = std::array::from_fn(|_| rng.rnd());
        assert_eq!(
            us,
            [
                0.40630031237378716,
                0.590646798722446,
                0.5958091835491359,
                0.09100268967449665,
                0.9242822963278741,
                0.808205850655213,
                0.7671284528914839,
                0.9752047171350569
            ]
        );
    }

    #[test]
    fn test_uniform_sequence() {
        let mut rng = Rng::from_seed(b"");
        let vs: [f64; 8] = std::array::from_fn(|i| rng.uniform(i as f64, i as f64 * 2.0 + 3.0));
        assert_eq!(
            vs,
            [
                2.5315538013819605,
                2.7396645257249475,
                3.3391379197128117,
                8.876758354716003,
                10.270520234014839,
                9.754763178527355,
                6.668700351845473,
                10.772844967897981
            ]
        );
    }

    #[test]
    fn test_gauss_sequence() {
        let mut rng = Rng::from_seed(b"");
        // Grab more samples than usual to get better coverage on the rejection sampling.
        let vs: [f64; 32] = std::array::from_fn(|_| rng.gauss(10.0, 2.0));
        assert_eq!(
            vs,
            [
                12.347622663830158,
                9.555644025458262,
                11.766414589742986,
                10.421065902979189,
                8.663257202843637,
                9.614660370965233,
                12.005698572460942,
                9.628512069963776,
                8.916242878757988,
                12.876768026032124,
                10.617258413596735,
                14.006192320114256,
                9.947034706000881,
                10.230187477724286,
                7.451754035760429,
                11.148342846306345,
                9.390119721601897,
                9.944130446086874,
                7.709356813603907,
                10.325955650684277,
                8.378478731186833,
                7.097538510395173,
                10.939522022890161,
                11.26899183993857,
                9.026276357070047,
                7.307428436569156,
                10.764942658443658,
                9.065278355076405,
                6.629640618523688,
                12.26010079693567,
                6.424702181087971,
                10.32136095339319
            ]
        );
    }

    #[test]
    fn test_gauss_commutes_when_cached() {
        let mut rng = Rng::from_seed(b"");
        rng.gauss(0.0, 0.0);
        let y1 = rng.gauss(0.0, 0.0);

        let mut rng = Rng::from_seed(b"");
        rng.gauss(0.0, 0.0);
        rng.rnd();
        rng.rnd();
        rng.rnd();
        let y2 = rng.gauss(0.0, 0.0);

        assert_eq!(y1, y2);
    }

    #[test]
    fn test_odds_sequence() {
        let mut rng = Rng::from_seed(b"");
        let vs: [bool; 16] = std::array::from_fn(|i| rng.odds(i as f64 / 16.0));
        assert_eq!(
            vs,
            [
                false, false, false, false, false, false, true, true, // 0..8
                false, true, false, false, true, true, true, true
            ]
        );
    }

    #[test]
    fn test_choice_sequence() {
        let mut rng = Rng::from_seed(b"");

        let colors = &["red", "green", "blue"];
        let fingers = &[1, 2, 3, 4, 5];

        let colors_vs: [&str; 8] = std::array::from_fn(|_| *rng.choice(colors));
        let fingers_vs: [u32; 8] = std::array::from_fn(|_| *rng.choice(fingers));

        assert_eq!(
            colors_vs,
            ["blue", "green", "red", "blue", "blue", "green", "red", "green"]
        );
        assert_eq!(fingers_vs, [5, 3, 5, 5, 3, 4, 3, 4]);
    }

    #[test]
    fn test_wc_distribution() {
        use std::collections::HashMap;
        let mut rng = Rng::from_seed(b"");
        let weighted_items = &[("red", 1), ("green", 3), ("blue", 2)];
        let mut counts = HashMap::new();
        for _ in 0..10000 {
            let color = *rng.wc(weighted_items);
            *counts.entry(color).or_default() += 1;
        }
        assert_eq!(
            counts,
            HashMap::from([("red", 1692), ("green", 4983), ("blue", 3325)])
        );
    }

    #[test]
    fn test_wc_with_float_weights_sequence() {
        let mut rng = Rng::from_seed(b"");

        let choices = &[("red", 1.3), ("green", 3.0), ("blue", 3.0)];
        let values: [&str; 20] = std::array::from_fn(|_| *rng.wc(choices));
        assert_eq!(
            values,
            [
                "blue", "green", "green", "blue", "blue", "blue", "red", "green", "blue", "green",
                "blue", "blue", "green", "blue", "green", "blue", "green", "blue", "blue", "red"
            ]
        );
    }

    #[test]
    fn test_shuffle_empty() {
        let mut rng = Rng::from_seed(b"");
        assert_eq!(rng.shuffle(Vec::<()>::new()), Vec::<()>::new());
    }

    #[test]
    fn test_shuffle_singleton() {
        let mut rng = Rng::from_seed(b"");
        assert_eq!(rng.shuffle(vec![777]), vec![777]);
        assert_eq!(rng.shuffle(vec![777]), vec![777]);
        assert_eq!(rng.shuffle(vec![777]), vec![777]);
    }

    #[test]
    fn test_shuffle_sequence() {
        let mut rng = Rng::from_seed(b"");

        let colors = vec!['r', 'o', 'y', 'g', 'b', 'i', 'v'];
        assert_eq!(
            rng.shuffle(colors.clone()),
            vec!['v', 'y', 'o', 'i', 'r', 'b', 'g']
        );
        assert_eq!(
            rng.shuffle(colors.clone()),
            vec!['r', 'i', 'y', 'v', 'o', 'b', 'g']
        );
        assert_eq!(
            rng.shuffle(colors.clone()),
            vec!['i', 'v', 'y', 'r', 'o', 'b', 'g']
        );
    }

    #[test]
    fn test_winnow_sequence() {
        let mut rng = Rng::from_seed(b"");

        let mut colors = vec!['r', 'o', 'y', 'g', 'b', 'i', 'v'];
        rng.winnow(&mut colors, 999);
        assert_eq!(colors, vec!['r', 'o', 'y', 'g', 'b', 'i', 'v']);
        rng.winnow(&mut colors, 4);
        assert_eq!(colors, vec!['r', 'g', 'b', 'v']);
        rng.winnow(&mut colors, 4);
        assert_eq!(colors, vec!['r', 'g', 'b', 'v']);
        rng.winnow(&mut colors, 1);
        assert_eq!(colors, vec!['r']);

        colors = vec!['r', 'o', 'y', 'g', 'b', 'i', 'v'];
        rng.winnow(&mut colors, 4);
        assert_eq!(colors, vec!['o', 'y', 'b', 'i']);
        rng.winnow(&mut colors, 1);
        assert_eq!(colors, vec!['o']);
    }
}

fn murmur2(bytes: &[u8], seed: u32) -> u32 {
    const K: usize = 16;
    const MASK: Wrapping<u32> = Wrapping(0xffff);
    const MASK_BYTE: Wrapping<u32> = Wrapping(0xff);
    const M: Wrapping<u32> = Wrapping(0x5bd1e995);

    let mut l: usize = bytes.len();
    let mut h = Wrapping(seed ^ (l as u32));
    let mut i = 0;

    let byte32 = |i: usize| Wrapping(u32::from(bytes[i]));

    while l >= 4 {
        let mut k = (byte32(i) & MASK_BYTE)
            | ((byte32(i + 1) & MASK_BYTE) << 8)
            | ((byte32(i + 2) & MASK_BYTE) << 16)
            | ((byte32(i + 3) & MASK_BYTE) << 24);
        i += 4;
        k = (k & MASK) * M + ((((k >> K) * M) & MASK) << K);
        k ^= k >> 24;
        k = (k & MASK) * M + ((((k >> K) * M) & MASK) << K);
        h = ((h & MASK) * M + ((((h >> K) * M) & MASK) << K)) ^ k;
        l -= 4;
    }
    if l >= 3 {
        h ^= (byte32(i + 2) & MASK_BYTE) << K;
    }
    if l >= 2 {
        h ^= (byte32(i + 1) & MASK_BYTE) << 8;
    }
    if l >= 1 {
        h ^= byte32(i) & MASK_BYTE;
        h = (h & MASK) * M + ((((h >> K) * M) & MASK) << K);
    }

    h ^= h >> 13;
    h = (h & MASK) * M + ((((h >> K) * M) & MASK) << K);
    h ^= h >> 15;

    h.0
}

#[cfg(test)]
mod murmur2_test {
    use super::*;
    use hex_literal::hex;

    #[test]
    fn test() {
        assert_eq!(murmur2(b"", 0), 0);
        assert_eq!(murmur2(b"\x12", 0), 0x85701953);
        assert_eq!(murmur2(b"\x12\x34", 0), 0xb106ed81);
        assert_eq!(murmur2(b"\x12\x34\x56", 0), 0xb21b79ab);
        assert_eq!(murmur2(b"\x12\x34\x56\x78", 0), 0x52bcf091);

        let bytes = &hex!("c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470");
        assert_eq!(murmur2(bytes, 0x64c1324d), 0x142b44e9);
        assert_eq!(murmur2(bytes, 0x045970e6), 0x788be436);
    }
}

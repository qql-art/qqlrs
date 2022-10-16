use std::num::Wrapping;

pub struct Rng {
    state: [u16; 4],
    #[allow(dead_code)]
    next_gaussian: Option<f64>,
}

impl Rng {
    pub fn from_seed(seed: &[u8]) -> Rng {
        let lower = murmur2(seed, 1690382925).to_be_bytes();
        let upper = murmur2(seed, 72970470).to_be_bytes();

        // NOTE(wchargin): There are endianness dragons here. The original JavaScript code uses
        // `DataView.setUint32` to portably write a big-endian integer into an `ArrayBuffer`.
        // However, this buffer is the backing storage of a `Uint16Array`, which later reads from
        // the data in platform-dependent order.
        //
        // This Rust port chooses to use the little-endian behavior everywhere for portability. The
        // original behavior can be recovered by changing `from_le_bytes` to `from_ne_bytes`.
        let state = [
            u16::from_le_bytes([lower[0], lower[1]]),
            u16::from_le_bytes([lower[2], lower[3]]),
            u16::from_le_bytes([upper[0], upper[1]]),
            u16::from_le_bytes([upper[2], upper[3]]),
        ];
        Rng {
            state,
            next_gaussian: None,
        }
    }

    /// Picks a random value uniformly distributed between `0.0` (inclusive) and `1.0` (exclusive).
    pub fn rnd(&mut self) -> f64 {
        const M0: Wrapping<u32> = Wrapping(0x7f2d);
        const M1: Wrapping<u32> = Wrapping(0x4c95);
        const M2: Wrapping<u32> = Wrapping(0xf42d);
        const M3: Wrapping<u32> = Wrapping(0x5851);
        const A0: Wrapping<u32> = Wrapping(0x814f);
        const A1: Wrapping<u32> = Wrapping(0xf767);
        const A2: Wrapping<u32> = Wrapping(0x7b7e);
        const A3: Wrapping<u32> = Wrapping(0x1405);

        // Advance internal state.
        let [s0, s1, s2, s3] = self.state.map(|x| Wrapping(u32::from(x)));

        let new0 = A0 + M0 * s0;
        let new1 = A1 + M0 * s1 + (M1 * s0 + (new0 >> 16));
        let new2 = A2 + M0 * s2 + M1 * s1 + (M2 * s0 + (new1 >> 16));
        let new3 = A3 + M0 * s3 + (M1 * s2 + M2 * s1) + (M3 * s0 + (new2 >> 16));

        self.state = [new0, new1, new2, new3].map(|x| x.0 as u16);

        // Calculate output function (XSH RR) using the old state.
        let [_s0, s1, s2, s3] = [s0, s1, s2, s3].map(|x| Wrapping(x.0 as i32));
        let xorshifted: u32 =
            ((s3 << 21) + (((s3 >> 2) ^ s2) << 5) + (((s2 >> 2) ^ s1) >> 11)).0 as u32;
        let fac: u32 = (xorshifted >> (s3.0 >> 11)) | (xorshifted << (-(s3.0 >> 11) & 31));
        2.0f64.powi(-32) * f64::from(fac)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use hex_literal::hex;

    #[test]
    fn test_seed_state() {
        assert_eq!(Rng::from_seed(b"").state, [0xeb00, 0x43ae, 0x85e9, 0x381a]);
        assert_eq!(
            Rng::from_seed(&hex!(
                "efa7bdd92b5e9cd9de9b54ac0e3dc60623f1c989a80ed9c5157fffff10c2a148"
            ))
            .state,
            [0xa894, 0x2177, 0x9757, 0x5069]
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

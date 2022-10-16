use std::num::Wrapping;

pub struct Rng {
    #[allow(dead_code)]
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

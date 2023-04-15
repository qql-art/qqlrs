#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Traits {
    pub flow_field: FlowField,
    pub turbulence: Turbulence,
    pub margin: Margin,
    pub color_variety: ColorVariety,
    pub color_mode: ColorMode,
    pub structure: Structure,
    pub bullseye_rings: BullseyeRings,
    pub ring_thickness: RingThickness,
    pub ring_size: RingSize,
    pub size_variety: SizeVariety,
    pub color_palette: ColorPalette,
    pub spacing: Spacing,
    pub version: Version,
}

impl Traits {
    pub fn from_seed(raw_seed: &[u8; 32]) -> Self {
        let mut remaining = u32::from_be_bytes(raw_seed[28..32].try_into().unwrap());
        fn pluck<T: Copy>(seed: &mut u32, options: &'static [(T, u32)]) -> T {
            if options.is_empty() {
                panic!("no options");
            }
            let num_bits: u32 = options.len().next_power_of_two().ilog2();
            let mask: u32 = (1 << num_bits) - 1;
            let index = *seed & mask;
            *seed >>= num_bits;
            let (option, _weight) = options[(index % options.len() as u32) as usize];
            option
        }
        Traits {
            flow_field: pluck(&mut remaining, FlowField::options()),
            turbulence: pluck(&mut remaining, Turbulence::options()),
            margin: pluck(&mut remaining, Margin::options()),
            color_variety: pluck(&mut remaining, ColorVariety::options()),
            color_mode: pluck(&mut remaining, ColorMode::options()),
            structure: pluck(&mut remaining, Structure::options()),
            bullseye_rings: {
                let ring_options = &[(true, 1), (false, 1)];
                BullseyeRings {
                    one: pluck(&mut remaining, ring_options),
                    three: pluck(&mut remaining, ring_options),
                    seven: pluck(&mut remaining, ring_options),
                }
            },
            ring_thickness: pluck(&mut remaining, RingThickness::options()),
            ring_size: pluck(&mut remaining, RingSize::options()),
            size_variety: pluck(&mut remaining, SizeVariety::options()),
            color_palette: pluck(&mut remaining, ColorPalette::options()),
            spacing: pluck(&mut remaining, Spacing::options()),
            version: Traits::get_version(raw_seed),
        }
    }

    fn get_version(raw_seed: &[u8; 32]) -> Version {
        let sentinel = &raw_seed[26..28];
        if sentinel != [0xff, 0xff] {
            return Version::Unversioned;
        }
        match raw_seed[28] >> 4 {
            0 => Version::V0,
            1 => Version::V1,
            _ => Version::Unversioned,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum Version {
    Unversioned,
    V0,
    V1,
}

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub struct BullseyeRings {
    pub one: bool,
    pub three: bool,
    pub seven: bool,
}

macro_rules! trait_enum {
    ($trait:ident { $($value:ident($weight:expr)),* $(,)? }) => {
        #[derive(Debug, PartialEq, Eq, Copy, Clone)]
        pub enum $trait {
            $($value),*
        }
        impl $trait {
            pub fn options() -> &'static [(Self, u32)] {
                &[$((Self::$value, $weight)),*]
            }
        }
    };
}

trait_enum!(FlowField {
    Horizontal(3),
    Diagonal(1),
    Vertical(3),
    RandomLinear(1),
    Explosive(1),
    Spiral(4),
    Circular(2),
    RandomRadial(1),
});

trait_enum!(Turbulence {
    None(0),
    Low(3),
    High(1),
});

trait_enum!(Margin {
    None(1),
    Crisp(1),
    Wide(2),
});

trait_enum!(ColorVariety {
    Low(2),
    Medium(4),
    High(3),
});

trait_enum!(ColorMode {
    Simple(2),
    Stacked(3),
    Zebra(1),
});

trait_enum!(Structure {
    Orbital(1),
    Formation(1),
    Shadows(1),
});

trait_enum!(RingThickness {
    Thin(1),
    Thick(2),
    Mixed(2),
});

trait_enum!(SizeVariety {
    Constant(1),
    Variable(3),
    Wild(1),
});

trait_enum!(RingSize {
    Small(4),
    Medium(3),
    Large(1),
});

trait_enum!(ColorPalette {
    Austin(1),
    Berlin(1),
    Edinburgh(2),
    Fidenza(2),
    Miami(1),
    Seattle(1),
    Seoul(2),
});

trait_enum!(Spacing {
    Dense(2),
    Medium(1),
    Sparse(1),
});

#[cfg(test)]
mod test {
    use super::*;
    use hex_literal::hex;

    #[test]
    fn test_decode_qql_1() {
        let raw_seed = &hex!("33c9371d25ce44a408f8a6473fbad86bf81e1a178c012cd49a85ffff14c54b46");
        let traits = Traits::from_seed(raw_seed);
        assert_eq!(
            traits,
            Traits {
                flow_field: FlowField::Circular,
                turbulence: Turbulence::None,
                margin: Margin::Wide,
                color_variety: ColorVariety::High,
                color_mode: ColorMode::Stacked,
                structure: Structure::Formation,
                bullseye_rings: BullseyeRings {
                    one: true,
                    three: false,
                    seven: true,
                },
                ring_thickness: RingThickness::Thick,
                ring_size: RingSize::Medium,
                size_variety: SizeVariety::Constant,
                color_palette: ColorPalette::Fidenza,
                spacing: Spacing::Sparse,
                version: Version::V1,
            }
        );
    }

    /// This seed has asymmetrical bullseye rings (1 and 3 but not 7), and is also unversioned
    /// (generated before the spirals patch).
    #[test]
    fn test_decode_qql_2() {
        let raw_seed = &hex!("e03a5189dac8182085e4adf66281f679fff2291d52a252d295b02feda9118a49");
        let traits = Traits::from_seed(raw_seed);
        assert_eq!(
            traits,
            Traits {
                flow_field: FlowField::Diagonal,
                turbulence: Turbulence::Low,
                margin: Margin::Wide,
                color_variety: ColorVariety::Low,
                color_mode: ColorMode::Stacked,
                structure: Structure::Formation,
                bullseye_rings: BullseyeRings {
                    one: true,
                    three: true,
                    seven: false,
                },
                ring_thickness: RingThickness::Thick,
                ring_size: RingSize::Small,
                size_variety: SizeVariety::Variable,
                color_palette: ColorPalette::Miami,
                spacing: Spacing::Dense,
                version: Version::Unversioned,
            }
        );
    }
}

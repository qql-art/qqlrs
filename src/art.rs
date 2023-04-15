use super::math::{modulo, pi, rescale};
use super::rand::Rng;
use super::traits::*;

// Use a constant width and height for all of our calculations to avoid
// float-precision based differences across different window sizes.
const VIRTUAL_W: f64 = 2000.0;
const VIRTUAL_H: f64 = 2500.0;

fn w(v: f64) -> f64 {
    VIRTUAL_W * v
}
fn h(v: f64) -> f64 {
    VIRTUAL_H * v
}

#[derive(Debug, Copy, Clone)]
pub enum FlowFieldSpec {
    Linear {
        default_theta: f64,
    },
    Radial {
        circularity: f64,
        direction: Direction,
        rotation: Rotation,
        default_theta: f64, // still needed for `ignore_flow_fields`
    },
}
#[derive(Debug, Copy, Clone)]
#[repr(u8)]
pub enum Direction {
    In,
    Out,
}
#[derive(Debug, Copy, Clone)]
#[repr(u8)]
pub enum Rotation {
    Ccw,
    Cw,
}

impl FlowFieldSpec {
    pub fn from_traits(traits: &Traits, rng: &mut Rng) -> Self {
        fn linear(mut default_theta: f64, rng: &mut Rng) -> FlowFieldSpec {
            if rng.odds(0.5) {
                default_theta = pi(1.0) - default_theta; // left-right
            }
            if rng.odds(0.5) {
                default_theta += pi(1.0); // up/down
            }
            default_theta = modulo(default_theta, pi(2.0));
            FlowFieldSpec::Linear { default_theta }
        }
        fn radial(circularity: f64, rng: &mut Rng) -> FlowFieldSpec {
            let direction = if rng.odds(0.5) {
                Direction::In
            } else {
                Direction::Out
            };
            let rotation = if rng.odds(0.5) {
                Rotation::Ccw
            } else {
                Rotation::Cw
            };
            let default_theta = *rng.wc(&[(pi(0.0), 1), (pi(0.25), 1), (pi(0.5), 1)]);
            FlowFieldSpec::Radial {
                circularity,
                direction,
                rotation,
                default_theta,
            }
        }

        match traits.flow_field {
            FlowField::Horizontal => linear(pi(0.0), rng),
            FlowField::Diagonal => linear(pi(0.25), rng),
            FlowField::Vertical => linear(pi(0.5), rng),
            FlowField::RandomLinear => linear(rng.uniform(pi(0.0), pi(0.5)), rng),
            FlowField::Explosive => radial(rng.uniform(0.2, 0.4), rng),
            FlowField::Spiral => radial(rng.uniform(0.4, 0.75), rng),
            FlowField::Circular => radial(rng.uniform(0.75, 1.02).min(1.0), rng),
            FlowField::RandomRadial => radial(rng.uniform(-0.01, 1.01).clamp(0.0, 1.0), rng),
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub struct SpacingSpec {
    multiplier: f64,
    constant: f64,
}

impl SpacingSpec {
    pub fn from_traits(traits: &Traits, rng: &mut Rng) -> Self {
        match traits.spacing {
            Spacing::Dense => SpacingSpec {
                multiplier: rng.gauss(1.0, 0.04).max(0.98),
                constant: w(rng.gauss(0.0, 0.002).max(0.0)),
            },
            Spacing::Medium => {
                if rng.odds(0.333) {
                    // mostly proportional
                    SpacingSpec {
                        multiplier: 1.15 + rng.gauss(0.0, 0.2).max(0.0),
                        constant: w(rng.gauss(0.0, 0.001).max(0.0)),
                    }
                } else if rng.odds(0.5) {
                    // mostly constant
                    SpacingSpec {
                        multiplier: rng.uniform(1.0, 1.03),
                        constant: w(0.003) + w(rng.gauss(0.0, 0.005).max(0.0)),
                    }
                } else {
                    // some of both
                    SpacingSpec {
                        multiplier: 1.05 + rng.gauss(0.0, 0.1).max(0.0),
                        constant: w(0.002) + w(rng.gauss(0.0, 0.0015).max(0.0)),
                    }
                }
            }
            Spacing::Sparse => {
                if rng.odds(0.333) {
                    // mostly proportional
                    SpacingSpec {
                        multiplier: 1.25 + rng.gauss(0.0, 0.5).max(0.0),
                        constant: w(rng.gauss(0.0, 0.002).max(0.0)),
                    }
                } else if rng.odds(0.5) {
                    // mostly constant
                    SpacingSpec {
                        multiplier: rng.uniform(1.01, 1.08),
                        constant: w(0.008) + w(rng.gauss(0.0, 0.02).max(0.0)),
                    }
                } else {
                    // some of both
                    SpacingSpec {
                        multiplier: 1.15 + rng.gauss(0.0, 0.3).max(0.0),
                        constant: w(0.005) + w(rng.gauss(0.0, 0.006).max(0.0)),
                    }
                }
            }
        }
    }
}

#[derive(Debug, Copy, Clone)]
struct ColorChangeOdds {
    group: f64,
    line: f64,
}

impl ColorChangeOdds {
    pub fn from_traits(traits: &Traits, rng: &mut Rng) -> Self {
        use ColorVariety::*;
        use Structure::*;
        let (group, line) = match (traits.structure, traits.color_variety) {
            (Shadows, Low) => (rng.gauss(0.15, 0.15), rng.uniform(-0.004, 0.002)),
            (Shadows, Medium) => (rng.gauss(0.55, 0.2), rng.uniform(0.01, 0.01)),
            (Shadows, High) => (rng.gauss(0.9, 0.1), rng.uniform(-0.1, 0.2)),
            (Formation, Low) => (rng.gauss(0.5, 0.2), rng.uniform(-0.002, 0.003)),
            (Formation, Medium) => (rng.gauss(0.75, 0.2), rng.uniform(-0.005, 0.01)),
            (Formation, High) => (rng.gauss(0.9, 0.1), rng.uniform(-0.1, 0.2)),
            (Orbital, Low) => (rng.gauss(0.11, 0.08), rng.uniform(-0.002, 0.0015)),
            (Orbital, Medium) => (rng.gauss(0.25, 0.1), rng.uniform(-0.01, 0.01)),
            (Orbital, High) => (rng.gauss(0.7, 0.2), rng.uniform(-0.1, 0.2)),
        };

        // adjust based on scale
        let mult1 = match traits.ring_size {
            RingSize::Small => 0.5,
            RingSize::Medium => 1.0, // calibrated for 'medium'
            RingSize::Large => 1.1,
        };

        // adjust based on spacing
        let mult2 = match traits.spacing {
            Spacing::Dense => 1.0, // calibrated for 'dense'
            Spacing::Medium => 1.1,
            Spacing::Sparse => 2.0,
        };

        let mult = mult1 * mult2;
        ColorChangeOdds {
            group: (group * mult).clamp(0.0, 1.0),
            line: (line * mult).clamp(0.0, 1.0),
        }
    }
}

#[derive(Debug)]
pub enum ScaleGenerator {
    Constant {
        mean: f64,
    },
    Variable {
        mean: f64,
        choices: &'static [(f64, f64)],
    },
    Wild {
        mean: f64,
        choices: &'static [(f64, f64)],
    },
}

impl ScaleGenerator {
    pub fn next(&self, rng: &mut Rng) -> f64 {
        match *self {
            ScaleGenerator::Constant { mean } => rng.gauss(mean, w(0.01).min(mean * 0.05)),
            ScaleGenerator::Variable { mean, .. } => rng.gauss(mean, w(0.035).min(mean * 0.15)),
            ScaleGenerator::Wild { mean, .. } => rng.gauss(mean, mean * 0.3),
        }
    }

    pub fn change(&mut self, rng: &mut Rng) {
        match self {
            ScaleGenerator::Constant { .. } => (),
            ScaleGenerator::Variable { mean, choices } => {
                *mean = *rng.wc(choices);
                *mean = rng.gauss(*mean, *mean * 0.1);
            }
            ScaleGenerator::Wild { mean, choices } => {
                *mean = *rng.wc(choices);
                *mean = rng.gauss(*mean, *mean * 0.3);
            }
        }
    }

    pub fn from_traits(traits: &Traits, rng: &mut Rng) -> ScaleGenerator {
        const XS: &[f64] = &[
            VIRTUAL_W * 0.0018, // 0
            VIRTUAL_W * 0.0025, // 1
        ];
        const S: &[f64] = &[
            VIRTUAL_W * 0.003, // 0
            VIRTUAL_W * 0.004, // 1
            VIRTUAL_W * 0.006, // 2
        ];
        const M: &[f64] = &[
            VIRTUAL_W * 0.012, // 0
            VIRTUAL_W * 0.017, // 1
            VIRTUAL_W * 0.023, // 2
            VIRTUAL_W * 0.048, // 3
        ];
        const L: &[f64] = &[
            VIRTUAL_W * 0.1,  // 0
            VIRTUAL_W * 0.15, // 1
            VIRTUAL_W * 0.2,  // 2
            VIRTUAL_W * 0.3,  // 3
        ];

        use RingSize::*;
        use SizeVariety::*;

        match traits.size_variety {
            Constant => {
                const WC_S: &[(f64, u32)] = &[(XS[1], 2), (S[0], 3), (S[1], 2), (S[2], 1)];
                const WC_M: &[(f64, u32)] = &[(M[0], 2), (M[1], 3), (M[2], 2), (M[3], 1)];
                const WC_L: &[(f64, u32)] = &[(L[0], 3), (L[1], 2), (L[2], 1)];
                let choices = match traits.ring_size {
                    Small => WC_S,
                    Medium => WC_M,
                    Large => WC_L,
                };
                let mean = *rng.wc(choices);
                if mean.is_nan() || mean == 0.0 {
                    panic!("bad scale");
                }
                ScaleGenerator::Constant { mean }
            }

            Variable => {
                const WC_S: &[(f64, f64)] = &[
                    (XS[1], 1.3),
                    (S[0], 2.0),
                    (S[1], 5.0),
                    (S[2], 8.0),
                    (M[0], 3.0),
                ];
                const WC_M: &[(f64, f64)] = &[
                    (S[2], 2.0),
                    (M[0], 8.0),
                    (M[1], 8.0),
                    (M[2], 13.0),
                    (M[3], 8.0),
                    (L[0], 5.0),
                ];
                const WC_L: &[(f64, f64)] = &[
                    (M[1], 0.5),
                    (M[2], 2.0),
                    (M[3], 2.0),
                    (L[0], 5.0),
                    (L[1], 8.0),
                    (L[2], 8.0),
                    (L[3], 4.0),
                ];
                let choices = match traits.ring_size {
                    Small => WC_S,
                    Medium => WC_M,
                    Large => WC_L,
                };
                let mean = *rng.wc(choices);
                ScaleGenerator::Variable { mean, choices }
            }

            Wild => {
                const SC_S: &[f64] = &[S[1], S[2], M[0], M[1], M[2]];
                const SC_M: &[f64] = &[S[2], M[0], M[1], M[2], M[3], L[0], L[1]];
                const SC_L: &[f64] = &[L[0], L[1], L[2], L[3]];

                const WC_S: &[(f64, f64)] = &[
                    (XS[0], 3.0),
                    (XS[1], 3.0),
                    (S[0], 3.0),
                    (S[1], 4.0),
                    (S[2], 4.0),
                    (M[0], 3.0),
                    (M[1], 3.0),
                    (M[2], 3.0),
                ];
                const WC_M: &[(f64, f64)] = &[
                    (XS[0], 1.0),
                    (XS[1], 1.0),
                    (S[0], 1.0),
                    (S[1], 1.0),
                    (S[2], 2.0),
                    (M[0], 3.0),
                    (M[1], 3.0),
                    (M[2], 3.0),
                    (M[3], 3.0),
                    (L[0], 2.0),
                    (L[1], 2.0),
                    (L[2], 1.0),
                ];
                const WC_L: &[(f64, f64)] = &[
                    (XS[0], 1.0),
                    (XS[1], 1.0),
                    (S[0], 1.0),
                    (S[1], 1.0),
                    (S[2], 1.0),
                    (M[0], 1.0),
                    (M[1], 1.0),
                    (M[2], 1.0),
                    (L[0], 2.0),
                    (L[1], 5.0),
                    (L[2], 5.0),
                    (L[3], 5.0),
                ];

                let (start_choices, choices) = match traits.ring_size {
                    Small => (SC_S, WC_S),
                    Medium => (SC_M, WC_M),
                    Large => (SC_L, WC_L),
                };
                let mean = *rng.choice(start_choices);
                ScaleGenerator::Wild { mean, choices }
            }
        }
    }
}

type RingCount = u32; // 1, 2, 3, or 7
pub struct BullseyeGenerator {
    density_mean: f64,
    density_variance: f64,
    weighted_ring_options: Vec<(RingCount, f64)>,
}
#[derive(Debug, Copy, Clone)]
pub struct Bullseye {
    rings: RingCount,
    density: f64,
}

impl BullseyeGenerator {
    pub fn from_traits(traits: &Traits, rng: &mut Rng) -> Self {
        let mut potential_ring_counts: Vec<RingCount> = Vec::with_capacity(3);
        if traits.bullseye_rings.one {
            potential_ring_counts.push(1);
        }
        if traits.bullseye_rings.three {
            potential_ring_counts.push(3);
        }
        if traits.bullseye_rings.seven {
            potential_ring_counts.push(7);
        }
        if potential_ring_counts.is_empty() {
            potential_ring_counts.push(2);
        }

        let (density_mean, density_variance) = match traits.ring_thickness {
            RingThickness::Thin => (0.85, 0.15),
            RingThickness::Thick => (0.28, 0.1),
            RingThickness::Mixed => (0.7, 1.0),
        };
        let dropoff = rescale(density_variance, (0.0, 1.0), (1.0, 0.35));

        let mut weight = 1.0;
        let mut n = potential_ring_counts.len() * 2;
        let mut weighted_ring_options = Vec::with_capacity(n);
        while n > 0 && weight > 0.001 {
            weighted_ring_options.push((*rng.choice(&potential_ring_counts[..]), weight));
            n -= 1;
            weight *= dropoff;
        }

        BullseyeGenerator {
            density_mean,
            density_variance,
            weighted_ring_options,
        }
    }

    pub fn next(&self, rng: &mut Rng) -> Bullseye {
        let density = rng
            .gauss(self.density_mean, self.density_variance / 2.0)
            .clamp(0.17, 0.93);
        let rings = *rng.wc(&self.weighted_ring_options[..]);
        Bullseye { rings, density }
    }
}

pub fn draw(seed: &[u8; 32]) {
    let mut rng = Rng::from_seed(&seed[..]);
    let traits = Traits::from_seed(seed);
    let flow_field_spec = FlowFieldSpec::from_traits(&traits, &mut rng);
    let spacing_spec = SpacingSpec::from_traits(&traits, &mut rng);
    let color_change_odds = ColorChangeOdds::from_traits(&traits, &mut rng);
    let scale_generator = ScaleGenerator::from_traits(&traits, &mut rng);
    let bullseye_generator = BullseyeGenerator::from_traits(&traits, &mut rng);
    for _ in 0..5 {
        println!("{:?}", bullseye_generator.next(&mut rng));
    }
}

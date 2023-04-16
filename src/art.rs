use super::color::{ColorDb, ColorKey};
use super::math::{angle, dist, modulo, pi, rescale};
use super::rand::Rng;
use super::traits::*;

// Use a constant width and height for all of our calculations to avoid
// float-precision based differences across different window sizes.
const VIRTUAL_W: f64 = 2000.0;
const VIRTUAL_H: f64 = 2500.0;

// Flow field boundaries, in virtual canvas space.
const LX: f64 = VIRTUAL_W * -0.2;
const RX: f64 = VIRTUAL_W * 1.2;
const TY: f64 = VIRTUAL_H * -0.2;
const BY: f64 = VIRTUAL_H * 1.2;

const SPC: f64 = 5.0; // spacing of points in the field, on each axis
const FLOW_FIELD_ROWS: usize = 700;
const FLOW_FIELD_COLS: usize = 560;

/// Tests that the hard-coded values for [`SPC`], [`FLOW_FIELD_ROWS`], and [`FLOW_FIELD_COLS`]
/// match the computed values used (either implicitly or explicitly) in the JavaScript algorithm.
/// These are hard-coded because we can't use [`f64::floor`] and friends in a const context.
#[cfg(test)]
#[test]
fn test_flow_field_dimensions() {
    assert_eq!(SPC, (VIRTUAL_W * 0.0025).floor());

    let mut x = LX;
    let mut cols = 0;
    while x < RX {
        cols += 1;
        x += SPC;
    }
    assert_eq!(cols, FLOW_FIELD_COLS);

    let mut y = TY;
    let mut cols = 0;
    while y < BY {
        cols += 1;
        y += SPC;
    }
    assert_eq!(cols, FLOW_FIELD_ROWS);
}

pub(crate) fn w(v: f64) -> f64 {
    VIRTUAL_W * v
}
pub(crate) fn h(v: f64) -> f64 {
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

        use super::traits::FlowField; // shadow `self::FlowField`
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
    pub multiplier: f64,
    pub constant: f64,
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
pub struct ColorChangeOdds {
    pub group: f64,
    pub line: f64,
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
    pub density_mean: f64,
    pub density_variance: f64,
    pub weighted_ring_options: Vec<(RingCount, f64)>,
}
#[derive(Debug, Copy, Clone)]
pub struct Bullseye {
    pub rings: RingCount,
    pub density: f64,
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

#[derive(Debug)]
pub struct ColorScheme {
    pub background: ColorKey,
    pub primary_seq: Vec<ColorKey>,
    pub secondary_seq: Vec<ColorKey>,
    pub splatter_odds: f64,
    pub splatter_center: (f64, f64),
    pub splatter_choices: Vec<ColorKey>,
}

impl ColorScheme {
    pub fn from_traits(traits: &Traits, color_db: &ColorDb, rng: &mut Rng) -> Self {
        let palette = color_db
            .palette(traits.color_palette)
            .unwrap_or_else(|| panic!("missing color data for palette {:?}", traits.color_palette));
        let bg = rng.wc(&palette.background_colors);
        let substitute = |c: ColorKey| -> Option<ColorKey> {
            bg.substitutions.get(&c).copied().unwrap_or(Some(c))
            // NOTE: `None` means "remove from palette", not "use original color"
        };
        let color_seq: Vec<ColorKey> = palette
            .color_seq
            .iter()
            .copied()
            .filter_map(substitute)
            .collect();
        let splatter_choices = {
            let splatter_opts: Vec<(ColorKey, f64)> = palette
                .splatter_colors
                .iter()
                .copied()
                .filter_map(|(c, w)| Some((substitute(c)?, w)))
                .collect();
            let num_choices = usize::max(1, rng.gauss(1.5, 2.0).round() as usize);
            let mut choices: Vec<ColorKey> = Vec::with_capacity(num_choices);
            for _ in 0..num_choices {
                choices.push(*rng.wc(&splatter_opts));
            }
            choices
        };

        use ColorVariety::*;
        let splatter_odds_choices: &[(f64, f64)] = match traits.color_variety {
            Low => &[(0.0, 4.0), (0.001, 2.0), (0.002, 2.0), (0.005, 2.0)],
            Medium => &[
                (0.0, 3.0),
                (0.002, 2.0),
                (0.005, 2.0),
                (0.01, 1.0),
                (0.03, 1.0),
            ],
            High => &[
                (0.0, 3.0),
                (0.002, 2.0),
                (0.005, 2.0),
                (0.01, 1.0),
                (0.03, 1.0),
                (0.08, 1.0),
                (0.5, 0.05),
            ],
        };
        let num_color_choices: &[(usize, u32)] = match traits.color_variety {
            Low => &[(1, 1), (2, 3), (3, 4), (4, 5), (5, 3)],
            Medium => &[(5, 1), (6, 2), (7, 3), (8, 5), (10, 3), (15, 2)],
            High => &[(10, 3), (12, 4), (15, 5), (20, 3), (25, 3)],
        };

        let mut primary_seq = color_seq;
        let mut secondary_seq = primary_seq.clone();
        let num_primary_colors = *rng.wc(num_color_choices);
        rng.winnow(&mut primary_seq, num_primary_colors);
        let num_secondary_colors = *rng.wc(num_color_choices);
        rng.winnow(&mut secondary_seq, num_secondary_colors);

        let splatter_center = (rng.uniform(w(-0.1), w(1.1)), rng.uniform(h(-0.1), h(1.1)));
        let splatter_odds = *rng.wc(splatter_odds_choices);

        ColorScheme {
            background: bg.color,
            primary_seq,
            secondary_seq,
            splatter_odds,
            splatter_center,
            splatter_choices,
        }
    }
}

// NOTE: This struct definition shadows the `traits::FlowField` enum.
#[derive(Debug)]
pub struct FlowField(pub Box<[[f64; FLOW_FIELD_ROWS]; FLOW_FIELD_COLS]>);

impl FlowField {
    pub fn build(spec: &FlowFieldSpec, traits: &Traits, rng: &mut Rng) -> Self {
        let mut ff = match spec {
            FlowFieldSpec::Linear { default_theta } => Self::raw_linear(*default_theta),
            FlowFieldSpec::Radial {
                circularity,
                direction,
                rotation,
                default_theta: _,
            } => Self::raw_circular(*circularity, *direction, *rotation, traits.version, rng),
        };
        let disturbances = Disturbance::build(traits, rng);
        ff.adjust(&disturbances);
        ff
    }

    fn raw_linear(default_theta: f64) -> Self {
        FlowField(Box::new(
            [[default_theta; FLOW_FIELD_ROWS]; FLOW_FIELD_COLS],
        ))
    }

    fn raw_circular(
        circularity: f64,
        direction: Direction,
        rotation: Rotation,
        version: Version,
        rng: &mut Rng,
    ) -> Self {
        let mut rot = circularity / 2.0;
        if let Direction::Out = direction {
            rot = 1.0 - rot;
        }
        if let Rotation::Cw = rotation {
            rot = 2.0 - rot;
        }
        rot = pi(rot);

        let mut flow_points = Box::new([[0.0; FLOW_FIELD_ROWS]; FLOW_FIELD_COLS]);

        let cx = {
            let fst = rng.uniform(w(0.0), w(1.0));
            *rng.wc(&[
                (fst, 2.0),
                (w(-2.0 / 3.0), 0.5),
                (w(-1.0 / 3.0), 1.0),
                (w(0.0), 1.0),
                (w(1.0 / 3.0), 1.5),
                (w(1.0 / 2.0), 1.5),
                (w(2.0 / 3.0), 1.5),
                (w(1.0), 1.5),
                (w(4.0 / 3.0), 1.0),
                (w(5.0 / 3.0), 0.5),
            ])
        };
        let fixed_weight = match version {
            Version::V1 => 0.5,
            _ => f64::NAN, // bug-compatibility with older versions
        };
        let cy = {
            let fst = rng.uniform(h(0.0), h(1.0));
            *rng.wc(&[
                (fst, 2.0),
                (h(-2.0 / 3.0), fixed_weight),
                (h(-1.0 / 3.0), 1.0),
                (h(0.0), 1.0),
                (h(1.0 / 3.0), 1.5),
                (h(1.0 / 2.0), 1.5),
                (h(2.0 / 3.0), 1.5),
                (h(1.0), 1.0),
                (h(4.0 / 3.0), 1.0),
                (h(5.0 / 3.0), 0.5),
            ])
        };

        let mut x = LX;
        for col in &mut *flow_points {
            let mut y = TY;
            for point in col {
                let mut a = angle((x, y), (cx, cy));
                if a.is_nan() {
                    a = 0.0;
                }
                *point = a + rot;
                y += SPC;
            }
            x += SPC;
        }
        FlowField(flow_points)
    }

    fn adjust(&mut self, disturbances: &[Disturbance]) {
        for d in disturbances {
            let Disturbance {
                center: (cx, cy),
                theta,
                radius,
            } = d;
            let (min_x, max_x) = (cx - radius, cx + radius);
            let (min_y, max_y) = (cy - radius, cy + radius);

            let min_i = usize::max(0, ((min_x - LX) / SPC).floor() as usize);
            let max_i = usize::min(FLOW_FIELD_COLS - 1, ((max_x - LX) / SPC).ceil() as usize);
            let min_j = usize::max(0, ((min_y - TY) / SPC).floor() as usize);
            let max_j = usize::min(FLOW_FIELD_ROWS - 1, ((max_y - TY) / SPC).ceil() as usize);

            for i in min_i..=max_i {
                let x = LX + SPC * i as f64;
                for j in min_j..=max_j {
                    let y = TY + SPC * j as f64;
                    let dist = dist((*cx, *cy), (x, y));
                    let theta_adjust = rescale(dist, (0.0, *radius), (*theta, 0.0));
                    self.0[i][j] += theta_adjust;
                }
            }
        }
    }
}

#[derive(Debug)]
struct Disturbance {
    center: (f64, f64),
    theta: f64,
    radius: f64,
}

impl Disturbance {
    fn build(traits: &Traits, rng: &mut Rng) -> Vec<Self> {
        let num: usize = match traits.turbulence {
            Turbulence::None => 0,
            Turbulence::Low => *rng.wc(&[(10, 2), (15, 3), (20, 2), (30, 1)]),
            Turbulence::High => *rng.wc(&[(20, 1), (30, 2), (40, 3), (50, 2), (60, 1)]),
        };
        let theta_variance: f64 = match traits.turbulence {
            Turbulence::None => 0.0,
            Turbulence::Low => *rng.wc(&[(pi(0.005), 1), (pi(0.01), 1)]),
            Turbulence::High => *rng.wc(&[(pi(0.05), 1), (pi(0.1), 1), (pi(0.15), 1)]),
        };

        let mut disturbances = Vec::with_capacity(num);
        for _ in 0..num {
            let center = (rng.uniform(LX, RX), rng.uniform(TY, BY));
            let theta = rng.gauss(0.0, theta_variance);
            let radius = rng.gauss(w(0.35), w(0.35)).abs().max(w(0.1));
            disturbances.push(Disturbance {
                center,
                theta,
                radius,
            })
        }
        disturbances
    }
}

#[derive(Debug, Copy, Clone)]
pub struct IgnoreFlowField {
    pub odds: f64,
}

impl IgnoreFlowField {
    pub fn build(rng: &mut Rng) -> Self {
        let odds = *rng.wc(&[(0.0, 10), (0.5, 2), (0.8, 1), (0.9, 1)]);
        IgnoreFlowField { odds }
    }
}

pub fn draw(seed: &[u8; 32], color_db: &ColorDb) {
    let traits = Traits::from_seed(seed);
    println!("traits: {:#?}", traits);
    let mut rng = Rng::from_seed(&seed[..]);

    let flow_field_spec = FlowFieldSpec::from_traits(&traits, &mut rng);
    let _spacing_spec = SpacingSpec::from_traits(&traits, &mut rng);
    let _color_change_odds = ColorChangeOdds::from_traits(&traits, &mut rng);
    let _scale_generator = ScaleGenerator::from_traits(&traits, &mut rng);
    let _bullseye_generator = BullseyeGenerator::from_traits(&traits, &mut rng);
    let _scheme = ColorScheme::from_traits(&traits, color_db, &mut rng);

    let _flow_field = FlowField::build(&flow_field_spec, &traits, &mut rng);
    let _ignore_flow_field = IgnoreFlowField::build(&mut rng);
    let _start_points = crate::layouts::generate_start_points(traits.structure, &mut rng);

    let named_colors = |seq: &[ColorKey]| -> Vec<&str> {
        seq.iter()
            .map(|&i| color_db.color(i).unwrap().name.as_str())
            .collect::<Vec<_>>()
    };

    println!("flow field spec: {:?}", flow_field_spec);
    println!("spacing spec: {:?}", _spacing_spec);
    println!("color change odds: {:?}", _color_change_odds);
    println!(
        "background: {:?}",
        color_db.color(_scheme.background).unwrap().name
    );
    println!("primary_seq: {:?}", named_colors(&_scheme.primary_seq));
    println!("secondary_seq: {:?}", named_colors(&_scheme.secondary_seq));

    println!(
        "flow field ({}x{}): top-left {:?}, bottom-right {:?}",
        _flow_field.0.len(),
        _flow_field.0[0].len(),
        _flow_field.0.first().unwrap().first().unwrap(),
        _flow_field.0.last().unwrap().last().unwrap()
    );
    println!("ignore flow field: {:?}", _ignore_flow_field);

    println!("start points groups (len={}):", _start_points.0.len());
    {
        let g0 = _start_points.0.first().unwrap();
        println!(
            "    first group (len={}) = {:?} ... {:?}",
            g0.len(),
            g0.first().unwrap(),
            g0.last().unwrap()
        );
    }
    {
        let glast = _start_points.0.last().unwrap();
        println!(
            "    last group  (len={}) = {:?} ... {:?}",
            glast.len(),
            glast.first().unwrap(),
            glast.last().unwrap()
        );
    }
}

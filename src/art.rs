use std::collections::BTreeMap;

use raqote::{DrawOptions, DrawTarget, PathBuilder, SolidSource, Source, StrokeStyle};

use super::color::{ColorDb, ColorKey, ColorSpec};
use super::config::{Animation, Config, FractionalViewport};
use super::layouts::StartPointGroups;
use super::math::{angle, cos, dist, modulo, pi, rescale, sin};
use super::rand::Rng;
use super::sectors::{Collider, Sectors};
use super::traits::*;

mod colors_used;
pub use colors_used::ColorsUsed;

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
        default_theta: f64, // still needed for `ignore_flow_field`
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

    fn default_theta(&self) -> f64 {
        match *self {
            FlowFieldSpec::Linear { default_theta, .. } => default_theta,
            FlowFieldSpec::Radial { default_theta, .. } => default_theta,
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
#[derive(Debug, Copy, Clone, PartialEq)]
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

    fn constant_flow_field(theta: f64) -> Box<[[f64; FLOW_FIELD_ROWS]; FLOW_FIELD_COLS]> {
        let vec_of_arrays = vec![[theta; FLOW_FIELD_ROWS]; FLOW_FIELD_COLS];
        let slice_of_arrays = vec_of_arrays.into_boxed_slice();
        slice_of_arrays.try_into().unwrap()
    }

    fn raw_linear(default_theta: f64) -> Self {
        FlowField(Self::constant_flow_field(default_theta))
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

        let mut flow_points = Self::constant_flow_field(0.0);

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
    pub default_theta: f64,
}

impl IgnoreFlowField {
    pub fn build(flow_field_spec: &FlowFieldSpec, rng: &mut Rng) -> Self {
        let default_theta = flow_field_spec.default_theta();
        let odds = *rng.wc(&[(0.0, 10), (0.5, 2), (0.8, 1), (0.9, 1)]);
        IgnoreFlowField {
            odds,
            default_theta,
        }
    }
}

#[derive(Debug)]
pub struct GroupedFlowLines(pub Vec<FlowLineGroup>);
type FlowLineGroup = Vec<FlowLine>;
type FlowLine = Vec<(f64, f64)>;

impl GroupedFlowLines {
    pub fn build(
        flow_field: FlowField,
        ignore_flow_field: IgnoreFlowField,
        start_point_groups: StartPointGroups,
        rng: &mut Rng,
    ) -> Self {
        let curve_length = *rng.choice(&[500, 650, 850]);
        let groups = start_point_groups
            .0
            .into_iter()
            .map(|group| {
                Self::build_group(group, curve_length, &flow_field, &ignore_flow_field, rng)
            })
            .collect::<Vec<FlowLineGroup>>();
        GroupedFlowLines(groups)
    }

    fn build_group(
        start_points: Vec<(f64, f64)>,
        curve_length: usize,
        flow_field: &FlowField,
        ignore_flow_field: &IgnoreFlowField,
        rng: &mut Rng,
    ) -> FlowLineGroup {
        let ignore = rng.odds(ignore_flow_field.odds);
        let step = w(0.002);
        start_points
            .into_iter()
            .map(|(mut x, mut y)| {
                let mut curve: FlowLine = Vec::with_capacity(curve_length);
                for _ in 0..curve_length {
                    #[allow(clippy::manual_range_contains)]
                    if x < LX || x >= RX || y < TY || y >= BY {
                        // Terminate the flow line as it has exited the flow field boundary.
                        curve.shrink_to_fit();
                        break;
                    }
                    let xi = ((x - LX) / SPC).floor() as usize;
                    let yi = ((y - TY) / SPC).floor() as usize;
                    let theta = if ignore {
                        ignore_flow_field.default_theta
                    } else {
                        flow_field.0[xi][yi]
                    };
                    curve.push((x, y));
                    x += step * cos(theta);
                    y += step * sin(theta);
                }
                curve
            })
            .collect()
    }
}

fn build_sectors(config: &Config) -> Sectors {
    const CHECK_MARGIN: f64 = 0.05;
    const CHECK_LEFT: f64 = -VIRTUAL_W * CHECK_MARGIN;
    const CHECK_RIGHT: f64 = VIRTUAL_W + VIRTUAL_W * CHECK_MARGIN;
    const CHECK_TOP: f64 = -VIRTUAL_H * CHECK_MARGIN;
    const CHECK_BOTTOM: f64 = VIRTUAL_H + VIRTUAL_H * CHECK_MARGIN;
    Sectors::new(config, CHECK_LEFT, CHECK_RIGHT, CHECK_TOP, CHECK_BOTTOM)
}

#[derive(Debug)]
pub struct MarginChecker {
    margin: f64,
    bottom_margin: f64,
}
impl MarginChecker {
    pub fn from_traits(traits: &Traits) -> Self {
        match traits.margin {
            Margin::None => Self {
                margin: w(-0.05),
                bottom_margin: w(-0.05),
            },
            Margin::Crisp => Self {
                margin: w(0.003),
                bottom_margin: w(0.003),
            },
            Margin::Wide => Self {
                margin: w(0.07),
                bottom_margin: w(0.08),
            },
        }
    }

    pub fn in_bounds(&self, (x, y): (f64, f64), spacing: f64) -> bool {
        let Self {
            margin,
            bottom_margin,
        } = *self;
        !(x - spacing < margin
            || x + spacing >= VIRTUAL_W - margin
            || y - spacing < margin
            || y + spacing > VIRTUAL_H - bottom_margin)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Point {
    pub position: (f64, f64),
    pub scale: f64,
    pub primary_color: Hsb,
    pub secondary_color: Hsb,
    pub bullseye: Bullseye,
}
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Hsb(pub f64, pub f64, pub f64);

pub struct Points(Vec<Point>);
pub struct GroupSizes(Vec<usize>);

impl Hsb {
    pub fn to_rgb(self) -> Rgb {
        let h = self.0;
        let s = self.1 / 100.0;
        let v = self.2 / 100.0;
        let chroma = s * v * 255.0;
        let h = h / 60.0;
        let secondary = chroma * (1.0 - (h % 2.0 - 1.0).abs());
        let (r, g, b) = match () {
            _ if h < 1.0 => (chroma, secondary, 0.0),
            _ if h < 2.0 => (secondary, chroma, 0.0),
            _ if h < 3.0 => (0.0, chroma, secondary),
            _ if h < 4.0 => (0.0, secondary, chroma),
            _ if h < 5.0 => (secondary, 0.0, chroma),
            _ => (chroma, 0.0, secondary),
        };
        let m = v * 255.0 - chroma;
        Rgb(r + m, g + m, b + m)
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Rgb(pub f64, pub f64, pub f64);

impl Rgb {
    pub fn to_solid_source(self) -> SolidSource {
        SolidSource {
            r: self.0 as u8,
            g: self.1 as u8,
            b: self.2 as u8,
            a: 255,
        }
    }
    pub fn to_source(self) -> Source<'static> {
        Source::Solid(self.to_solid_source())
    }
}

impl Point {
    fn num_drawn_rings(&self) -> RingCount {
        let natural_rings = self.bullseye.rings;
        // TODO(wchargin): Simplify under the assumption that `natural_rings` starts as 1, 2, 3, or 7.
        if self.scale < rescale(self.bullseye.density, (0.15, 1.0), (w(0.0039), w(0.001))) {
            natural_rings.min(1)
        } else if self.scale < rescale(self.bullseye.density, (0.15, 1.0), (w(0.0072), w(0.0029))) {
            natural_rings.min(2)
        } else if self.scale < w(0.01) {
            natural_rings.min(3)
        } else if self.scale < w(0.012) {
            natural_rings.min(4)
        } else if self.scale < w(0.014) {
            natural_rings.min(5)
        } else if self.scale < w(0.017) {
            natural_rings.min(6)
        } else if self.scale < w(0.02) {
            natural_rings.min(7)
        } else if self.scale < w(0.023) {
            natural_rings.min(8)
        } else {
            natural_rings
        }
    }
}

impl Points {
    #[allow(clippy::too_many_arguments)]
    pub fn build(
        traits: &Traits,
        color_db: &ColorDb,
        grouped_flow_lines: GroupedFlowLines,
        color_scheme: &ColorScheme,
        color_change_odds: &ColorChangeOdds,
        spacing_spec: &SpacingSpec,
        bullseye_generator: &mut BullseyeGenerator,
        scale_generator: &mut ScaleGenerator,
        sectors: &mut Sectors,
        colors_used: &mut ColorsUsed,
        rng: &mut Rng,
    ) -> (Points, GroupSizes) {
        fn random_idx(len: usize, rng: &mut Rng) -> usize {
            rng.uniform(0.0, len as f64) as usize
        }
        let mut primary_color_idx = random_idx(color_scheme.primary_seq.len(), rng);
        let mut secondary_color_idx = random_idx(color_scheme.secondary_seq.len(), rng);
        let mut base_bullseye_spec = bullseye_generator.next(rng);

        let margin_checker = MarginChecker::from_traits(traits);

        let mut all_points = Vec::new();
        let mut group_sizes = Vec::with_capacity(grouped_flow_lines.0.len());
        for group in grouped_flow_lines.0 {
            if rng.odds(color_change_odds.group) {
                primary_color_idx =
                    pick_next_color(&color_scheme.primary_seq, primary_color_idx, rng);
                secondary_color_idx =
                    pick_next_color(&color_scheme.secondary_seq, secondary_color_idx, rng);
                base_bullseye_spec = bullseye_generator.next(rng);
            }
            let old_size = all_points.len();
            Self::build_group(
                &mut all_points,
                color_db,
                group,
                color_scheme,
                primary_color_idx,
                secondary_color_idx,
                color_change_odds,
                spacing_spec,
                bullseye_generator,
                base_bullseye_spec,
                scale_generator,
                sectors,
                &margin_checker,
                colors_used,
                rng,
            );
            let new_size = all_points.len();
            group_sizes.push(new_size - old_size);
        }
        (Points(all_points), GroupSizes(group_sizes))
    }

    #[allow(clippy::too_many_arguments)]
    fn build_group(
        dest: &mut Vec<Point>,
        color_db: &ColorDb,
        group: FlowLineGroup,
        color_scheme: &ColorScheme,
        mut primary_color_idx: usize,
        mut secondary_color_idx: usize,
        color_change_odds: &ColorChangeOdds,
        spacing_spec: &SpacingSpec,
        bullseye_generator: &mut BullseyeGenerator,
        mut bullseye: Bullseye,
        scale_generator: &mut ScaleGenerator,
        sectors: &mut Sectors,
        margin_checker: &MarginChecker,
        colors_used: &mut ColorsUsed,
        rng: &mut Rng,
    ) {
        if rng.odds(color_change_odds.line) {
            scale_generator.change(rng);
        }
        let scale = scale_generator.next(rng);

        let mut primary_color_spec = color_db
            .color(color_scheme.primary_seq[primary_color_idx])
            .expect("invalid color");
        let mut primary_color = spec_to_color(
            color_scheme.primary_seq[primary_color_idx],
            primary_color_spec,
            colors_used,
            rng,
        );

        let mut secondary_color_spec = color_db
            .color(color_scheme.secondary_seq[secondary_color_idx])
            .expect("invalid color");
        let mut secondary_color = spec_to_color(
            color_scheme.secondary_seq[secondary_color_idx],
            secondary_color_spec,
            colors_used,
            rng,
        );

        for flow_line in group {
            for (x, y) in flow_line {
                let mut multiplier = spacing_spec.multiplier;
                if scale > w(0.015) {
                    multiplier = multiplier.max(1.02);
                }
                let spacing_radius =
                    f64::max(scale * multiplier + spacing_spec.constant, scale * 0.75);
                if !margin_checker.in_bounds((x, y), spacing_radius) {
                    continue;
                }
                if !sectors.test_and_add(Collider {
                    position: (x, y),
                    radius: spacing_radius,
                }) {
                    continue;
                }
                primary_color = perturb_color(primary_color, primary_color_spec, rng);
                secondary_color = perturb_color(secondary_color, secondary_color_spec, rng);
                dest.push(Point {
                    position: (x, y),
                    scale,
                    primary_color,
                    secondary_color,
                    bullseye,
                })
            }

            // For the next line/dot-sequence, potentially change the color and scale
            scale_generator.change(rng); // NOTE: This doesn't update `scale` (?).
            if rng.odds(color_change_odds.line) {
                primary_color_idx =
                    pick_next_color(&color_scheme.primary_seq, primary_color_idx, rng);
                primary_color_spec = color_db
                    .color(color_scheme.primary_seq[primary_color_idx])
                    .expect("invalid color");
                bullseye = bullseye_generator.next(rng);
            }
            if rng.odds(color_change_odds.line) {
                secondary_color_idx =
                    pick_next_color(&color_scheme.secondary_seq, secondary_color_idx, rng);
                secondary_color_spec = color_db
                    .color(color_scheme.secondary_seq[secondary_color_idx])
                    .expect("invalid color");
            }
        }
    }
}

fn pick_next_color(seq: &[ColorKey], current_idx: usize, rng: &mut Rng) -> usize {
    // We pick the next color by taking a step forward or backward in
    // the color sequence from our current position. (And less often,
    // we take multiple steps.)
    let mut step: isize = *rng.wc(&[(1, 0.64), (2, 0.24), (3, 0.1), (4, 0.02)]);
    if rng.odds(0.5) {
        step *= -1;
    }
    (current_idx as isize + step).rem_euclid(seq.len() as isize) as usize
}

fn spec_to_color(
    key: ColorKey,
    spec: &ColorSpec,
    colors_used: &mut ColorsUsed,
    rng: &mut Rng,
) -> Hsb {
    colors_used.insert(key);
    let base = Hsb(spec.hue, spec.sat, spec.bright);
    perturb_color(base, spec, rng)
}

fn perturb_color(color: Hsb, spec: &ColorSpec, rng: &mut Rng) -> Hsb {
    let hue = modulo(
        rng.gauss(color.0, spec.hue_variance)
            .clamp(spec.hue_min, spec.hue_max),
        360.0,
    );
    let sat = rng
        .gauss(color.1, spec.sat_variance)
        .clamp(spec.sat_min, spec.sat_max);
    let bright = rng
        .gauss(color.2, spec.bright_variance)
        .clamp(spec.bright_min, spec.bright_max);
    Hsb(hue, sat, bright)
}

fn adjust_draw_radius(config: &Config, points: &mut [Point]) {
    if config.inflate_draw_radius {
        for p in points {
            p.scale = p.scale.max(w(0.00041));
        }
    }
}

/// For [`ColorMode::Stacked`] pieces, this is `Some((dx, dy))`; for other pieces, this is `None`.
#[derive(Debug, Copy, Clone)]
struct StackOffset(Option<(f64, f64)>);

impl StackOffset {
    fn build(traits: &Traits, rng: &mut Rng) -> Self {
        match traits.color_mode {
            ColorMode::Stacked => Self(Some((
                rng.gauss(0.0, w(0.0013)),
                rng.gauss(0.0, w(0.0013)).abs(),
            ))),
            _ => Self(None),
        }
    }
}

struct PaintCtx<PM: PaintMode> {
    maybe_draw_target: PM::DrawTarget,
    min_circle_steps: f64,
    viewport: VirtualViewport,
    /// Ratio mapping from "virtual space" (used for layout) to "raster space" (actual pixels on
    /// the output `DrawTarget`).
    scale_ratio: f32,
}

/// A viewport/crop specification in virtual canvas space, where the horizontal axis ranges from
/// `0.0` to `VIRTUAL_W` and the vertical axis ranges from `0.0` to `VIRTUAL_H`.
#[derive(Debug)]
struct VirtualViewport {
    left: f64,
    right: f64,
    top: f64,
    bottom: f64,
}

impl<'a> From<&'a FractionalViewport> for VirtualViewport {
    fn from(vp: &'a FractionalViewport) -> Self {
        Self {
            left: vp.left() * VIRTUAL_W,
            right: vp.right() * VIRTUAL_W,
            top: vp.top() * VIRTUAL_H,
            bottom: vp.bottom() * VIRTUAL_H,
        }
    }
}

/// Computes the actual output canvas dimensions given a viewport and the width of the full virtual
/// canvas (as passed to `--width`). E.g., if the viewport specifies a width of 25% and the full
/// width is 1000px, the output width will be 250px.
fn canvas_dimensions(fvp: &FractionalViewport, full_width: i32) -> (i32, i32) {
    let full_height = ((full_width as i64) * 5 / 4) as i32;
    let w = (f64::from(full_width) * fvp.width()).round() as i32;
    let h = (f64::from(full_height) * fvp.height()).round() as i32;
    (w, h)
}

impl<PM: PaintMode> PaintCtx<PM> {
    /// Assembles a new paint context.
    ///
    /// The given `FractionalViewport` overrides any viewport set in the `Config`.
    fn new(config: &Config, fvp: &FractionalViewport, canvas_width: i32) -> Self {
        let virtual_vp = VirtualViewport::from(fvp);

        let (w, h) = canvas_dimensions(fvp, canvas_width);
        let dt = PM::new_draw_target(w, h);

        let min_circle_steps = f64::max(8.0, config.min_circle_steps.unwrap_or(0) as f64);
        let scale_ratio = (canvas_width as f64 / VIRTUAL_W) as f32;

        Self {
            maybe_draw_target: dt,
            min_circle_steps,
            viewport: virtual_vp,
            scale_ratio,
        }
    }

    fn draw_target(&mut self) -> Option<&mut DrawTarget> {
        PM::as_draw_target(&mut self.maybe_draw_target)
    }
}

#[derive(Debug, Copy, Clone)]
enum Background {
    Transparent,
    Opaque,
}

mod paint_mode {
    use raqote::DrawTarget;

    pub trait PaintMode {
        type DrawTarget;
        type Components: Send;

        fn new_draw_target(width: i32, height: i32) -> Self::DrawTarget;
        fn as_draw_target(data: &mut Self::DrawTarget) -> Option<&mut DrawTarget>;

        fn decompose(dt: Self::DrawTarget) -> Self::Components;
        fn compose(dt: Self::Components) -> Self::DrawTarget;
        fn superimpose(dt: &mut Self::DrawTarget, components: &Self::Components, x: i32, y: i32);

        fn respect_chunks() -> bool;
    }

    /// Actually paint objects. This is the usual mode of operation.
    #[derive(Debug, Copy, Clone)]
    pub struct Paint;

    /// Do not paint. Used for advancing the RNG state.
    #[derive(Debug, Copy, Clone)]
    pub struct Skip;

    pub struct Components {
        width: i32,
        height: i32,
        data: Vec<u32>,
    }

    impl PaintMode for Paint {
        type DrawTarget = DrawTarget;
        type Components = Components;

        fn new_draw_target(width: i32, height: i32) -> Self::DrawTarget {
            DrawTarget::new(width, height)
        }
        fn as_draw_target(data: &mut DrawTarget) -> Option<&mut DrawTarget> {
            Some(data)
        }

        fn decompose(dt: Self::DrawTarget) -> Self::Components {
            Components {
                width: dt.width(),
                height: dt.height(),
                data: dt.into_inner(),
            }
        }
        fn compose(components: Self::Components) -> Self::DrawTarget {
            DrawTarget::from_backing(components.width, components.height, components.data)
        }
        fn superimpose(dt: &mut Self::DrawTarget, components: &Self::Components, x: i32, y: i32) {
            let img = raqote::Image {
                width: components.width,
                height: components.height,
                data: &components.data,
            };
            super::superimpose(dt, img, (x, y))
        }

        fn respect_chunks() -> bool {
            true
        }
    }

    impl PaintMode for Skip {
        type DrawTarget = ();
        type Components = ();

        fn new_draw_target(_width: i32, _height: i32) -> Self::DrawTarget {}
        fn as_draw_target(_data: &mut ()) -> Option<&mut DrawTarget> {
            None
        }

        fn decompose(_dt: Self::DrawTarget) -> Self::Components {}
        fn compose(_components: Self::Components) -> Self::DrawTarget {}
        fn superimpose(_dt: &mut Self::DrawTarget, _components: &Self::Components, _: i32, _: i32) {
        }

        fn respect_chunks() -> bool {
            false
        }
    }
}
use paint_mode::PaintMode;

/// Normal (non-splatter) points to be rendered, if any. If normal points are to be rendered, they
/// may generate splatter points, and the caller must specify what to do with those.
enum NormalPoints<'a> {
    Some {
        points: &'a [Point],
        splatter_sink: SplatterSink<'a>,
    },
    None,
}

/// As normal points are painted and generate splatter points, how should they be handled?
enum SplatterSink<'a> {
    /// Any generated splatter points should be painted immediately after all normal points are
    /// painted. This is the standard QQL behavior for a one-shot render.
    Immediate,
    /// Any generated splatter points should be pushed into a queue to be rendered later (for
    /// incremental rendering).
    Deferred(&'a mut Vec<Point>),
    /// Any generated splatter points should be ignored; entropy will not be consumed to compute
    /// their positions and they will not be drawn. This is equivalent to using `Deferred(_)` into
    /// a buffer that is dropped after the render completes.
    Ignored,
}

#[allow(clippy::too_many_arguments)]
fn render<PM: PaintMode>(
    canvas_width: i32,
    background: Background,
    traits: &Traits,
    color_db: &ColorDb,
    config: &Config,
    stack_offset: &StackOffset,
    mut normal_points: NormalPoints<'_>,
    extra_splatter_points: &[Point],
    color_scheme: &ColorScheme,
    colors_used: &mut ColorsUsed,
    rng: &mut Rng,
) -> PM::DrawTarget {
    let full_fvp = &config.viewport.as_ref().cloned().unwrap_or_default();

    let background_color = {
        let spec = color_db
            .color(color_scheme.background)
            .expect("invalid background");
        Hsb(spec.hue, spec.sat, spec.bright)
            .to_rgb()
            .to_solid_source()
    };

    let (hsteps, vsteps): (u32, u32) = if PM::respect_chunks() {
        (config.chunks.w.into(), config.chunks.h.into())
    } else {
        (1, 1)
    };
    let num_chunks: usize = (hsteps * vsteps).try_into().expect("too many chunks!");

    let canvas_dims = canvas_dimensions(full_fvp, canvas_width);
    let chunk_origin = |chunk_x: u32, chunk_y: u32| -> (i32, i32) {
        let (w, h) = canvas_dims;
        let x = f64::from(w) * (f64::from(chunk_x) / f64::from(hsteps));
        let y = f64::from(h) * (f64::from(chunk_y) / f64::from(vsteps));
        (x.round() as i32, y.round() as i32)
    };

    // Pull some `Sync` values off `normal_points`, just the bits that worker threads need.
    let (normal_points_slice, splatter_sink_immediate): (&[Point], bool) = match normal_points {
        NormalPoints::Some {
            points,
            ref splatter_sink,
        } => (points, matches!(splatter_sink, SplatterSink::Immediate)),
        NormalPoints::None => (&[], false),
    };

    struct Output<PM: PaintMode> {
        left_px: i32,
        top_px: i32,
        // `DrawTarget` is `!Send`, so we break it down into its components.
        dt_components: PM::Components,
        colors_used: ColorsUsed,
        splatter_points: Vec<Point>,
        rng: Rng,
    }
    let process_chunk = |x: u32, y: u32, mut rng: Rng| -> Output<PM> {
        let (left_px, top_px) = chunk_origin(x, y);
        let (right_px, bottom_px) = chunk_origin(x + 1, y + 1);
        let (width_px, height_px) = (right_px - left_px, bottom_px - top_px);
        eprintln!(
            "painting chunk ({}, {}): {}x{}+{}+{}px",
            x, y, width_px, height_px, left_px, top_px
        );
        let (width_ratio, height_ratio) = (
            full_fvp.width() / f64::from(canvas_dims.0),
            full_fvp.height() / f64::from(canvas_dims.1),
        );
        let fvp = FractionalViewport::from_whlt(
            f64::from(width_px) * width_ratio,
            f64::from(height_px) * height_ratio,
            f64::from(left_px) * width_ratio + full_fvp.left(),
            f64::from(top_px) * height_ratio + full_fvp.top(),
        );
        let mut pctx = PaintCtx::<PM>::new(config, &fvp, canvas_width);
        match (background, pctx.draw_target()) {
            (Background::Transparent, _) => (),
            (Background::Opaque, None) => (),
            (Background::Opaque, Some(dt)) => dt.clear(background_color),
        };
        let mut colors_used = ColorsUsed::new();
        let mut new_splatter_points = Vec::new();
        paint_normal_points(
            &mut pctx,
            traits,
            normal_points_slice,
            stack_offset,
            color_scheme,
            &mut new_splatter_points,
            &mut rng,
        );
        if splatter_sink_immediate {
            paint_splatter_points(
                &mut pctx,
                color_db,
                new_splatter_points.as_slice(),
                color_scheme,
                &mut colors_used,
                &mut rng,
            );
            new_splatter_points.clear();
        }
        paint_splatter_points(
            &mut pctx,
            color_db,
            extra_splatter_points,
            color_scheme,
            &mut colors_used,
            &mut rng,
        );
        Output {
            left_px,
            top_px,
            dt_components: PM::decompose(pctx.maybe_draw_target),
            colors_used,
            splatter_points: new_splatter_points,
            rng,
        }
    };
    let mut process_splatters = |new_splatters: &[Point]| match &mut normal_points {
        NormalPoints::None => (),
        NormalPoints::Some { splatter_sink, .. } => match splatter_sink {
            SplatterSink::Immediate => assert!(new_splatters.is_empty()),
            SplatterSink::Ignored => (),
            SplatterSink::Deferred(sink) => {
                sink.extend_from_slice(new_splatters);
            }
        },
    };

    // Skip compositing if there's only one chunk.
    if num_chunks == 1 {
        let output = process_chunk(0, 0, rng.clone());
        *rng = output.rng;
        process_splatters(&output.splatter_points);
        colors_used.extend(&output.colors_used);
        return PM::compose(output.dt_components);
    }

    // Otherwise, render each chunk in its own thread, compositing as we go on the main thread.
    let (tx_output, rx_output) = std::sync::mpsc::sync_channel::<Output<PM>>(num_chunks);
    std::thread::scope(|s| {
        for x in 0..hsteps {
            for y in 0..vsteps {
                let rng = rng.clone();
                let tx_output = tx_output.clone();
                s.spawn(move || {
                    let output = process_chunk(x, y, rng);
                    tx_output.send(output).unwrap();
                });
            }
        }
        drop(tx_output);

        let mut pctx_final = PaintCtx::<PM>::new(config, full_fvp, canvas_width);
        let mut chunks_composited = 0;
        let mut expected_splatter_points = None;
        while let Ok(output) = rx_output.recv() {
            if chunks_composited == 0 {
                *rng = output.rng;
                process_splatters(&output.splatter_points);
                expected_splatter_points = Some(output.splatter_points);
            } else {
                if output.rng != *rng {
                    panic!("rng state mismatch");
                }
                if cfg!(debug)
                    && output.splatter_points != *expected_splatter_points.as_ref().unwrap()
                {
                    panic!("new splatter points mismatch");
                }
            }
            PM::superimpose(
                &mut pctx_final.maybe_draw_target,
                &output.dt_components,
                output.left_px,
                output.top_px,
            );
            colors_used.extend(&output.colors_used);
            chunks_composited += 1;
        }
        assert_eq!(chunks_composited, num_chunks, "missing some chunks");
        pctx_final.maybe_draw_target
    })
}

fn paint_normal_points<PM: PaintMode>(
    pctx: &mut PaintCtx<PM>,
    traits: &Traits,
    points: &[Point],
    stack_offset: &StackOffset,
    color_scheme: &ColorScheme,
    splatter_points: &mut Vec<Point>,
    rng: &mut Rng,
) {
    let is_zebra = matches!(traits.color_mode, ColorMode::Zebra);
    for p in points {
        let (x, y) = p.position;

        // splatter is much more likely closer to the splatter center
        let splatter_odds_adjustment = f64::powf(
            rescale(
                dist(p.position, color_scheme.splatter_center),
                (0.0, w(1.4)),
                (1.0, 0.0),
            ),
            2.5,
        );
        if rng.odds(color_scheme.splatter_odds * splatter_odds_adjustment) {
            splatter_points.push(p.clone());
        }
        if let Some((xoff, yoff)) = stack_offset.0 {
            draw_ring_dot(
                &Point {
                    position: (x + xoff, y + yoff),
                    primary_color: p.secondary_color,
                    bullseye: Bullseye {
                        density: p.bullseye.density * rng.gauss(0.99, 0.03),
                        rings: p.bullseye.rings,
                    },
                    ..p.clone()
                },
                pctx,
                rng,
            );
        }
        if is_zebra {
            draw_ring_dot(p, pctx, rng);
        } else {
            let mut p = p.clone();
            p.secondary_color = p.primary_color;
            draw_ring_dot(&p, pctx, rng);
        }
    }
}

fn paint_splatter_points<PM: PaintMode>(
    pctx: &mut PaintCtx<PM>,
    color_db: &ColorDb,
    splatter_points: &[Point],
    color_scheme: &ColorScheme,
    colors_used: &mut ColorsUsed,
    rng: &mut Rng,
) {
    for p in splatter_points {
        let splatter_color = *rng.choice(&color_scheme.splatter_choices);
        let final_color = spec_to_color(
            splatter_color,
            color_db.color(splatter_color).expect("invalid color"),
            colors_used,
            rng,
        );
        let mut p = p.clone();
        p.primary_color = final_color;
        p.secondary_color = final_color;
        p.bullseye.density = f64::max(0.17, p.bullseye.density * 0.7);
        draw_ring_dot(&p, pctx, rng);
    }
}

fn draw_ring_dot<PM: PaintMode>(pt: &Point, pctx: &mut PaintCtx<PM>, rng: &mut Rng) {
    let num_rings = pt.num_drawn_rings();
    let band_step = pt.scale / num_rings as f64;

    // lower fill density results in higher thickness
    let band_thickness = w(0.0004).max(band_step * (1.0 - pt.bullseye.density));

    // when we have more rings, there is less room to shift them around
    let variance_adjust = rescale(pt.bullseye.density, (0.1, 1.0), (0.5, 1.2));
    let position_variance = if num_rings >= 7 {
        variance_adjust * rescale(num_rings as f64, (7.0, 9.0), (0.008, 0.005))
    } else {
        variance_adjust * rescale(num_rings as f64, (1.0, 7.0), (0.022, 0.008))
    };

    let mut band_num = 0;
    let mut r = pt.scale;
    while r > w(0.0004) {
        let color = if band_num % 2 == 0 {
            pt.primary_color
        } else {
            pt.secondary_color
        };
        band_num += 1;

        let band_center_x = rng.gauss(pt.position.0, w(0.0005).min(r * position_variance));
        let band_center_y = rng.gauss(pt.position.1, w(0.0005).min(r * position_variance));

        let thickness_variance = rescale(pt.bullseye.density, (0.1, 1.0), (0.01, 0.13));
        let mut final_thickness = rng.gauss(band_thickness, band_thickness * thickness_variance);
        if r < w(0.002) && num_rings == 1 {
            final_thickness = rescale(pt.bullseye.density, (0.0, 1.0), (r, r * 0.25));
        }

        // avoid super fat, large "donuts"
        if num_rings == 1 && pt.scale > w(0.02) {
            final_thickness = rescale(final_thickness, (w(0.003), w(0.08)), (w(0.003), w(0.05)));
            final_thickness = final_thickness
                .min(rescale(pt.scale, (0.0, w(0.1)), (w(0.003), w(0.04))))
                .min(w(0.04));
        }

        draw_messy_circle(
            (band_center_x, band_center_y),
            r,
            final_thickness,
            variance_adjust,
            color,
            pctx,
            rng,
        );

        r -= band_step;
    }
}

fn draw_messy_circle<PM: PaintMode>(
    (x, y): (f64, f64),
    r: f64,
    thickness: f64,
    variance_adjust: f64,
    color: Hsb,
    pctx: &mut PaintCtx<PM>,
    rng: &mut Rng,
) {
    let source = color.to_rgb().to_source();

    let num_rounds_divisor = if thickness > w(0.02) {
        rescale(thickness, (w(0.02), w(0.04)), (w(0.00021), w(0.00022)))
    } else if thickness > w(0.006) {
        rescale(thickness, (w(0.006), w(0.02)), (w(0.00015), w(0.00021)))
    } else if thickness > w(0.003) {
        rescale(thickness, (w(0.003), w(0.006)), (w(0.00012), w(0.00015)))
    } else {
        rescale(thickness, (0.0, w(0.006)), (w(0.00016), w(0.00012)))
    };
    let num_rounds = f64::ceil((thickness / num_rounds_divisor).max(1.0)) as usize;

    for i in 0..num_rounds {
        let r = rescale(i as f64, (0.0, num_rounds as f64), (r, r - thickness));
        let variance_ratio =
            variance_adjust * rescale(thickness, (w(0.001), w(0.04)), (0.08, 0.03));
        let mut position_variance = variance_adjust * w(0.0015).min(thickness * variance_ratio);
        let mut thickness_variance_multiplier = 1.0;
        if i < 5 {
            position_variance *= 1.5;
            thickness_variance_multiplier = 2.0;
        }
        let (x, y) = (
            rng.gauss(x, position_variance),
            rng.gauss(y, position_variance),
        );

        let mean_thickness = if thickness > w(0.02) {
            rescale(thickness, (w(0.02), w(0.04)), (w(0.0007), w(0.00073)))
        } else if thickness > w(0.006) {
            rescale(thickness, (w(0.006), w(0.02)), (w(0.0005), w(0.0007)))
        } else {
            rescale(thickness, (w(0.001), w(0.006)), (w(0.0001), w(0.0005)))
        };
        let thickness_variance_factor =
            thickness_variance_multiplier * rescale(thickness, (w(0.001), w(0.04)), (0.25, 1.1));
        let mut single_line_variance = mean_thickness * thickness_variance_factor;
        if r < w(0.002) {
            single_line_variance = mean_thickness * 0.1;
        }
        let thickness = rng
            .gauss(mean_thickness, single_line_variance)
            .max(w(0.0002));
        draw_clean_circle((x, y), r, thickness, 0.007, &source, pctx, rng);
    }
}

fn draw_clean_circle<PM: PaintMode>(
    (x, y): (f64, f64),
    r: f64,
    thickness: f64,
    eccentricity: f64,
    source: &Source,
    pctx: &mut PaintCtx<PM>,
    rng: &mut Rng,
) {
    let r = (r - thickness * 0.5).max(w(0.0002));
    let stroke_weight = thickness * 0.95;

    let variance = w(0.0015).min(r * eccentricity);
    let rx = rng.gauss(r, variance);
    let ry = rng.gauss(r, variance);

    // The JavaScript algorithm computes an unused `startingTheta = rng.uniform(0.0, pi(2.0))`.
    // We don't need to compute that, but we need to burn a uniform deviate to keep RNG synced.
    rng.rnd();

    if let Some(dt) = PM::as_draw_target(&mut pctx.maybe_draw_target) {
        if x + (rx + stroke_weight / 2.0) < pctx.viewport.left
            || x - (rx + stroke_weight / 2.0) > pctx.viewport.right
            || y + (ry + stroke_weight / 2.0) < pctx.viewport.top
            || y - (ry + stroke_weight / 2.0) > pctx.viewport.bottom
        {
            // Circle is entirely outside viewport; skip painting it. There are no more stateful RNG
            // calls past this point, so we can bail entirely, skipping `dt.stroke` (vast majority of
            // time spent) and also the trigonometric functions (not nearly as expensive but do show up
            // on the profile).
            return;
        }

        let num_steps = (r * pi(2.0) / w(0.0005)).max(pctx.min_circle_steps);
        let step = pi(2.0) / num_steps;

        let mut pb = PathBuilder::new();
        let mut theta = 0.0;
        while theta < pi(2.0) {
            let x = (x - pctx.viewport.left + rx * theta.cos()) as f32 * pctx.scale_ratio;
            let y = (y - pctx.viewport.top + ry * theta.sin()) as f32 * pctx.scale_ratio;
            if theta == 0.0 {
                pb.move_to(x, y);
            } else {
                pb.line_to(x, y);
            }
            theta += step;
        }
        pb.close();
        let path = pb.finish();

        dt.stroke(
            &path,
            source,
            &StrokeStyle {
                width: stroke_weight as f32 * pctx.scale_ratio,
                ..StrokeStyle::default()
            },
            &DrawOptions::new(),
        );
    }
}

fn as_image(dt: &DrawTarget) -> raqote::Image {
    raqote::Image {
        width: dt.width(),
        height: dt.height(),
        data: dt.get_data(),
    }
}
fn superimpose(onto: &mut DrawTarget, layer: raqote::Image, (x, y): (i32, i32)) {
    onto.draw_image_at(x as f32, y as f32, &layer, &DrawOptions::new());
}

pub struct Frame<'a> {
    pub dt: &'a DrawTarget,
    pub number: Option<u32>,
}
pub struct RenderData {
    pub canvas: DrawTarget,
    pub num_points: usize,
    pub colors_used: ColorsUsed,
    pub ring_counts_used: BTreeMap<RingCount, usize>,
}

pub fn draw<F: FnMut(Frame)>(
    seed: &[u8; 32],
    color_db: &ColorDb,
    config: &Config,
    canvas_width: i32,
    mut consume_frame: F,
) -> RenderData {
    let traits = Traits::from_seed(seed);
    let mut rng = Rng::from_seed(&seed[..]);
    eprintln!("initialized traits");

    let flow_field_spec = FlowFieldSpec::from_traits(&traits, &mut rng);
    let spacing_spec = SpacingSpec::from_traits(&traits, &mut rng);
    let color_change_odds = ColorChangeOdds::from_traits(&traits, &mut rng);
    let mut scale_generator = ScaleGenerator::from_traits(&traits, &mut rng);
    let mut bullseye_generator = BullseyeGenerator::from_traits(&traits, &mut rng);
    let color_scheme = ColorScheme::from_traits(&traits, color_db, &mut rng);

    let flow_field = FlowField::build(&flow_field_spec, &traits, &mut rng);
    eprintln!("built flow field");
    let ignore_flow_field = IgnoreFlowField::build(&flow_field_spec, &mut rng);
    let start_points = StartPointGroups::build(traits.structure, &mut rng);

    let grouped_flow_lines =
        GroupedFlowLines::build(flow_field, ignore_flow_field, start_points, &mut rng);
    let mut sectors: Sectors = build_sectors(config);
    let mut colors_used = ColorsUsed::new();
    let (mut points, group_sizes) = Points::build(
        &traits,
        color_db,
        grouped_flow_lines,
        &color_scheme,
        &color_change_odds,
        &spacing_spec,
        &mut bullseye_generator,
        &mut scale_generator,
        &mut sectors,
        &mut colors_used,
        &mut rng,
    );
    eprintln!("laid out points");
    let num_points = points.0.len();
    let mut ring_counts_used = BTreeMap::new();
    for pt in &points.0 {
        *ring_counts_used.entry(pt.num_drawn_rings()).or_default() += 1;
    }

    let () = adjust_draw_radius(config, points.0.as_mut_slice());
    let stack_offset = StackOffset::build(&traits, &mut rng);

    let batch_sizes = match config.animate {
        Animation::None => None,
        Animation::Groups => Some(group_sizes.0),
        Animation::Points { step } => {
            let step = step as usize;
            let (n_groups, remainder) = (num_points / step, num_points % step);
            let mut result = vec![step; n_groups];
            if remainder > 0 {
                result.push(remainder);
            }
            Some(result)
        }
    };

    let dt = match batch_sizes {
        None => {
            let dt = render::<paint_mode::Paint>(
                canvas_width,
                Background::Opaque,
                &traits,
                color_db,
                config,
                &stack_offset,
                NormalPoints::Some {
                    points: points.0.as_slice(),
                    splatter_sink: SplatterSink::Immediate,
                },
                &[], // no extra splatter points
                &color_scheme,
                &mut colors_used,
                &mut rng,
            );
            consume_frame(Frame {
                dt: &dt,
                number: None,
            });
            eprintln!("drew points");
            dt
        }

        Some(batch_sizes) => {
            let old_rng = rng.clone();
            // For the first frame, render just the background.
            let mut fb = render::<paint_mode::Paint>(
                canvas_width,
                Background::Opaque,
                &traits,
                color_db,
                config,
                &stack_offset,
                NormalPoints::None,
                &[], // no extra splatter points
                &color_scheme,
                &mut colors_used,
                &mut rng,
            );
            if old_rng != rng {
                panic!("painting background changed rng");
            }
            // https://github.com/rust-lang/rust-clippy/issues/11650
            #[allow(clippy::drop_non_drop)]
            drop(old_rng);

            struct EagerSplatters {
                /// A transparent-background frame buffer containing all splatter points that have
                /// been painted so far.
                layer: DrawTarget,
                /// A spare canvas that can be used for compositing at each frame emission.
                output_buf: DrawTarget,
                /// The RNG state after all normal points and after any splatter points that have
                /// been painted so far.
                rng: Rng,
                /// Colors used by splatters only. Saved separately to preserve iteration order.
                colors_used: ColorsUsed,
            }
            enum Splatters {
                Eager(EagerSplatters),
                Deferred(Vec<Point>),
            }
            let mut splatters = if config.splatter_immediately {
                let layer = DrawTarget::new(fb.width(), fb.height());
                let output_buf = DrawTarget::new(fb.width(), fb.height());
                // Compute output state by pre-rendering all the normal points.
                eprintln!("pre-rendering normal points to seek for splatter state");
                let mut rng = rng.clone();
                render::<paint_mode::Skip>(
                    canvas_width,
                    Background::Transparent,
                    &traits,
                    color_db,
                    config,
                    &stack_offset,
                    NormalPoints::Some {
                        points: points.0.as_slice(),
                        splatter_sink: SplatterSink::Ignored,
                    },
                    &[], // no extra splatter points
                    &color_scheme,
                    &mut ColorsUsed::new(),
                    &mut rng,
                );
                Splatters::Eager(EagerSplatters {
                    layer,
                    output_buf,
                    rng,
                    colors_used: ColorsUsed::new(),
                })
            } else {
                Splatters::Deferred(Vec::new())
            };

            let mut frame_number = 0;
            consume_frame(Frame {
                dt: &fb,
                number: Some(frame_number),
            });
            frame_number += 1;

            let mut emit_incremental_frame =
                |layer: &DrawTarget, splatters: Option<&mut EagerSplatters>| {
                    assert_eq!((layer.width(), layer.height()), (fb.width(), fb.height()));
                    superimpose(&mut fb, as_image(layer), (0, 0));
                    let buf = match splatters {
                        None => &mut fb,
                        Some(splatters) => {
                            let buf = &mut splatters.output_buf;
                            buf.get_data_mut().copy_from_slice(fb.get_data());
                            superimpose(buf, as_image(&splatters.layer), (0, 0));
                            buf
                        }
                    };
                    consume_frame(Frame {
                        dt: buf,
                        number: Some(frame_number),
                    });
                    frame_number += 1;
                };

            let mut points = points.0.as_slice();
            for size in batch_sizes {
                let (batch, rest) = points.split_at(size);
                match &mut splatters {
                    Splatters::Deferred(splatter_points) => {
                        let dt = render::<paint_mode::Paint>(
                            canvas_width,
                            Background::Transparent,
                            &traits,
                            color_db,
                            config,
                            &stack_offset,
                            NormalPoints::Some {
                                points: batch,
                                splatter_sink: SplatterSink::Deferred(splatter_points),
                            },
                            &[], // no extra splatter points
                            &color_scheme,
                            &mut colors_used,
                            &mut rng,
                        );
                        emit_incremental_frame(&dt, None);
                    }
                    Splatters::Eager(splatters) => {
                        let mut these_splatters = Vec::new();
                        let normal_layer = render::<paint_mode::Paint>(
                            canvas_width,
                            Background::Transparent,
                            &traits,
                            color_db,
                            config,
                            &stack_offset,
                            NormalPoints::Some {
                                points: batch,
                                splatter_sink: SplatterSink::Deferred(&mut these_splatters),
                            },
                            &[], // no extra splatter points
                            &color_scheme,
                            &mut colors_used,
                            &mut rng,
                        );
                        let splatter_layer = render::<paint_mode::Paint>(
                            canvas_width,
                            Background::Transparent,
                            &traits,
                            color_db,
                            config,
                            &stack_offset,
                            NormalPoints::None,
                            &these_splatters,
                            &color_scheme,
                            &mut splatters.colors_used,
                            &mut splatters.rng,
                        );
                        superimpose(&mut splatters.layer, as_image(&splatter_layer), (0, 0));
                        emit_incremental_frame(&normal_layer, Some(splatters));
                    }
                }
                points = rest;
            }

            // Finish processing splatters: either render them all if they were deferred, or
            // register their colors used now that we've gotten all the normal points' colors.
            match splatters {
                Splatters::Eager(splatters) => colors_used.extend(&splatters.colors_used),
                Splatters::Deferred(splatter_points) => {
                    let dt = render::<paint_mode::Paint>(
                        canvas_width,
                        Background::Transparent,
                        &traits,
                        color_db,
                        config,
                        &stack_offset,
                        NormalPoints::None,
                        splatter_points.as_slice(),
                        &color_scheme,
                        &mut colors_used,
                        &mut rng,
                    );
                    emit_incremental_frame(&dt, None);
                }
            }
            fb
        }
    };

    RenderData {
        canvas: dt,
        num_points,
        colors_used,
        ring_counts_used,
    }
}

#[cfg(test)]
mod test {
    use super::*;

    /// Tests that the hard-coded values for [`SPC`], [`FLOW_FIELD_ROWS`], and [`FLOW_FIELD_COLS`]
    /// match the computed values used (either implicitly or explicitly) in the JavaScript algorithm.
    /// These are hard-coded because we can't use [`f64::floor`] and friends in a const context.
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

    #[test]
    fn test_spec_to_color() {
        let key: ColorKey = 123;
        let spec = ColorSpec {
            name: "Austin Blue".to_string(),
            hue: 205.0,
            hue_min: 203.0,
            hue_max: 207.0,
            hue_variance: 1.0,
            sat: 85.0,
            sat_min: 83.0,
            sat_max: 87.0,
            sat_variance: 1.0,
            bright: 70.0,
            bright_min: 68.0,
            bright_max: 72.0,
            bright_variance: 1.0,
        };
        let mut colors_used: ColorsUsed = ColorsUsed::new();
        let mut rng = Rng::from_seed(&[]);

        let color = spec_to_color(key, &spec, &mut colors_used, &mut rng);
        let expected = Hsb(206.1738113319151, 84.77782201272913, 70.8832072948715);

        assert_eq!(colors_used.as_slice(), &[key]);
        assert_eq!(color, expected);
    }
}

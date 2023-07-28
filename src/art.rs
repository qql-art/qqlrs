use std::collections::HashSet;

use raqote::{DrawOptions, DrawTarget, PathBuilder, SolidSource, Source, StrokeStyle};

use super::color::{ColorDb, ColorKey, ColorSpec};
use super::config::Config;
use super::layouts::StartPointGroups;
use super::math::{angle, cos, dist, modulo, pi, rescale, sin};
use super::rand::Rng;
use super::sectors::{Collider, Sectors};
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

#[derive(Debug, Clone)]
pub struct Point {
    pub position: (f64, f64),
    pub scale: f64,
    pub primary_color: Hsb,
    pub secondary_color: Hsb,
    pub bullseye: Bullseye,
}
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Hsb(pub f64, pub f64, pub f64);
pub struct Points(pub Vec<Point>);

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
    pub fn to_source(self) -> Source<'static> {
        Source::Solid(SolidSource {
            r: self.0 as u8,
            g: self.1 as u8,
            b: self.2 as u8,
            a: 255,
        })
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
        colors_used: &mut HashSet<ColorKey>,
        rng: &mut Rng,
    ) -> Points {
        fn random_idx(len: usize, rng: &mut Rng) -> usize {
            rng.uniform(0.0, len as f64) as usize
        }
        let mut primary_color_idx = random_idx(color_scheme.primary_seq.len(), rng);
        let mut secondary_color_idx = random_idx(color_scheme.secondary_seq.len(), rng);
        let mut base_bullseye_spec = bullseye_generator.next(rng);

        let margin_checker = MarginChecker::from_traits(traits);

        let mut all_points = Vec::new();
        for group in grouped_flow_lines.0 {
            if rng.odds(color_change_odds.group) {
                primary_color_idx =
                    pick_next_color(&color_scheme.primary_seq, primary_color_idx, rng);
                secondary_color_idx =
                    pick_next_color(&color_scheme.secondary_seq, secondary_color_idx, rng);
                base_bullseye_spec = bullseye_generator.next(rng);
            }
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
        }
        Points(all_points)
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
        colors_used: &mut HashSet<ColorKey>,
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
    colors_used: &mut HashSet<ColorKey>,
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

struct PaintCtx {
    dt: DrawTarget,
    min_circle_steps: f64,
}

impl PaintCtx {
    fn new(config: &Config, canvas_width: i32) -> Self {
        let canvas_height = canvas_width * 5 / 4;
        let dt = DrawTarget::new(canvas_width, canvas_height);

        let min_circle_steps = f64::max(8.0, config.min_circle_steps.unwrap_or(0) as f64);

        Self {
            dt,
            min_circle_steps,
        }
    }
}

fn paint(
    canvas_width: i32,
    traits: &Traits,
    color_db: &ColorDb,
    config: &Config,
    points: Points,
    color_scheme: &ColorScheme,
    colors_used: &mut HashSet<ColorKey>,
    rng: &mut Rng,
) -> DrawTarget {
    let mut pctx = PaintCtx::new(config, canvas_width);
    pctx.dt.fill_rect(
        0.0,
        0.0,
        pctx.dt.width() as f32,
        pctx.dt.height() as f32,
        {
            let spec = color_db
                .color(color_scheme.background)
                .expect("invalid background");
            &Hsb(spec.hue, spec.sat, spec.bright).to_rgb().to_source()
        },
        &DrawOptions::new(),
    );

    let stack_offset = match traits.color_mode {
        ColorMode::Stacked => Some((rng.gauss(0.0, w(0.0013)), rng.gauss(0.0, w(0.0013)).abs())),
        _ => None,
    };
    let is_zebra = matches!(traits.color_mode, ColorMode::Zebra);

    let mut splatter_points = Vec::new();

    for mut p in points.0 {
        let (x, y) = p.position;
        if config.inflate_draw_radius {
            p.scale = p.scale.max(w(0.00041));
        }

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
        if let Some((xoff, yoff)) = stack_offset {
            draw_ring_dot(
                &Point {
                    position: (x + xoff, y + yoff),
                    primary_color: p.secondary_color,
                    bullseye: Bullseye {
                        density: p.bullseye.density * rng.gauss(0.99, 0.03),
                        rings: p.bullseye.rings,
                    },
                    ..p
                },
                &mut pctx,
                rng,
            );
        }
        if !is_zebra {
            p.secondary_color = p.primary_color;
        }
        draw_ring_dot(&p, &mut pctx, rng);
    }

    for mut p in splatter_points {
        let splatter_color = *rng.choice(&color_scheme.splatter_choices);
        let final_color = spec_to_color(
            splatter_color,
            color_db.color(splatter_color).expect("invalid color"),
            colors_used,
            rng,
        );
        p.primary_color = final_color;
        p.secondary_color = final_color;
        p.bullseye.density = f64::max(0.17, p.bullseye.density * 0.7);
        draw_ring_dot(&p, &mut pctx, rng);
    }
    pctx.dt
}

fn draw_ring_dot(pt: &Point, pctx: &mut PaintCtx, rng: &mut Rng) {
    let mut num_rings = pt.bullseye.rings;
    // TODO(wchargin): Simplify under the assumption that `num_rings` starts as 1, 2, 3, or 7.
    if pt.scale < rescale(pt.bullseye.density, (0.15, 1.0), (w(0.0039), w(0.001))) {
        num_rings = num_rings.min(1);
    } else if pt.scale < rescale(pt.bullseye.density, (0.15, 1.0), (w(0.0072), w(0.0029))) {
        num_rings = num_rings.min(2);
    } else if pt.scale < w(0.01) {
        num_rings = num_rings.min(3);
    } else if pt.scale < w(0.012) {
        num_rings = num_rings.min(4);
    } else if pt.scale < w(0.014) {
        num_rings = num_rings.min(5);
    } else if pt.scale < w(0.017) {
        num_rings = num_rings.min(6);
    } else if pt.scale < w(0.02) {
        num_rings = num_rings.min(7);
    } else if pt.scale < w(0.023) {
        num_rings = num_rings.min(8);
    }
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

fn draw_messy_circle(
    (x, y): (f64, f64),
    r: f64,
    thickness: f64,
    variance_adjust: f64,
    color: Hsb,
    pctx: &mut PaintCtx,
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

fn draw_clean_circle(
    (x, y): (f64, f64),
    r: f64,
    thickness: f64,
    eccentricity: f64,
    source: &Source,
    pctx: &mut PaintCtx,
    rng: &mut Rng,
) {
    let dt = &mut pctx.dt;

    let r = (r - thickness * 0.5).max(w(0.0002));
    let stroke_weight = (thickness * 0.95 * dt.width() as f64 / VIRTUAL_W) as f32;

    let variance = w(0.0015).min(r * eccentricity);
    let rx = rng.gauss(r, variance);
    let ry = rng.gauss(r, variance);

    // The JavaScript algorithm computes an unused `startingTheta = rng.uniform(0.0, pi(2.0))`.
    // We don't need to compute that, but we need to burn a uniform deviate to keep RNG synced.
    rng.rnd();
    let num_steps = (r * pi(2.0) / w(0.0005)).max(pctx.min_circle_steps);
    let step = pi(2.0) / num_steps;

    let mut pb = PathBuilder::new();
    let mut theta = 0.0;
    while theta < pi(2.0) {
        let wr = dt.width() as f32 / VIRTUAL_W as f32;
        let x = (x + rx * theta.cos()) as f32 * wr;
        let y = (y + ry * theta.sin()) as f32 * wr;
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
            width: stroke_weight,
            ..StrokeStyle::default()
        },
        &DrawOptions::new(),
    );
}

pub struct Render {
    pub dt: DrawTarget,
    pub num_points: u64,
    pub colors_used: HashSet<ColorKey>,
}

pub fn draw(seed: &[u8; 32], color_db: &ColorDb, config: &Config, canvas_width: i32) -> Render {
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
    let mut sectors: Sectors = build_sectors(&config);
    let mut colors_used: HashSet<ColorKey> = HashSet::new();
    let points = Points::build(
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
    let num_points = points.0.len() as u64;
    let dt = paint(
        canvas_width,
        &traits,
        color_db,
        &config,
        points,
        &color_scheme,
        &mut colors_used,
        &mut rng,
    );
    eprintln!("drew points");
    Render {
        dt,
        num_points,
        colors_used,
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
        let mut colors_used: HashSet<ColorKey> = HashSet::new();
        let mut rng = Rng::from_seed(&[]);

        let color = spec_to_color(key, &spec, &mut colors_used, &mut rng);
        let expected = Hsb(206.1738113319151, 84.77782201272913, 70.8832072948715);

        assert_eq!(colors_used, HashSet::from([key]));
        assert_eq!(color, expected);
    }
}

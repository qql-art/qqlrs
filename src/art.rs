use super::math::{modulo, pi};
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

pub fn draw(seed: &[u8; 32]) {
    let mut rng = Rng::from_seed(&seed[..]);
    let traits = Traits::from_seed(seed);
    let flow_field_spec = FlowFieldSpec::from_traits(&traits, &mut rng);
    let spacing_spec = SpacingSpec::from_traits(&traits, &mut rng);
    let color_change_odds = ColorChangeOdds::from_traits(&traits, &mut rng);
    dbg!(traits, flow_field_spec, spacing_spec, color_change_odds); // TODO
}

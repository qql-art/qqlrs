use std::f64::consts::PI;

#[inline(always)]
pub fn pi(v: f64) -> f64 {
    PI * v
}

pub fn modulo(n: f64, m: f64) -> f64 {
    // Properly, this would be just `n.rem_euclid(m)`, but the QQL implementation behaves
    // differently when *both* `n` and `m` are negative. Out of an abundance of caution, we copy
    // that implementation, even though it's slower.
    ((n % m) + m) % m
}

pub fn rescale(value: f64, (old_min, old_max): (f64, f64), (new_min, new_max): (f64, f64)) -> f64 {
    let clamped = value.clamp(old_min, old_max);
    let old_spread = old_max - old_min;
    let new_spread = new_max - new_min;
    new_min + (clamped - old_min) * (new_spread / old_spread)
}

/// If `table` is the image of `linspace(min, max, _)` under a function `f` whose period divides
/// `max - min`, then `interpolate(table, min, max, z)` approximates `f(z)`.
fn interpolate(table: &[f64], min: f64, max: f64, z: f64) -> f64 {
    // Coerce value to [min, max) assuming periodicity.
    let value = modulo(z - min, max - min) + min;

    let rescaled = rescale(value, (min, max), (0.0, (table.len() - 1) as f64));
    let index = rescaled.floor() as usize; // This is within [0, table.length - 1).
    let fraction = rescaled - index as f64; // This is within [0, 1).

    // Function evaluated at value is within [start, end) based on index.
    let start = table[index];
    let end = table[index + 1];

    // Interpolate within [start, end) using fractional part.
    start + (end - start) * fraction
}

/// Piecewise-linear approximation to [`f64::cos`].
pub fn cos(z: f64) -> f64 {
    const TABLE: &[f64] = &[
        1.0, 0.99179, 0.96729, 0.92692, 0.87132, 0.80141, 0.71835, 0.62349, 0.51839, 0.40478,
        0.28453, 0.1596, 0.03205, -0.09602, -0.22252, -0.34537, -0.46254, -0.57212, -0.6723,
        -0.76145, -0.83809, -0.90097, -0.94906, -0.98156, -0.99795, -0.99795, -0.98156, -0.94906,
        -0.90097, -0.83809, -0.76145, -0.6723, -0.57212, -0.46254, -0.34537, -0.22252, -0.09602,
        0.03205, 0.1596, 0.28453, 0.40478, 0.51839, 0.62349, 0.71835, 0.80141, 0.87132, 0.92692,
        0.96729, 0.99179, 1.0,
    ];
    interpolate(TABLE, 0.0, 2.0 * PI, z)
}

/// Piecewise-linear approximation to [`f64::sin`].
pub fn sin(z: f64) -> f64 {
    const TABLE: &[f64] = &[
        0.0, 0.12788, 0.25365, 0.37527, 0.49072, 0.59811, 0.69568, 0.78183, 0.85514, 0.91441,
        0.95867, 0.98718, 0.99949, 0.99538, 0.97493, 0.93847, 0.8866, 0.82017, 0.74028, 0.64823,
        0.54553, 0.43388, 0.31511, 0.19116, 0.06407, -0.06407, -0.19116, -0.31511, -0.43388,
        -0.54553, -0.64823, -0.74028, -0.82017, -0.8866, -0.93847, -0.97493, -0.99538, -0.99949,
        -0.98718, -0.95867, -0.91441, -0.85514, -0.78183, -0.69568, -0.59811, -0.49072, -0.37527,
        -0.25365, -0.12788, -0.0,
    ];
    interpolate(TABLE, 0.0, 2.0 * PI, z)
}

/// Approximation to [`f64::sqrt`] using Newton's method with fixed convergence parameters.
fn sqrt(value: f64) -> f64 {
    const MAX_ITERATIONS: usize = 1000;
    const EPSILON: f64 = 1e-14;
    const TARGET: f64 = 1e-7;

    if value < 0.0 {
        panic!("argument to sqrt must be non-negative");
    }

    let mut guess = value;
    for _ in 0..MAX_ITERATIONS {
        let error = guess * guess - value;
        if error.abs() < TARGET {
            return guess;
        }
        let divisor = 2.0 * guess;
        if divisor <= EPSILON {
            return guess;
        }
        guess -= error / divisor;
    }
    guess
}

/// Computes the distance between two points.
///
/// Approximately like [`f64::hypot(x2 - x1, y2 - y1)`][f64::hypot].
pub fn dist((x1, y1): (f64, f64), (x2, y2): (f64, f64)) -> f64 {
    let dx = x1 - x2;
    let dy = y1 - y2;
    sqrt(dx * dx + dy * dy)
}

/// Fast lower-bound approximation of [`dist`].
///
/// **WARNING:** In the JavaScript source, the function `distLowerBound` actually implements an
/// upper bound, and vice versa. This implementation of `dist_lower_bound` corresponds to the
/// JavaScript function `distUpperBound`.
pub fn dist_lower_bound((x1, y1): (f64, f64), (x2, y2): (f64, f64)) -> f64 {
    let dx = (x1 - x2).abs();
    let dy = (y1 - y2).abs();
    let min = f64::min(dx, dy);
    let max = f64::max(dx, dy);

    let alpha = 1007.0 / 1110.0;
    let beta = 441.0 / 1110.0;

    alpha * max + beta * min
}

/// Fast upper-bound approximation of [`dist`].
///
/// **WARNING:** In the JavaScript source, the function `distUpperBound` actually implements a
/// lower bound, and vice versa. This implementation of `dist_upper_bound` corresponds to the
/// JavaScript function `distLowerBound`.
pub fn dist_upper_bound((x1, y1): (f64, f64), (x2, y2): (f64, f64)) -> f64 {
    let dx = (x1 - x2).abs();
    let dy = (y1 - y2).abs();
    let min = f64::min(dx, dy);
    let max = f64::max(dx, dy);

    let beta = 441.0 / 1024.0;

    max + beta * min
}

/// "Fast" atan2 implementation using a polynomial approximation.
/// Adapted from <https://stackoverflow.com/questions/46210708>.
fn atan2(y: f64, x: f64) -> f64 {
    let ax = x.abs();
    let ay = y.abs();
    let mx = f64::max(ay, ax);
    let mn = f64::min(ay, ax);
    let a = mn / mx;

    // Minimax polynomial approximation to atan(a) on [0,1]
    let s = a * a;
    let c = s * a;
    let q = s * s;
    let mut r = 0.024840285 * q + 0.18681418;
    let t = -0.094097948 * q - 0.33213072;
    r = r * s + t;
    r = r * c + a;

    // Map to full circle
    if ay > ax {
        r = 1.57079637 - r;
    }
    if x < 0.0 {
        r = 3.14159274 - r;
    }
    if y < 0.0 {
        r = -r;
    }
    r
}

/// Computes the angle from `(x1, y1)` to `(x2, y2)`, as a value in radians from 0 to 2π.
pub fn angle((x1, y1): (f64, f64), (x2, y2): (f64, f64)) -> f64 {
    let a = atan2(y2 - y1, x2 - x1);
    modulo(a, pi(2.0))
}

pub fn add_polar_offset((x, y): (f64, f64), theta: f64, r: f64) -> (f64, f64) {
    (x + r * cos(theta), y + r * sin(theta))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pi() {
        assert_eq!(pi(0.0), 0.0);
        assert_eq!(pi(1.0), PI);
        assert_eq!(pi(-3.7), -3.7 * PI);
        assert!(pi(f64::NAN).is_nan());
    }

    #[test]
    fn test_modulo() {
        let a = 4.0;
        let b = 3.0;
        let z = a % b;

        assert_eq!(modulo(a, b), z);
        assert_eq!(modulo(a + 10.0 * b, b), z);
        assert_eq!(modulo(a - 10.0 * b, b), z);

        assert_eq!(modulo(a, -b), z - b);
        assert_eq!(modulo(a + 10.0 * b, -b), z - b);
        assert_eq!(modulo(a - 10.0 * b, -b), z - b);
    }

    #[test]
    fn test_rescale() {
        assert_eq!(rescale(2.0625, (1.0625, 5.0625), (10.0, 20.0)), 12.5);
    }

    #[test]
    fn test_sin_cos() {
        const TEST_CASES: &'static [(f64, f64, f64)] = &[
            (0.0, 0.0, 1.0),
            (0.1, 0.09972839720069836, 0.9935973557943562),
            (1.0, 0.8403747950252756, 0.5395579585710483),
            (1.5707963267948966, 0.9984625, 0.00003250000000000475),
            (2.0, 0.9074940439786922, -0.41534209884358286),
            (3.141592653589793, 0.0, -0.99795),
            (6.283185307179586, 0.0, 1.0),
            (10.0, -0.5439582041429563, -0.8389752174069942),
            (-0.1, -0.09972839720069898, 0.993597355794356),
            (-1.0, -0.8403747950252756, 0.5395579585710483),
        ];
        for &(z, sin_z, cos_z) in TEST_CASES {
            let actual_sin_z = sin(z);
            let actual_cos_z = cos(z);
            if sin_z != actual_sin_z {
                panic!("sin({}): got {}, want {}", z, actual_sin_z, sin_z);
            }
            if cos_z != actual_cos_z {
                panic!("cos({}): got {}, want {}", z, actual_cos_z, cos_z);
            }
        }

        let actual_sin_nan = sin(f64::NAN);
        let actual_cos_nan = cos(f64::NAN);
        if !actual_sin_nan.is_nan() {
            panic!("sin(NaN): got {}, want NaN", actual_sin_nan);
        }
        if !actual_cos_nan.is_nan() {
            panic!("cos(NaN): got {}, want NaN", actual_cos_nan);
        }
    }

    #[test]
    fn test_sqrt() {
        const TEST_CASES: &'static [(f64, f64)] = &[
            (0.0, 0.0),
            (0.1, 0.3162277665175675),
            (1.0, 1.0),
            (2.0, 1.4142135623746899),
            (3456.789, 58.79446402511048),
        ];
        for &(z, sqrt_z) in TEST_CASES {
            let actual_sqrt_z = sqrt(z);
            if sqrt_z != actual_sqrt_z {
                panic!("sqrt({}): got {}, want {}", z, actual_sqrt_z, sqrt_z);
            }
        }

        let actual_sqrt_nan = sqrt(f64::NAN);
        if !actual_sqrt_nan.is_nan() {
            panic!("sqrt(NaN): got {}, want NaN", actual_sqrt_nan);
        }
    }

    #[test]
    fn test_dist_and_bounds() {
        struct TestCase {
            points: ((f64, f64), (f64, f64)),
            lb: f64,
            d: f64,
            ub: f64,
        }
        const TEST_CASES: &'static [TestCase] = &[
            TestCase {
                points: ((0.0, 0.0), (3.0, 4.0)),
                lb: 4.82072072072072,
                d: 5.000000000053723,
                ub: 5.2919921875,
            },
            TestCase {
                points: ((1.0, 2.0), (3.0, 4.0)),
                lb: 2.609009009009009,
                d: 2.8284271250498643,
                ub: 2.861328125,
            },
            TestCase {
                points: ((10.0, 20.0), (15.0, 32.0)),
                lb: 12.872972972972972,
                d: 13.0,
                ub: 14.1533203125,
            },
        ];
        for &TestCase { points, lb, d, ub } in TEST_CASES {
            let (p1, p2) = points;
            let actual_lb = dist_lower_bound(p1, p2);
            let actual_d = dist(p1, p2);
            let actual_ub = dist_upper_bound(p1, p2);
            if (lb, d, ub) != (actual_lb, actual_d, actual_ub) {
                panic!(
                    "{:?} ~> {:?}: got {} .. {} .. {}, want {} .. {} .. {}",
                    p1, p2, actual_lb, actual_d, actual_ub, lb, d, ub
                );
            }
            if !(actual_ub >= actual_d) || !(actual_lb <= actual_d) {
                panic!(
                    "{:?} ~> {:?}: bad bounds: {} .. {} .. {}",
                    p1, p2, actual_lb, actual_d, actual_ub
                );
            }
        }
    }

    #[test]
    fn test_angle() {
        struct TestCase {
            points: ((f64, f64), (f64, f64)),
            angle: f64,
        }
        const TEST_CASES: &'static [TestCase] = &[
            TestCase {
                points: ((1.0, 2.0), (3.0, 5.0)),
                angle: 0.9827989414909313,
            },
            TestCase {
                points: ((1.0, 2.0), (3.0, -5.0)),
                angle: 4.990698112028704,
            },
            TestCase {
                points: ((1.0, 2.0), (-3.0, 5.0)),
                angle: 2.4980739417195164,
            },
            TestCase {
                points: ((1.0, 2.0), (-3.0, -5.0)),
                angle: 4.19326091952823,
            },
        ];
        for &TestCase { points, angle } in TEST_CASES {
            let (p1, p2) = points;
            let actual_angle = super::angle(p1, p2);
            if angle != actual_angle {
                panic!(
                    "angle({:?} ~> {:?}): got {}, want {}",
                    p1, p2, actual_angle, angle
                );
            }
        }
    }

    #[test]
    fn test_add_polar_offset() {
        assert_eq!(
            add_polar_offset((10.0, 20.0), PI / 6.0, 1.0),
            (10.865494166666666, 20.499669166666667),
        );
    }
}

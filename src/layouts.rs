use super::art::{h, w};
use super::math::{add_polar_offset, dist, pi};
use super::rand::Rng;
use super::traits::Structure;

#[derive(Debug)]
pub struct StartPointGroups(pub Vec<Vec<(f64, f64)>>);

impl StartPointGroups {
    pub fn build(structure: Structure, rng: &mut Rng) -> StartPointGroups {
        match structure {
            Structure::Orbital => orbital(rng),
            Structure::Shadows => shadows(rng),
            Structure::Formation => formation(rng),
        }
    }
}

fn orbital(rng: &mut Rng) -> StartPointGroups {
    let base_step = *rng.wc(&[
        (w(0.01), 3.0),
        (w(0.02), 2.0),
        (w(0.04), 1.0),
        (w(0.06), 1.0),
        (w(0.08), 1.0),
        (w(0.16), 0.5),
    ]);
    let radial_step = base_step * 0.5;

    let radial_group_step = *rng.wc(&[(w(0.07), 0.333), (w(0.15), 0.333), (w(0.3), 0.333)]);

    let center_x = *rng.wc(&[
        (w(0.5), 0.3),
        (w(0.333), 0.2),
        (w(0.666), 0.2),
        (w(-0.333), 0.1),
        (w(1.333), 0.1),
        (w(-1.6), 0.05),
        (w(1.6), 0.05),
    ]);
    let center_y = *rng.wc(&[
        (h(0.5), 0.3),
        (h(0.333), 0.2),
        (h(0.666), 0.2),
        (h(-0.333), 0.1),
        (h(1.333), 0.1),
        (h(-1.6), 0.05),
        (h(1.6), 0.05),
    ]);
    let center = (center_x, center_y);

    let h0 = h(-1.0 / 3.0);
    let h1 = h(4.0 / 3.0);
    let w0 = w(-1.0 / 3.0);
    let w1 = w(4.0 / 3.0);
    let in_bounds = |(x, y)| x > w0 && x < w1 && y > h0 && y < h1;

    let max_radius = w(0.05)
        + (0.0f64)
            .max(dist((center_x, center_y), (0.0, 0.0)))
            .max(dist((center_x, center_y), (w(1.0), 0.0)))
            .max(dist((center_x, center_y), (w(1.0), h(1.0))))
            .max(dist((center_x, center_y), (0.0, h(1.0))));
    let split_offset = rng.uniform(0.0, pi(2.0));

    let mut groups = Vec::new();
    let mut group_radius = w(0.001);
    while group_radius < max_radius {
        let num_splits = *rng.choice(&[1, 2, 3]);
        let split_len = pi(2.0) / num_splits as f64;

        let mut theta = split_offset;
        while theta < split_offset + pi(2.0) {
            let mut group = Vec::new();
            let mut radius = group_radius;
            while radius < group_radius + radial_group_step {
                let circumference = radius * pi(2.0);
                let steps_wanted = circumference / base_step;
                let theta_step = f64::max(pi(0.005), pi(2.0) / steps_wanted);
                let mut inner_theta = theta;
                while inner_theta < theta + split_len {
                    let point = add_polar_offset(center, inner_theta, radius);
                    if in_bounds(point) {
                        group.push(point);
                    }
                    inner_theta += theta_step;
                }
                radius += radial_step;
            }
            groups.push(group);
            theta += split_len;
        }
        group_radius += radial_group_step;
    }
    StartPointGroups(groups)
}

fn shadows(rng: &mut Rng) -> StartPointGroups {
    let num_circles = *rng.choice(&[5, 7, 10, 20, 30, 60]);
    struct Circle {
        center: (f64, f64),
        radius: f64,
    }

    let p_square = *rng.choice(&[0.0, 0.5, 1.0]);
    let columnar_square = rng.odds(0.5);
    let outward_radial = rng.odds(0.5);

    let collides = |c1: &Circle, c2: &Circle| {
        let d = dist(c1.center, c2.center);
        d < c1.radius + c2.radius
    };

    let radial_fill = |c: Circle, rng: &mut Rng| -> Vec<(f64, f64)> {
        let radius_step = w(0.02);
        let circumference_step = w(0.01);
        let mut radius = c.radius;
        let mut group = Vec::new();
        while radius > 0.0 {
            let num_steps = (radius * pi(2.0)) / circumference_step;
            let theta_step = pi(2.0) / num_steps;
            let mut theta = 0.0;
            while theta < pi(2.01) {
                group.push(add_polar_offset(c.center, theta, radius));
                theta += theta_step;
            }
            radius -= radius_step;
        }
        if outward_radial {
            group.reverse();
        }
        if rng.odds(0.05) {
            group = rng.shuffle(group);
        }
        group
    };

    let square_fill = |c: Circle, rng: &mut Rng| -> Vec<(f64, f64)> {
        let step = *rng.wc(&[
            (w(0.0075), 0.37),
            (w(0.01), 0.35),
            (w(0.02), 0.25),
            (w(0.04), 0.02),
            (w(0.08), 0.01),
        ]);
        let radius = c.radius;
        let r2 = radius * radius;
        let mut group = Vec::new();
        let mut offset1 = -radius;
        while offset1 < radius {
            let mut offset2 = -radius;
            while offset2 < radius {
                let (x, y) = if columnar_square {
                    (c.center.0 + offset1, c.center.1 + offset2)
                } else {
                    (c.center.0 + offset2, c.center.1 + offset1)
                };
                let dx = c.center.0 - x;
                let dy = c.center.1 - y;
                if dx * dx + dy * dy < r2 {
                    group.push((x, y));
                }
                offset2 += step;
            }
            offset1 += step;
        }
        group
    };

    let fill = |c: Circle, rng: &mut Rng| {
        if rng.odds(p_square) {
            square_fill(c, rng)
        } else {
            radial_fill(c, rng)
        }
    };

    let mut iter = 0;
    let mut circles = Vec::with_capacity(num_circles);
    while circles.len() < num_circles && iter < 1000 {
        iter += 1;
        let c = Circle {
            center: (rng.uniform(w(0.0), w(1.0)), rng.uniform(h(0.0), h(1.0))),
            radius: rng.uniform(w(0.05), w(0.5)),
        };
        if circles.iter().all(|c2| !collides(&c, c2)) {
            circles.push(c);
        }
    }
    let groups = circles.into_iter().map(|c| fill(c, rng)).collect();
    StartPointGroups(groups)
}

fn formation(rng: &mut Rng) -> StartPointGroups {
    let step = *rng.wc(&[
        (w(0.0075), 0.37),
        (w(0.01), 0.35),
        (w(0.02), 0.25),
        (w(0.04), 0.02),
        (w(0.08), 0.01),
    ]);

    let num_horizontal_steps = *rng.wc(&[
        (1, 0.7),
        (2, 0.35),
        (3, 0.25),
        (4, 0.1),
        (5, 0.05),
        (7, 0.05),
    ]);
    let num_vertical_steps = *rng.wc(&[
        (1, 0.4),
        (2, 0.35),
        (3, 0.25),
        (4, 0.1),
        (5, 0.05),
        (7, 0.05),
    ]);

    let horizontal_step_len = w(1.2) / num_horizontal_steps as f64;
    let vertical_step_len = h(1.2) / num_vertical_steps as f64;

    let skip_odds = *rng.wc(&[(0.0, 0.5), (0.1, 0.3), (0.2, 0.15), (0.5, 0.05)]);

    let mut starting_chunks = Vec::with_capacity(num_horizontal_steps * num_vertical_steps);
    {
        let mut x = w(-0.1);
        while x < w(1.1) {
            let mut y = h(-0.1);
            while y < h(1.1) {
                starting_chunks.push((x, y));
                y += vertical_step_len;
            }
            x += horizontal_step_len;
        }
    }
    let starting_chunks = rng.shuffle(starting_chunks);
    let starting_chunks = starting_chunks
        .into_iter()
        .enumerate()
        .filter(|&(i, _pt)| i == 0 || !rng.odds(skip_odds))
        .map(|(_i, pt)| pt);

    let groups = starting_chunks
        .map(|(x_start, y_start)| {
            let mut group = Vec::new();
            let mut y = y_start;
            while y < y_start + vertical_step_len {
                let mut x = x_start;
                while x < x_start + horizontal_step_len {
                    group.push((x, y));
                    x += step;
                }
                y += step;
            }
            group
        })
        .collect();

    StartPointGroups(groups)
}

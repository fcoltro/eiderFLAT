//! Small numeric helpers shared across the kernel and every crate that consumes
//! it: angle normalisation and point/segment distance. Centralising these keeps
//! the many ad-hoc reimplementations (degree wraps, sweep wraps, point-to-line
//! distances) consistent and in one tested place.

const TAU: f64 = std::f64::consts::TAU;
const PI: f64 = std::f64::consts::PI;

/// Normalise a radian angle into the half-open range `(-π, π]`.
pub fn wrap_pi(mut a: f64) -> f64 {
    while a <= -PI {
        a += TAU;
    }
    while a > PI {
        a -= TAU;
    }
    a
}

/// Normalise a radian angle into `[0, 2π)`.
pub fn wrap_tau(mut a: f64) -> f64 {
    while a < 0.0 {
        a += TAU;
    }
    while a >= TAU {
        a -= TAU;
    }
    a
}

/// Normalise a degree angle into `[0, 360)`.
pub fn wrap_deg360(mut a: f64) -> f64 {
    while a < 0.0 {
        a += 360.0;
    }
    while a >= 360.0 {
        a -= 360.0;
    }
    a
}

/// Normalise `a` into the turn that begins at `start`: `[start, start + 2π)`.
/// Used to bring an arc parameter into a primitive's own angular domain.
pub fn wrap_from(a: f64, start: f64) -> f64 {
    start + wrap_tau(a - start)
}

/// Squared distance from point `p` to the segment `a`–`b` (avoids a `sqrt` on hot
/// paths such as hit-testing). Degenerate segments fall back to point distance.
pub fn point_segment_dist_sq(p: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len_sq = dx * dx + dy * dy;
    let t = if len_sq < 1e-20 {
        0.0
    } else {
        (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / len_sq).clamp(0.0, 1.0)
    };
    let (fx, fy) = (a.0 + t * dx, a.1 + t * dy);
    (p.0 - fx).powi(2) + (p.1 - fy).powi(2)
}

/// Distance from point `p` to the segment `a`–`b`.
pub fn point_segment_dist(p: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 {
    point_segment_dist_sq(p, a, b).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrap_pi_brings_into_range() {
        assert!((wrap_pi(3.0 * PI) - PI).abs() < 1e-12);
        assert!((wrap_pi(-3.0 * PI) - PI).abs() < 1e-12);
        assert!((wrap_pi(0.0)).abs() < 1e-12);
        for &a in &[-10.0, -1.0, 0.0, 1.0, 7.0, 100.0] {
            let w = wrap_pi(a);
            assert!(w > -PI - 1e-12 && w <= PI + 1e-12);
        }
    }

    #[test]
    fn wrap_tau_and_deg_are_nonnegative_in_range() {
        assert!((wrap_tau(-0.1) - (TAU - 0.1)).abs() < 1e-12);
        assert!((wrap_tau(TAU + 0.2) - 0.2).abs() < 1e-12);
        assert!((wrap_deg360(-90.0) - 270.0).abs() < 1e-9);
        assert!((wrap_deg360(450.0) - 90.0).abs() < 1e-9);
    }

    #[test]
    fn wrap_from_lands_in_start_turn() {
        let s = 1.5;
        let w = wrap_from(s - 0.3, s);
        assert!(w >= s - 1e-12 && w < s + TAU);
        assert!((w - (s + TAU - 0.3)).abs() < 1e-9);
    }

    #[test]
    fn point_segment_distance_basics() {
        // Foot of perpendicular inside the segment.
        assert!((point_segment_dist((1.0, 1.0), (0.0, 0.0), (2.0, 0.0)) - 1.0).abs() < 1e-12);
        // Past the end → distance to the endpoint.
        assert!((point_segment_dist((3.0, 0.0), (0.0, 0.0), (2.0, 0.0)) - 1.0).abs() < 1e-12);
        // Degenerate segment → point distance.
        assert!((point_segment_dist((3.0, 4.0), (0.0, 0.0), (0.0, 0.0)) - 5.0).abs() < 1e-12);
    }
}

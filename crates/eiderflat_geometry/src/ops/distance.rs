use crate::curve::{Curve, CurveSegment};

#[derive(Clone, Debug)]
pub struct ProjectionResult {
    pub point: (f64, f64),
    pub t: f64,
    pub distance: f64,
}

pub fn point_to_curve_distance(curve: &Curve, px: f64, py: f64) -> f64 {
    project_point_onto_curve(curve, px, py).distance
}

pub fn project_point_onto_curve(curve: &Curve, px: f64, py: f64) -> ProjectionResult {
    use Curve::*;

    match curve {
        Line(l) => {
            let (ax, ay) = l.p0.to_f64();
            let (bx, by) = l.p1.to_f64();
            let dx = bx - ax;
            let dy = by - ay;
            let len_sq = dx * dx + dy * dy;
            let t = if len_sq < 1e-20 {
                0.0
            } else {
                ((px - ax) * dx + (py - ay) * dy) / len_sq
            }
            .clamp(0.0, 1.0);
            let qx = ax + t * dx;
            let qy = ay + t * dy;
            let d = ((px - qx).powi(2) + (py - qy).powi(2)).sqrt();
            return ProjectionResult {
                point: (qx, qy),
                t,
                distance: d,
            };
        }
        Arc(a) => {
            let (cx, cy) = a.center.to_f64();
            let r = a.radius;
            let angle = (py - cy).atan2(px - cx);
            // Clamp to arc domain
            let angle_clamped = clamp_angle(angle, a.start_angle, a.end_angle);
            let qx = cx + r * angle_clamped.cos();
            let qy = cy + r * angle_clamped.sin();
            let d = ((px - qx).powi(2) + (py - qy).powi(2)).sqrt();
            return ProjectionResult {
                point: (qx, qy),
                t: angle_clamped,
                distance: d,
            };
        }
        Bezier(b) => {
            let (x0, y0) = b.p0.to_f64();
            let (x1, y1) = b.p1.to_f64();
            let (x2, y2) = b.p2.to_f64();
            let (x3, y3) = b.p3.to_f64();
            let ev = move |t: f64| {
                let u = 1.0 - t;
                (
                    u * u * u * x0 + 3.0 * u * u * t * x1 + 3.0 * u * t * t * x2 + t * t * t * x3,
                    u * u * u * y0 + 3.0 * u * u * t * y1 + 3.0 * u * t * t * y2 + t * t * t * y3,
                )
            };
            return golden_section_projection_fn(&ev, (0.0, 1.0), px, py, 32);
        }
        _ => {}
    }

    let ev = |t: f64| curve.evaluate_f64(t);
    golden_section_projection_fn(&ev, curve.domain(), px, py, 32)
}

fn clamp_angle(angle: f64, start: f64, end: f64) -> f64 {
    let pi2 = 2.0 * std::f64::consts::PI;
    let mut a = angle - start;
    while a < 0.0 {
        a += pi2;
    }
    while a > pi2 {
        a -= pi2;
    }
    let span = {
        let mut s = end - start;
        while s <= 0.0 {
            s += pi2;
        }
        s
    };
    if a <= span {
        start + a
    } else {
        // Closest endpoint
        let d_start = a.min(pi2 - a);
        let d_end = a - span;
        if d_start < d_end { start } else { end }
    }
}

fn golden_section_projection_fn(
    ev: &dyn Fn(f64) -> (f64, f64),
    domain: (f64, f64),
    px: f64,
    py: f64,
    samples: usize,
) -> ProjectionResult {
    let (t0, t1) = domain;
    let dt = (t1 - t0) / samples as f64;
    let dist_sq = |t: f64| {
        let (qx, qy) = ev(t);
        (qx - px).powi(2) + (qy - py).powi(2)
    };

    let mut best_t = t0;
    let mut best_d = f64::INFINITY;
    for i in 0..=samples {
        let t = t0 + i as f64 * dt;
        let d = dist_sq(t);
        if d < best_d {
            best_d = d;
            best_t = t;
        }
    }

    let mut a = (best_t - dt).max(t0);
    let mut b = (best_t + dt).min(t1);
    // Golden-section search that carries the two interior probes across
    // iterations, so each step needs only one new curve evaluation (the
    // expensive part for splines) instead of two.
    let phi = (5f64.sqrt() - 1.0) / 2.0;
    let mut c = b - phi * (b - a);
    let mut d = a + phi * (b - a);
    let mut fc = dist_sq(c);
    let mut fd = dist_sq(d);
    for _ in 0..50 {
        if fc < fd {
            b = d;
            d = c;
            fd = fc;
            c = b - phi * (b - a);
            fc = dist_sq(c);
        } else {
            a = c;
            c = d;
            fc = fd;
            d = a + phi * (b - a);
            fd = dist_sq(d);
        }
        if (b - a).abs() < 1e-12 {
            break;
        }
    }
    let t_opt = (a + b) / 2.0;
    let (qx, qy) = ev(t_opt);
    let d = ((px - qx).powi(2) + (py - qy).powi(2)).sqrt();
    ProjectionResult {
        point: (qx, qy),
        t: t_opt,
        distance: d,
    }
}

pub fn curve_to_curve_distance(c1: &Curve, c2: &Curve) -> f64 {
    let (t0_1, t1_1) = c1.domain();
    let (t0_2, t1_2) = c2.domain();
    let n = 16;
    let mut best = f64::INFINITY;
    for i in 0..=n {
        let t = t0_1 + (t1_1 - t0_1) * i as f64 / n as f64;
        let (px, py) = c1.evaluate_f64(t);
        let d = point_to_curve_distance(c2, px, py);
        if d < best {
            best = d;
        }
    }
    for i in 0..=n {
        let t = t0_2 + (t1_2 - t0_2) * i as f64 / n as f64;
        let (px, py) = c2.evaluate_f64(t);
        let d = point_to_curve_distance(c1, px, py);
        if d < best {
            best = d;
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::point::Point2d;
    use crate::primitives::{CircularArc, LineSeg};

    fn pt(x: i64, y: i64) -> Point2d {
        Point2d::from_i64(x, y)
    }

    #[test]
    fn point_to_line_distance() {
        let line = Curve::Line(LineSeg::from_endpoints(pt(0, 0), pt(4, 0)));
        let d = point_to_curve_distance(&line, 2.0, 3.0);
        assert!((d - 3.0).abs() < 1e-9, "d={}", d);
    }

    #[test]
    fn point_to_circle_distance() {
        let arc = Curve::Arc(CircularArc::new(
            pt(0, 0),
            5.0,
            -std::f64::consts::PI,
            std::f64::consts::PI,
        ));
        let d = point_to_curve_distance(&arc, 8.0, 0.0);
        assert!((d - 3.0).abs() < 1e-6, "d={}", d);
    }

    #[test]
    fn projection_onto_line() {
        let line = Curve::Line(LineSeg::from_endpoints(pt(0, 0), pt(4, 0)));
        let proj = project_point_onto_curve(&line, 3.0, 5.0);
        assert!((proj.point.0 - 3.0).abs() < 1e-9);
        assert!((proj.point.1).abs() < 1e-9);
        assert!((proj.distance - 5.0).abs() < 1e-9);
    }

    #[test]
    fn projection_onto_arc_slightly_negative() {
        let arc = Curve::Arc(CircularArc::new(pt(0, 0), 5.0, 0.0, std::f64::consts::PI));
        let proj = project_point_onto_curve(&arc, 5.0, -0.1);
        assert!((proj.point.0 - 5.0).abs() < 1e-4);
        assert!((proj.point.1 - 0.0).abs() < 1e-4);
        assert!((proj.t - 0.0).abs() < 1e-4);
    }
}

use crate::curve::Curve;
use crate::nurbs::{NurbsCurve, RationalBezier};
use crate::point::Point2d;
use crate::primitives::{CircularArc, CubicBezier, EllipticalArc, LineSeg, PolyCurve};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform2d {
    pub m00: f64,
    pub m01: f64,
    pub tx: f64,
    pub m10: f64,
    pub m11: f64,
    pub ty: f64,
}

impl Transform2d {
    pub fn identity() -> Self {
        Transform2d {
            m00: 1.0,
            m01: 0.0,
            tx: 0.0,
            m10: 0.0,
            m11: 1.0,
            ty: 0.0,
        }
    }

    pub fn translation(dx: f64, dy: f64) -> Self {
        let mut t = Self::identity();
        t.tx = dx;
        t.ty = dy;
        t
    }

    pub fn scale(sx: f64, sy: f64) -> Self {
        Transform2d {
            m00: sx,
            m01: 0.0,
            tx: 0.0,
            m10: 0.0,
            m11: sy,
            ty: 0.0,
        }
    }

    pub fn scale_uniform(s: f64) -> Self {
        Self::scale(s, s)
    }

    pub fn scale_about(center: &Point2d, sx: f64, sy: f64) -> Self {
        Self::translation(center.x, center.y)
            .compose(&Self::scale(sx, sy))
            .compose(&Self::translation(-center.x, -center.y))
    }

    pub fn mirror_x() -> Self {
        Self::scale(1.0, -1.0)
    }

    pub fn mirror_line(p0: &Point2d, p1: &Point2d) -> Self {
        let dx = p1.x - p0.x;
        let dy = p1.y - p0.y;
        let len_sq = dx * dx + dy * dy;
        assert!(len_sq != 0.0, "mirror line needs two distinct points");
        let r00 = (dx * dx - dy * dy) / len_sq;
        let r01 = (2.0 * dx * dy) / len_sq;
        let r11 = (dy * dy - dx * dx) / len_sq;
        let refl = Transform2d {
            m00: r00,
            m01: r01,
            tx: 0.0,
            m10: r01,
            m11: r11,
            ty: 0.0,
        };
        Self::translation(p0.x, p0.y)
            .compose(&refl)
            .compose(&Self::translation(-p0.x, -p0.y))
    }

    pub fn rotation_quarter_turns(n: i32) -> Self {
        let (c, s) = match n.rem_euclid(4) {
            0 => (1.0, 0.0),
            1 => (0.0, 1.0),
            2 => (-1.0, 0.0),
            _ => (0.0, -1.0),
        };
        Transform2d {
            m00: c,
            m01: -s,
            tx: 0.0,
            m10: s,
            m11: c,
            ty: 0.0,
        }
    }

    pub fn rotation(angle: f64) -> Self {
        let c = angle.cos();
        let s = angle.sin();
        Transform2d {
            m00: c,
            m01: -s,
            tx: 0.0,
            m10: s,
            m11: c,
            ty: 0.0,
        }
    }

    pub fn rotation_about(center: &Point2d, angle: f64) -> Self {
        Self::translation(center.x, center.y)
            .compose(&Self::rotation(angle))
            .compose(&Self::translation(-center.x, -center.y))
    }

    pub fn compose(&self, other: &Transform2d) -> Transform2d {
        Transform2d {
            m00: self.m00 * other.m00 + self.m01 * other.m10,
            m01: self.m00 * other.m01 + self.m01 * other.m11,
            tx: self.m00 * other.tx + self.m01 * other.ty + self.tx,
            m10: self.m10 * other.m00 + self.m11 * other.m10,
            m11: self.m10 * other.m01 + self.m11 * other.m11,
            ty: self.m10 * other.tx + self.m11 * other.ty + self.ty,
        }
    }

    pub fn apply_point(&self, p: &Point2d) -> Point2d {
        Point2d {
            x: self.m00 * p.x + self.m01 * p.y + self.tx,
            y: self.m10 * p.x + self.m11 * p.y + self.ty,
        }
    }

    /// Applies only the linear (rotation/scale/shear) part to a direction vector,
    /// ignoring translation. Use this for directions/normals rather than positions.
    pub fn apply_vector(&self, dx: f64, dy: f64) -> (f64, f64) {
        (self.m00 * dx + self.m01 * dy, self.m10 * dx + self.m11 * dy)
    }

    pub fn determinant(&self) -> f64 {
        self.m00 * self.m11 - self.m01 * self.m10
    }

    pub fn scale_factor(&self) -> f64 {
        self.determinant().abs().sqrt()
    }

    pub fn rotation_angle(&self) -> f64 {
        self.m10.atan2(self.m00)
    }

    pub fn is_reflection(&self) -> bool {
        self.determinant() < 0.0
    }
}

impl Transform2d {
    pub fn apply_curve(&self, curve: &Curve) -> Curve {
        match curve {
            Curve::Line(l) => Curve::Line(LineSeg::from_endpoints(
                self.apply_point(&l.p0),
                self.apply_point(&l.p1),
            )),
            Curve::Bezier(b) => Curve::Bezier(CubicBezier::new(
                self.apply_point(&b.p0),
                self.apply_point(&b.p1),
                self.apply_point(&b.p2),
                self.apply_point(&b.p3),
            )),
            Curve::Arc(a) => Curve::Arc(self.apply_arc(a)),
            Curve::Ellipse(e) => Curve::Ellipse(self.apply_ellipse(e)),
            Curve::Poly(pc) => {
                let segs = pc.segments.iter().map(|s| self.apply_curve(s)).collect();
                Curve::Poly(Box::new(PolyCurve::new(segs)))
            }
            Curve::Rational(rb) => {
                let points = rb.points.iter().map(|p| self.apply_point(p)).collect();
                Curve::Rational(RationalBezier::new(points, rb.weights.clone()))
            }
            Curve::Nurbs(nc) => {
                let control = nc.control.iter().map(|p| self.apply_point(p)).collect();
                Curve::Nurbs(NurbsCurve::new(control, nc.weights.clone()))
            }
        }
    }

    fn apply_arc(&self, a: &CircularArc) -> CircularArc {
        let new_center = self.apply_point(&a.center);
        let new_radius = a.radius * self.scale_factor();
        let rot = self.rotation_angle();
        let (start, end) = if self.is_reflection() {
            (-a.end_angle + rot, -a.start_angle + rot)
        } else {
            (a.start_angle + rot, a.end_angle + rot)
        };
        CircularArc::new(new_center, new_radius, start, end)
    }

    fn apply_ellipse(&self, e: &EllipticalArc) -> EllipticalArc {
        let new_center = self.apply_point(&e.center);
        let sf = self.scale_factor();
        let new_major = e.semi_major * sf;
        let new_minor = e.semi_minor * sf;
        let rot = self.rotation_angle();
        let new_rotation = e.rotation + rot;
        let (start, end) = if self.is_reflection() {
            (-e.end_angle + rot, -e.start_angle + rot)
        } else {
            (e.start_angle + rot, e.end_angle + rot)
        };
        EllipticalArc::new(new_center, new_major, new_minor, new_rotation, start, end)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::curve::CurveSegment;

    fn pt(x: i64, y: i64) -> Point2d {
        Point2d::from_i64(x, y)
    }

    #[test]
    fn translate_point_exact() {
        let t = Transform2d::translation(3.0, -2.0);
        assert_eq!(t.apply_point(&pt(5, 5)), pt(8, 3));
    }

    #[test]
    fn scale_about_center() {
        let t = Transform2d::scale_about(&pt(1, 1), 2.0, 2.0);
        assert_eq!(t.apply_point(&pt(3, 3)), pt(5, 5));
        assert_eq!(t.apply_point(&pt(1, 1)), pt(1, 1));
    }

    #[test]
    fn quarter_turn_exact() {
        let t = Transform2d::rotation_quarter_turns(1);
        assert_eq!(t.apply_point(&pt(1, 0)), pt(0, 1));
        assert_eq!(t.apply_point(&pt(0, 1)), pt(-1, 0));
    }

    #[test]
    fn mirror_x_axis() {
        let t = Transform2d::mirror_x();
        assert_eq!(t.apply_point(&pt(3, 4)), pt(3, -4));
        assert!(t.is_reflection());
    }

    #[test]
    fn mirror_diagonal_line() {
        let t = Transform2d::mirror_line(&pt(0, 0), &pt(1, 1));
        assert_eq!(t.apply_point(&pt(3, 0)), pt(0, 3));
    }

    #[test]
    fn compose_translate_then_scale() {
        let t = Transform2d::scale(2.0, 2.0).compose(&Transform2d::translation(1.0, 1.0));
        assert_eq!(t.apply_point(&pt(2, 3)), pt(6, 8));
    }

    #[test]
    fn bezier_is_affine_invariant() {
        let bz = Curve::Bezier(CubicBezier::new(pt(0, 0), pt(1, 2), pt(3, 2), pt(4, 0)));
        let t = Transform2d::translation(10.0, 5.0);
        let moved = t.apply_curve(&bz);
        let (x, y) = bz.evaluate_f64(0.5);
        let (mx, my) = moved.evaluate_f64(0.5);
        assert!((mx - (x + 10.0)).abs() < 1e-9 && (my - (y + 5.0)).abs() < 1e-9);
    }

    #[test]
    fn line_transform_endpoints() {
        let l = Curve::Line(LineSeg::from_endpoints(pt(0, 0), pt(2, 0)));
        let t = Transform2d::rotation_quarter_turns(1);
        if let Curve::Line(moved) = t.apply_curve(&l) {
            assert_eq!(moved.p0, pt(0, 0));
            assert_eq!(moved.p1, pt(0, 2));
        } else {
            panic!("expected line");
        }
    }

    #[test]
    fn arc_translate_and_scale() {
        let arc = Curve::Arc(CircularArc::new(pt(0, 0), 2.0, 0.0, std::f64::consts::PI));
        let t = Transform2d::scale_uniform(3.0);
        if let Curve::Arc(a) = t.apply_curve(&arc) {
            assert!((a.radius - 6.0).abs() < 1e-6);
            assert_eq!(a.center, pt(0, 0));
        } else {
            panic!("expected arc");
        }
    }

    #[test]
    fn mirror_arc_reverses_sweep() {
        let arc = CircularArc::new(pt(0, 0), 2.0, 0.0, std::f64::consts::FRAC_PI_2);
        let mirrored = match Transform2d::mirror_x().apply_curve(&Curve::Arc(arc)) {
            Curve::Arc(a) => a,
            _ => panic!("expected arc"),
        };

        let (sx, sy) = mirrored.evaluate_f64(mirrored.start_angle);
        let (ex, ey) = mirrored.evaluate_f64(mirrored.end_angle);
        assert!(
            (sx - 0.0).abs() < 1e-6 && (sy + 2.0).abs() < 1e-6,
            "mirrored start {:?}",
            (sx, sy)
        );
        assert!(
            (ex - 2.0).abs() < 1e-6 && (ey - 0.0).abs() < 1e-6,
            "mirrored end {:?}",
            (ex, ey)
        );

        assert!(
            (mirrored.included_angle() - std::f64::consts::FRAC_PI_2).abs() < 1e-6,
            "included angle {}",
            mirrored.included_angle()
        );
    }
}

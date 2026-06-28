use crate::clip::{BoolOp, clip};
use crate::region::Region;
use crate::weld::{WELD_TOL, weld_region};
use eiderflat_geometry::{Curve, CurveSegment, LineSeg, Point2d, tessellate_curve};

pub fn union(a: &Region, b: &Region) -> Region {
    loops_to_region(clip_regions(a, b, BoolOp::Union))
}

pub fn intersection(a: &Region, b: &Region) -> Region {
    loops_to_region(clip_regions(a, b, BoolOp::Intersection))
}

pub fn difference(a: &Region, b: &Region) -> Region {
    loops_to_region(clip_regions(a, b, BoolOp::Difference))
}

pub fn xor(a: &Region, b: &Region) -> Region {
    let mut loops = clip_regions(a, b, BoolOp::Difference);
    loops.extend(clip_regions(b, a, BoolOp::Difference));
    loops_to_region(loops)
}

fn clip_regions(a: &Region, b: &Region, op: BoolOp) -> Vec<Vec<Point2d>> {
    let a = weld_region(a, WELD_TOL);
    let b = weld_region(b, WELD_TOL);
    let pa = flatten_loop(&a.outer);
    let pb = flatten_loop(&b.outer);
    if pa.len() < 3 || pb.len() < 3 {
        return Vec::new();
    }
    clip(&pa, &pb, op)
}

fn flatten_loop(curves: &[Curve]) -> Vec<Point2d> {
    let tol = (loop_diag(curves) * 1e-3).max(1e-6);
    let mut pts: Vec<Point2d> = Vec::new();
    for c in curves {
        for q in tessellate_curve(c, tol) {
            if pts.last().is_none_or(|l| dist2(l, &q) > 1e-18) {
                pts.push(q);
            }
        }
    }
    if pts.len() >= 2 && dist2(&pts[0], pts.last().unwrap()) < 1e-18 {
        pts.pop();
    }
    pts
}

fn loops_to_region(loops: Vec<Vec<Point2d>>) -> Region {
    let mut loops: Vec<Vec<Point2d>> = loops.into_iter().filter(|l| l.len() >= 3).collect();
    if loops.is_empty() {
        return Region::new(Vec::new());
    }
    let mut oi = 0;
    for i in 1..loops.len() {
        if poly_area(&loops[i]) > poly_area(&loops[oi]) {
            oi = i;
        }
    }
    let outer_pts = loops.remove(oi);
    let outer = poly_to_lines(&outer_pts);
    let outer_region = Region::new(outer.clone());
    let holes: Vec<Vec<Curve>> = loops
        .into_iter()
        .filter(|l| {
            let (cx, cy) = poly_centroid(l);
            outer_region.contains_point(cx, cy)
        })
        .map(|l| poly_to_lines(&l))
        .collect();
    Region::with_holes(outer, holes)
}

fn poly_to_lines(pts: &[Point2d]) -> Vec<Curve> {
    (0..pts.len())
        .map(|i| Curve::Line(LineSeg::from_endpoints(pts[i], pts[(i + 1) % pts.len()])))
        .collect()
}

fn poly_area(pts: &[Point2d]) -> f64 {
    let mut a = 0.0;
    for i in 0..pts.len() {
        let (x0, y0) = pts[i].to_f64();
        let (x1, y1) = pts[(i + 1) % pts.len()].to_f64();
        a += x0 * y1 - x1 * y0;
    }
    (a / 2.0).abs()
}

fn poly_centroid(pts: &[Point2d]) -> (f64, f64) {
    let (mut sx, mut sy) = (0.0, 0.0);
    for p in pts {
        let (x, y) = p.to_f64();
        sx += x;
        sy += y;
    }
    let n = pts.len() as f64;
    (sx / n, sy / n)
}

fn loop_diag(curves: &[Curve]) -> f64 {
    let mut min = (f64::MAX, f64::MAX);
    let mut max = (f64::MIN, f64::MIN);
    for c in curves {
        let bb = c.bounding_box();
        min.0 = min.0.min(bb.min.x);
        min.1 = min.1.min(bb.min.y);
        max.0 = max.0.max(bb.max.x);
        max.1 = max.1.max(bb.max.y);
    }
    ((max.0 - min.0).powi(2) + (max.1 - min.1).powi(2)).sqrt()
}

fn dist2(a: &Point2d, b: &Point2d) -> f64 {
    let (ax, ay) = a.to_f64();
    let (bx, by) = b.to_f64();
    (ax - bx).powi(2) + (ay - by).powi(2)
}

#[cfg(test)]
mod tests {
    use super::*;
    use eiderflat_geometry::{CubicBezier, LineSeg, Point2d};

    fn square(x0: i64, y0: i64, x1: i64, y1: i64) -> Region {
        Region::new(vec![
            Curve::Line(LineSeg::from_endpoints(
                Point2d::from_i64(x0, y0),
                Point2d::from_i64(x1, y0),
            )),
            Curve::Line(LineSeg::from_endpoints(
                Point2d::from_i64(x1, y0),
                Point2d::from_i64(x1, y1),
            )),
            Curve::Line(LineSeg::from_endpoints(
                Point2d::from_i64(x1, y1),
                Point2d::from_i64(x0, y1),
            )),
            Curve::Line(LineSeg::from_endpoints(
                Point2d::from_i64(x0, y1),
                Point2d::from_i64(x0, y0),
            )),
        ])
    }

    #[test]
    fn difference_excludes_overlap() {
        let d = difference(&square(0, 0, 4, 4), &square(2, 2, 6, 6));
        assert!(d.contains_point(1.0, 1.0), "A-only region stays");
        assert!(!d.contains_point(3.0, 3.0), "the overlap corner is removed");
        assert!(!d.contains_point(5.0, 5.0), "B-only was never in A");
    }

    #[test]
    fn intersection_keeps_only_overlap() {
        let i = intersection(&square(0, 0, 3, 3), &square(2, 2, 5, 5));
        assert!(i.contains_point(2.5, 2.5), "overlap is inside");
        assert!(!i.contains_point(1.0, 1.0), "A-only excluded");
        assert!(!i.contains_point(4.0, 4.0), "B-only excluded");
    }

    #[test]
    fn union_covers_both() {
        let u = union(&square(0, 0, 3, 3), &square(2, 2, 5, 5));
        assert!(u.contains_point(1.0, 1.0), "deep in A");
        assert!(u.contains_point(4.0, 4.0), "deep in B");
        assert!(u.contains_point(2.5, 2.5), "the overlap");
        assert!(
            !u.contains_point(4.0, 1.0),
            "between the squares, outside both"
        );
        assert!(!u.contains_point(10.0, 10.0), "far outside");
    }

    #[test]
    fn boolean_welds_open_input_boundary() {
        let g = 1e-9;
        let a = Region::new(vec![
            Curve::Line(LineSeg::from_endpoints(
                Point2d::from_f64(0.0, 0.0),
                Point2d::from_f64(4.0, 0.0),
            )),
            Curve::Line(LineSeg::from_endpoints(
                Point2d::from_f64(4.0, 0.0),
                Point2d::from_f64(4.0, 4.0),
            )),
            Curve::Line(LineSeg::from_endpoints(
                Point2d::from_f64(4.0, 4.0),
                Point2d::from_f64(0.0, 4.0),
            )),
            Curve::Line(LineSeg::from_endpoints(
                Point2d::from_f64(g, 4.0),
                Point2d::from_f64(g, g),
            )),
        ]);
        let d = difference(&a, &square(2, 2, 6, 6));
        assert!(
            d.contains_point(1.0, 1.0),
            "welded A−B keeps the A-only region"
        );
        assert!(!d.contains_point(3.0, 3.0), "the overlap corner is removed");
    }

    #[test]
    fn boolean_over_bezier_boundary_is_fast() {
        let a = Region::new(vec![
            Curve::Bezier(CubicBezier::new(
                Point2d::from_f64(0.0, 0.0),
                Point2d::from_f64(1.0, 3.0),
                Point2d::from_f64(3.0, -3.0),
                Point2d::from_f64(4.0, 0.0),
            )),
            Curve::Line(LineSeg::from_endpoints(
                Point2d::from_f64(4.0, 0.0),
                Point2d::from_f64(4.0, 4.0),
            )),
            Curve::Line(LineSeg::from_endpoints(
                Point2d::from_f64(4.0, 4.0),
                Point2d::from_f64(0.0, 4.0),
            )),
            Curve::Line(LineSeg::from_endpoints(
                Point2d::from_f64(0.0, 4.0),
                Point2d::from_f64(0.0, 0.0),
            )),
        ]);
        let b = square(1, 1, 3, 5);
        let t = std::time::Instant::now();
        let _ = difference(&a, &b);
        let _ = union(&a, &b);
        let _ = intersection(&a, &b);
        assert!(
            t.elapsed().as_millis() < 500,
            "boolean over Bézier too slow: {:?}",
            t.elapsed()
        );
    }

    fn ngon(cx: f64, cy: f64, r: f64, n: usize) -> Region {
        let pts: Vec<Point2d> = (0..n)
            .map(|i| {
                let a = std::f64::consts::TAU * i as f64 / n as f64;
                Point2d::from_f64(cx + r * a.cos(), cy + r * a.sin())
            })
            .collect();
        let segs = (0..n)
            .map(|i| Curve::Line(LineSeg::from_endpoints(pts[i], pts[(i + 1) % n])))
            .collect();
        Region::new(segs)
    }

    #[test]
    fn union_of_overlapping_circles_classifies_correctly() {
        let u = union(&ngon(7.0, 6.0, 4.0, 48), &ngon(12.0, 6.0, 4.0, 48));
        assert!(!u.outer.is_empty(), "union must produce a boundary");
        assert!(
            u.contains_point(7.0, 6.0),
            "center of circle 1 is inside the union"
        );
        assert!(
            u.contains_point(12.0, 6.0),
            "center of circle 2 is inside the union"
        );
        assert!(u.contains_point(9.5, 6.0), "the lens is inside the union");
        assert!(!u.contains_point(0.0, 6.0), "far-left point is outside");
        assert!(!u.contains_point(20.0, 6.0), "far-right point is outside");
        assert!(!u.contains_point(9.5, 20.0), "far-above point is outside");
    }

    #[test]
    fn xor_excludes_overlap() {
        let x = xor(&square(0, 0, 3, 3), &square(2, 2, 5, 5));
        assert!(
            !x.outer.is_empty(),
            "xor of overlapping squares is non-empty"
        );
        assert!(
            !x.contains_point(2.5, 2.5),
            "the overlap is excluded from xor"
        );
        assert!(!x.contains_point(10.0, 10.0), "far outside is excluded");
    }
}

use egui::Stroke;
use eiderflat_geometry::{Curve, CurveSegment};
const TESS_TOL_PX: f32 = 0.3;
const TESS_TOL_PX_SQ: f32 = TESS_TOL_PX * TESS_TOL_PX;
const TESS_MAX_DEPTH: u32 = 18;
const TESS_MAX_POINTS: usize = 20_000;

pub(super) fn draw_curve(
    painter: &egui::Painter,
    c: &Curve,
    to_screen: &impl Fn(f64, f64) -> egui::Pos2,
    stroke: Stroke,
) {
    match c {
        Curve::Line(l) => {
            let (x0, y0) = l.p0.to_f64();
            let (x1, y1) = l.p1.to_f64();
            painter.line_segment([to_screen(x0, y0), to_screen(x1, y1)], stroke);
        }
        other => {
            painter.add(egui::Shape::line(flatten_curve(other, to_screen), stroke));
        }
    }
}

pub(super) fn flatten_curve(
    c: &Curve,
    to_screen: &impl Fn(f64, f64) -> egui::Pos2,
) -> Vec<egui::Pos2> {
    let (t0, t1) = c.domain();
    let eval = |t: f64| {
        let (x, y) = c.evaluate_f64(t);
        to_screen(x, y)
    };
    let mut pts: Vec<egui::Pos2> = Vec::with_capacity(64);
    const SPANS: usize = 4;
    // Evaluate each span endpoint exactly once and thread it through the
    // recursion, so a curve point is never re-evaluated at a shared parameter.
    let mut a = t0;
    let mut pa = eval(t0);
    pts.push(pa);
    for i in 0..SPANS {
        let b = t0 + (t1 - t0) * (i + 1) as f64 / SPANS as f64;
        let pb = eval(b);
        tessellate(&eval, a, pa, b, pb, 0, &mut pts);
        a = b;
        pa = pb;
    }
    pts
}

#[allow(clippy::too_many_arguments)]
fn tessellate(
    eval: &impl Fn(f64) -> egui::Pos2,
    t0: f64,
    p0: egui::Pos2,
    t1: f64,
    p1: egui::Pos2,
    depth: u32,
    out: &mut Vec<egui::Pos2>,
) {
    if out.len() >= TESS_MAX_POINTS {
        return;
    }
    let tm = 0.5 * (t0 + t1);
    let pm = eval(tm);
    if depth >= TESS_MAX_DEPTH || point_seg_dist_sq(pm, p0, p1) <= TESS_TOL_PX_SQ {
        out.push(p1);
    } else {
        tessellate(eval, t0, p0, tm, pm, depth + 1, out);
        tessellate(eval, tm, pm, t1, p1, depth + 1, out);
    }
}

/// Squared distance from `p` to segment `a`–`b`. Avoids a `sqrt` on hot paths
/// (tessellation tolerance tests) where only a comparison against a squared
/// threshold is needed; take `.sqrt()` of the result when an actual distance
/// is wanted.
pub(super) fn point_seg_dist_sq(p: egui::Pos2, a: egui::Pos2, b: egui::Pos2) -> f32 {
    let abx = b.x - a.x;
    let aby = b.y - a.y;
    let len2 = abx * abx + aby * aby;
    if len2 < 1e-12 {
        return (p.x - a.x).powi(2) + (p.y - a.y).powi(2);
    }
    let t = (((p.x - a.x) * abx + (p.y - a.y) * aby) / len2).clamp(0.0, 1.0);
    let cx = a.x + t * abx;
    let cy = a.y + t * aby;
    (p.x - cx).powi(2) + (p.y - cy).powi(2)
}

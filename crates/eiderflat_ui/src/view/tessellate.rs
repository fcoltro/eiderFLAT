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

/// Draw a curve with a line-type dash pattern. `pattern_px` alternates lengths
/// in screen pixels, sign-encoded: a positive value is a drawn dash, a negative
/// value a gap, and a zero a dot. An empty pattern draws solid (like
/// [`draw_curve`]).
pub(super) fn draw_curve_patterned(
    painter: &egui::Painter,
    c: &Curve,
    to_screen: &impl Fn(f64, f64) -> egui::Pos2,
    stroke: Stroke,
    pattern_px: &[f32],
) {
    if pattern_px.is_empty() {
        draw_curve(painter, c, to_screen, stroke);
        return;
    }
    let pts = match c {
        Curve::Line(l) => {
            let (x0, y0) = l.p0.to_f64();
            let (x1, y1) = l.p1.to_f64();
            vec![to_screen(x0, y0), to_screen(x1, y1)]
        }
        other => flatten_curve(other, to_screen),
    };
    draw_patterned_polyline(painter, &pts, stroke, pattern_px);
}

/// Walk a dash pattern along a polyline, carrying the pattern phase across
/// vertices so dashes stay continuous through corners.
pub(super) fn draw_patterned_polyline(
    painter: &egui::Painter,
    pts: &[egui::Pos2],
    stroke: Stroke,
    pattern_px: &[f32],
) {
    if pts.len() < 2 {
        return;
    }
    // Total pattern length; if it's degenerate, fall back to a solid line so we
    // never spin forever on an all-zero pattern.
    let total: f32 = pattern_px.iter().map(|v| v.abs()).sum();
    if total <= 1e-3 {
        painter.add(egui::Shape::line(pts.to_vec(), stroke));
        return;
    }
    let dot_r = (stroke.width * 0.6).max(0.6);
    let mut pi = 0usize;
    let mut rem = pattern_px[0].abs();
    // Pen state for the current element: > 0 dash, 0 dot, < 0 gap.
    let mut elem = pattern_px[0];
    // Draw a dot if the very first element is one.
    if elem == 0.0 {
        painter.circle_filled(pts[0], dot_r, stroke.color);
    }
    let mut guard = 0u32;
    for seg in pts.windows(2) {
        let (a, b) = (seg[0], seg[1]);
        let d = b - a;
        let seg_len = d.length();
        if seg_len < 1e-6 {
            continue;
        }
        let dir = d / seg_len;
        let mut cursor = 0.0f32;
        while cursor < seg_len - 1e-6 {
            guard += 1;
            if guard > 200_000 {
                return;
            }
            if elem == 0.0 {
                // Dot at the current position, then advance to the next element.
                painter.circle_filled(a + dir * cursor, dot_r, stroke.color);
                pi = (pi + 1) % pattern_px.len();
                elem = pattern_px[pi];
                rem = elem.abs();
                continue;
            }
            let take = rem.min(seg_len - cursor);
            if elem > 0.0 && take > 0.0 {
                painter.line_segment([a + dir * cursor, a + dir * (cursor + take)], stroke);
            }
            cursor += take;
            rem -= take;
            if rem <= 1e-4 {
                pi = (pi + 1) % pattern_px.len();
                elem = pattern_px[pi];
                rem = elem.abs();
            }
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

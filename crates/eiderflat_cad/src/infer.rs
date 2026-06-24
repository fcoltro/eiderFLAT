//! Line-extension guide ("tracking"). While grip-dragging a line's endpoint the
//! cursor can lock onto that line's *original* axis, so the endpoint can be
//! pulled while staying colinear with where the line was.
//!
//! Deliberately tiny and self-contained: it does not touch object snapping. The
//! caller resolves object snaps first and only consults this when none claimed
//! the cursor.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GuideKind {
    Extension,
}

impl GuideKind {
    pub fn label(self) -> &'static str {
        match self {
            GuideKind::Extension => "Extension",
        }
    }
}

/// An infinite construction line: passes through `origin` along unit `dir`.
#[derive(Clone, Copy, Debug)]
pub struct Guide {
    pub kind: GuideKind,
    pub origin: (f64, f64),
    pub dir: (f64, f64),
}

#[derive(Clone, Debug)]
pub struct InferResult {
    /// The resolved cursor position (projected onto the guide).
    pub point: (f64, f64),
    /// The guide that produced the lock. Drawn by the UI.
    pub guides: Vec<Guide>,
}

type P = (f64, f64);

fn norm((x, y): P) -> Option<P> {
    let l = (x * x + y * y).sqrt();
    (l > 1e-9).then(|| (x / l, y / l))
}

fn dist(a: P, b: P) -> f64 {
    (a.0 - b.0).hypot(a.1 - b.1)
}

/// Perpendicular distance from `p` to the infinite line of `g`.
fn line_dist(g: &Guide, p: P) -> f64 {
    ((p.0 - g.origin.0) * -g.dir.1 + (p.1 - g.origin.1) * g.dir.0).abs()
}

/// Foot of `p` projected onto `g`.
fn project_on(g: &Guide, p: P) -> P {
    let t = (p.0 - g.origin.0) * g.dir.0 + (p.1 - g.origin.1) * g.dir.1;
    (g.origin.0 + g.dir.0 * t, g.origin.1 + g.dir.1 * t)
}

/// Lock the cursor onto the infinite axis through `p0`–`p1` (a line's *original*
/// endpoints). Returns the projected point and a single Extension guide when the
/// cursor is within `tol` (world units) of the axis.
pub fn infer_axis(p0: P, p1: P, cursor: P, tol: f64) -> Option<InferResult> {
    if tol <= 0.0 {
        return None;
    }
    let dir = norm((p1.0 - p0.0, p1.1 - p0.1))?;
    // Anchor at the endpoint farther from the cursor (the stationary one you are
    // not dragging toward) so the ray reads as the line's fixed part.
    let origin = if dist(p0, cursor) >= dist(p1, cursor) {
        p0
    } else {
        p1
    };
    let g = Guide {
        kind: GuideKind::Extension,
        origin,
        dir,
    };
    if line_dist(&g, cursor) > tol {
        return None;
    }
    Some(InferResult {
        point: project_on(&g, cursor),
        guides: vec![g],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn axis_lock_keeps_an_endpoint_colinear() {
        // A line along 45°. Dragging its far endpoint, the cursor near the axis
        // snaps exactly back onto it.
        let res = infer_axis((0.0, 0.0), (10.0, 10.0), (6.2, 5.8), 0.5).expect("axis lock");
        assert!(
            (res.point.0 - res.point.1).abs() < 1e-9,
            "snaps onto y=x, got {:?}",
            res.point
        );
        assert_eq!(res.guides.len(), 1);
        assert_eq!(res.guides[0].kind, GuideKind::Extension);
    }

    #[test]
    fn axis_no_lock_when_far_off() {
        assert!(infer_axis((0.0, 0.0), (10.0, 0.0), (5.0, 4.0), 0.5).is_none());
    }
}

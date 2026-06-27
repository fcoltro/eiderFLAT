use crate::properties::{Color, LineTypeRef, LineWeight, XData};
use eiderflat_geometry::{BoundingBox, Curve, CurveSegment, Point2d, Transform2d};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EntityId(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum HatchPattern {
    #[default]
    Solid,
    Lines {
        angle_deg: f64,
        spacing: f64,
    },
    Cross {
        angle_deg: f64,
        spacing: f64,
    },
    Dots {
        spacing: f64,
    },
}

#[derive(Clone, Debug)]
pub enum EntityKind {
    Curve(Curve),
    Point(Point2d),
    Text {
        anchor: Point2d,
        content: String,
        height: f64,
        rotation: f64,
        font: Option<String>,
    },
    XLine {
        through: Point2d,
        dir: (f64, f64),
    },
    Ray {
        from: Point2d,
        dir: (f64, f64),
    },
    Insert {
        block: String,
        transform: Transform2d,
    },
    Hatch {
        boundary: Vec<Curve>,
        holes: Vec<Vec<Curve>>,
        fill: (u8, u8, u8),
        pattern: HatchPattern,
    },
    /// Aligned linear dimension between two points. The dimension line is
    /// parallel to `p1`→`p2` and passes through `line`, which sets the offset
    /// distance and side; the measured value is the distance `p1`→`p2`.
    Dimension {
        p1: Point2d,
        p2: Point2d,
        line: Point2d,
        height: f64,
        /// User text shown instead of the measured value when `Some`.
        override_text: Option<String>,
    },
    /// Axis-locked linear dimension between two points. When `vertical` is false
    /// it measures the horizontal distance `|x2 − x1|` with a horizontal dimension
    /// line through `line.y`; when true, the vertical distance through `line.x`.
    OrthoDim {
        p1: Point2d,
        p2: Point2d,
        line: Point2d,
        vertical: bool,
        height: f64,
        override_text: Option<String>,
    },
    /// Angular dimension at vertex `center`, measuring the angle between the rays
    /// `center`→`p1` and `center`→`p2`. The dimension arc passes through `line`,
    /// which sets the arc radius and which of the two angular sectors is labelled.
    AngularDim {
        center: Point2d,
        p1: Point2d,
        p2: Point2d,
        line: Point2d,
        height: f64,
        override_text: Option<String>,
    },
    /// Radius (or diameter, when `diameter` is set) dimension of a circle/arc with
    /// the given `center`; `edge` is a point on the circle that fixes the leader
    /// direction and the measured radius (`|center → edge|`).
    RadialDim {
        center: Point2d,
        edge: Point2d,
        diameter: bool,
        height: f64,
        override_text: Option<String>,
    },
}

/// A tangency constraint on a circle: it stays tangent to entity `target`, with
/// `near` a hint point (the original pick) used to keep the same solution/side
/// when the circle is re-solved during editing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TangentRef {
    pub target: EntityId,
    pub near: Point2d,
}

#[derive(Clone, Debug)]
pub struct Entity {
    pub id: EntityId,
    pub kind: EntityKind,
    pub layer: usize,
    pub color: Color,
    pub line_type: LineTypeRef,
    pub line_weight: LineWeight,
    pub transparency: f64,
    pub xdata: XData,
    /// Tangency constraints (currently only circles created by TTR/TTT). Editing a
    /// constrained circle re-solves it to stay tangent to these targets.
    pub tangents: Vec<TangentRef>,
}

impl Entity {
    pub fn new(id: EntityId, kind: EntityKind, layer: usize) -> Self {
        Entity {
            id,
            kind,
            layer,
            color: Color::ByLayer,
            line_type: LineTypeRef::ByLayer,
            line_weight: LineWeight::ByLayer,
            transparency: 0.0,
            xdata: XData::default(),
            tangents: Vec::new(),
        }
    }

    pub fn bounding_box(&self) -> Option<BoundingBox> {
        match &self.kind {
            EntityKind::Curve(c) => Some(c.bounding_box()),
            EntityKind::Point(p) => Some(BoundingBox::new(*p, *p)),
            EntityKind::Text {
                anchor,
                height,
                content,
                ..
            } => {
                let w = 0.6 * height * content.chars().count() as f64;
                let (ax, ay) = anchor.to_f64();
                Some(BoundingBox::from_corners(ax, ay, ax + w, ay + height))
            }
            EntityKind::Insert { .. } => None,
            EntityKind::XLine { .. } | EntityKind::Ray { .. } => None,
            EntityKind::Hatch { boundary, .. } => boundary
                .iter()
                .map(|c| c.bounding_box())
                .reduce(|a, b| a.union(&b)),
            EntityKind::Dimension { p1, p2, line, .. }
            | EntityKind::OrthoDim { p1, p2, line, .. } => {
                Some(bbox_of(&[p1.to_f64(), p2.to_f64(), line.to_f64()]))
            }
            EntityKind::AngularDim {
                center,
                p1,
                p2,
                line,
                ..
            } => Some(bbox_of(&[
                center.to_f64(),
                p1.to_f64(),
                p2.to_f64(),
                line.to_f64(),
            ])),
            EntityKind::RadialDim { center, edge, .. } => {
                Some(bbox_of(&[center.to_f64(), edge.to_f64()]))
            }
        }
    }

    pub fn transform(&mut self, t: &Transform2d) {
        // A free transform (move/rotate/scale/mirror) repositions the circle off
        // its tangent lines, so the in-place tangency no longer holds — drop it.
        self.tangents.clear();
        self.kind = match &self.kind {
            EntityKind::Curve(c) => EntityKind::Curve(t.apply_curve(c)),
            EntityKind::Point(p) => EntityKind::Point(t.apply_point(p)),
            EntityKind::Text {
                anchor,
                content,
                height,
                rotation,
                font,
            } => EntityKind::Text {
                anchor: t.apply_point(anchor),
                content: content.clone(),
                height: height * t.scale_factor(),
                rotation: rotation + t.rotation_angle(),
                font: font.clone(),
            },
            EntityKind::XLine { through, dir } => EntityKind::XLine {
                through: t.apply_point(through),
                dir: transform_dir(t, dir),
            },
            EntityKind::Ray { from, dir } => EntityKind::Ray {
                from: t.apply_point(from),
                dir: transform_dir(t, dir),
            },
            EntityKind::Insert { block, transform } => EntityKind::Insert {
                block: block.clone(),
                transform: t.compose(transform),
            },
            EntityKind::Hatch {
                boundary,
                holes,
                fill,
                pattern,
            } => EntityKind::Hatch {
                boundary: boundary.iter().map(|c| t.apply_curve(c)).collect(),
                holes: holes
                    .iter()
                    .map(|h| h.iter().map(|c| t.apply_curve(c)).collect())
                    .collect(),
                fill: *fill,
                pattern: transform_pattern(pattern, t),
            },
            EntityKind::Dimension {
                p1,
                p2,
                line,
                height,
                override_text,
            } => EntityKind::Dimension {
                p1: t.apply_point(p1),
                p2: t.apply_point(p2),
                line: t.apply_point(line),
                height: height * t.scale_factor(),
                override_text: override_text.clone(),
            },
            EntityKind::OrthoDim {
                p1,
                p2,
                line,
                vertical,
                height,
                override_text,
            } => EntityKind::OrthoDim {
                p1: t.apply_point(p1),
                p2: t.apply_point(p2),
                line: t.apply_point(line),
                vertical: *vertical,
                height: height * t.scale_factor(),
                override_text: override_text.clone(),
            },
            EntityKind::AngularDim {
                center,
                p1,
                p2,
                line,
                height,
                override_text,
            } => EntityKind::AngularDim {
                center: t.apply_point(center),
                p1: t.apply_point(p1),
                p2: t.apply_point(p2),
                line: t.apply_point(line),
                height: height * t.scale_factor(),
                override_text: override_text.clone(),
            },
            EntityKind::RadialDim {
                center,
                edge,
                diameter,
                height,
                override_text,
            } => EntityKind::RadialDim {
                center: t.apply_point(center),
                edge: t.apply_point(edge),
                diameter: *diameter,
                height: height * t.scale_factor(),
                override_text: override_text.clone(),
            },
        };
    }

    pub fn transformed(&self, t: &Transform2d) -> Entity {
        let mut e = self.clone();
        e.transform(t);
        e
    }

    pub fn as_curve(&self) -> Option<&Curve> {
        if let EntityKind::Curve(c) = &self.kind {
            Some(c)
        } else {
            None
        }
    }
}

/// Axis-aligned bounding box covering a set of points (at least one).
fn bbox_of(pts: &[(f64, f64)]) -> BoundingBox {
    let (mut minx, mut miny) = pts[0];
    let (mut maxx, mut maxy) = pts[0];
    for &(x, y) in pts {
        minx = minx.min(x);
        miny = miny.min(y);
        maxx = maxx.max(x);
        maxy = maxy.max(y);
    }
    BoundingBox::from_corners(minx, miny, maxx, maxy)
}

fn transform_pattern(p: &HatchPattern, t: &Transform2d) -> HatchPattern {
    let s = t.scale_factor();
    let rot = t.rotation_angle().to_degrees();
    match *p {
        HatchPattern::Solid => HatchPattern::Solid,
        HatchPattern::Lines { angle_deg, spacing } => HatchPattern::Lines {
            angle_deg: angle_deg + rot,
            spacing: spacing * s,
        },
        HatchPattern::Cross { angle_deg, spacing } => HatchPattern::Cross {
            angle_deg: angle_deg + rot,
            spacing: spacing * s,
        },
        HatchPattern::Dots { spacing } => HatchPattern::Dots {
            spacing: spacing * s,
        },
    }
}

fn transform_dir(t: &Transform2d, dir: &(f64, f64)) -> (f64, f64) {
    let (dx, dy) = dir;
    (t.m00 * dx + t.m01 * dy, t.m10 * dx + t.m11 * dy)
}

#[cfg(test)]
mod tests {
    use super::*;
    use eiderflat_geometry::LineSeg;

    fn pt(x: i64, y: i64) -> Point2d {
        Point2d::from_i64(x, y)
    }

    #[test]
    fn entity_bbox_for_line() {
        let line = Curve::Line(LineSeg::from_endpoints(pt(0, 0), pt(4, 3)));
        let e = Entity::new(EntityId(1), EntityKind::Curve(line), 0);
        let bb = e.bounding_box().unwrap();
        assert_eq!(bb.min, pt(0, 0));
        assert_eq!(bb.max, pt(4, 3));
    }

    #[test]
    fn move_entity_translates_geometry() {
        let line = Curve::Line(LineSeg::from_endpoints(pt(0, 0), pt(2, 0)));
        let mut e = Entity::new(EntityId(1), EntityKind::Curve(line), 0);
        e.transform(&Transform2d::translation(5.0, 3.0));
        let c = e.as_curve().unwrap();
        if let Curve::Line(l) = c {
            assert_eq!(l.p0, pt(5, 3));
            assert_eq!(l.p1, pt(7, 3));
        } else {
            panic!()
        }
    }

    #[test]
    fn transformed_keeps_original() {
        let line = Curve::Line(LineSeg::from_endpoints(pt(0, 0), pt(2, 0)));
        let e = Entity::new(EntityId(1), EntityKind::Curve(line), 0);
        let moved = e.transformed(&Transform2d::translation(10.0, 0.0));
        // Original unchanged
        if let Curve::Line(l) = e.as_curve().unwrap() {
            assert_eq!(l.p0, pt(0, 0));
        }
        // Copy moved
        if let Curve::Line(l) = moved.as_curve().unwrap() {
            assert_eq!(l.p0, pt(10, 0));
        }
    }

    #[test]
    fn infinite_lines_have_no_bbox() {
        let e = Entity::new(
            EntityId(1),
            EntityKind::XLine {
                through: pt(0, 0),
                dir: (1.0, 0.0),
            },
            0,
        );
        assert!(e.bounding_box().is_none());
    }
}

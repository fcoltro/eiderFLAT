use eiderflat_document::{EntityId, EntityKind};
use eiderflat_geometry::{
    CircularArc, Curve, EllipticalArc, LineSeg, NurbsCurve, Point2d, PolyCurve, Transform2d,
    cv_spline_segments,
};
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug)]
pub enum Tool {
    Select,
    Line {
        last: Option<Point2d>,
    },
    Circle {
        center: Option<Point2d>,
    },
    Arc3 {
        pts: Vec<Point2d>,
    },
    /// Arc by start point, then centre, then end point (CCW from start to end).
    ArcStartCenterEnd {
        start: Option<Point2d>,
        center: Option<Point2d>,
    },
    /// Arc by centre, then start point, then end point (CCW from start to end).
    ArcCenterStartEnd {
        center: Option<Point2d>,
        start: Option<Point2d>,
    },
    /// Circle by two diameter endpoints.
    CircleTwoPoint {
        first: Option<Point2d>,
    },
    /// Circle through three points.
    CircleThreePoint {
        pts: Vec<Point2d>,
    },
    /// Circle of a given radius tangent to two picked entities (TTR).
    CircleTtr {
        radius: f64,
        first: Option<EntityId>,
    },
    /// Circle tangent to three picked entities (TTT).
    CircleTtt {
        picks: Vec<EntityId>,
    },
    /// Line tangent to a circle/arc — from a point, or common to two circles.
    TangentLine {
        first: Option<TanAnchor>,
    },
    /// Aligned linear dimension: pick two points, then the dimension-line offset.
    Dimension {
        p1: Option<Point2d>,
        p2: Option<Point2d>,
    },
    /// Axis-locked linear dimension (horizontal when `vertical` is false): pick
    /// two points, then the dimension-line offset.
    DimOrtho {
        vertical: bool,
        p1: Option<Point2d>,
        p2: Option<Point2d>,
    },
    /// Angular dimension: pick the vertex, a point on each ray, then the arc
    /// location. `pts` accumulates [vertex, ray1, ray2]; the 4th click places it.
    DimAngular {
        pts: Vec<Point2d>,
    },
    /// Angular dimension from two picked lines: pick line A, pick line B (their
    /// intersection is the vertex), then click to place the dimension arc. `geom`
    /// holds (vertex, ray-point-A, ray-point-B) once both lines are picked.
    DimAngularLines {
        a: Option<EntityId>,
        geom: Option<(Point2d, Point2d, Point2d)>,
    },
    /// Radius/diameter dimension: pick a circle or arc (sets `center`), then click
    /// to aim the leader. `diameter` chooses ⌀ vs R.
    DimRadial {
        diameter: bool,
        center: Option<Point2d>,
        radius: f64,
    },
    Ellipse {
        center: Option<Point2d>,
        axis_end: Option<Point2d>,
    },
    Rectangle {
        first: Option<Point2d>,
    },
    Move {
        base: Option<Point2d>,
        ids: Vec<EntityId>,
    },
    Copy {
        base: Option<Point2d>,
        ids: Vec<EntityId>,
    },
    Spline {
        pts: Vec<Point2d>,
    },
    Polyline {
        pts: Vec<Point2d>,
    },
    Polygon {
        center: Option<Point2d>,
        sides: Option<usize>,
    },
    Text {
        anchor: Option<Point2d>,
        height: f64,
    },
    Rotate {
        base: Option<Point2d>,
        ids: Vec<EntityId>,
    },
    Scale {
        base: Option<Point2d>,
        reference: Option<f64>,
        ids: Vec<EntityId>,
    },
    Mirror {
        first: Option<Point2d>,
        ids: Vec<EntityId>,
    },
    Trim,
    Extend,
    Offset {
        dist: f64,
        source: Option<EntityId>,
    },
    Fillet {
        radius: f64,
        first: Option<EntityId>,
    },
    Chamfer {
        dist: f64,
        first: Option<EntityId>,
    },
    Stretch {
        c1: Option<Point2d>,
        c2: Option<Point2d>,
        base: Option<Point2d>,
        ids: Vec<EntityId>,
    },
    Hatch,
}

/// First pick of the tangent-line tool: a free start point, or a picked
/// circle/arc (with the point it was clicked at, used to choose between the
/// possible tangents).
#[derive(Clone, Debug)]
pub enum TanAnchor {
    Point(Point2d),
    Circle(EntityId, Point2d),
}

#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug)]
pub enum ToolEvent {
    Pending,
    Create(Vec<EntityKind>),
    Transform { ids: Vec<EntityId>, t: Transform2d },
    CopyOf { ids: Vec<EntityId>, t: Transform2d },
}

impl Tool {
    pub fn name(&self) -> &'static str {
        match self {
            Tool::Select => "SELECT",
            Tool::Line { .. } => "LINE",
            Tool::Circle { .. } => "CIRCLE",
            Tool::Arc3 { .. } => "ARC",
            Tool::ArcStartCenterEnd { .. } => "ARC SCE",
            Tool::ArcCenterStartEnd { .. } => "ARC CSE",
            Tool::CircleTwoPoint { .. } => "CIRCLE 2P",
            Tool::CircleThreePoint { .. } => "CIRCLE 3P",
            Tool::CircleTtr { .. } => "CIRCLE TTR",
            Tool::CircleTtt { .. } => "CIRCLE TTT",
            Tool::TangentLine { .. } => "TANGENT",
            Tool::Dimension { .. } => "DIMENSION",
            Tool::DimOrtho { vertical: true, .. } => "DIM VERTICAL",
            Tool::DimOrtho { .. } => "DIM HORIZONTAL",
            Tool::DimAngular { .. } => "DIM ANGULAR",
            Tool::DimAngularLines { .. } => "DIM ANGULAR (2 lines)",
            Tool::DimRadial { diameter: true, .. } => "DIM DIAMETER",
            Tool::DimRadial { .. } => "DIM RADIUS",
            Tool::Ellipse { .. } => "ELLIPSE",
            Tool::Rectangle { .. } => "RECTANGLE",
            Tool::Move { .. } => "MOVE",
            Tool::Copy { .. } => "COPY",
            Tool::Spline { .. } => "SPLINE",
            Tool::Polyline { .. } => "POLYLINE",
            Tool::Polygon { .. } => "POLYGON",
            Tool::Text { .. } => "TEXT",
            Tool::Rotate { .. } => "ROTATE",
            Tool::Scale { .. } => "SCALE",
            Tool::Mirror { .. } => "MIRROR",
            Tool::Trim => "TRIM",
            Tool::Extend => "EXTEND",
            Tool::Hatch => "HATCH",
            Tool::Offset { .. } => "OFFSET",
            Tool::Fillet { .. } => "FILLET",
            Tool::Chamfer { .. } => "CHAMFER",
            Tool::Stretch { .. } => "STRETCH",
        }
    }

    pub fn is_continuous(&self) -> bool {
        matches!(self, Tool::Line { .. })
    }

    /// Tools driven by clicking existing entities (rather than placing free
    /// points). The canvas shows a pick box and highlights the entity under the
    /// cursor for these.
    pub fn picks_entities(&self) -> bool {
        matches!(
            self,
            Tool::Trim
                | Tool::Extend
                | Tool::Offset { .. }
                | Tool::Fillet { .. }
                | Tool::Chamfer { .. }
                | Tool::CircleTtr { .. }
                | Tool::CircleTtt { .. }
                | Tool::TangentLine { .. }
                | Tool::DimRadial { center: None, .. }
                | Tool::DimAngularLines { geom: None, .. }
        )
    }

    pub fn wants_point_snap(&self) -> bool {
        !matches!(
            self,
            Tool::Select
                | Tool::Trim
                | Tool::Extend
                | Tool::Hatch
                | Tool::Offset { .. }
                | Tool::Fillet { .. }
                | Tool::Chamfer { .. }
                | Tool::CircleTtr { .. }
                | Tool::CircleTtt { .. }
        )
    }

    pub fn on_point(&mut self, p: Point2d) -> ToolEvent {
        match self {
            Tool::Select | Tool::Text { .. } => ToolEvent::Pending,

            Tool::Line { last } => {
                let ev = match last.take() {
                    Some(prev) => ToolEvent::Create(vec![EntityKind::Curve(Curve::Line(
                        LineSeg::from_endpoints(prev, p),
                    ))]),
                    None => ToolEvent::Pending,
                };
                *last = Some(p);
                ev
            }

            Tool::Circle { center } => {
                match center.take() {
                    None => {
                        *center = Some(p);
                        ToolEvent::Pending
                    }
                    Some(c) => {
                        let d = c.dist_f64(&p);
                        if d < 1e-9 {
                            *center = Some(c);
                            ToolEvent::Pending
                        } else {
                            let r = d;
                            *self = Tool::Circle { center: None };
                            ToolEvent::Create(vec![EntityKind::Curve(Curve::Arc(
                                CircularArc::new(c, r, 0.0, std::f64::consts::TAU),
                            ))])
                        }
                    }
                }
            }

            Tool::Arc3 { pts } => {
                pts.push(p);
                if pts.len() == 3 {
                    let arc = CircularArc::from_three_points(&pts[0], &pts[1], &pts[2]);
                    *self = Tool::Arc3 { pts: vec![] };
                    match arc {
                        Some(a) => ToolEvent::Create(vec![EntityKind::Curve(Curve::Arc(a))]),
                        None => ToolEvent::Pending,
                    }
                } else {
                    ToolEvent::Pending
                }
            }

            Tool::ArcStartCenterEnd { start, center } => match (*start, *center) {
                (None, _) => {
                    *start = Some(p);
                    ToolEvent::Pending
                }
                (Some(_), None) => {
                    *center = Some(p);
                    ToolEvent::Pending
                }
                (Some(s), Some(c)) => match arc_start_center_end(&s, &c, &p) {
                    Some(a) => {
                        *self = Tool::ArcStartCenterEnd {
                            start: None,
                            center: None,
                        };
                        ToolEvent::Create(vec![EntityKind::Curve(Curve::Arc(a))])
                    }
                    None => ToolEvent::Pending,
                },
            },

            Tool::ArcCenterStartEnd { center, start } => match (*center, *start) {
                (None, _) => {
                    *center = Some(p);
                    ToolEvent::Pending
                }
                (Some(_), None) => {
                    *start = Some(p);
                    ToolEvent::Pending
                }
                (Some(c), Some(s)) => match arc_start_center_end(&s, &c, &p) {
                    Some(a) => {
                        *self = Tool::ArcCenterStartEnd {
                            center: None,
                            start: None,
                        };
                        ToolEvent::Create(vec![EntityKind::Curve(Curve::Arc(a))])
                    }
                    None => ToolEvent::Pending,
                },
            },

            Tool::CircleTwoPoint { first } => match first.take() {
                None => {
                    *first = Some(p);
                    ToolEvent::Pending
                }
                Some(a) => {
                    let d = a.dist_f64(&p);
                    if d < 1e-9 {
                        *first = Some(a);
                        ToolEvent::Pending
                    } else {
                        *self = Tool::CircleTwoPoint { first: None };
                        ToolEvent::Create(vec![EntityKind::Curve(Curve::Arc(CircularArc::new(
                            a.midpoint(&p),
                            d / 2.0,
                            0.0,
                            std::f64::consts::TAU,
                        )))])
                    }
                }
            },

            Tool::Dimension { p1, p2 } => match (*p1, *p2) {
                (None, _) => {
                    *p1 = Some(p);
                    ToolEvent::Pending
                }
                (Some(_), None) => {
                    *p2 = Some(p);
                    ToolEvent::Pending
                }
                (Some(a), Some(b)) => {
                    *self = Tool::Dimension { p1: None, p2: None };
                    ToolEvent::Create(vec![EntityKind::Dimension {
                        p1: a,
                        p2: b,
                        line: p,
                        height: 2.5,
                        override_text: None,
                    }])
                }
            },

            Tool::DimOrtho { vertical, p1, p2 } => {
                let vert = *vertical;
                match (*p1, *p2) {
                    (None, _) => {
                        *p1 = Some(p);
                        ToolEvent::Pending
                    }
                    (Some(_), None) => {
                        *p2 = Some(p);
                        ToolEvent::Pending
                    }
                    (Some(a), Some(b)) => {
                        *self = Tool::DimOrtho {
                            vertical: vert,
                            p1: None,
                            p2: None,
                        };
                        ToolEvent::Create(vec![EntityKind::OrthoDim {
                            p1: a,
                            p2: b,
                            line: p,
                            vertical: vert,
                            height: 2.5,
                            override_text: None,
                        }])
                    }
                }
            }

            Tool::DimAngular { pts } => {
                // Collect vertex, ray-1 point, ray-2 point; the 4th click places
                // the dimension arc and commits. Snapshot first so the `*self`
                // reassignment doesn't overlap the `pts` borrow.
                let ready = (pts.len() >= 3).then(|| (pts[0], pts[1], pts[2]));
                match ready {
                    None => {
                        pts.push(p);
                        ToolEvent::Pending
                    }
                    Some((center, a, b)) => {
                        *self = Tool::DimAngular { pts: vec![] };
                        ToolEvent::Create(vec![EntityKind::AngularDim {
                            center,
                            p1: a,
                            p2: b,
                            line: p,
                            height: 2.5,
                            override_text: None,
                        }])
                    }
                }
            }

            Tool::DimAngularLines { geom, .. } => {
                // Both lines are picked in `handle_modify_click`; this free-point
                // click places the arc and commits.
                match *geom {
                    Some((center, a, b)) => {
                        *self = Tool::DimAngularLines {
                            a: None,
                            geom: None,
                        };
                        ToolEvent::Create(vec![EntityKind::AngularDim {
                            center,
                            p1: a,
                            p2: b,
                            line: p,
                            height: 2.5,
                            override_text: None,
                        }])
                    }
                    None => ToolEvent::Pending,
                }
            }

            Tool::DimRadial {
                diameter,
                center,
                radius,
            } => {
                // The circle/arc pick is handled in `handle_modify_click`, which
                // fills in `center`/`radius`. This click aims the leader at `p`
                // (a point on the circle in the cursor direction) and commits.
                let snap = center.map(|c| (c, *radius, *diameter));
                match snap {
                    Some((c, r, dia)) => {
                        let (cx, cy) = c.to_f64();
                        let (dx, dy) = (p.x - cx, p.y - cy);
                        let len = (dx * dx + dy * dy).sqrt();
                        let edge = if len > 1e-9 {
                            Point2d::from_f64(cx + dx / len * r, cy + dy / len * r)
                        } else {
                            Point2d::from_f64(cx + r, cy)
                        };
                        *self = Tool::DimRadial {
                            diameter: dia,
                            center: None,
                            radius: 0.0,
                        };
                        ToolEvent::Create(vec![EntityKind::RadialDim {
                            center: c,
                            edge,
                            diameter: dia,
                            height: 2.5,
                            override_text: None,
                        }])
                    }
                    None => ToolEvent::Pending,
                }
            }

            Tool::CircleThreePoint { pts } => {
                pts.push(p);
                if pts.len() == 3 {
                    let res =
                        eiderflat_geometry::circle_through_three_points(pts[0], pts[1], pts[2]);
                    *self = Tool::CircleThreePoint { pts: vec![] };
                    match res {
                        Some((c, r)) => ToolEvent::Create(vec![EntityKind::Curve(Curve::Arc(
                            CircularArc::new(c, r, 0.0, std::f64::consts::TAU),
                        ))]),
                        None => ToolEvent::Pending,
                    }
                } else {
                    ToolEvent::Pending
                }
            }

            Tool::Ellipse { center, axis_end } => match (*center, *axis_end) {
                (None, _) => {
                    *center = Some(p);
                    ToolEvent::Pending
                }
                (Some(c), None) => {
                    if c.dist_f64(&p) < 1e-9 {
                        ToolEvent::Pending
                    } else {
                        *axis_end = Some(p);
                        ToolEvent::Pending
                    }
                }
                (Some(c), Some(a)) => match ellipse_from_axes(&c, &a, &p) {
                    Some(e) => {
                        *self = Tool::Ellipse {
                            center: None,
                            axis_end: None,
                        };
                        ToolEvent::Create(vec![EntityKind::Curve(Curve::Ellipse(e))])
                    }
                    None => ToolEvent::Pending,
                },
            },

            Tool::Rectangle { first } => match first.take() {
                None => {
                    *first = Some(p);
                    ToolEvent::Pending
                }
                Some(c0) => {
                    *self = Tool::Rectangle { first: None };
                    ToolEvent::Create(vec![closed_polycurve(rectangle_curves(&c0, &p))])
                }
            },

            Tool::Move { base, ids } => match base.take() {
                None => {
                    *base = Some(p);
                    ToolEvent::Pending
                }
                Some(b) => {
                    let t = Transform2d::translation(p.x - b.x, p.y - b.y);
                    let ids = std::mem::take(ids);
                    ToolEvent::Transform { ids, t }
                }
            },

            Tool::Copy { base, ids } => match base.take() {
                None => {
                    *base = Some(p);
                    ToolEvent::Pending
                }
                Some(b) => {
                    let t = Transform2d::translation(p.x - b.x, p.y - b.y);
                    ToolEvent::CopyOf {
                        ids: ids.clone(),
                        t,
                    }
                }
            },

            Tool::Spline { pts } => {
                pts.push(p);
                ToolEvent::Pending
            }

            Tool::Polyline { pts } => {
                pts.push(p);
                ToolEvent::Pending
            }

            Tool::Polygon { center, sides } => match center.take() {
                None => {
                    if sides.is_none() {
                        ToolEvent::Pending
                    } else {
                        *center = Some(p);
                        ToolEvent::Pending
                    }
                }
                Some(c) => {
                    let cx = c.x;
                    let cy = c.y;
                    let rx = p.x;
                    let ry = p.y;
                    let dx = rx - cx;
                    let dy = ry - cy;
                    let r = (dx * dx + dy * dy).sqrt();
                    let n = sides.unwrap_or(0);
                    if r < 1e-9 || n < 3 {
                        *center = Some(c);
                        ToolEvent::Pending
                    } else {
                        let start_angle = dy.atan2(dx);
                        let mut verts = Vec::with_capacity(n);
                        for i in 0..n {
                            let angle =
                                start_angle + (i as f64) * std::f64::consts::TAU / (n as f64);
                            verts.push(Point2d::from_f64(
                                cx + r * angle.cos(),
                                cy + r * angle.sin(),
                            ));
                        }
                        let mut segments = Vec::new();
                        for i in 0..n {
                            segments.push(Curve::Line(LineSeg::from_endpoints(
                                verts[i],
                                verts[(i + 1) % n],
                            )));
                        }
                        *self = Tool::Polygon {
                            center: None,
                            sides: Some(n),
                        };
                        ToolEvent::Create(vec![closed_polycurve(segments)])
                    }
                }
            },

            Tool::Rotate { base, ids } => match base.take() {
                None => {
                    *base = Some(p);
                    ToolEvent::Pending
                }
                Some(b) => {
                    let angle = (p.y - b.y).atan2(p.x - b.x);
                    let t = Transform2d::rotation_about(&b, angle);
                    ToolEvent::Transform {
                        ids: std::mem::take(ids),
                        t,
                    }
                }
            },

            Tool::Scale {
                base,
                reference,
                ids,
            } => match *base {
                None => {
                    *base = Some(p);
                    ToolEvent::Pending
                }
                Some(b) => match *reference {
                    None => {
                        *reference = Some(b.dist_f64(&p).max(1e-9));
                        ToolEvent::Pending
                    }
                    Some(r1) => {
                        let factor = (b.dist_f64(&p) / r1).max(1e-9);
                        let s = factor;
                        let t = Transform2d::scale_about(&b, s, s);
                        ToolEvent::Transform {
                            ids: std::mem::take(ids),
                            t,
                        }
                    }
                },
            },

            Tool::Mirror { first, ids } => match first.take() {
                None => {
                    *first = Some(p);
                    ToolEvent::Pending
                }
                Some(f) => {
                    let t = Transform2d::mirror_line(&f, &p);
                    ToolEvent::Transform {
                        ids: std::mem::take(ids),
                        t,
                    }
                }
            },

            Tool::Trim
            | Tool::Extend
            | Tool::Hatch
            | Tool::Offset { .. }
            | Tool::Fillet { .. }
            | Tool::Chamfer { .. }
            | Tool::Stretch { .. }
            | Tool::CircleTtr { .. }
            | Tool::CircleTtt { .. }
            | Tool::TangentLine { .. } => ToolEvent::Pending,
        }
    }

    pub fn reset(&mut self) {
        match self {
            Tool::Line { last } => *last = None,
            Tool::Circle { center } => *center = None,
            Tool::Arc3 { pts } => pts.clear(),
            Tool::ArcStartCenterEnd { start, center } => {
                *start = None;
                *center = None;
            }
            Tool::ArcCenterStartEnd { center, start } => {
                *center = None;
                *start = None;
            }
            Tool::CircleTwoPoint { first } => *first = None,
            Tool::CircleThreePoint { pts } => pts.clear(),
            Tool::CircleTtr { first, .. } => *first = None,
            Tool::CircleTtt { picks } => picks.clear(),
            Tool::TangentLine { first } => *first = None,
            Tool::Dimension { p1, p2 } => {
                *p1 = None;
                *p2 = None;
            }
            Tool::DimOrtho { p1, p2, .. } => {
                *p1 = None;
                *p2 = None;
            }
            Tool::DimAngular { pts } => pts.clear(),
            Tool::DimAngularLines { a, geom } => {
                *a = None;
                *geom = None;
            }
            Tool::DimRadial { center, radius, .. } => {
                *center = None;
                *radius = 0.0;
            }
            Tool::Ellipse { center, axis_end } => {
                *center = None;
                *axis_end = None;
            }
            Tool::Rectangle { first } => *first = None,
            Tool::Move { base, .. } | Tool::Copy { base, .. } => *base = None,
            Tool::Spline { pts } => pts.clear(),
            Tool::Polyline { pts } => pts.clear(),
            Tool::Polygon { center, .. } => *center = None,
            Tool::Rotate { base, .. } => *base = None,
            Tool::Scale {
                base, reference, ..
            } => {
                *base = None;
                *reference = None;
            }
            Tool::Mirror { first, .. } => *first = None,
            Tool::Offset { source, .. } => *source = None,
            Tool::Fillet { first, .. } => *first = None,
            Tool::Chamfer { first, .. } => *first = None,
            Tool::Stretch { c1, c2, base, .. } => {
                *c1 = None;
                *c2 = None;
                *base = None;
            }
            Tool::Text { anchor, .. } => *anchor = None,
            Tool::Trim | Tool::Extend | Tool::Hatch | Tool::Select => {}
        }
    }

    pub fn has_pending_input(&self) -> bool {
        match self {
            Tool::Line { last } => last.is_some(),
            Tool::Circle { center } => center.is_some(),
            Tool::Arc3 { pts } => !pts.is_empty(),
            Tool::ArcStartCenterEnd { start, .. } => start.is_some(),
            Tool::ArcCenterStartEnd { center, .. } => center.is_some(),
            Tool::CircleTwoPoint { first } => first.is_some(),
            Tool::CircleThreePoint { pts } => !pts.is_empty(),
            Tool::CircleTtr { first, .. } => first.is_some(),
            Tool::CircleTtt { picks } => !picks.is_empty(),
            Tool::TangentLine { first } => first.is_some(),
            Tool::Dimension { p1, .. } => p1.is_some(),
            Tool::DimOrtho { p1, .. } => p1.is_some(),
            Tool::DimAngular { pts } => !pts.is_empty(),
            Tool::DimAngularLines { a, geom } => a.is_some() || geom.is_some(),
            Tool::DimRadial { center, .. } => center.is_some(),
            Tool::Ellipse { center, .. } => center.is_some(),
            Tool::Rectangle { first } => first.is_some(),
            Tool::Move { base, .. } | Tool::Copy { base, .. } => base.is_some(),
            Tool::Spline { pts } => !pts.is_empty(),
            Tool::Polyline { pts } => !pts.is_empty(),
            Tool::Polygon { center, .. } => center.is_some(),
            Tool::Rotate { base, .. } => base.is_some(),
            Tool::Scale { base, .. } => base.is_some(),
            Tool::Mirror { first, .. } => first.is_some(),
            Tool::Offset { source, .. } => source.is_some(),
            Tool::Fillet { first, .. } => first.is_some(),
            Tool::Chamfer { first, .. } => first.is_some(),
            Tool::Stretch { c1, .. } => c1.is_some(),
            Tool::Text { anchor, .. } => anchor.is_some(),
            Tool::Trim | Tool::Extend | Tool::Hatch | Tool::Select => false,
        }
    }

    pub fn preview(&self, cursor: &Point2d) -> Vec<Curve> {
        match self {
            Tool::Line { last: Some(p) } => vec![Curve::Line(LineSeg::from_endpoints(*p, *cursor))],
            Tool::Circle { center: Some(c) } => {
                let d = c.dist_f64(cursor);
                if d < 1e-9 {
                    vec![]
                } else {
                    let r = d;
                    vec![Curve::Arc(CircularArc::new(
                        *c,
                        r,
                        0.0,
                        std::f64::consts::TAU,
                    ))]
                }
            }
            Tool::Rectangle { first: Some(c0) } => rectangle_curves(c0, cursor),
            Tool::Ellipse {
                center: Some(c),
                axis_end: None,
            } => vec![Curve::Line(LineSeg::from_endpoints(*c, *cursor))],
            Tool::Ellipse {
                center: Some(c),
                axis_end: Some(a),
            } => match ellipse_from_axes(c, a, cursor) {
                Some(e) => vec![Curve::Ellipse(e)],
                None => vec![Curve::Line(LineSeg::from_endpoints(*c, *a))],
            },
            Tool::Arc3 { pts } if pts.len() == 1 => {
                vec![Curve::Line(LineSeg::from_endpoints(pts[0], *cursor))]
            }
            Tool::Arc3 { pts } if pts.len() == 2 => {
                match CircularArc::from_three_points(&pts[0], &pts[1], cursor) {
                    Some(a) => vec![Curve::Arc(a)],
                    None => vec![Curve::Line(LineSeg::from_endpoints(pts[1], *cursor))],
                }
            }
            Tool::ArcStartCenterEnd {
                start: Some(s),
                center: None,
            } => vec![Curve::Line(LineSeg::from_endpoints(*s, *cursor))],
            Tool::ArcStartCenterEnd {
                start: Some(s),
                center: Some(c),
            } => match arc_start_center_end(s, c, cursor) {
                Some(a) => vec![Curve::Arc(a)],
                None => vec![Curve::Line(LineSeg::from_endpoints(*c, *cursor))],
            },
            Tool::ArcCenterStartEnd {
                center: Some(c),
                start: None,
            } => vec![Curve::Line(LineSeg::from_endpoints(*c, *cursor))],
            Tool::ArcCenterStartEnd {
                center: Some(c),
                start: Some(s),
            } => match arc_start_center_end(s, c, cursor) {
                Some(a) => vec![Curve::Arc(a)],
                None => vec![Curve::Line(LineSeg::from_endpoints(*s, *cursor))],
            },
            Tool::Dimension {
                p1: Some(a),
                p2: None,
            } => vec![Curve::Line(LineSeg::from_endpoints(*a, *cursor))],
            // With both points placed, the full dimension preview (extension
            // lines, arrows, text) is drawn by the canvas instead of a curve.
            Tool::CircleTwoPoint { first: Some(a) } => {
                let d = a.dist_f64(cursor);
                if d < 1e-9 {
                    vec![]
                } else {
                    vec![Curve::Arc(CircularArc::new(
                        a.midpoint(cursor),
                        d / 2.0,
                        0.0,
                        std::f64::consts::TAU,
                    ))]
                }
            }
            Tool::CircleThreePoint { pts } if pts.len() == 1 => {
                vec![Curve::Line(LineSeg::from_endpoints(pts[0], *cursor))]
            }
            Tool::CircleThreePoint { pts } if pts.len() == 2 => {
                match eiderflat_geometry::circle_through_three_points(pts[0], pts[1], *cursor) {
                    Some((c, r)) => vec![Curve::Arc(CircularArc::new(
                        c,
                        r,
                        0.0,
                        std::f64::consts::TAU,
                    ))],
                    None => vec![Curve::Line(LineSeg::from_endpoints(pts[1], *cursor))],
                }
            }
            Tool::Move { base: Some(b), .. }
            | Tool::Copy { base: Some(b), .. }
            | Tool::Rotate { base: Some(b), .. }
            | Tool::Scale { base: Some(b), .. }
            | Tool::Mirror { first: Some(b), .. }
            | Tool::Stretch { base: Some(b), .. } => {
                vec![Curve::Line(LineSeg::from_endpoints(*b, *cursor))]
            }
            Tool::Spline { pts } => {
                let mut cv = pts.clone();
                cv.push(*cursor);
                let mut out = Vec::new();
                for i in 0..cv.len().saturating_sub(1) {
                    out.push(Curve::Line(LineSeg::from_endpoints(cv[i], cv[i + 1])));
                }
                if pts.len() >= 3 {
                    out.extend(cv_spline_segments(pts).into_iter().map(Curve::Rational));
                }
                out
            }
            Tool::Polyline { pts } => {
                let mut curves = Vec::new();
                for i in 0..pts.len().saturating_sub(1) {
                    curves.push(Curve::Line(LineSeg::from_endpoints(pts[i], pts[i + 1])));
                }
                if let Some(last) = pts.last() {
                    curves.push(Curve::Line(LineSeg::from_endpoints(*last, *cursor)));
                }
                curves
            }
            Tool::Polygon {
                center: Some(c),
                sides: Some(n),
            } => {
                let cx = c.x;
                let cy = c.y;
                let rx = cursor.x;
                let ry = cursor.y;
                let dx = rx - cx;
                let dy = ry - cy;
                let r = (dx * dx + dy * dy).sqrt();
                let start_angle = dy.atan2(dx);
                let n = *n;
                let mut verts = Vec::with_capacity(n);
                for i in 0..n {
                    let angle = start_angle + (i as f64) * std::f64::consts::TAU / (n as f64);
                    verts.push(Point2d::from_f64(
                        cx + r * angle.cos(),
                        cy + r * angle.sin(),
                    ));
                }
                let mut curves = Vec::new();
                for i in 0..n {
                    curves.push(Curve::Line(LineSeg::from_endpoints(
                        verts[i],
                        verts[(i + 1) % n],
                    )));
                }
                curves
            }
            _ => vec![],
        }
    }

    pub fn reference_point(&self) -> Option<Point2d> {
        match self {
            Tool::Line { last } => *last,
            Tool::Circle { center } => *center,
            Tool::Rectangle { first } => *first,
            Tool::Arc3 { pts } => pts.last().cloned(),
            Tool::ArcStartCenterEnd { start, center } => (*center).or(*start),
            Tool::ArcCenterStartEnd { center, start } => (*start).or(*center),
            Tool::CircleTwoPoint { first } => *first,
            Tool::CircleThreePoint { pts } => pts.last().cloned(),
            Tool::Ellipse { center, axis_end } => (*axis_end).or(*center),
            Tool::Move { base, .. } => *base,
            Tool::Copy { base, .. } => *base,
            Tool::Spline { pts } => pts.last().cloned(),
            Tool::Polyline { pts } => pts.last().cloned(),
            Tool::Polygon { center, .. } => *center,
            Tool::Rotate { base, .. } => *base,
            Tool::Scale { base, .. } => *base,
            Tool::Mirror { first, .. } => *first,
            Tool::Stretch { base, c1, .. } => (*base).or(*c1),
            Tool::Text { .. }
            | Tool::Trim
            | Tool::Extend
            | Tool::Hatch
            | Tool::Offset { .. }
            | Tool::Fillet { .. }
            | Tool::Chamfer { .. }
            | Tool::CircleTtr { .. }
            | Tool::CircleTtt { .. } => None,
            Tool::TangentLine { first } => match first {
                Some(TanAnchor::Point(p)) => Some(*p),
                _ => None,
            },
            Tool::Dimension { p1, p2 } => (*p2).or(*p1),
            Tool::DimOrtho { p1, p2, .. } => (*p2).or(*p1),
            Tool::DimAngular { pts } => pts.last().cloned(),
            Tool::DimAngularLines { geom, .. } => geom.map(|(v, _, _)| v),
            Tool::DimRadial { center, .. } => *center,
            Tool::Select => None,
        }
    }

    pub fn commit(&mut self) -> ToolEvent {
        match self {
            Tool::Polyline { pts } => {
                if pts.len() >= 2 {
                    let mut segments = Vec::new();
                    for i in 0..pts.len() - 1 {
                        segments.push(Curve::Line(LineSeg::from_endpoints(pts[i], pts[i + 1])));
                    }
                    let poly = PolyCurve::new(segments);
                    *self = Tool::Polyline { pts: Vec::new() };
                    ToolEvent::Create(vec![EntityKind::Curve(Curve::Poly(Box::new(poly)))])
                } else {
                    *self = Tool::Polyline { pts: Vec::new() };
                    ToolEvent::Pending
                }
            }
            Tool::Spline { pts } => {
                let ev = spline_event(pts);
                *self = Tool::Spline { pts: Vec::new() };
                ev
            }
            _ => ToolEvent::Pending,
        }
    }

    pub fn close_and_commit(&mut self) -> ToolEvent {
        match self {
            Tool::Polyline { pts } => {
                if pts.len() >= 2 {
                    let mut segments = Vec::new();
                    for i in 0..pts.len() - 1 {
                        segments.push(Curve::Line(LineSeg::from_endpoints(pts[i], pts[i + 1])));
                    }
                    segments.push(Curve::Line(LineSeg::from_endpoints(
                        *pts.last().unwrap(),
                        pts[0],
                    )));
                    let poly = PolyCurve::new(segments);
                    *self = Tool::Polyline { pts: Vec::new() };
                    ToolEvent::Create(vec![EntityKind::Curve(Curve::Poly(Box::new(poly)))])
                } else {
                    *self = Tool::Polyline { pts: Vec::new() };
                    ToolEvent::Pending
                }
            }
            Tool::Spline { pts } => {
                let mut cv = pts.clone();
                if cv.len() >= 3 {
                    cv.push(cv[0]);
                }
                let ev = spline_event(&cv);
                *self = Tool::Spline { pts: Vec::new() };
                ev
            }
            _ => ToolEvent::Pending,
        }
    }
}
fn spline_event(cv: &[Point2d]) -> ToolEvent {
    match cv.len() {
        0 | 1 => ToolEvent::Pending,
        2 => ToolEvent::Create(vec![EntityKind::Curve(Curve::Line(
            LineSeg::from_endpoints(cv[0], cv[1]),
        ))]),
        _ => ToolEvent::Create(vec![EntityKind::Curve(Curve::Nurbs(NurbsCurve::uniform(
            cv.to_vec(),
        )))]),
    }
}
fn ellipse_from_axes(center: &Point2d, axis_end: &Point2d, p3: &Point2d) -> Option<EllipticalArc> {
    let dx = axis_end.x - center.x;
    let dy = axis_end.y - center.y;
    let semi_major = (dx * dx + dy * dy).sqrt();
    if semi_major < 1e-9 {
        return None;
    }
    let rotation = dy.atan2(dx);
    let (nx, ny) = (-rotation.sin(), rotation.cos());
    let semi_minor = ((p3.x - center.x) * nx + (p3.y - center.y) * ny).abs();
    if semi_minor < 1e-9 {
        return None;
    }
    Some(EllipticalArc::new(
        *center,
        semi_major,
        semi_minor,
        rotation,
        0.0,
        std::f64::consts::TAU,
    ))
}

fn rectangle_curves(c0: &Point2d, c1: &Point2d) -> Vec<Curve> {
    let (x0, x1) = order(c0.x, c1.x);
    let (y0, y1) = order(c0.y, c1.y);
    let p = |x: f64, y: f64| Point2d::new(x, y);
    let corners = [p(x0, y0), p(x1, y0), p(x1, y1), p(x0, y1)];
    (0..4)
        .map(|i| Curve::Line(LineSeg::from_endpoints(corners[i], corners[(i + 1) % 4])))
        .collect()
}
fn closed_polycurve(curves: Vec<Curve>) -> EntityKind {
    EntityKind::Curve(Curve::Poly(Box::new(PolyCurve::new(curves))))
}

/// Build the CCW arc from `start` to `end` about `center`. The end point is
/// projected onto the circle through `start`, so it need only give a direction.
/// Returns `None` if `start` coincides with `center`.
fn arc_start_center_end(start: &Point2d, center: &Point2d, end: &Point2d) -> Option<CircularArc> {
    let r = center.dist_f64(start);
    if r < 1e-9 {
        return None;
    }
    let sa = (start.y - center.y).atan2(start.x - center.x);
    let mut ea = (end.y - center.y).atan2(end.x - center.x);
    while ea <= sa {
        ea += std::f64::consts::TAU;
    }
    Some(CircularArc::new(*center, r, sa, ea))
}

fn order(a: f64, b: f64) -> (f64, f64) {
    if a <= b { (a, b) } else { (b, a) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pt(x: i64, y: i64) -> Point2d {
        Point2d::from_i64(x, y)
    }

    #[test]
    fn line_tool_chains_segments() {
        let mut t = Tool::Line { last: None };
        assert!(matches!(t.on_point(pt(0, 0)), ToolEvent::Pending));
        match t.on_point(pt(5, 0)) {
            ToolEvent::Create(es) => assert_eq!(es.len(), 1),
            o => panic!("{:?}", o),
        }
        assert!(matches!(t.on_point(pt(5, 5)), ToolEvent::Create(_)));
        assert!(t.is_continuous());
    }

    #[test]
    fn circle_tool_center_radius() {
        let mut t = Tool::Circle { center: None };
        assert!(matches!(t.on_point(pt(0, 0)), ToolEvent::Pending));
        match t.on_point(pt(3, 4)) {
            ToolEvent::Create(es) => {
                assert_eq!(es.len(), 1);
                if let EntityKind::Curve(Curve::Arc(a)) = &es[0] {
                    assert!((a.radius - 5.0).abs() < 1e-6);
                } else {
                    panic!()
                }
            }
            o => panic!("{:?}", o),
        }
    }

    #[test]
    fn ellipse_tool_center_axis_minor() {
        let mut t = Tool::Ellipse {
            center: None,
            axis_end: None,
        };
        assert!(matches!(t.on_point(pt(0, 0)), ToolEvent::Pending));
        assert!(matches!(t.on_point(pt(10, 0)), ToolEvent::Pending));
        match t.on_point(pt(0, 4)) {
            ToolEvent::Create(es) => {
                assert_eq!(es.len(), 1);
                if let EntityKind::Curve(Curve::Ellipse(e)) = &es[0] {
                    assert!((e.semi_major - 10.0).abs() < 1e-6);
                    assert!((e.semi_minor - 4.0).abs() < 1e-6);
                    assert!(e.rotation.abs() < 1e-9);
                } else {
                    panic!("expected an ellipse, got {:?}", es[0])
                }
            }
            o => panic!("{:?}", o),
        }
        assert!(matches!(
            t,
            Tool::Ellipse {
                center: None,
                axis_end: None
            }
        ));
    }

    #[test]
    fn rectangle_tool_makes_four_sides() {
        let mut t = Tool::Rectangle { first: None };
        assert!(matches!(t.on_point(pt(0, 0)), ToolEvent::Pending));
        match t.on_point(pt(4, 3)) {
            ToolEvent::Create(es) => {
                assert_eq!(es.len(), 1, "one entity, not four loose lines");
                if let EntityKind::Curve(Curve::Poly(poly)) = &es[0] {
                    assert_eq!(poly.segments.len(), 4);
                } else {
                    panic!("expected PolyCurve, got {:?}", es[0]);
                }
            }
            o => panic!("{:?}", o),
        }
    }

    #[test]
    fn move_tool_emits_translation() {
        let ids = vec![EntityId(1), EntityId(2)];
        let mut t = Tool::Move {
            base: None,
            ids: ids.clone(),
        };
        assert!(matches!(t.on_point(pt(0, 0)), ToolEvent::Pending));
        match t.on_point(pt(10, 5)) {
            ToolEvent::Transform { ids: got, t } => {
                assert_eq!(got, ids);
                assert_eq!(t.apply_point(&pt(0, 0)), pt(10, 5));
            }
            o => panic!("{:?}", o),
        }
    }

    #[test]
    fn copy_tool_emits_copy() {
        let mut t = Tool::Copy {
            base: None,
            ids: vec![EntityId(7)],
        };
        t.on_point(pt(1, 1));
        assert!(matches!(t.on_point(pt(4, 1)), ToolEvent::CopyOf { .. }));
    }

    #[test]
    fn arc3_needs_three_points() {
        let mut t = Tool::Arc3 { pts: vec![] };
        assert!(matches!(t.on_point(pt(1, 0)), ToolEvent::Pending));
        assert!(matches!(t.on_point(pt(0, 1)), ToolEvent::Pending));
        assert!(matches!(t.on_point(pt(-1, 0)), ToolEvent::Create(_)));
    }

    #[test]
    fn arc3_preview_matches_commit() {
        let start = pt(1, 0);
        let mid = pt(0, 1);
        let end = pt(-1, 0);

        let prev = Tool::Arc3 {
            pts: vec![start, mid],
        };
        let preview = prev.preview(&end);
        let pa = match preview.as_slice() {
            [Curve::Arc(a)] => *a,
            other => panic!("expected one arc in preview, got {:?}", other),
        };

        let mut t = Tool::Arc3 { pts: vec![] };
        t.on_point(start);
        t.on_point(mid);
        let committed = match t.on_point(end) {
            ToolEvent::Create(es) => match es.as_slice() {
                [EntityKind::Curve(Curve::Arc(a))] => *a,
                other => panic!("expected one arc, got {:?}", other),
            },
            o => panic!("{:?}", o),
        };

        assert!((pa.center.to_f64().0 - committed.center.to_f64().0).abs() < 1e-9);
        assert!((pa.center.to_f64().1 - committed.center.to_f64().1).abs() < 1e-9);
        assert!((pa.start_angle - committed.start_angle).abs() < 1e-9);
        assert!((pa.end_angle - committed.end_angle).abs() < 1e-9);
        assert!(
            (pa.included_angle() - std::f64::consts::PI).abs() < 1e-6,
            "expected a 180° arc, got {}",
            pa.included_angle()
        );
    }

    #[test]
    fn reset_clears_partial() {
        let mut t = Tool::Line { last: None };
        t.on_point(pt(0, 0));
        assert!(t.has_pending_input());
        t.reset();
        assert!(!t.has_pending_input());
    }

    #[test]
    fn polygon_creates_regular_polygon() {
        let mut t = Tool::Polygon {
            center: None,
            sides: Some(5),
        };
        assert!(matches!(t.on_point(pt(0, 0)), ToolEvent::Pending));
        match t.on_point(pt(10, 0)) {
            ToolEvent::Create(es) => {
                assert_eq!(es.len(), 1, "one entity, not five loose lines");
                if let EntityKind::Curve(Curve::Poly(poly)) = &es[0] {
                    assert_eq!(poly.segments.len(), 5);
                    if let Curve::Line(l) = &poly.segments[0] {
                        assert!(
                            (l.p0.x - 10.0).abs() < 1e-6 && l.p0.y.abs() < 1e-6,
                            "first vertex on the cursor ray"
                        );
                    } else {
                        panic!("expected Line segment");
                    }
                } else {
                    panic!("expected PolyCurve, got {:?}", es[0]);
                }
            }
            o => panic!("{:?}", o),
        }
    }

    #[test]
    fn cv_spline_commits_to_editable_nurbs() {
        let mut t = Tool::Spline { pts: vec![] };
        let cvs = [
            pt(0, 0),
            pt(5, 5),
            pt(10, -5),
            pt(15, 0),
            pt(20, 6),
            pt(25, 0),
        ];
        for p in cvs {
            assert!(matches!(t.on_point(p), ToolEvent::Pending));
        }
        match t.commit() {
            ToolEvent::Create(es) => match &es[0] {
                EntityKind::Curve(Curve::Nurbs(nc)) => {
                    assert_eq!(nc.control.len(), cvs.len());
                    assert_eq!(nc.control[0], cvs[0]);
                    assert!(nc.weights.iter().all(|&w| w == 1.0));
                }
                o => panic!("expected a Nurbs curve, got {:?}", o),
            },
            o => panic!("{:?}", o),
        }
        assert!(matches!(t, Tool::Spline { ref pts } if pts.is_empty()));
    }

    #[test]
    fn spline_preview_matches_commit_geometry() {
        let pts = vec![pt(0, 0), pt(5, 8), pt(12, 2)];
        let t = Tool::Spline { pts: pts.clone() };
        let commit_segs = eiderflat_geometry::cv_spline_segments(&pts);
        let cursor = Point2d::from_i64(99, -40);
        let preview_segs: Vec<_> = t
            .preview(&cursor)
            .into_iter()
            .filter_map(|c| match c {
                Curve::Rational(rb) => Some(rb),
                _ => None,
            })
            .collect();

        assert_eq!(
            preview_segs, commit_segs,
            "preview spline must match the committed geometry (cursor excluded)"
        );
    }

    #[test]
    fn cv_spline_two_points_is_a_line() {
        let mut t = Tool::Spline { pts: vec![] };
        t.on_point(pt(0, 0));
        t.on_point(pt(4, 2));
        assert!(matches!(t.commit(),
            ToolEvent::Create(es) if matches!(es[0], EntityKind::Curve(Curve::Line(_)))));
    }

    #[test]
    fn polyline_accumulates_and_commits() {
        let mut t = Tool::Polyline { pts: vec![] };
        assert!(matches!(t.on_point(pt(0, 0)), ToolEvent::Pending));
        assert!(matches!(t.on_point(pt(5, 5)), ToolEvent::Pending));
        assert!(matches!(t.on_point(pt(10, 0)), ToolEvent::Pending));

        match t.commit() {
            ToolEvent::Create(es) => {
                assert_eq!(es.len(), 1);
                if let EntityKind::Curve(Curve::Poly(poly)) = &es[0] {
                    assert_eq!(poly.segments.len(), 2);
                } else {
                    panic!("expected PolyCurve");
                }
            }
            o => panic!("{:?}", o),
        }
    }

    #[test]
    fn polyline_accumulates_and_closes() {
        let mut t = Tool::Polyline { pts: vec![] };
        assert!(matches!(t.on_point(pt(0, 0)), ToolEvent::Pending));
        assert!(matches!(t.on_point(pt(5, 5)), ToolEvent::Pending));
        assert!(matches!(t.on_point(pt(10, 0)), ToolEvent::Pending));

        // Close and Commit
        match t.close_and_commit() {
            ToolEvent::Create(es) => {
                assert_eq!(es.len(), 1);
                if let EntityKind::Curve(Curve::Poly(poly)) = &es[0] {
                    assert_eq!(poly.segments.len(), 3);
                } else {
                    panic!("expected PolyCurve");
                }
            }
            o => panic!("{:?}", o),
        }
    }
}

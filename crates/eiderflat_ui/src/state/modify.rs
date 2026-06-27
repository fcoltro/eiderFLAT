use super::AppState;
use crate::tools::Tool;
use eiderflat_cad::pick_at;
use eiderflat_document::EntityId;
use eiderflat_geometry::Point2d;

impl AppState {
    pub(crate) fn handle_modify_click(&mut self, p: &Point2d) -> bool {
        use eiderflat_cad::edit;
        let px = p.x;
        let py = p.y;
        let tol = self.view.pixel_world_size() * 6.0;
        let pick = |s: &Self| pick_at(&s.document, px, py, tol).filter(|&id| id != s.origin_id);

        match self.tool.clone() {
            Tool::Trim => {
                if let Some(id) = pick(self) {
                    self.history.snapshot(&self.document);
                    let cutters: Vec<EntityId> = self
                        .document
                        .iter()
                        .map(|e| e.id)
                        .filter(|&i| i != id && i != self.origin_id)
                        .collect();
                    edit::trim(&mut self.document, id, &cutters, px, py);
                    self.selection.clear();
                }
                true
            }
            Tool::Hatch => {
                self.hatch_at_point(px, py);
                true
            }
            Tool::Extend => {
                if let Some(id) = pick(self) {
                    let boundaries: Vec<EntityId> = self
                        .document
                        .iter()
                        .map(|e| e.id)
                        .filter(|&i| i != id && i != self.origin_id)
                        .collect();
                    self.history.snapshot(&self.document);
                    if !edit::extend(&mut self.document, id, &boundaries, px, py) {
                        self.history.discard_last();
                    }
                }
                true
            }
            Tool::Offset { dist, source } => {
                match source {
                    None => {
                        if let Some(id) = pick(self) {
                            self.tool = Tool::Offset {
                                dist,
                                source: Some(id),
                            };
                        }
                    }
                    Some(src) => {
                        if let Some(c) = self.document.get(src).and_then(|e| e.as_curve()).cloned()
                        {
                            let plus = eiderflat_geometry::offset_curve(&c, dist.abs());
                            let minus = eiderflat_geometry::offset_curve(&c, -dist.abs());
                            let dp = eiderflat_geometry::point_to_curve_distance(&plus, px, py);
                            let dm = eiderflat_geometry::point_to_curve_distance(&minus, px, py);
                            let signed = if dp <= dm { dist.abs() } else { -dist.abs() };
                            self.history.snapshot(&self.document);
                            edit::offset(&mut self.document, &[src], signed);
                        }
                        self.tool = Tool::Offset { dist, source: None };
                    }
                }
                true
            }
            Tool::DimRadial {
                diameter,
                center: None,
                ..
            } => {
                // Pick a circle or arc; its centre + radius drive the dimension.
                // The follow-up "aim the leader" click is a free point, handled by
                // the normal tool path once `center` is set.
                if let Some(id) = pick(self)
                    && let Some((c, r)) = self
                        .document
                        .get(id)
                        .and_then(|e| e.as_curve())
                        .and_then(circle_center_radius)
                {
                    self.tool = Tool::DimRadial {
                        diameter,
                        center: Some(c),
                        radius: r,
                    };
                }
                true
            }
            Tool::DimAngularLines { a, geom: None } => {
                // Pick two lines; their intersection is the angle vertex and a far
                // point on each line gives the ray directions.
                if let Some(id) = pick(self)
                    && line_endpoints_of(self, id).is_some()
                {
                    match a {
                        None => {
                            self.tool = Tool::DimAngularLines {
                                a: Some(id),
                                geom: None,
                            };
                        }
                        Some(first) if first != id => {
                            if let Some(g) = angular_from_lines(self, first, id) {
                                self.tool = Tool::DimAngularLines {
                                    a: Some(first),
                                    geom: Some(g),
                                };
                            }
                        }
                        _ => {}
                    }
                }
                true
            }
            Tool::Fillet { radius, first } => {
                if let Some(id) = pick(self) {
                    match first {
                        None => {
                            self.tool = Tool::Fillet {
                                radius,
                                first: Some(id),
                            }
                        }
                        Some(a) => {
                            if a != id {
                                self.history.snapshot(&self.document);
                                edit::fillet(&mut self.document, a, id, radius, px, py);
                            }
                            self.tool = Tool::Fillet {
                                radius,
                                first: None,
                            };
                        }
                    }
                }
                true
            }
            Tool::Chamfer { dist, first } => {
                if let Some(id) = pick(self) {
                    match first {
                        None => {
                            self.tool = Tool::Chamfer {
                                dist,
                                first: Some(id),
                            }
                        }
                        Some(a) => {
                            if a != id {
                                self.history.snapshot(&self.document);
                                edit::chamfer(&mut self.document, a, id, dist, dist);
                            }
                            self.tool = Tool::Chamfer { dist, first: None };
                        }
                    }
                }
                true
            }
            Tool::CircleTtr { radius, first } => {
                if let Some(id) = pick(self) {
                    match first {
                        None => {
                            self.tool = Tool::CircleTtr {
                                radius,
                                first: Some(id),
                            }
                        }
                        Some(a) => {
                            if a != id {
                                self.add_tangent_circle_ttr(a, id, radius, *p);
                            }
                            self.tool = Tool::CircleTtr {
                                radius,
                                first: None,
                            };
                        }
                    }
                }
                true
            }
            Tool::CircleTtt { mut picks } => {
                if let Some(id) = pick(self)
                    && !picks.contains(&id)
                {
                    picks.push(id);
                    if picks.len() == 3 {
                        self.add_tangent_circle_ttt([picks[0], picks[1], picks[2]], *p);
                        self.tool = Tool::CircleTtt { picks: Vec::new() };
                    } else {
                        self.tool = Tool::CircleTtt { picks };
                    }
                }
                true
            }
            Tool::TangentLine { first } => {
                self.handle_tangent_line_click(first, p);
                true
            }
            Tool::Stretch { c1, c2, base, ids } => {
                match (c1, c2, base) {
                    (None, _, _) => {
                        let ids = if self.selection.is_empty() {
                            self.document
                                .iter()
                                .map(|e| e.id)
                                .filter(|&i| i != self.origin_id)
                                .collect()
                        } else {
                            self.selection.clone()
                        };
                        self.tool = Tool::Stretch {
                            c1: Some(*p),
                            c2: None,
                            base: None,
                            ids,
                        };
                    }
                    (Some(a), None, _) => {
                        self.tool = Tool::Stretch {
                            c1: Some(a),
                            c2: Some(*p),
                            base: None,
                            ids,
                        }
                    }
                    (Some(a), Some(b), None) => {
                        self.tool = Tool::Stretch {
                            c1: Some(a),
                            c2: Some(b),
                            base: Some(*p),
                            ids,
                        }
                    }
                    (Some(a), Some(b), Some(bp)) => {
                        let (ax, ay) = a.to_f64();
                        let (bx, by) = b.to_f64();
                        let window = (ax.min(bx), ay.min(by), ax.max(bx), ay.max(by));
                        let dx = px - bp.x;
                        let dy = py - bp.y;
                        self.history.snapshot(&self.document);
                        edit::stretch(&mut self.document, &ids, window, dx, dy);
                        self.tool = Tool::Stretch {
                            c1: None,
                            c2: None,
                            base: None,
                            ids: vec![],
                        };
                    }
                }
                true
            }
            _ => false,
        }
    }

    /// Circle of `radius` tangent to entities `a` and `b`, nearest the pick.
    /// Records the two tangency constraints so the circle stays tangent on edit.
    fn add_tangent_circle_ttr(&mut self, a: EntityId, b: EntityId, radius: f64, near: Point2d) {
        let (Some(c1), Some(c2)) = (
            self.document.get(a).and_then(|e| e.as_curve()).cloned(),
            self.document.get(b).and_then(|e| e.as_curve()).cloned(),
        ) else {
            return;
        };
        if let Some((center, r)) = eiderflat_geometry::tangent_circle_ttr(&c1, &c2, radius, near)
            && let Some(id) = self.create_full_circle(center, r)
        {
            self.set_tangents(
                id,
                vec![
                    eiderflat_document::TangentRef { target: a, near },
                    eiderflat_document::TangentRef { target: b, near },
                ],
            );
        }
    }

    /// Circle tangent to three entities, nearest the final pick. Records all three
    /// tangency constraints.
    fn add_tangent_circle_ttt(&mut self, ids: [EntityId; 3], near: Point2d) {
        let curves: Vec<_> = ids
            .iter()
            .filter_map(|&id| self.document.get(id).and_then(|e| e.as_curve()).cloned())
            .collect();
        if curves.len() != 3 {
            return;
        }
        if let Some((center, r)) =
            eiderflat_geometry::tangent_circle_ttt(&curves[0], &curves[1], &curves[2], near)
            && let Some(id) = self.create_full_circle(center, r)
        {
            self.set_tangents(
                id,
                ids.iter()
                    .map(|&t| eiderflat_document::TangentRef { target: t, near })
                    .collect(),
            );
        }
    }

    fn set_tangents(&mut self, id: EntityId, tangents: Vec<eiderflat_document::TangentRef>) {
        if let Some(e) = self.document.get_mut(id) {
            e.tangents = tangents;
        }
    }

    /// Add a full circle as an entity, applying the new-object line defaults.
    /// Returns the new entity id (so callers can attach constraints).
    fn create_full_circle(&mut self, center: Point2d, r: f64) -> Option<EntityId> {
        if r <= 1e-9 {
            return None;
        }
        let arc = eiderflat_geometry::CircularArc::new(center, r, 0.0, std::f64::consts::TAU);
        self.history.snapshot(&self.document);
        let id = self.document.add(eiderflat_document::EntityKind::Curve(
            eiderflat_geometry::Curve::Arc(arc),
        ));
        self.apply_new_entity_defaults(id);
        Some(id)
    }

    /// Add a line segment as an entity, applying the new-object line defaults.
    fn create_line(&mut self, a: Point2d, b: Point2d) {
        if a.dist_f64(&b) < 1e-9 {
            return;
        }
        self.apply_tool_event(crate::tools::ToolEvent::Create(vec![
            eiderflat_document::EntityKind::Curve(eiderflat_geometry::Curve::Line(
                eiderflat_geometry::LineSeg::from_endpoints(a, b),
            )),
        ]));
    }

    /// `(centre, radius)` if the entity is a full circle / arc, else `None`.
    fn circle_of(&self, id: EntityId) -> Option<(Point2d, f64)> {
        match self.document.get(id).and_then(|e| e.as_curve()) {
            Some(eiderflat_geometry::Curve::Arc(a)) => Some((a.center, a.radius)),
            _ => None,
        }
    }

    /// Drive the tangent-line tool from a click. First pick is a free point or a
    /// circle/arc; the second resolves into a tangent line (from a point to a
    /// circle, or a common tangent of two circles).
    fn handle_tangent_line_click(&mut self, first: Option<crate::tools::TanAnchor>, p: &Point2d) {
        use crate::tools::{TanAnchor, Tool};
        let tol = self.view.pixel_world_size() * 6.0;
        let picked = pick_at(&self.document, p.x, p.y, tol).filter(|&id| id != self.origin_id);
        let picked_circle = picked.and_then(|id| self.circle_of(id).map(|c| (id, c)));

        let nearest = |pts: &[Point2d], target: Point2d| -> Option<Point2d> {
            pts.iter().copied().min_by(|a, b| {
                a.dist_sq(&target)
                    .partial_cmp(&b.dist_sq(&target))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
        };

        match first {
            None => {
                let anchor = match picked_circle {
                    Some((id, _)) => TanAnchor::Circle(id, *p),
                    None => TanAnchor::Point(*p),
                };
                self.tool = Tool::TangentLine {
                    first: Some(anchor),
                };
            }
            // From a free point, tangent to the picked circle.
            Some(TanAnchor::Point(pt)) => {
                if let Some((_, (o, r))) = picked_circle {
                    let touches = eiderflat_geometry::tangent_points_from_point(o, r, pt);
                    if let Some(t) = nearest(&touches, *p) {
                        self.create_line(pt, t);
                    }
                    self.tool = Tool::TangentLine { first: None };
                }
                // Otherwise keep waiting for a circle/arc pick.
            }
            Some(TanAnchor::Circle(aid, aclick)) => {
                let Some((o1, r1)) = self.circle_of(aid) else {
                    self.tool = Tool::TangentLine { first: None };
                    return;
                };
                match picked_circle {
                    // Common tangent between the two circles.
                    Some((bid, (o2, r2))) if bid != aid => {
                        let segs = eiderflat_geometry::common_tangent_segments(o1, r1, o2, r2);
                        let best = segs.into_iter().min_by(|x, y| {
                            let cost =
                                |s: &(Point2d, Point2d)| s.0.dist_sq(&aclick) + s.1.dist_sq(p);
                            cost(x)
                                .partial_cmp(&cost(y))
                                .unwrap_or(std::cmp::Ordering::Equal)
                        });
                        if let Some((t1, t2)) = best {
                            self.create_line(t1, t2);
                        }
                        self.tool = Tool::TangentLine { first: None };
                    }
                    // Second pick is a free point: tangent from it to the circle.
                    _ => {
                        let touches = eiderflat_geometry::tangent_points_from_point(o1, r1, *p);
                        if let Some(t) = nearest(&touches, aclick) {
                            self.create_line(*p, t);
                        }
                        self.tool = Tool::TangentLine { first: None };
                    }
                }
            }
        }
    }

    pub fn trim_extend_preview(&self) -> Option<TrimExtendPreview> {
        use eiderflat_cad::edit;
        let (px, py) = self.cursor_world;
        let tol = self.view.pixel_world_size() * 6.0;
        let id = pick_at(&self.document, px, py, tol)?;
        match self.tool {
            Tool::Trim => {
                let cutters: Vec<EntityId> = self
                    .document
                    .iter()
                    .map(|e| e.id)
                    .filter(|&i| i != id)
                    .collect();
                edit::trim_preview(&self.document, id, &cutters, px, py)
                    .map(TrimExtendPreview::Remove)
            }
            Tool::Extend => {
                let boundaries: Vec<EntityId> = self
                    .document
                    .iter()
                    .map(|e| e.id)
                    .filter(|&i| i != id)
                    .collect();
                edit::extend_preview(&self.document, id, &boundaries, px, py)
                    .map(TrimExtendPreview::Extension)
            }
            _ => None,
        }
    }
}

pub enum TrimExtendPreview {
    Remove(eiderflat_geometry::Curve),
    Extension(eiderflat_geometry::Curve),
}

/// Extract a (centre, radius) pair from a circular curve — a full circle or a
/// circular arc. Returns `None` for anything else, so radius/diameter dimensions
/// only attach to round geometry.
fn circle_center_radius(c: &eiderflat_geometry::Curve) -> Option<(Point2d, f64)> {
    match c {
        eiderflat_geometry::Curve::Arc(a) => Some((a.center, a.radius)),
        _ => None,
    }
}

/// Endpoints of a picked straight line entity, or `None` if it isn't a line.
fn line_endpoints_of(app: &AppState, id: EntityId) -> Option<(Point2d, Point2d)> {
    match app.document.get(id)?.as_curve()? {
        eiderflat_geometry::Curve::Line(l) => Some((l.p0, l.p1)),
        _ => None,
    }
}

/// Build angular-dimension geometry (vertex + a ray point on each line) from two
/// picked lines, using their infinite-line intersection as the vertex.
fn angular_from_lines(
    app: &AppState,
    a: EntityId,
    b: EntityId,
) -> Option<(Point2d, Point2d, Point2d)> {
    let (a0, a1) = line_endpoints_of(app, a)?;
    let (b0, b1) = line_endpoints_of(app, b)?;
    let vertex = eiderflat_geometry::intersect_lines_unbounded(
        &eiderflat_geometry::LineSeg::from_endpoints(a0, a1),
        &eiderflat_geometry::LineSeg::from_endpoints(b0, b1),
    )?;
    // Ray point on each line = the endpoint farthest from the vertex.
    let far = |p: Point2d, q: Point2d| {
        if vertex.dist_f64(&p) >= vertex.dist_f64(&q) {
            p
        } else {
            q
        }
    };
    Some((vertex, far(a0, a1), far(b0, b1)))
}

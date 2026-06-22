use eiderflat_geometry::{Curve, CurveSegment, BoundingBox, Point2d, LineSeg, intersect};
use eiderflat_document::{Document, EntityId, EntityKind};

pub fn pick_at(doc: &Document, x: f64, y: f64, tol: f64) -> Option<EntityId> {
    for e in doc.editable_entities().rev() {
        if let Some(bb) = e.bounding_box()
            && (x < bb.min.x - tol || x > bb.max.x + tol
                || y < bb.min.y - tol || y > bb.max.y + tol) { continue; }
        match &e.kind {
            EntityKind::Curve(c) => {
                let pr = eiderflat_geometry::project_point_onto_curve(c, x, y);
                if pr.distance <= tol { return Some(e.id); }
            }
            EntityKind::Point(p) => {
                let (px, py) = p.to_f64();
                if ((px - x).powi(2) + (py - y).powi(2)).sqrt() <= tol { return Some(e.id); }
            }
            EntityKind::Text { .. } => {
                return Some(e.id);
            }
            EntityKind::Hatch { boundary, holes, .. }
                if crate::hatch::region_contains(boundary, holes, x, y) => return Some(e.id),
            _ => {}
        }
    }
    None
}

pub fn select_window(doc: &Document, rect: &BoundingBox) -> Vec<EntityId> {
    doc.editable_entities()
        .filter(|e| e.bounding_box().is_some_and(|bb| bbox_inside(&bb, rect)))
        .map(|e| e.id)
        .collect()
}

pub fn select_crossing(doc: &Document, rect: &BoundingBox) -> Vec<EntityId> {
    doc.editable_entities()
        .filter(|e| match &e.kind {
            EntityKind::Curve(c) => curve_touches_rect(c, rect),
            _ => e.bounding_box().is_some_and(|bb| bb.intersects(rect)),
        })
        .map(|e| e.id)
        .collect()
}

pub fn select_fence(doc: &Document, fence: &[Point2d]) -> Vec<EntityId> {
    if fence.len() < 2 { return vec![]; }
    let segs: Vec<LineSeg> = fence.windows(2)
        .map(|w| LineSeg::from_endpoints(w[0], w[1]))
        .collect();

    doc.editable_entities()
        .filter(|e| {
            if let EntityKind::Curve(c) = &e.kind {
                segs.iter().any(|s| !intersect(&Curve::Line(s.clone()), c).is_empty())
            } else { false }
        })
        .map(|e| e.id)
        .collect()
}

pub fn select_by<F: Fn(&eiderflat_document::Entity) -> bool>(doc: &Document, pred: F) -> Vec<EntityId> {
    doc.editable_entities().filter(|e| pred(e)).map(|e| e.id).collect()
}

fn bbox_inside(inner: &BoundingBox, outer: &BoundingBox) -> bool {
    inner.min.x >= outer.min.x && inner.max.x <= outer.max.x &&
    inner.min.y >= outer.min.y && inner.max.y <= outer.max.y
}

fn curve_touches_rect(c: &Curve, rect: &BoundingBox) -> bool {
    if !c.bounding_box().intersects(rect) { return false; }
    let (t0, t1) = c.domain();
    for i in 0..=8 {
        let t = t0 + (t1 - t0) * i as f64 / 8.0;
        let (x, y) = c.evaluate_f64(t);
        if rect.contains_point_f64(x, y) { return true; }
    }
    let (x0, y0) = rect.min.to_f64();
    let (x1, y1) = rect.max.to_f64();
    let corners = [
        Point2d::from_f64(x0, y0), Point2d::from_f64(x1, y0),
        Point2d::from_f64(x1, y1), Point2d::from_f64(x0, y1),
    ];
    for i in 0..4 {
        let side = Curve::Line(LineSeg::from_endpoints(
            corners[i], corners[(i + 1) % 4]));
        if !intersect(&side, c).is_empty() { return true; }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use eiderflat_document::EntityKind;

    fn pt(x: i64, y: i64) -> Point2d { Point2d::from_i64(x, y) }
    fn line(x0: i64, y0: i64, x1: i64, y1: i64) -> EntityKind {
        EntityKind::Curve(Curve::Line(LineSeg::from_endpoints(pt(x0,y0), pt(x1,y1))))
    }

    fn sample_doc() -> (Document, EntityId, EntityId) {
        let mut doc = Document::new();
        let a = doc.add(line(1, 1, 3, 3));
        let b = doc.add(line(4, 4, 8, 8));
        (doc, a, b)
    }

    #[test]
    fn window_selects_only_fully_inside() {
        let (doc, a, b) = sample_doc();
        let rect = BoundingBox::from_corners(0.0, 0.0, 5.0, 5.0);
        let sel = select_window(&doc, &rect);
        assert!(sel.contains(&a));
        assert!(!sel.contains(&b), "partially-outside entity must not be window-selected");
    }

    #[test]
    fn crossing_selects_touching() {
        let (doc, a, b) = sample_doc();
        let rect = BoundingBox::from_corners(0.0, 0.0, 5.0, 5.0);
        let sel = select_crossing(&doc, &rect);
        assert!(sel.contains(&a));
        assert!(sel.contains(&b), "crossing entity must be selected");
    }

    #[test]
    fn pick_at_finds_curve() {
        let (doc, a, _) = sample_doc();
        assert_eq!(pick_at(&doc, 2.0, 2.0, 0.1), Some(a));
        assert_eq!(pick_at(&doc, 100.0, 100.0, 0.1), None);
    }

    #[test]
    fn fence_crosses_entities() {
        let (doc, _a, b) = sample_doc();
        let fence = vec![pt(5, 3), pt(5, 9)];
        let sel = select_fence(&doc, &fence);
        assert!(sel.contains(&b));
    }

    #[test]
    fn select_by_layer() {
        let mut doc = Document::new();
        doc.layers.add(eiderflat_document::Layer::new("special"));
        let special_idx = doc.layers.index_of("special").unwrap();
        doc.add(line(0,0,1,1));
        let s = doc.add_on_layer(line(2,2,3,3), special_idx);
        let sel = select_by(&doc, |e| e.layer == special_idx);
        assert_eq!(sel, vec![s]);
    }

    #[test]
    fn pick_respects_layer_lock() {
        let mut doc = Document::new();
        let id = doc.add(line(0, 0, 4, 0));
        doc.layers.get_mut(0).unwrap().locked = true;
        assert_eq!(pick_at(&doc, 2.0, 0.0, 0.1), None);
        let _ = id;
    }
}

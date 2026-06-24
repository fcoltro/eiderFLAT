use eiderflat_document::EntityKind;
use eiderflat_geometry::{Curve, LineSeg, Point2d};
use eiderflat_ui::AppState;

fn line(x0: i64, y0: i64, x1: i64, y1: i64) -> EntityKind {
    EntityKind::Curve(Curve::Line(LineSeg::from_endpoints(
        Point2d::from_i64(x0, y0),
        Point2d::from_i64(x1, y1),
    )))
}

fn click(a: &mut AppState, wx: f64, wy: f64) {
    let (sx, sy) = a.view.world_to_screen(wx, wy);
    a.canvas_click(sx, sy);
}

fn app() -> AppState {
    let mut a = AppState::new(1200.0, 800.0);
    a.snap_on = false;
    a
}

#[test]
fn trim_tool_cuts_picked_span() {
    let mut a = app();
    a.add_entity(line(0, 0, 10, 0));
    a.add_entity(line(3, -1, 3, 1));
    a.add_entity(line(7, -1, 7, 1));
    let before = a.document.len();
    a.run_command("TRIM");
    click(&mut a, 5.0, 0.0);
    assert_eq!(
        a.document.len(),
        before + 1,
        "trim should split target into two"
    );
}

#[test]
fn trim_ignores_object_snap_when_picking() {
    let mut a = AppState::new(1200.0, 800.0);
    a.snap_on = true;
    a.add_entity(line(0, 0, 10, 0));
    a.add_entity(line(3, -1, 3, 1));
    a.add_entity(line(7, -1, 7, 1));
    let before = a.document.len();
    a.run_command("TRIM");
    let (sx, sy) = a.view.world_to_screen(3.1, 0.05);
    a.pointer_moved(sx, sy);
    assert!(
        a.active_snap.is_none(),
        "entity-picking tools must not object-snap"
    );
    click(&mut a, 5.0, 0.0);
    assert_eq!(
        a.document.len(),
        before + 1,
        "trim must still cut the picked span"
    );
}

#[test]
fn offset_tool_creates_parallel_curve() {
    let mut a = app();
    a.add_entity(line(0, 0, 10, 0));
    let before = a.document.len();
    a.run_command("OFFSET");
    a.run_command("2");
    click(&mut a, 5.0, 0.0);
    click(&mut a, 5.0, 4.0);
    assert_eq!(
        a.document.len(),
        before + 1,
        "offset should add one parallel curve"
    );
}

#[test]
fn fillet_tool_adds_arc() {
    let mut a = app();
    a.add_entity(line(10, 0, 0, 0));
    a.add_entity(line(0, 0, 0, 10));
    let before = a.document.len();
    a.run_command("FILLET");
    a.run_command("2");
    click(&mut a, 5.0, 0.0);
    click(&mut a, 0.0, 5.0);
    assert_eq!(
        a.document.len(),
        before + 1,
        "fillet adds one arc (lines trimmed in place)"
    );
    assert!(
        a.document
            .iter()
            .any(|e| matches!(&e.kind, EntityKind::Curve(Curve::Arc(_)))),
        "a fillet arc should exist"
    );
}

#[test]
fn fillet_triangle_caps_radius_across_shared_edges() {
    use eiderflat_ui::state::CornerKind;
    use std::collections::HashMap;

    // Three separate lines forming a right triangle. Each side is shared by two
    // corners, so the uniform fillet radius must be small enough that both
    // tangents fit on every side. Without that the trims overrun and mangle the
    // sides (lines shoot far past the triangle).
    let mut a = app();
    let i1 = a.add_entity(line(0, 0, 10, 0));
    let i2 = a.add_entity(line(10, 0, 0, 10));
    let i3 = a.add_entity(line(0, 10, 0, 0));
    a.selection = vec![i1, i2, i3];

    let corners = a.detect_corners();
    assert_eq!(corners.len(), 3, "triangle has three corners");

    let cap = a.corner_group_cap(&corners[0], CornerKind::Fillet);

    // Sum the tangent lengths each corner consumes on every edge it touches.
    let mut budget: HashMap<_, (f64, f64)> = HashMap::new();
    for c in &corners {
        let t = cap / (c.interior_angle() * 0.5).tan();
        for (id, len) in [(c.a, c.len_a), (c.b, c.len_b)] {
            let e = budget.entry(id).or_insert((0.0, f64::INFINITY));
            e.0 += t;
            e.1 = e.1.min(len);
        }
    }
    for (sum_t, len) in budget.values() {
        assert!(
            *sum_t <= *len + 1e-6,
            "fillet tangents {sum_t:.3} overrun a shared edge of length {len:.3}"
        );
    }

    // End to end: a huge requested radius is clamped, yielding one arc per
    // corner with every endpoint still inside the triangle's bounding box.
    a.begin_corner_action(corners[0]);
    a.set_corner_size(1e6);
    a.apply_corner_action();
    let arcs = a
        .document
        .iter()
        .filter(|e| matches!(&e.kind, EntityKind::Curve(Curve::Arc(_))))
        .count();
    assert_eq!(arcs, 3, "one fillet arc per corner");
    for e in a.document.iter() {
        if let EntityKind::Curve(Curve::Line(l)) = &e.kind {
            for p in [l.p0.to_f64(), l.p1.to_f64()] {
                assert!(
                    p.0 > -0.5 && p.0 < 10.5 && p.1 > -0.5 && p.1 < 10.5,
                    "line endpoint {p:?} escaped the triangle"
                );
            }
        }
    }
}

#[test]
fn rotate_tool_turns_selection() {
    let mut a = app();
    let id = a.add_entity(line(1, 0, 2, 0));
    a.selection = vec![id];
    a.run_command("ROTATE");
    click(&mut a, 0.0, 0.0);
    click(&mut a, 0.0, 1.0);
    if let Some(Curve::Line(l)) = a.document.get(id).unwrap().as_curve() {
        assert!(
            l.p0.x.abs() < 1e-4 && (l.p0.y - 1.0).abs() < 1e-4,
            "(1,0) → (0,1), got {:?}",
            l.p0.to_f64()
        );
    } else {
        panic!("expected a line")
    }
}

#[test]
fn mirror_tool_reflects_selection() {
    let mut a = app();
    let id = a.add_entity(line(1, 2, 3, 4));
    a.selection = vec![id];
    a.run_command("MIRROR");
    click(&mut a, 0.0, 0.0);
    click(&mut a, 1.0, 0.0);
    if let Some(Curve::Line(l)) = a.document.get(id).unwrap().as_curve() {
        let (x, y) = l.p0.to_f64();
        assert!(
            (x - 1.0).abs() < 1e-4 && (y + 2.0).abs() < 1e-4,
            "(1,2) → (1,-2), got ({x},{y})"
        );
    } else {
        panic!("expected a line")
    }
}

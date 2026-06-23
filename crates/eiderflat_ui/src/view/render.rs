use super::tessellate::draw_curve;
use crate::state::AppState;
use crate::tools::Tool;
use egui::{Color32, Stroke, pos2, vec2};
use eiderflat_document::{Color, EntityId, EntityKind};
use eiderflat_geometry::{Curve, CurveSegment, Point2d};

pub(super) fn tool_prompt(tool: &Tool) -> String {
    match tool {
        Tool::Line { last } => {
            if last.is_none() {
                "Specify start point".into()
            } else {
                "Specify next point or length".into()
            }
        }
        Tool::Circle { center } => {
            if center.is_none() {
                "Specify center point".into()
            } else {
                "Specify radius".into()
            }
        }
        Tool::Rectangle { first } => {
            if first.is_none() {
                "Specify first corner".into()
            } else {
                "Specify opposite corner".into()
            }
        }
        Tool::Arc3 { pts } => match pts.len() {
            0 => "Specify start point".into(),
            1 => "Specify second point".into(),
            _ => "Specify end point".into(),
        },
        Tool::Ellipse { center, axis_end } => match (center, axis_end) {
            (None, _) => "Specify center of ellipse".into(),
            (Some(_), None) => "Specify end of first axis".into(),
            (Some(_), Some(_)) => "Specify distance to other axis".into(),
        },
        Tool::Move { base, .. } => {
            if base.is_none() {
                "Specify base point".into()
            } else {
                "Specify destination".into()
            }
        }
        Tool::Copy { base, .. } => {
            if base.is_none() {
                "Specify base point".into()
            } else {
                "Specify destination".into()
            }
        }
        Tool::Rotate { base, .. } => {
            if base.is_none() {
                "Specify base point".into()
            } else {
                "Specify rotation angle".into()
            }
        }
        Tool::Scale { base, .. } => {
            if base.is_none() {
                "Specify base point".into()
            } else {
                "Specify scale factor".into()
            }
        }
        Tool::Mirror { first, .. } => {
            if first.is_none() {
                "Specify first point of mirror axis".into()
            } else {
                "Specify second point of mirror axis".into()
            }
        }
        Tool::Polygon { center, sides } => {
            if center.is_none() {
                match sides {
                    Some(n) => format!("Sides: {n} — click the center (or type a new count)"),
                    None => "Type the number of sides (3+), then click the center".into(),
                }
            } else {
                "Click to set the radius".into()
            }
        }
        Tool::Spline { pts } => {
            if pts.is_empty() {
                "Specify first control point".into()
            } else {
                format!(
                    "Specify next control vertex ({} placed) — Enter/right-click finishes, C closes",
                    pts.len()
                )
            }
        }
        Tool::Polyline { pts } => {
            if pts.is_empty() {
                "Specify start point".into()
            } else {
                "Specify next point — Enter/right-click finishes".into()
            }
        }
        Tool::Text { anchor, .. } => {
            if anchor.is_none() {
                "Specify text anchor point".into()
            } else {
                "Type the text, Enter to place".into()
            }
        }
        Tool::Offset { source, .. } => {
            if source.is_none() {
                "Click the curve to offset (type a distance first)".into()
            } else {
                "Click the side to offset towards".into()
            }
        }
        Tool::Trim => "Click the segment piece to cut away".into(),
        Tool::Extend => "Click the end to lengthen".into(),
        Tool::Hatch => "Click inside an area to hatch it".into(),
        Tool::Fillet { first, .. } => {
            if first.is_none() {
                "Pick the first line".into()
            } else {
                "Pick the second line".into()
            }
        }
        Tool::Chamfer { first, .. } => {
            if first.is_none() {
                "Pick the first line".into()
            } else {
                "Pick the second line".into()
            }
        }
        Tool::Stretch { c1, c2, base, .. } => match (c1, c2, base) {
            (None, _, _) => "Specify first corner of crossing window".into(),
            (Some(_), None, _) => "Specify opposite corner".into(),
            (_, _, None) => "Specify base point".into(),
            _ => "Specify destination".into(),
        },
        Tool::Select => "Click an entity, or drag a window".into(),
    }
}

pub(super) fn draw_prompt_chip(painter: &egui::Painter, rect: egui::Rect, text: &str) {
    let galley = painter.layout_no_wrap(
        text.to_string(),
        egui::FontId::proportional(13.0),
        crate::theme::TEXT,
    );
    let pad = vec2(14.0, 7.0);
    let size = galley.size() + pad * 2.0;
    let bottom_center = pos2(
        rect.center().x - size.x / 2.0,
        rect.bottom() - 56.0 - size.y,
    );
    let bg = egui::Rect::from_min_size(bottom_center, size);
    painter.rect(
        bg,
        16.0,
        Color32::from_rgba_unmultiplied(27, 34, 46, 235),
        Stroke::new(1.0, crate::theme::OUTLINE),
        egui::StrokeKind::Middle,
    );
    painter.galley(bg.min + pad, galley, crate::theme::TEXT);
}

pub(super) fn draw_grid(
    painter: &egui::Painter,
    app: &AppState,
    rect: egui::Rect,
    to_screen: &impl Fn(f64, f64) -> egui::Pos2,
) {
    // Subtle background gradient: a faint lift toward the top, fading into the
    // flat canvas colour at the bottom (mirrors the reference mockup).
    {
        let top = Color32::from_rgb(17, 21, 29);
        let bot = crate::theme::CANVAS_BG;
        let mut mesh = egui::epaint::Mesh::default();
        mesh.colored_vertex(rect.left_top(), top);
        mesh.colored_vertex(rect.right_top(), top);
        mesh.colored_vertex(rect.right_bottom(), bot);
        mesh.colored_vertex(rect.left_bottom(), bot);
        mesh.add_triangle(0, 1, 2);
        mesh.add_triangle(0, 2, 3);
        painter.add(egui::Shape::mesh(mesh));
    }

    let major = app.view.grid_spacing();
    if !(major.is_finite() && major > 0.0) {
        return;
    }
    let (x0, y0, x1, y1) = app.view.visible_bounds();
    // Continuous grid lines (replacing the old dot grid). Every fifth line is
    // emphasised; the world axes are brighter still.
    let minor = Stroke::new(1.0, Color32::from_rgb(24, 28, 36));
    let major_line = Stroke::new(1.0, Color32::from_rgb(33, 39, 49));
    let axis = Stroke::new(1.0, Color32::from_rgb(58, 66, 80));

    // Index of the first/over-the-edge grid line, so we can tag every 5th one.
    let ix0 = (x0 / major).floor() as i64;
    let iy0 = (y0 / major).floor() as i64;

    let mut i = ix0;
    let mut gx = ix0 as f64 * major;
    while gx <= x1 {
        let sx = to_screen(gx, y0).x;
        let stroke = if i % 5 == 0 { major_line } else { minor };
        painter.line_segment([pos2(sx, rect.top()), pos2(sx, rect.bottom())], stroke);
        i += 1;
        gx += major;
    }

    let mut j = iy0;
    let mut gy = iy0 as f64 * major;
    while gy <= y1 {
        let sy = to_screen(x0, gy).y;
        let stroke = if j % 5 == 0 { major_line } else { minor };
        painter.line_segment([pos2(rect.left(), sy), pos2(rect.right(), sy)], stroke);
        j += 1;
        gy += major;
    }

    // World axes on top of the grid.
    if x0 <= 0.0 && x1 >= 0.0 {
        let a = to_screen(0.0, y0);
        painter.line_segment([pos2(a.x, rect.top()), pos2(a.x, rect.bottom())], axis);
    }
    if y0 <= 0.0 && y1 >= 0.0 {
        let a = to_screen(x0, 0.0);
        painter.line_segment([pos2(rect.left(), a.y), pos2(rect.right(), a.y)], axis);
    }
}

pub(super) fn draw_scale_bar(painter: &egui::Painter, app: &AppState, rect: egui::Rect) {
    let pws = app.view.pixel_world_size();
    if !(pws.is_finite() && pws > 0.0) {
        return;
    }
    let target_px = 120.0_f64;
    let raw = target_px * pws;
    let mag = raw.log10().floor();
    let base = 10f64.powf(mag);
    let nice = if raw / base < 1.5 {
        base
    } else if raw / base < 3.5 {
        2.0 * base
    } else if raw / base < 7.5 {
        5.0 * base
    } else {
        10.0 * base
    };
    let bar_px = (nice / pws) as f32;
    if !bar_px.is_finite() || bar_px <= 0.0 {
        return;
    }

    let unit = app.document.settings.units.short_name();
    let label = format!("{} {}", format_distance(nice), unit);
    let label = label.trim_end().to_string();
    let margin = 16.0;
    let y = rect.bottom() - margin;
    let x1 = rect.right() - margin;
    let x0 = x1 - bar_px;
    let cap = 5.0;
    // Blue scale bar with a soft dark shadow so it stays legible over drawings.
    let bar = Stroke::new(2.0, crate::theme::ACCENT);
    let shadow = Stroke::new(3.5, Color32::from_rgba_unmultiplied(0, 0, 0, 150));
    for s in [shadow, bar] {
        painter.line_segment([pos2(x0, y), pos2(x1, y)], s);
        painter.line_segment([pos2(x0, y - cap), pos2(x0, y + cap)], s);
        painter.line_segment([pos2(x1, y - cap), pos2(x1, y + cap)], s);
    }
    // Value sits in a darker rounded chip, centred above the bar.
    let tx = (x0 + x1) / 2.0;
    let galley = painter.layout_no_wrap(
        label.clone(),
        egui::FontId::monospace(12.0),
        crate::theme::TEXT,
    );
    let pad = vec2(8.0, 3.0);
    let chip = egui::Rect::from_center_size(
        pos2(tx, y - cap - 2.0 - galley.size().y / 2.0 - pad.y),
        galley.size() + pad * 2.0,
    );
    painter.rect(
        chip,
        7.0,
        crate::theme::PANEL_GLASS,
        Stroke::new(1.0, crate::theme::OUTLINE),
        egui::StrokeKind::Inside,
    );
    painter.text(
        chip.center(),
        egui::Align2::CENTER_CENTER,
        &label,
        egui::FontId::monospace(12.0),
        crate::theme::ACCENT_BRIGHT,
    );
}
pub(super) fn format_distance(d: f64) -> String {
    if d >= 1.0 && (d.fract()).abs() < 1e-9 {
        format!("{}", d.round() as i64)
    } else {
        let s = format!("{:.6}", d);
        let s = s.trim_end_matches('0').trim_end_matches('.');
        s.to_string()
    }
}

pub(super) fn draw_dashed_line(
    painter: &egui::Painter,
    start: egui::Pos2,
    end: egui::Pos2,
    stroke: Stroke,
    dash_length: f32,
    gap_length: f32,
) {
    if !start.x.is_finite() || !start.y.is_finite() || !end.x.is_finite() || !end.y.is_finite() {
        return;
    }
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let len = (dx * dx + dy * dy).sqrt();
    if !len.is_finite() || len < 1e-6 {
        return;
    }
    let ux = dx / len;
    let uy = dy / len;

    let mut dist = 0.0;
    let mut count = 0;
    while dist < len && count < 1000 {
        let next_dist = (dist + dash_length).min(len);
        let p1 = pos2(start.x + ux * dist, start.y + uy * dist);
        let p2 = pos2(start.x + ux * next_dist, start.y + uy * next_dist);
        painter.line_segment([p1, p2], stroke);
        dist += dash_length + gap_length;
        count += 1;
    }
}
pub(super) fn draw_transform_ghost(
    painter: &egui::Painter,
    app: &AppState,
    to_screen: &impl Fn(f64, f64) -> egui::Pos2,
) {
    use eiderflat_geometry::Transform2d;
    let (cx, cy) = app.cursor_world;
    let ghost = Stroke::new(1.5, crate::theme::PREVIEW);
    if let Tool::Offset {
        dist,
        source: Some(src),
    } = &app.tool
    {
        if let Some(c) = app.document.get(*src).and_then(|e| e.as_curve()) {
            let plus = eiderflat_geometry::offset_curve(c, dist.abs());
            let minus = eiderflat_geometry::offset_curve(c, -dist.abs());
            let dp = eiderflat_geometry::point_to_curve_distance(&plus, cx, cy);
            let dm = eiderflat_geometry::point_to_curve_distance(&minus, cx, cy);
            let chosen = if dp <= dm { plus } else { minus };
            draw_curve(painter, &chosen, to_screen, ghost);
        }
        return;
    }

    let (t, ids): (Transform2d, &Vec<eiderflat_document::EntityId>) = match &app.tool {
        Tool::Move { base: Some(b), ids } | Tool::Copy { base: Some(b), ids } => {
            let (bx, by) = b.to_f64();
            (Transform2d::translation(cx - bx, cy - by), ids)
        }
        Tool::Rotate { base: Some(b), ids } => {
            let (bx, by) = b.to_f64();
            (
                Transform2d::rotation_about(b, (cy - by).atan2(cx - bx)),
                ids,
            )
        }
        Tool::Scale {
            base: Some(b),
            reference: Some(r1),
            ids,
        } => {
            let factor = (b.dist_f64(&Point2d::from_f64(cx, cy)) / r1).max(1e-9);
            (Transform2d::scale_about(b, factor, factor), ids)
        }
        Tool::Mirror {
            first: Some(f),
            ids,
        } => {
            let (fx, fy) = f.to_f64();
            if (cx - fx).hypot(cy - fy) < 1e-9 {
                return;
            }
            (Transform2d::mirror_line(f, &Point2d::from_f64(cx, cy)), ids)
        }
        _ => return,
    };
    let sel = if ids.is_empty() { &app.selection } else { ids };
    for &id in sel {
        if id == app.origin_id {
            continue;
        }
        if let Some(c) = app.document.get(id).and_then(|e| e.as_curve()) {
            draw_curve(painter, &t.apply_curve(c), to_screen, ghost);
        }
    }
}
pub(super) fn draw_trim_extend_preview(
    painter: &egui::Painter,
    app: &AppState,
    to_screen: &impl Fn(f64, f64) -> egui::Pos2,
) {
    use crate::state::TrimExtendPreview;
    match app.trim_extend_preview() {
        Some(TrimExtendPreview::Remove(curve)) => {
            let danger = Stroke::new(2.5, crate::theme::PREVIEW);
            draw_curve(painter, &curve, to_screen, danger);
        }
        Some(TrimExtendPreview::Extension(curve)) => {
            let ghost = Stroke::new(1.5, crate::theme::PREVIEW);
            draw_curve(painter, &curve, to_screen, ghost);
        }
        None => {}
    }
}

pub(super) fn layer_visible(app: &AppState, e: &eiderflat_document::Entity) -> bool {
    app.document
        .layers
        .get(e.layer)
        .map(|l| l.on)
        .unwrap_or(true)
}

pub(super) fn resolve_color(app: &AppState, e: &eiderflat_document::Entity) -> (u8, u8, u8) {
    match &e.color {
        Color::Rgb(r, g, b) => (*r, *g, *b),
        _ => app
            .document
            .layers
            .get(e.layer)
            .map(|l| l.color)
            .unwrap_or((220, 220, 220)),
    }
}

pub(super) fn refresh_hatch_cache(app: &AppState, cache: &mut super::HatchCache) {
    use std::collections::HashSet;
    let target = (app.view.pixel_world_size() * 0.4).max(1e-9);
    let bucket = target.log2().floor();
    let tol = 2f64.powf(bucket);
    let mut live: HashSet<EntityId> = HashSet::new();
    for e in app.document.iter() {
        if let EntityKind::Hatch {
            boundary, holes, ..
        } = &e.kind
        {
            live.insert(e.id);
            // The bucket is part of the signature, so this recomputes (and only
            // recomputes) when the geometry or the zoom level changes — keeping
            // the fill and outline crisp as you zoom in.
            let sig = hatch_signature(boundary, holes, bucket as i64);
            if cache.get(&e.id).map(|(s, _, _)| *s) != Some(sig) {
                let tris = eiderflat_cad::triangulate_hatch_with_tol(boundary, holes, tol);
                let loops = eiderflat_cad::hatch_outline_loops(boundary, holes, tol);
                cache.insert(e.id, (sig, tris, loops));
            }
        }
    }
    cache.retain(|id, _| live.contains(id));
}

fn hatch_signature(boundary: &[Curve], holes: &[Vec<Curve>], tol_bucket: i64) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    fn feed(loop_: &[Curve], h: &mut DefaultHasher) {
        (loop_.len() as u64).hash(h);
        for c in loop_ {
            let (t0, t1) = c.domain();
            for k in 0..=4 {
                let t = t0 + (t1 - t0) * k as f64 / 4.0;
                let (x, y) = c.evaluate_f64(t);
                x.to_bits().hash(h);
                y.to_bits().hash(h);
            }
        }
    }
    let mut h = DefaultHasher::new();
    tol_bucket.hash(&mut h);
    feed(boundary, &mut h);
    (holes.len() as u64).hash(&mut h);
    for hole in holes {
        feed(hole, &mut h);
    }
    h.finish()
}

#[allow(clippy::too_many_arguments)]
pub(super) fn draw_entity(
    painter: &egui::Painter,
    app: &AppState,
    e: &eiderflat_document::Entity,
    origin: egui::Pos2,
    stroke: Stroke,
    hatch_tris: Option<&[[Point2d; 3]]>,
    hatch_loops: Option<&[Vec<Point2d>]>,
) {
    let to_screen = |wx: f64, wy: f64| {
        let (sx, sy) = app.view.world_to_screen(wx, wy);
        pos2(origin.x + sx as f32, origin.y + sy as f32)
    };

    if e.id == app.origin_id {
        let origin_screen = to_screen(0.0, 0.0);
        let stroke_x = Stroke::new(1.5, Color32::from_rgb(255, 60, 60));
        let stroke_y = Stroke::new(1.5, Color32::from_rgb(60, 220, 60));
        painter.line_segment(
            [origin_screen, pos2(origin_screen.x + 18.0, origin_screen.y)],
            stroke_x,
        );
        painter.line_segment(
            [
                pos2(origin_screen.x + 18.0, origin_screen.y),
                pos2(origin_screen.x + 14.0, origin_screen.y - 3.0),
            ],
            stroke_x,
        );
        painter.line_segment(
            [
                pos2(origin_screen.x + 18.0, origin_screen.y),
                pos2(origin_screen.x + 14.0, origin_screen.y + 3.0),
            ],
            stroke_x,
        );
        // X Label
        painter.text(
            pos2(origin_screen.x + 24.0, origin_screen.y),
            egui::Align2::CENTER_CENTER,
            "X",
            egui::FontId::proportional(10.0),
            stroke_x.color,
        );

        // Y axis line:
        painter.line_segment(
            [origin_screen, pos2(origin_screen.x, origin_screen.y - 18.0)],
            stroke_y,
        );
        painter.line_segment(
            [
                pos2(origin_screen.x, origin_screen.y - 18.0),
                pos2(origin_screen.x - 3.0, origin_screen.y - 14.0),
            ],
            stroke_y,
        );
        painter.line_segment(
            [
                pos2(origin_screen.x, origin_screen.y - 18.0),
                pos2(origin_screen.x + 3.0, origin_screen.y - 14.0),
            ],
            stroke_y,
        );
        painter.text(
            pos2(origin_screen.x, origin_screen.y - 24.0),
            egui::Align2::CENTER_CENTER,
            "Y",
            egui::FontId::proportional(10.0),
            stroke_y.color,
        );
        painter.circle_filled(origin_screen, 3.0, Color32::from_rgb(180, 195, 220));
        painter.circle_stroke(
            origin_screen,
            5.0,
            Stroke::new(1.0, Color32::from_rgb(80, 90, 110)),
        );
        return;
    }

    match &e.kind {
        EntityKind::Curve(c) => draw_curve(painter, c, &to_screen, stroke),
        EntityKind::Point(p) => {
            let (x, y) = p.to_f64();
            painter.circle_filled(to_screen(x, y), 2.0, stroke.color);
        }
        EntityKind::Text {
            anchor,
            content,
            height,
            rotation,
            font,
        } => {
            let (x, y) = anchor.to_f64();
            let font = crate::fonts::text_font_id(
                painter.ctx(),
                font.as_deref(),
                *height as f32 * app.view.zoom as f32,
            );
            let galley = painter.layout_no_wrap(content.clone(), font, stroke.color);
            let angle = -(*rotation as f32);
            let h = galley.size().y;
            let (sn, cs) = angle.sin_cos();
            let pos = to_screen(x, y) + vec2(h * sn, -h * cs);
            let mut shape = egui::epaint::TextShape::new(pos, galley, stroke.color);
            shape.angle = angle;
            painter.add(shape);
        }
        EntityKind::Hatch {
            boundary,
            holes,
            fill,
            pattern,
        } => {
            use eiderflat_document::HatchPattern;
            let fill_col = Color32::from_rgb(fill.0, fill.1, fill.2);
            match pattern {
                HatchPattern::Solid => {
                    let computed;
                    let tris: &[[Point2d; 3]] = match hatch_tris {
                        Some(t) => t,
                        None => {
                            computed = eiderflat_cad::triangulate_hatch(boundary, holes);
                            &computed
                        }
                    };
                    if !tris.is_empty() {
                        let mut mesh = egui::epaint::Mesh::default();
                        for t in tris {
                            let base = mesh.vertices.len() as u32;
                            for v in t {
                                mesh.colored_vertex(to_screen(v.x, v.y), fill_col);
                            }
                            mesh.add_triangle(base, base + 1, base + 2);
                        }
                        painter.add(egui::Shape::mesh(mesh));
                    }
                }
                HatchPattern::Lines { .. } | HatchPattern::Cross { .. } => {
                    let pat = Stroke::new(1.0, fill_col);
                    for (a, b) in eiderflat_cad::hatch_pattern_lines(boundary, holes, *pattern) {
                        painter.line_segment([to_screen(a.x, a.y), to_screen(b.x, b.y)], pat);
                    }
                }
                HatchPattern::Dots { .. } => {
                    for p in eiderflat_cad::hatch_pattern_dots(boundary, holes, *pattern) {
                        painter.circle_filled(to_screen(p.x, p.y), 1.3, fill_col);
                    }
                }
            }
            // Stroke the outline from the cached, pre-flattened loops when we
            // have them (avoids re-flattening every boundary curve each frame —
            // critical for heavy boundaries such as outlined text). Fall back to
            // adaptive per-curve drawing only when no cache is available.
            match hatch_loops {
                Some(loops) => {
                    for ring in loops {
                        if ring.len() >= 2 {
                            let pts: Vec<egui::Pos2> =
                                ring.iter().map(|p| to_screen(p.x, p.y)).collect();
                            painter.add(egui::Shape::closed_line(pts, stroke));
                        }
                    }
                }
                None => {
                    for seg in boundary {
                        draw_curve(painter, seg, &to_screen, stroke);
                    }
                    for hole in holes {
                        for seg in hole {
                            draw_curve(painter, seg, &to_screen, stroke);
                        }
                    }
                }
            }
        }
        _ => {}
    }
}

pub(super) fn corner_glass_frame() -> egui::Frame {
    egui::Frame::new()
        .inner_margin(egui::Margin::symmetric(4, 3))
        .corner_radius(egui::CornerRadius::same(8))
        .fill(Color32::from_rgba_unmultiplied(26, 32, 42, 235))
        .stroke(Stroke::new(1.0, Color32::from_rgb(0, 200, 255)))
        .shadow(egui::epaint::Shadow {
            offset: [0, 3],
            blur: 14,
            spread: 0,
            color: Color32::from_black_alpha(130),
        })
}

pub(super) fn draw_corner_preview(
    painter: &egui::Painter,
    app: &AppState,
    ca: &crate::state::CornerAction,
    to_screen: &impl Fn(f64, f64) -> egui::Pos2,
) {
    let accent = Color32::from_rgb(0, 220, 255);
    let stroke = Stroke::new(2.0, Color32::from_rgba_unmultiplied(0, 220, 255, 128));
    let seg = |p: (f64, f64), q: (f64, f64)| [to_screen(p.0, p.1), to_screen(q.0, q.1)];
    let mut group = app.corner_group(&ca.geom, ca.kind);
    if group.is_empty() {
        group.push(ca.geom);
    }
    for g in &group {
        match ca.kind {
            crate::state::CornerKind::Fillet => {
                if let Some(sol) =
                    eiderflat_cad::edit::solve_fillet(g.edge_a, g.edge_b, ca.size, g.corner)
                {
                    draw_trimmed_edge(painter, &g.edge_a, g.corner, sol.ta, to_screen, stroke);
                    draw_trimmed_edge(painter, &g.edge_b, g.corner, sol.tb, to_screen, stroke);
                    draw_arc_short(
                        painter, sol.center, ca.size, sol.ta, sol.tb, to_screen, stroke,
                    );
                }
            }
            crate::state::CornerKind::Chamfer => {
                let far_a = (
                    g.corner.0 + g.dir_a.0 * g.len_a,
                    g.corner.1 + g.dir_a.1 * g.len_a,
                );
                let far_b = (
                    g.corner.0 + g.dir_b.0 * g.len_b,
                    g.corner.1 + g.dir_b.1 * g.len_b,
                );
                let p1 = (
                    g.corner.0 + g.dir_a.0 * ca.size,
                    g.corner.1 + g.dir_a.1 * ca.size,
                );
                let p2 = (
                    g.corner.0 + g.dir_b.0 * ca.size,
                    g.corner.1 + g.dir_b.1 * ca.size,
                );
                painter.line_segment(seg(far_a, p1), stroke);
                painter.line_segment(seg(far_b, p2), stroke);
                painter.line_segment(seg(p1, p2), stroke);
            }
        }
    }

    let cur = to_screen(app.cursor_world.0, app.cursor_world.1);
    painter.circle_filled(cur, 4.0, accent);
    let label = match ca.kind {
        crate::state::CornerKind::Fillet => format!("R {:.2}", ca.size),
        crate::state::CornerKind::Chamfer => format!("{:.2}", ca.size),
    };
    painter.text(
        pos2(cur.x + 9.0, cur.y - 9.0),
        egui::Align2::LEFT_BOTTOM,
        label,
        egui::FontId::monospace(12.0),
        accent,
    );
}
fn draw_trimmed_edge(
    painter: &egui::Painter,
    edge: &eiderflat_cad::edit::CornerEdge,
    corner: (f64, f64),
    trim_pt: (f64, f64),
    to_screen: &impl Fn(f64, f64) -> egui::Pos2,
    stroke: Stroke,
) {
    use eiderflat_cad::edit::CornerEdge;
    let sq = |p: (f64, f64)| (p.0 - corner.0).powi(2) + (p.1 - corner.1).powi(2);
    match *edge {
        CornerEdge::Line { p0, p1 } => {
            let far = if sq(p0) > sq(p1) { p0 } else { p1 };
            painter.line_segment(
                [to_screen(far.0, far.1), to_screen(trim_pt.0, trim_pt.1)],
                stroke,
            );
        }
        CornerEdge::Arc {
            cx,
            cy,
            r,
            start,
            end,
        } => {
            let sp = (cx + r * start.cos(), cy + r * start.sin());
            let ep = (cx + r * end.cos(), cy + r * end.sin());
            let far_angle = if sq(sp) > sq(ep) { start } else { end };
            draw_arc_short(
                painter,
                (cx, cy),
                r,
                polar(cx, cy, r, far_angle),
                trim_pt,
                to_screen,
                stroke,
            );
        }
    }
}

fn polar(cx: f64, cy: f64, r: f64, angle: f64) -> (f64, f64) {
    (cx + r * angle.cos(), cy + r * angle.sin())
}

fn draw_arc_short(
    painter: &egui::Painter,
    center: (f64, f64),
    r: f64,
    a: (f64, f64),
    b: (f64, f64),
    to_screen: &impl Fn(f64, f64) -> egui::Pos2,
    stroke: Stroke,
) {
    let a0 = (a.1 - center.1).atan2(a.0 - center.0);
    let a1 = (b.1 - center.1).atan2(b.0 - center.0);
    let mut d = a1 - a0;
    while d > std::f64::consts::PI {
        d -= std::f64::consts::TAU;
    }
    while d < -std::f64::consts::PI {
        d += std::f64::consts::TAU;
    }
    let n = 28;
    let pts: Vec<_> = (0..=n)
        .map(|i| {
            let ang = a0 + d * (i as f64 / n as f64);
            to_screen(center.0 + r * ang.cos(), center.1 + r * ang.sin())
        })
        .collect();
    painter.add(egui::Shape::line(pts, stroke));
}

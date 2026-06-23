use egui::{Color32, Pos2, Rect, Response, Sense, Stroke, Ui, Vec2, pos2, vec2};
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum Icon {
    Select,
    Line,
    Circle,
    Ellipse,
    Arc,
    Rectangle,
    Polygon,
    Spline,
    Polyline,
    Text,
    Move,
    Copy,
    Rotate,
    Scale,
    Mirror,
    Offset,
    Trim,
    Extend,
    Fillet,
    Chamfer,
    Stretch,
    Explode,
    Join,
    Hatch,
    Undo,
    Redo,
    Eye,
    EyeOff,
    ZoomIn,
    ZoomOut,
    ZoomFit,
    Pan,
    AddLayer,
    Delete,
}

pub fn paint_icon(
    painter: &egui::Painter,
    _ctx: &egui::Context,
    icon: Icon,
    rect: Rect,
    fg: Color32,
) {
    // Native vector glyph (same renderer as the icon buttons) so palette rows
    // and the inspector header match the toolbars exactly.
    icon.draw(painter, rect, fg);
}

fn rasterize_svg(svg: &str, w: u32, h: u32) -> Option<resvg::tiny_skia::Pixmap> {
    let tree = resvg::usvg::Tree::from_str(svg, &resvg::usvg::Options::default()).ok()?;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(w, h)?;
    let size = tree.size();
    let scale = (w as f32 / size.width()).min(h as f32 / size.height());
    let tx = (w as f32 - size.width() * scale) * 0.5;
    let ty = (h as f32 - size.height() * scale) * 0.5;
    let transform = resvg::tiny_skia::Transform::from_scale(scale, scale).post_translate(tx, ty);
    resvg::render(&tree, transform, &mut pixmap.as_mut());
    Some(pixmap)
}

pub fn app_icon() -> egui::IconData {
    const SIZE: u32 = 256;
    let svg = include_str!("../assets/logotype/symbol.svg");
    match rasterize_svg(svg, SIZE, SIZE) {
        Some(pixmap) => {
            let mut rgba = pixmap.data().to_vec();
            for px in rgba.chunks_exact_mut(4) {
                let a = px[3] as u32;
                if a > 0 && a < 255 {
                    px[0] = (px[0] as u32 * 255 / a).min(255) as u8;
                    px[1] = (px[1] as u32 * 255 / a).min(255) as u8;
                    px[2] = (px[2] as u32 * 255 / a).min(255) as u8;
                }
            }
            egui::IconData {
                rgba,
                width: SIZE,
                height: SIZE,
            }
        }
        None => egui::IconData {
            rgba: vec![0; 4],
            width: 1,
            height: 1,
        },
    }
}

pub fn logo_texture(ctx: &egui::Context) -> Option<egui::TextureHandle> {
    let id = egui::Id::new("eiderflat_logo_tex");
    if let Some(t) = ctx.data(|d| d.get_temp::<egui::TextureHandle>(id)) {
        return Some(t);
    }
    let svg = include_str!("../assets/logotype/logotype.svg");
    let pixmap = rasterize_svg(svg, 768, 419)?;
    let image = egui::ColorImage::from_rgba_premultiplied(
        [pixmap.width() as usize, pixmap.height() as usize],
        pixmap.data(),
    );
    let tex = ctx.load_texture("eiderflat_logo", image, egui::TextureOptions::LINEAR);
    ctx.data_mut(|d| d.insert_temp(id, tex.clone()));
    Some(tex)
}

const ICON_SIZE: f32 = 30.0;
/// Fixed glyph size (in points) so all icon buttons share one visual size,
/// independent of their button box.
const GLYPH_PX: f32 = 18.0;

pub fn icon_button(ui: &mut Ui, icon: Icon, tooltip: &str, active: bool) -> Response {
    icon_button_sized(ui, icon, tooltip, active, ICON_SIZE)
}

pub fn icon_button_sized(
    ui: &mut Ui,
    icon: Icon,
    tooltip: &str,
    active: bool,
    size: f32,
) -> Response {
    let (raw_rect, mut response) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    let hovered = response.hovered() && ui.is_enabled();
    let ppp = ui.ctx().pixels_per_point();
    let rect = snap_rect(raw_rect, ppp);
    let anim = ui.ctx().animate_bool(response.id, hovered);
    let act = ui
        .ctx()
        .animate_bool(response.id.with("active"), active && ui.is_enabled());

    let radius = (size * 0.27).round().clamp(8.0, 13.0);
    let painter = ui.painter_at(rect);
    // Hover background fades in; the active tool gets a filled accent pill + border.
    if anim > 0.001 && act < 0.5 {
        painter.rect_filled(
            rect,
            radius,
            crate::theme::WIDGET_HOVER.gamma_multiply(anim * 0.9),
        );
    }
    if act > 0.001 {
        painter.rect(
            rect,
            radius,
            crate::theme::ACCENT.gamma_multiply(0.18 * act),
            Stroke::new(1.0, crate::theme::ACCENT.gamma_multiply(0.55 * act)),
            egui::StrokeKind::Inside,
        );
    }

    let enabled = ui.is_enabled();
    let fg = if !enabled {
        crate::theme::TEXT_DIM
    } else if act > 0.5 {
        crate::theme::ACCENT_BRIGHT
    } else {
        Color32::from_rgb(214, 224, 240)
    };

    // The glyph is a fixed pixel size regardless of the button box, so every
    // icon across the UI (toolbars, layer rows, the contextual popup, undo/redo,
    // …) renders at the same visual size. Smaller buttons keep a little padding.
    // Icons are native vector strokes (not rasterized SVG): crisp at any DPI and
    // one consistent on-screen size.
    let glyph = GLYPH_PX.min(size - 6.0).max(8.0);
    let area = snap_rect(
        Rect::from_center_size(rect.center(), Vec2::splat(glyph)),
        ppp,
    );
    icon.draw(&painter, area, fg);

    if hovered {
        response = response.on_hover_text(tooltip);
    }
    response
}

fn snap_rect(r: Rect, pixels_per_point: f32) -> Rect {
    let snap = |v: f32| (v * pixels_per_point).round() / pixels_per_point;
    Rect::from_min_max(
        pos2(snap(r.min.x), snap(r.min.y)),
        pos2(snap(r.max.x), snap(r.max.y)),
    )
}

fn p(r: Rect, fx: f32, fy: f32) -> Pos2 {
    pos2(r.left() + fx * r.width(), r.top() + fy * r.height())
}

fn arc_pts(center: Pos2, radius: f32, a0: f32, a1: f32, n: usize) -> Vec<Pos2> {
    (0..=n)
        .map(|i| {
            let t = a0 + (a1 - a0) * (i as f32 / n as f32);
            center + vec2(t.cos() * radius, t.sin() * radius)
        })
        .collect()
}

fn arrowhead(painter: &egui::Painter, tip: Pos2, dx: f32, dy: f32, size: f32, s: Stroke) {
    for ang in [2.6f32, -2.6f32] {
        let (c, sn) = (ang.cos(), ang.sin());
        let bx = dx * c - dy * sn;
        let by = dx * sn + dy * c;
        painter.line_segment([tip, tip + vec2(bx, by) * size], s);
    }
}

/// CAD node marker: the small filled square shown on defining points.
fn node(painter: &egui::Painter, at: Pos2, r: Rect, color: Color32) {
    let s = r.width() * 0.13;
    painter.rect_filled(Rect::from_center_size(at, Vec2::splat(s)), 0.5, color);
}

fn round_line(painter: &egui::Painter, a: Pos2, b: Pos2, s: Stroke) {
    painter.line_segment([a, b], s);
    let cap = s.width * 0.5;
    painter.circle_filled(a, cap, s.color);
    painter.circle_filled(b, cap, s.color);
}

fn dashed(painter: &egui::Painter, a: Pos2, b: Pos2, s: Stroke) {
    let v = b - a;
    let len = v.length();
    if len < 1e-3 {
        return;
    }
    let dir = v / len;
    let dash = (len / 5.0).clamp(2.0, 4.0);
    let mut d = 0.0;
    while d < len {
        let e = (d + dash).min(len);
        painter.line_segment([a + dir * d, a + dir * e], s);
        d += dash * 2.0;
    }
}

impl Icon {
    fn draw(self, painter: &egui::Painter, r: Rect, color: Color32) {
        let s = Stroke::new(2.0, color);
        let thin = Stroke::new(1.3, color);
        let dim = Stroke::new(1.6, crate::theme::ACCENT);
        let ah = r.width() * 0.22;
        match self {
            Icon::Select => {
                let pts = vec![
                    p(r, 0.18, 0.00),
                    p(r, 0.18, 0.80),
                    p(r, 0.38, 0.61),
                    p(r, 0.53, 0.97),
                    p(r, 0.67, 0.91),
                    p(r, 0.52, 0.55),
                    p(r, 0.82, 0.55),
                    p(r, 0.18, 0.00),
                ];
                painter.add(egui::Shape::line(pts, s));
            }

            Icon::Line => {
                let a = p(r, 0.06, 0.94);
                let b = p(r, 0.94, 0.06);
                painter.line_segment([a, b], s);
                node(painter, a, r, color);
                node(painter, b, r, color);
            }
            Icon::Polyline => {
                let pts = [
                    p(r, 0.02, 0.90),
                    p(r, 0.30, 0.14),
                    p(r, 0.62, 0.64),
                    p(r, 0.98, 0.06),
                ];
                painter.add(egui::Shape::line(pts.to_vec(), s));
                for q in pts {
                    node(painter, q, r, color);
                }
            }
            Icon::Circle => {
                painter.circle_stroke(r.center(), r.width() * 0.48, s);
                painter.circle_filled(r.center(), r.width() * 0.07, color); // center point
            }
            Icon::Ellipse => {
                let (cx, cy) = (0.5, 0.5);
                let (rx, ry) = (0.46, 0.30);
                let n = 40;
                let mut pts = Vec::with_capacity(n + 1);
                for i in 0..=n {
                    let a = i as f32 / n as f32 * std::f32::consts::TAU;
                    pts.push(p(r, cx + rx * a.cos(), cy + ry * a.sin()));
                }
                painter.add(egui::Shape::line(pts, s));
                painter.circle_filled(r.center(), r.width() * 0.07, color); // center point
            }
            Icon::Arc => {
                let c = p(r, 0.5, 0.6156);
                let rad = r.width() * 0.4956;
                let pts = arc_pts(c, rad, 0.381, -3.523, 24);
                let (first, mid, last) = (pts[0], pts[pts.len() / 2], *pts.last().unwrap());
                painter.add(egui::Shape::line(pts, s));
                node(painter, first, r, color);
                node(painter, mid, r, color);
                node(painter, last, r, color);
            }
            Icon::Rectangle => {
                let rect = Rect::from_min_max(p(r, 0.04, 0.18), p(r, 0.96, 0.82));
                painter.rect_stroke(rect, 0.0, s, egui::StrokeKind::Middle);
                node(painter, rect.left_top(), r, color);
                node(painter, rect.right_bottom(), r, color);
            }
            Icon::Polygon => {
                let c = r.center();
                let rad = r.width() * 0.5;
                let pts: Vec<Pos2> = (0..6)
                    .map(|i| {
                        let a =
                            std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::TAU / 6.0;
                        c + vec2(a.cos() * rad, -a.sin() * rad)
                    })
                    .collect();
                painter.add(egui::Shape::closed_line(pts, s));
                painter.circle_filled(c, r.width() * 0.07, color); // center point
            }
            Icon::Spline => {
                let (p0, p1, p2, p3) = (
                    p(r, 0.04, 0.92),
                    p(r, 0.22, 0.02),
                    p(r, 0.78, 0.98),
                    p(r, 0.96, 0.08),
                );
                dashed(painter, p0, p1, dim);
                dashed(painter, p1, p2, dim);
                dashed(painter, p2, p3, dim);
                let n = 18;
                let pts: Vec<Pos2> = (0..=n)
                    .map(|i| {
                        let t = i as f32 / n as f32;
                        let u = 1.0 - t;
                        pos2(
                            u * u * u * p0.x
                                + 3.0 * u * u * t * p1.x
                                + 3.0 * u * t * t * p2.x
                                + t * t * t * p3.x,
                            u * u * u * p0.y
                                + 3.0 * u * u * t * p1.y
                                + 3.0 * u * t * t * p2.y
                                + t * t * t * p3.y,
                        )
                    })
                    .collect();
                painter.add(egui::Shape::line(pts, s));
                for q in [p0, p1, p2, p3] {
                    node(painter, q, r, color);
                }
            }
            Icon::Text => {
                painter.line_segment([p(r, 0.12, 0.97), p(r, 0.50, 0.02)], s);
                painter.line_segment([p(r, 0.50, 0.02), p(r, 0.88, 0.97)], s);
                painter.line_segment([p(r, 0.27, 0.62), p(r, 0.73, 0.62)], s);
            }

            Icon::Move => {
                let c = r.center();
                for (dx, dy) in [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
                    let tip = c + vec2(dx, dy) * r.width() * 0.5;
                    painter.line_segment([c, tip], s);
                    arrowhead(painter, tip, dx, dy, ah, s);
                }
            }
            Icon::Copy => {
                painter.rect_stroke(
                    Rect::from_min_max(p(r, 0.05, 0.05), p(r, 0.62, 0.62)),
                    1.0,
                    dim,
                    egui::StrokeKind::Middle,
                );
                painter.rect_stroke(
                    Rect::from_min_max(p(r, 0.36, 0.36), p(r, 0.95, 0.95)),
                    1.0,
                    s,
                    egui::StrokeKind::Middle,
                );
            }
            Icon::Rotate => {
                let c = r.center();
                let rad = r.width() * 0.42;
                painter.add(egui::Shape::line(arc_pts(c, rad, -1.1, 3.3, 24), s));
                let a = 3.3f32;
                let tip = c + vec2(a.cos() * rad, a.sin() * rad);
                arrowhead(painter, tip, -a.sin(), a.cos(), ah, s);
                painter.circle_filled(c, r.width() * 0.07, color); // pivot
            }
            Icon::Scale => {
                let big = Rect::from_min_max(p(r, 0.05, 0.05), p(r, 0.95, 0.95));
                for (a, b) in [
                    (big.left_top(), big.right_top()),
                    (big.right_top(), big.right_bottom()),
                    (big.right_bottom(), big.left_bottom()),
                    (big.left_bottom(), big.left_top()),
                ] {
                    dashed(painter, a, b, dim);
                }
                painter.rect_stroke(
                    Rect::from_min_max(p(r, 0.05, 0.55), p(r, 0.45, 0.95)),
                    0.0,
                    s,
                    egui::StrokeKind::Middle,
                );
                let tip = p(r, 0.86, 0.14);
                painter.line_segment([p(r, 0.45, 0.55), tip], s);
                arrowhead(painter, tip, 0.7, -0.7, ah, s);
            }
            Icon::Mirror => {
                let axis_top = p(r, 0.5, 0.0);
                let axis_bot = p(r, 0.5, 1.0);
                dashed(painter, axis_top, axis_bot, dim);
                painter.add(egui::Shape::closed_line(
                    vec![p(r, 0.06, 0.18), p(r, 0.38, 0.5), p(r, 0.06, 0.82)],
                    s,
                ));
                painter.add(egui::Shape::closed_line(
                    vec![p(r, 0.94, 0.18), p(r, 0.62, 0.5), p(r, 0.94, 0.82)],
                    dim,
                ));
            }
            Icon::Offset => {
                painter.rect_stroke(
                    Rect::from_min_max(p(r, 0.05, 0.05), p(r, 0.95, 0.95)),
                    3.0,
                    s,
                    egui::StrokeKind::Middle,
                );
                painter.rect_stroke(
                    Rect::from_min_max(p(r, 0.30, 0.30), p(r, 0.70, 0.70)),
                    2.0,
                    dim,
                    egui::StrokeKind::Middle,
                );
            }
            Icon::Trim => {
                let q = |x: f32, y: f32| p(r, x / 24.0, y / 24.0);
                let w = r.width() * 0.083;
                let blade = Stroke::new(w, color);
                let ring_r = r.width() * 0.083;
                painter.circle_stroke(q(4.0, 8.0), ring_r, blade);
                painter.circle_stroke(q(4.0, 16.0), ring_r, blade);
                round_line(painter, q(5.42, 9.42), q(8.0, 12.0), blade);
                round_line(painter, q(14.0, 6.0), q(5.42, 14.58), blade);
                round_line(painter, q(10.8, 14.8), q(14.0, 18.0), blade);
                let dash = Stroke::new(w, crate::theme::ACCENT);
                round_line(painter, q(14.0, 12.0), q(16.0, 12.0), dash);
                round_line(painter, q(20.0, 12.0), q(22.0, 12.0), dash);
            }
            Icon::Extend => {
                painter.line_segment([p(r, 0.02, 0.5), p(r, 0.45, 0.5)], s);
                dashed(painter, p(r, 0.45, 0.5), p(r, 0.80, 0.5), dim);
                arrowhead(painter, p(r, 0.84, 0.5), 1.0, 0.0, ah, s);
                painter.line_segment([p(r, 0.92, 0.05), p(r, 0.92, 0.95)], s); // boundary
            }
            Icon::Fillet => {
                dashed(painter, p(r, 0.12, 0.58), p(r, 0.12, 0.92), dim);
                dashed(painter, p(r, 0.12, 0.92), p(r, 0.46, 0.92), dim);
                painter.line_segment([p(r, 0.12, 0.04), p(r, 0.12, 0.58)], s);
                painter.line_segment([p(r, 0.46, 0.92), p(r, 0.96, 0.92)], s);
                painter.add(egui::Shape::line(
                    arc_pts(
                        p(r, 0.46, 0.58),
                        r.width() * 0.34,
                        std::f32::consts::PI,
                        std::f32::consts::FRAC_PI_2,
                        16,
                    ),
                    s,
                ));
            }
            Icon::Chamfer => {
                dashed(painter, p(r, 0.12, 0.58), p(r, 0.12, 0.92), dim);
                dashed(painter, p(r, 0.12, 0.92), p(r, 0.46, 0.92), dim);
                painter.line_segment([p(r, 0.12, 0.04), p(r, 0.12, 0.58)], s);
                painter.line_segment([p(r, 0.46, 0.92), p(r, 0.96, 0.92)], s);
                painter.line_segment([p(r, 0.12, 0.58), p(r, 0.46, 0.92)], s);
            }
            Icon::Stretch => {
                let win = Rect::from_min_max(p(r, 0.05, 0.22), p(r, 0.52, 0.78));
                for (a, b) in [
                    (win.left_top(), win.right_top()),
                    (win.right_top(), win.right_bottom()),
                    (win.right_bottom(), win.left_bottom()),
                    (win.left_bottom(), win.left_top()),
                ] {
                    dashed(painter, a, b, dim);
                }
                let grip = p(r, 0.52, 0.5);
                node(painter, grip, r, color);
                let tip = p(r, 0.92, 0.5);
                painter.line_segment([grip, tip], s);
                arrowhead(painter, tip, 1.0, 0.0, ah, s);
            }
            Icon::Explode => {
                let inner = Rect::from_center_size(r.center(), Vec2::splat(r.width() * 0.30));
                for (a, b) in [
                    (inner.left_top(), inner.right_top()),
                    (inner.right_top(), inner.right_bottom()),
                    (inner.right_bottom(), inner.left_bottom()),
                    (inner.left_bottom(), inner.left_top()),
                ] {
                    dashed(painter, a, b, dim);
                }
                for (fx, fy, dx, dy) in [
                    (0.16f32, 0.16f32, -1.0f32, -1.0f32),
                    (0.84, 0.16, 1.0, -1.0),
                    (0.84, 0.84, 1.0, 1.0),
                    (0.16, 0.84, -1.0, 1.0),
                ] {
                    let n = (dx * dx + dy * dy).sqrt();
                    let (ux, uy) = (dx / n, dy / n);
                    let tip = p(r, fx, fy);
                    painter.line_segment([tip - vec2(ux, uy) * (r.width() * 0.16), tip], s);
                    arrowhead(painter, tip, ux, uy, ah, s);
                }
            }
            Icon::Join => {
                node(painter, p(r, 0.5, 0.5), r, color);
                painter.line_segment([p(r, 0.05, 0.5), p(r, 0.34, 0.5)], s);
                arrowhead(painter, p(r, 0.40, 0.5), 1.0, 0.0, ah, s);
                painter.line_segment([p(r, 0.95, 0.5), p(r, 0.66, 0.5)], s);
                arrowhead(painter, p(r, 0.60, 0.5), -1.0, 0.0, ah, s);
            }
            Icon::Hatch => {
                painter.rect_stroke(
                    Rect::from_min_max(p(r, 0.1, 0.1), p(r, 0.9, 0.9)),
                    1.0,
                    s,
                    egui::StrokeKind::Middle,
                );
                painter.line_segment([p(r, 0.1, 0.5), p(r, 0.5, 0.1)], dim);
                painter.line_segment([p(r, 0.1, 0.9), p(r, 0.9, 0.1)], dim);
                painter.line_segment([p(r, 0.5, 0.9), p(r, 0.9, 0.5)], dim);
            }

            Icon::Undo => {
                let c = p(r, 0.5, 0.55);
                let rad = r.width() * 0.44;
                painter.add(egui::Shape::line(arc_pts(c, rad, -0.25, -2.90, 18), s));
                let a = -2.90f32;
                let tip = c + vec2(a.cos() * rad, a.sin() * rad);
                arrowhead(painter, tip, a.sin(), -a.cos(), ah, s);
            }
            Icon::Redo => {
                let c = p(r, 0.5, 0.55);
                let rad = r.width() * 0.44;
                painter.add(egui::Shape::line(arc_pts(c, rad, -2.89, -0.24, 18), s));
                let a = -0.24f32;
                let tip = c + vec2(a.cos() * rad, a.sin() * rad);
                arrowhead(painter, tip, -a.sin(), a.cos(), ah, s);
            }

            Icon::Eye | Icon::EyeOff => {
                let n = 12;
                let lid = |sign: f32| -> Vec<Pos2> {
                    (0..=n)
                        .map(|i| {
                            let t = i as f32 / n as f32;
                            p(
                                r,
                                0.02 + 0.96 * t,
                                0.5 + sign * 0.40 * (std::f32::consts::PI * t).sin(),
                            )
                        })
                        .collect()
                };
                painter.add(egui::Shape::line(lid(-1.0), s));
                painter.add(egui::Shape::line(lid(1.0), s));
                painter.circle_stroke(r.center(), r.width() * 0.15, s);
                if self == Icon::EyeOff {
                    painter.line_segment([p(r, 0.12, 0.95), p(r, 0.88, 0.05)], s);
                }
            }

            Icon::ZoomIn | Icon::ZoomOut => {
                let c = p(r, 0.42, 0.42);
                let rad = r.width() * 0.34;
                painter.circle_stroke(c, rad, s);
                let h0 = c + vec2(rad * 0.72, rad * 0.72);
                painter.line_segment([h0, p(r, 0.96, 0.96)], s);
                painter.line_segment([c - vec2(rad * 0.5, 0.0), c + vec2(rad * 0.5, 0.0)], s);
                if self == Icon::ZoomIn {
                    painter.line_segment([c - vec2(0.0, rad * 0.5), c + vec2(0.0, rad * 0.5)], s);
                }
            }
            Icon::ZoomFit => {
                let k = 0.22;
                for (cx, cy, dx, dy) in [
                    (0.04, 0.04, 1.0, 1.0),
                    (0.96, 0.04, -1.0, 1.0),
                    (0.96, 0.96, -1.0, -1.0),
                    (0.04, 0.96, 1.0, -1.0),
                ] {
                    let corner = p(r, cx, cy);
                    painter.line_segment([corner, p(r, cx + dx * k, cy)], s);
                    painter.line_segment([corner, p(r, cx, cy + dy * k)], s);
                }
                painter.rect_stroke(
                    Rect::from_min_max(p(r, 0.32, 0.38), p(r, 0.68, 0.62)),
                    0.0,
                    thin,
                    egui::StrokeKind::Middle,
                );
            }

            Icon::Delete => {
                // Trash can: lid, handle, tapering body, two inner slots.
                painter.line_segment([p(r, 0.12, 0.24), p(r, 0.88, 0.24)], s); // lid
                painter.line_segment([p(r, 0.40, 0.12), p(r, 0.60, 0.12)], s); // handle top
                painter.line_segment([p(r, 0.40, 0.12), p(r, 0.40, 0.24)], s);
                painter.line_segment([p(r, 0.60, 0.12), p(r, 0.60, 0.24)], s);
                painter.add(egui::Shape::line(
                    vec![
                        p(r, 0.20, 0.24),
                        p(r, 0.26, 0.90),
                        p(r, 0.74, 0.90),
                        p(r, 0.80, 0.24),
                    ],
                    s,
                ));
                painter.line_segment([p(r, 0.42, 0.36), p(r, 0.44, 0.80)], thin);
                painter.line_segment([p(r, 0.58, 0.36), p(r, 0.56, 0.80)], thin);
            }
            Icon::AddLayer => {
                // A sheet (diamond, seen in perspective) plus a "+" to add one.
                painter.add(egui::Shape::closed_line(
                    vec![
                        p(r, 0.40, 0.08),
                        p(r, 0.74, 0.28),
                        p(r, 0.40, 0.48),
                        p(r, 0.06, 0.28),
                    ],
                    s,
                ));
                // Front edge of a second sheet underneath.
                painter.add(egui::Shape::line(
                    vec![p(r, 0.06, 0.40), p(r, 0.40, 0.60), p(r, 0.74, 0.40)],
                    dim,
                ));
                // Plus badge (add).
                painter.line_segment([p(r, 0.62, 0.74), p(r, 0.90, 0.74)], s);
                painter.line_segment([p(r, 0.76, 0.60), p(r, 0.76, 0.88)], s);
            }
            Icon::Pan => {
                // Open hand: palm with three fingers and a thumb.
                painter.add(egui::Shape::line(
                    vec![
                        p(r, 0.28, 0.52),
                        p(r, 0.28, 0.82),
                        p(r, 0.70, 0.92),
                        p(r, 0.78, 0.56),
                    ],
                    s,
                ));
                for fx in [0.36f32, 0.50, 0.64] {
                    painter.line_segment([p(r, fx, 0.50), p(r, fx, 0.22)], s);
                }
                painter.line_segment([p(r, 0.28, 0.56), p(r, 0.16, 0.46)], s); // thumb
            }
        }
    }
}

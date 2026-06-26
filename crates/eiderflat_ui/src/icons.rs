use egui::{Color32, Pos2, Rect, Response, Sense, Stroke, Ui, Vec2, pos2, vec2};
use std::collections::HashMap;
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

impl Icon {
    fn svg_src(self) -> Option<&'static str> {
        Some(match self {
            Icon::Select => include_str!("../assets/icons/tool_selection.svg"),
            Icon::Line => include_str!("../assets/icons/tool_line.svg"),
            Icon::Circle => include_str!("../assets/icons/tool_circle.svg"),
            Icon::Ellipse => include_str!("../assets/icons/tool_ellipse.svg"),
            Icon::Arc => include_str!("../assets/icons/tool_arc.svg"),
            Icon::Rectangle => include_str!("../assets/icons/tool_rectangle.svg"),
            Icon::Polygon => include_str!("../assets/icons/tool_polygon.svg"),
            Icon::Spline => include_str!("../assets/icons/tool_spline.svg"),
            Icon::Polyline => include_str!("../assets/icons/tool_polyline.svg"),
            Icon::Text => include_str!("../assets/icons/tool_text.svg"),
            Icon::Move => include_str!("../assets/icons/tool_move.svg"),
            Icon::Copy => include_str!("../assets/icons/tool_copy.svg"),
            Icon::Rotate => include_str!("../assets/icons/tool_rotate.svg"),
            Icon::Scale => include_str!("../assets/icons/tool_scale.svg"),
            Icon::Mirror => include_str!("../assets/icons/tool_mirror.svg"),
            Icon::Offset => include_str!("../assets/icons/tool_offset.svg"),
            Icon::Trim => include_str!("../assets/icons/tool_trim.svg"),
            Icon::Extend => include_str!("../assets/icons/tool_extend.svg"),
            Icon::Fillet => include_str!("../assets/icons/tool_fillet.svg"),
            Icon::Chamfer => include_str!("../assets/icons/tool_chamfer.svg"),
            Icon::Stretch => include_str!("../assets/icons/tool_stretch.svg"),
            Icon::Explode => include_str!("../assets/icons/tool_explode.svg"),
            Icon::Join => include_str!("../assets/icons/tool_join.svg"),
            Icon::Hatch => include_str!("../assets/icons/tool_hatch.svg"),
            Icon::Undo => include_str!("../assets/icons/ui_undo.svg"),
            Icon::Redo => include_str!("../assets/icons/ui_redo.svg"),
            Icon::Eye => include_str!("../assets/icons/ui_visible.svg"),
            Icon::EyeOff => include_str!("../assets/icons/ui_hide.svg"),
            Icon::ZoomIn => include_str!("../assets/icons/ui_zoom_in.svg"),
            Icon::ZoomOut => include_str!("../assets/icons/ui_zoom_out.svg"),
            Icon::ZoomFit => include_str!("../assets/icons/ui_zoom_extents.svg"),
            Icon::Pan => include_str!("../assets/icons/ui_pan.svg"),
            Icon::AddLayer => include_str!("../assets/icons/ui_add_layer.svg"),
            Icon::Delete => include_str!("../assets/icons/ui_delete.svg"),
        })
    }
}

fn hex(c: Color32) -> String {
    format!("#{:02x}{:02x}{:02x}", c.r(), c.g(), c.b())
}

fn recolour(src: &str, fg: Color32, accent: Color32) -> String {
    let (fg, accent) = (hex(fg), hex(accent));
    src.replace(": #ec2024", &format!(": {accent}"))
        .replace(": red", &format!(": {accent}"))
        .replace(": #000", &format!(": {fg}"))
}

/// Cache key: (icon id, fg rgba, accent rgba, width px, height px).
type SvgCacheKey = (u8, [u8; 4], [u8; 4], u16, u16);

#[derive(Clone, Default)]
struct SvgIconCache(HashMap<SvgCacheKey, egui::TextureHandle>);

/// Fraction of the glyph box that an icon's content (its drawn bounding box)
/// should fill. Tuned to the eye/trash icons so they keep their natural size
/// while the more-padded tool glyphs are scaled up to match.
const ICON_FILL: f32 = 0.78;

/// Bounding box (inclusive, in pixels) of the non-transparent content in a
/// premultiplied-RGBA buffer, or `None` if the buffer is fully transparent.
fn alpha_bbox(data: &[u8], w: u32, h: u32) -> Option<(u32, u32, u32, u32)> {
    let (mut minx, mut miny, mut maxx, mut maxy) = (w, h, 0u32, 0u32);
    let mut any = false;
    for y in 0..h {
        for x in 0..w {
            if data[((y * w + x) * 4 + 3) as usize] > 8 {
                any = true;
                minx = minx.min(x);
                miny = miny.min(y);
                maxx = maxx.max(x);
                maxy = maxy.max(y);
            }
        }
    }
    any.then_some((minx, miny, maxx, maxy))
}

fn svg_texture(
    ctx: &egui::Context,
    icon: Icon,
    fg: Color32,
    accent: Color32,
    w: u32,
    h: u32,
) -> Option<egui::TextureHandle> {
    let key = (
        icon as u8,
        fg.to_array(),
        accent.to_array(),
        w as u16,
        h as u16,
    );
    let id = egui::Id::new("eiderflat_svg_icon_cache");
    if let Some(tex) = ctx.data(|d| {
        d.get_temp::<SvgIconCache>(id)
            .and_then(|c| c.0.get(&key).cloned())
    }) {
        return Some(tex);
    }

    let svg = recolour(icon.svg_src()?, fg, accent);
    let tree = resvg::usvg::Tree::from_str(&svg, &resvg::usvg::Options::default()).ok()?;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(w, h)?;

    // First pass: render the whole 32×32 viewBox into the box so we can measure
    // the drawn content's bounds. Each icon leaves a different margin inside the
    // viewBox, so a plain viewBox fit makes some glyphs look smaller than others.
    let base = (w.min(h) as f32) / 32.0;
    let bx = (w as f32 - 32.0 * base) * 0.5;
    let by = (h as f32 - 32.0 * base) * 0.5;
    let probe = resvg::tiny_skia::Transform::from_scale(base, base).post_translate(bx, by);
    resvg::render(&tree, probe, &mut pixmap.as_mut());

    // Second pass: re-render the SAME artwork so its larger side fills a fixed
    // fraction of the box (ICON_FILL), centred — every icon then has the same
    // visual size as the eye/trash, without changing any of the shapes.
    if let Some((minx, miny, maxx, maxy)) = alpha_bbox(pixmap.data(), w, h) {
        let cw = (maxx - minx + 1) as f32;
        let ch = (maxy - miny + 1) as f32;
        let target = (w.min(h) as f32) * ICON_FILL;
        let k = target / cw.max(ch).max(1.0);
        // Content centre (pixels) → svg units (inverse of the probe transform).
        let scx = ((minx as f32 + maxx as f32 + 1.0) * 0.5 - bx) / base;
        let scy = ((miny as f32 + maxy as f32 + 1.0) * 0.5 - by) / base;
        let scale = base * k;
        let tx = w as f32 * 0.5 - scx * scale;
        let ty = h as f32 * 0.5 - scy * scale;
        pixmap = resvg::tiny_skia::Pixmap::new(w, h)?;
        let transform =
            resvg::tiny_skia::Transform::from_scale(scale, scale).post_translate(tx, ty);
        resvg::render(&tree, transform, &mut pixmap.as_mut());
    }

    let image = egui::ColorImage::from_rgba_premultiplied([w as usize, h as usize], pixmap.data());
    let tex = ctx.load_texture(
        format!("svg_icon_{}", icon as u8),
        image,
        egui::TextureOptions::LINEAR,
    );
    ctx.data_mut(|d| {
        d.get_temp_mut_or_insert_with::<SvgIconCache>(id, SvgIconCache::default)
            .0
            .insert(key, tex.clone());
    });
    Some(tex)
}

pub fn paint_icon(
    painter: &egui::Painter,
    ctx: &egui::Context,
    icon: Icon,
    rect: Rect,
    fg: Color32,
) {
    let ppp = ctx.pixels_per_point();
    let w = (rect.width() * ppp).round().max(1.0) as u32;
    let h = (rect.height() * ppp).round().max(1.0) as u32;
    if let Some(tex) = svg_texture(ctx, icon, fg, crate::theme::SNAP, w, h) {
        painter.image(
            tex.id(),
            rect,
            Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
            Color32::WHITE,
        );
    }
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

/// The app symbol rasterized to a square PNG of `size`×`size` px (straight
/// alpha). Used to build the executable/launcher icons.
pub fn app_icon_png(size: u32) -> Option<Vec<u8>> {
    let svg = include_str!("../assets/logotype/symbol.svg");
    rasterize_svg(svg, size, size)?.encode_png().ok()
}

/// A multi-resolution Windows `.ico` of the app symbol as raw file bytes, with
/// one PNG-compressed entry per size (read by Windows Vista+). Embedded into the
/// `.exe` by the app's build script so the file shows the icon in Explorer.
pub fn app_icon_ico() -> Option<Vec<u8>> {
    let sizes = [16u32, 24, 32, 48, 64, 128, 256];
    let mut entries: Vec<(u32, Vec<u8>)> = Vec::with_capacity(sizes.len());
    for &s in &sizes {
        entries.push((s, app_icon_png(s)?));
    }
    let mut out = Vec::new();
    out.extend_from_slice(&0u16.to_le_bytes()); // reserved
    out.extend_from_slice(&1u16.to_le_bytes()); // type: icon
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    let mut offset = 6 + entries.len() * 16; // header + directory entries
    for (s, png) in &entries {
        let dim = if *s >= 256 { 0u8 } else { *s as u8 }; // 256 encodes as 0
        out.push(dim); // width
        out.push(dim); // height
        out.push(0); // palette colors
        out.push(0); // reserved
        out.extend_from_slice(&1u16.to_le_bytes()); // color planes
        out.extend_from_slice(&32u16.to_le_bytes()); // bits per pixel
        out.extend_from_slice(&(png.len() as u32).to_le_bytes());
        out.extend_from_slice(&(offset as u32).to_le_bytes());
        offset += png.len();
    }
    for (_, png) in &entries {
        out.extend_from_slice(png);
    }
    Some(out)
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

    let accent = if enabled { crate::theme::ACCENT } else { fg };
    // The glyph is a fixed pixel size regardless of the button box, so every
    // icon across the UI (toolbars, layer rows, the contextual popup, undo/redo,
    // …) renders at the same visual size. Smaller buttons keep a little padding.
    let glyph = GLYPH_PX.min(size - 6.0).max(8.0);
    let area = snap_rect(
        Rect::from_center_size(rect.center(), Vec2::splat(glyph)),
        ppp,
    );
    let drawn_svg = if icon.svg_src().is_some() {
        let w = (area.width() * ppp).round().max(1.0) as u32;
        let h = (area.height() * ppp).round().max(1.0) as u32;
        svg_texture(ui.ctx(), icon, fg, accent, w, h).inspect(|tex| {
            painter.image(
                tex.id(),
                area,
                Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                Color32::WHITE,
            );
        })
    } else {
        None
    };
    if drawn_svg.is_none() {
        icon.draw(&painter, area, fg);
    }

    if hovered {
        response = response.on_hover_ui(|ui| rich_tooltip(ui, tooltip));
    }
    response
}

/// Render a button tooltip from a string like `"Offset  (Shift+O) — type a
/// distance"`: the name in bold, the parenthesised shortcut as a keycap chip, and
/// any trailing `— description` as a dimmed second line.
fn rich_tooltip(ui: &mut Ui, text: &str) {
    // Split off a leading "(shortcut)" group, if present.
    let (head, keys) = match (text.find('('), text.find(')')) {
        (Some(o), Some(c)) if c > o => (
            text[..o].trim().to_string(),
            Some(text[o + 1..c].trim().to_string()),
        ),
        _ => (text.to_string(), None),
    };
    // Anything after the ")" that begins with an em dash is the description.
    let desc = text
        .find(')')
        .map(|c| text[c + 1..].trim())
        .unwrap_or("")
        .trim_start_matches('—')
        .trim()
        .to_string();

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 7.0;
        ui.label(egui::RichText::new(head).strong().color(crate::theme::TEXT));
        if let Some(k) = keys.filter(|k| !k.is_empty()) {
            let galley = ui.painter().layout_no_wrap(
                k.clone(),
                egui::FontId::monospace(11.0),
                crate::theme::TEXT,
            );
            let pad = Vec2::new(5.0, 2.0);
            let size = galley.size() + pad * 2.0;
            let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
            ui.painter().rect(
                rect,
                4.0,
                Color32::from_rgba_unmultiplied(40, 48, 64, 235),
                Stroke::new(1.0, crate::theme::OUTLINE),
                egui::StrokeKind::Inside,
            );
            ui.painter()
                .galley(rect.min + pad, galley, crate::theme::TEXT);
        }
    });
    if !desc.is_empty() {
        ui.label(
            egui::RichText::new(desc)
                .size(11.5)
                .color(crate::theme::TEXT_DIM),
        );
    }
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

            Icon::Pan | Icon::AddLayer | Icon::Delete => {}
        }
    }
}

use egui::{Color32, Rect, Response, Sense, Stroke, Ui, Vec2, pos2};
use std::collections::HashMap;
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum Icon {
    Select,
    Line,
    Circle,
    Circle2P,
    Circle3P,
    CircleTtr,
    CircleTtt,
    Ellipse,
    Arc,
    ArcStartCenterEnd,
    ArcCenterStartEnd,
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
    Plus,
    Minus,
    Dimension,
    DimAngle,
    DimRadius,
    DimDiameter,
}

impl Icon {
    /// The raw PNG bytes for this icon, embedded at compile time. Every icon is
    /// a 32×32 RGBA glyph drawn in the shared white + blue-accent palette.
    fn png_bytes(self) -> &'static [u8] {
        match self {
            Icon::Select => include_bytes!("../assets/icons/icons_curves_ui_select.png"),
            Icon::Line => include_bytes!("../assets/icons/icons_curves_tool_line.png"),
            Icon::Circle => include_bytes!("../assets/icons/icons_curves_tool_circle.png"),
            Icon::Circle2P => include_bytes!("../assets/icons/icons_curves_tool_circle_2p.png"),
            Icon::Circle3P => include_bytes!("../assets/icons/icons_curves_tool_circle_3p.png"),
            Icon::CircleTtr => {
                include_bytes!("../assets/icons/icons_curves_tool_circle_tan_tan_radius.png")
            }
            Icon::CircleTtt => {
                include_bytes!("../assets/icons/icons_curves_tool_circle_tan_tan_tan.png")
            }
            Icon::Ellipse => include_bytes!("../assets/icons/icons_curves_tool_ellipse.png"),
            Icon::Arc => include_bytes!("../assets/icons/icons_curves_tool_arc_3P.png"),
            Icon::ArcStartCenterEnd => {
                include_bytes!("../assets/icons/icons_curves_tool_arc_start_center_end.png")
            }
            Icon::ArcCenterStartEnd => {
                include_bytes!("../assets/icons/icons_curves_tool_arc_center_start_end.png")
            }
            Icon::Rectangle => include_bytes!("../assets/icons/icons_curves_tool_rectangle.png"),
            Icon::Polygon => include_bytes!("../assets/icons/icons_curves_tool_polygon.png"),
            Icon::Spline => include_bytes!("../assets/icons/icons_curves_tool_spline.png"),
            Icon::Polyline => include_bytes!("../assets/icons/icons_curves_tool_polyline.png"),
            Icon::Text => include_bytes!("../assets/icons/icons_curves_tool_text.png"),
            Icon::Move => include_bytes!("../assets/icons/icons_curves_mod_move.png"),
            Icon::Copy => include_bytes!("../assets/icons/icons_curves_mod_copy.png"),
            Icon::Rotate => include_bytes!("../assets/icons/icons_curves_mod_rotate.png"),
            Icon::Scale => include_bytes!("../assets/icons/icons_curves_mod_scale.png"),
            Icon::Mirror => include_bytes!("../assets/icons/icons_curves_mod_mirror.png"),
            Icon::Offset => include_bytes!("../assets/icons/icons_curves_mod_offset.png"),
            Icon::Trim => include_bytes!("../assets/icons/icons_curves_mod_trim.png"),
            Icon::Extend => include_bytes!("../assets/icons/icons_curves_mod_extend.png"),
            Icon::Fillet => include_bytes!("../assets/icons/icons_curves_mod_fillet.png"),
            Icon::Chamfer => include_bytes!("../assets/icons/icons_curves_mod_chamfer.png"),
            Icon::Stretch => include_bytes!("../assets/icons/icons_curves_mod_stretch.png"),
            Icon::Explode => include_bytes!("../assets/icons/icons_curves_mod_disjoint.png"),
            Icon::Join => include_bytes!("../assets/icons/icons_curves_mod_join.png"),
            Icon::Hatch => include_bytes!("../assets/icons/icons_curves_tool_hatch.png"),
            Icon::Undo => include_bytes!("../assets/icons/icons_curves_ui_undo.png"),
            Icon::Redo => include_bytes!("../assets/icons/icons_curves_ui_redo.png"),
            Icon::Eye => include_bytes!("../assets/icons/icons_curves_ui_show.png"),
            Icon::EyeOff => include_bytes!("../assets/icons/icons_curves_ui_hide.png"),
            Icon::ZoomIn => include_bytes!("../assets/icons/icons_curves_ui_zoom_in.png"),
            Icon::ZoomOut => include_bytes!("../assets/icons/icons_curves_ui_zoom_out.png"),
            Icon::ZoomFit => include_bytes!("../assets/icons/icons_curves_ui_zoom_extents.png"),
            Icon::Pan => include_bytes!("../assets/icons/icons_curves_ui_pan.png"),
            Icon::AddLayer => include_bytes!("../assets/icons/icons_curves_ui_add_layer.png"),
            Icon::Delete => include_bytes!("../assets/icons/icons_curves_ui_delete.png"),
            Icon::Plus => include_bytes!("../assets/icons/icons_curves_ui_plus.png"),
            Icon::Minus => include_bytes!("../assets/icons/icons_curves_ui_minus.png"),
            Icon::Dimension => include_bytes!("../assets/icons/icons_curves_tool_dimension.png"),
            Icon::DimAngle => include_bytes!("../assets/icons/icons_curves_tool_dim_angle.png"),
            Icon::DimRadius => include_bytes!("../assets/icons/icons_curves_dim_radius.png"),
            Icon::DimDiameter => include_bytes!("../assets/icons/icons_curves_dim_diameter.png"),
        }
    }
}

#[derive(Clone, Default)]
struct IconTextureCache(HashMap<u8, egui::TextureHandle>);

/// Decode the icon's embedded PNG into a GPU texture, cached per icon id. The
/// glyph is stored at its native 32×32 resolution and scaled (LINEAR) to the
/// requested box at paint time.
fn icon_texture(ctx: &egui::Context, icon: Icon) -> Option<egui::TextureHandle> {
    let id = egui::Id::new("eiderflat_icon_texture_cache");
    if let Some(tex) = ctx.data(|d| {
        d.get_temp::<IconTextureCache>(id)
            .and_then(|c| c.0.get(&(icon as u8)).cloned())
    }) {
        return Some(tex);
    }

    let pixmap = resvg::tiny_skia::Pixmap::decode_png(icon.png_bytes()).ok()?;
    let image = egui::ColorImage::from_rgba_premultiplied(
        [pixmap.width() as usize, pixmap.height() as usize],
        pixmap.data(),
    );
    let tex = ctx.load_texture(
        format!("icon_{}", icon as u8),
        image,
        egui::TextureOptions::LINEAR,
    );
    ctx.data_mut(|d| {
        d.get_temp_mut_or_insert_with::<IconTextureCache>(id, IconTextureCache::default)
            .0
            .insert(icon as u8, tex.clone());
    });
    Some(tex)
}

/// Paint an icon into `rect`. The glyphs ship pre-coloured (white + blue
/// accent); `tint` multiplies that artwork — pass `Color32::WHITE` to render it
/// faithfully, or a dimmer/coloured value to fade or shade it.
pub fn paint_icon(
    painter: &egui::Painter,
    ctx: &egui::Context,
    icon: Icon,
    rect: Rect,
    tint: Color32,
) {
    if let Some(tex) = icon_texture(ctx, icon) {
        painter.image(
            tex.id(),
            rect,
            Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
            tint,
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

    // The glyphs ship pre-coloured, so a disabled button just fades the artwork
    // rather than recolouring it; enabled buttons render it faithfully.
    let tint = if ui.is_enabled() {
        Color32::WHITE
    } else {
        Color32::WHITE.gamma_multiply(0.4)
    };
    // The glyph is a fixed pixel size regardless of the button box, so every
    // icon across the UI (toolbars, layer rows, the contextual popup, undo/redo,
    // …) renders at the same visual size. Smaller buttons keep a little padding.
    let glyph = GLYPH_PX.min(size - 6.0).max(8.0);
    let area = snap_rect(
        Rect::from_center_size(rect.center(), Vec2::splat(glyph)),
        ppp,
    );
    paint_icon(&painter, ui.ctx(), icon, area, tint);

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

use egui::{Color32, Context, CornerRadius, FontFamily, FontId, Stroke, TextStyle, Visuals};
pub mod tok {
    pub const SP_2: f32 = 6.0;
    pub const SP_3: f32 = 8.0;
    pub const R_SM: u8 = 8;
    pub const R_MD: u8 = 11;
    pub const R_LG: u8 = 16;
    pub const T_XS: f32 = 11.0;
    pub const T_SM: f32 = 12.5;
    pub const T_LG: f32 = 15.0;
}
// ── Planar CAD palette ─────────────────────────────────────────────────────
// Deep blue-black canvas, translucent "glass" panels, a #3b82f6 blue accent.
pub const CANVAS_BG: Color32 = Color32::from_rgb(10, 12, 16); // #0a0c10
pub const PANEL_BG: Color32 = Color32::from_rgb(20, 25, 36); // opaque panel base
// NOTE: egui composites with premultiplied alpha, and `from_rgba_unmultiplied`
// isn't const, so translucent constants store hand-premultiplied bytes
// (r*a/255, g*a/255, b*a/255, a). Passing full RGB with a low alpha to
// `from_rgba_premultiplied` would blow out to a near-solid colour.
/// Translucent floating-panel fill — rgba(17,22,33,~0.87).
pub const PANEL_GLASS: Color32 = Color32::from_rgba_premultiplied(15, 19, 29, 222);
pub const WIDGET_BG: Color32 = Color32::from_rgba_premultiplied(12, 12, 12, 12); // white @ ~5%
pub const WIDGET_HOVER: Color32 = Color32::from_rgba_premultiplied(22, 22, 22, 22); // white @ ~9%
pub const ACCENT: Color32 = Color32::from_rgb(59, 130, 246); // #3b82f6
pub const ACCENT_BRIGHT: Color32 = Color32::from_rgb(108, 168, 255); // #6ca8ff
pub const ACCENT_DIM: Color32 = Color32::from_rgba_premultiplied(12, 27, 50, 52); // accent @ ~20%
pub const SNAP: Color32 = Color32::from_rgb(245, 185, 74); // #f5b94a amber
pub const PREVIEW: Color32 = SNAP;
pub const GUIDE: Color32 = Color32::from_rgb(255, 90, 160); // #ff5aa0 smart-guide magenta
pub const STATUS_GREEN: Color32 = Color32::from_rgb(55, 211, 153); // #37d399 — all saved
pub const STATUS_AMBER: Color32 = Color32::from_rgb(245, 185, 74); // #f5b94a — unsaved changes
pub const STATUS_RED: Color32 = Color32::from_rgb(240, 96, 96); // #f06060 — never saved
pub const TEXT: Color32 = Color32::from_rgb(233, 239, 248); // #e9eff8
pub const TEXT_DIM: Color32 = Color32::from_rgb(140, 152, 172); // ~rgba(233,239,248,0.5)
pub const OUTLINE: Color32 = Color32::from_rgba_premultiplied(16, 16, 16, 16); // white @ ~6%

/// A rounded translucent "glass" frame used for every floating panel/pill.
pub fn glass(radius: u8) -> egui::Frame {
    egui::Frame::new()
        .fill(PANEL_GLASS)
        .stroke(Stroke::new(1.0, OUTLINE))
        .corner_radius(CornerRadius::same(radius))
        .inner_margin(egui::Margin::same(8))
        .shadow(egui::epaint::Shadow {
            offset: [0, 10],
            blur: 38,
            spread: 0,
            color: Color32::from_black_alpha(110),
        })
}

pub fn apply(ctx: &Context) {
    let mut v = Visuals::dark();
    v.panel_fill = CANVAS_BG;
    v.window_fill = PANEL_GLASS;
    v.extreme_bg_color = Color32::from_rgba_unmultiplied(255, 255, 255, 12); // text-edit backgrounds
    v.faint_bg_color = WIDGET_BG;
    v.window_stroke = Stroke::new(1.0, OUTLINE);
    v.window_corner_radius = CornerRadius::same(tok::R_LG);
    v.menu_corner_radius = CornerRadius::same(tok::R_MD);
    let panel_shadow = egui::epaint::Shadow {
        offset: [0, 8],
        blur: 34,
        spread: 0,
        color: Color32::from_black_alpha(120),
    };
    v.window_shadow = panel_shadow;
    v.popup_shadow = panel_shadow;
    v.selection.bg_fill = ACCENT_DIM;
    v.selection.stroke = Stroke::new(1.0, ACCENT);
    v.hyperlink_color = ACCENT_BRIGHT;
    v.override_text_color = Some(TEXT);
    let r = CornerRadius::same(tok::R_SM);
    v.widgets.noninteractive.bg_fill = PANEL_GLASS;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, OUTLINE);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT_DIM);
    v.widgets.noninteractive.corner_radius = r;
    v.widgets.inactive.bg_fill = WIDGET_BG;
    v.widgets.inactive.weak_bg_fill = WIDGET_BG;
    v.widgets.inactive.bg_stroke = Stroke::NONE;
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT);
    v.widgets.inactive.corner_radius = r;
    v.widgets.hovered.bg_fill = WIDGET_HOVER;
    v.widgets.hovered.weak_bg_fill = WIDGET_HOVER;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, ACCENT_DIM);
    v.widgets.hovered.fg_stroke = Stroke::new(1.2, Color32::WHITE);
    v.widgets.hovered.corner_radius = r;
    v.widgets.active.bg_fill = ACCENT_DIM;
    v.widgets.active.weak_bg_fill = ACCENT_DIM;
    v.widgets.active.bg_stroke = Stroke::new(1.0, ACCENT);
    v.widgets.active.fg_stroke = Stroke::new(1.2, Color32::WHITE);
    v.widgets.active.corner_radius = r;
    v.widgets.open.bg_fill = WIDGET_HOVER;
    v.widgets.open.weak_bg_fill = WIDGET_HOVER;
    v.widgets.open.bg_stroke = Stroke::new(1.0, ACCENT_DIM);
    v.widgets.open.corner_radius = r;
    ctx.set_visuals(v);
    ctx.global_style_mut(|s| {
        s.spacing.item_spacing = egui::vec2(tok::SP_2, 5.0);
        s.spacing.button_padding = egui::vec2(7.0, 4.0);
        s.spacing.menu_margin = egui::Margin::same(tok::SP_3 as i8);
        // Hold tooltips back briefly so hovering across the dense toolbars doesn't
        // flash a tip on every icon the pointer crosses; once shown, let them
        // linger long enough to read.
        s.interaction.tooltip_delay = 0.45;
        s.interaction.tooltip_grace_time = 0.25;
        s.text_styles = [
            (
                TextStyle::Small,
                FontId::new(tok::T_XS, FontFamily::Proportional),
            ),
            (
                TextStyle::Body,
                FontId::new(tok::T_SM, FontFamily::Proportional),
            ),
            (
                TextStyle::Button,
                FontId::new(tok::T_SM, FontFamily::Proportional),
            ),
            (
                TextStyle::Heading,
                FontId::new(tok::T_LG, FontFamily::Proportional),
            ),
            (
                TextStyle::Monospace,
                FontId::new(tok::T_SM, FontFamily::Monospace),
            ),
        ]
        .into();
    });
}

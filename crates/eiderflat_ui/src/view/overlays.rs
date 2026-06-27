use super::UiState;
use super::render::corner_glass_frame;
use crate::state::AppState;
use crate::tools::Tool;
use egui::{Color32, Stroke, pos2};
use eiderflat_geometry::{Curve, CurveSegment, Point2d, curvature_at, normal_at};

/// Draw a curvature comb for `curve`: teeth normal to the curve whose length is
/// proportional to curvature, with an envelope through the tips. Reveals
/// smoothness/inflections on splines and arcs. `scale` is the world-unit tooth
/// length per unit curvature; `samples` is the tooth count.
pub(super) fn curvature_comb(
    painter: &egui::Painter,
    app: &AppState,
    curve: &Curve,
    origin: egui::Pos2,
    scale: f64,
    samples: usize,
) {
    let to_screen = |wx: f64, wy: f64| {
        let (sx, sy) = app.view.world_to_screen(wx, wy);
        pos2(origin.x + sx as f32, origin.y + sy as f32)
    };
    let (t0, t1) = curve.domain();
    let n = samples.max(2);
    let tooth = Stroke::new(1.0, Color32::from_rgb(190, 120, 255));
    let envelope = Stroke::new(1.5, Color32::from_rgb(150, 90, 230));
    let mut tips: Vec<egui::Pos2> = Vec::with_capacity(n + 1);
    for i in 0..=n {
        let t = t0 + (t1 - t0) * i as f64 / n as f64;
        let k = match curvature_at(curve, t) {
            Some(k) if k.is_finite() => k,
            _ => continue,
        };
        let (x, y) = curve.evaluate_f64(t);
        let (nx, ny) = normal_at(curve, t);
        let nlen = (nx * nx + ny * ny).sqrt();
        if nlen < 1e-12 {
            continue;
        }
        // Tooth points toward the centre of curvature (along -normal*sign(k)).
        let d = -k * scale;
        let base = to_screen(x, y);
        let tip = to_screen(x + nx / nlen * d, y + ny / nlen * d);
        painter.line_segment([base, tip], tooth);
        tips.push(tip);
    }
    if tips.len() >= 2 {
        painter.add(egui::Shape::line(tips, envelope));
    }
}
/// A compact, non-editable readout pinned just below-right of the cursor while a
/// transform tool (Move/Copy/Rotate/Scale) is mid-operation. These tools have no
/// dynamic-input HUD, so without this there is no live feedback on how far you've
/// moved or how far you've turned.
pub(super) fn cursor_readout(ctx: &egui::Context, app: &AppState, origin: egui::Pos2) {
    let (cx, cy) = app.cursor_world;
    let text = match &app.tool {
        Tool::Move { base: Some(b), .. } | Tool::Copy { base: Some(b), .. } => {
            let (bx, by) = b.to_f64();
            let (dx, dy) = (cx - bx, cy - by);
            Some(format!(
                "Δ {:.2}, {:.2}   {:.2}",
                dx,
                dy,
                (dx * dx + dy * dy).sqrt()
            ))
        }
        Tool::Rotate { base: Some(b), .. } => {
            let (bx, by) = b.to_f64();
            let a = eiderflat_geometry::wrap_deg360((cy - by).atan2(cx - bx).to_degrees());
            Some(format!("{:.1}°", a))
        }
        Tool::Scale {
            base: Some(b),
            reference,
            ..
        } => {
            let (bx, by) = b.to_f64();
            let d = ((cx - bx).powi(2) + (cy - by).powi(2)).sqrt();
            match reference {
                Some(r) if *r > 1e-9 => Some(format!("×{:.3}", d / r)),
                _ => Some(format!("{:.2}", d)),
            }
        }
        _ => None,
    };
    let Some(text) = text else { return };

    let cur = app.view.world_to_screen(cx, cy);
    let pos = pos2(
        origin.x + cur.0 as f32 + 18.0,
        origin.y + cur.1 as f32 + 16.0,
    );
    egui::Area::new(egui::Id::new("cursor_readout"))
        .fixed_pos(pos)
        .order(egui::Order::Foreground)
        .interactable(false)
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(Color32::from_rgba_unmultiplied(15, 19, 29, 200))
                .stroke(Stroke::new(1.0, crate::theme::OUTLINE))
                .corner_radius(crate::theme::tok::R_SM)
                .inner_margin(egui::Margin::symmetric(8, 4))
                .show(ui, |ui| {
                    ui.label(
                        egui::RichText::new(text)
                            .monospace()
                            .size(12.0)
                            .color(crate::theme::ACCENT_BRIGHT),
                    );
                });
        });
}

pub(super) fn dyn_line_hud(
    ctx: &egui::Context,
    app: &mut AppState,
    ui_state: &mut UiState,
    origin: egui::Pos2,
) {
    let line_ref = if let Tool::Line { last: Some(p0) } = &app.tool {
        Some(p0.to_f64())
    } else {
        None
    };
    if let (true, Some((rx, ry))) = (app.dyn_on, line_ref) {
        let (cx, cy) = app.cursor_world;
        let live_len = ((cx - rx).powi(2) + (cy - ry).powi(2)).sqrt();
        let live_ang = eiderflat_geometry::wrap_deg360((cy - ry).atan2(cx - rx).to_degrees());

        let len_id = egui::Id::new("dyn_len");
        let ang_id = egui::Id::new("dyn_ang");
        if !ctx.memory(|m| m.has_focus(len_id)) {
            ui_state.dyn_length = format!("{:.2}", live_len);
        }
        if !ctx.memory(|m| m.has_focus(ang_id)) {
            ui_state.dyn_angle = format!("{:.1}", live_ang);
        }

        let cur = app.view.world_to_screen(cx, cy);
        let hud_pos = pos2(
            origin.x + cur.0 as f32 + 18.0,
            origin.y + cur.1 as f32 - 38.0,
        );
        let first_show = !ui_state.dyn_active;
        let mut commit = false;
        egui::Area::new(egui::Id::new("dyn_input_hud"))
            .fixed_pos(hud_pos)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                corner_glass_frame().show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("L")
                                .size(12.0)
                                .color(Color32::from_gray(170)),
                        );
                        let lr = ui.add(
                            egui::TextEdit::singleline(&mut ui_state.dyn_length)
                                .id(len_id)
                                .desired_width(58.0),
                        );
                        ui.label(
                            egui::RichText::new("∠")
                                .size(12.0)
                                .color(Color32::from_gray(170)),
                        );
                        let ar = ui.add(
                            egui::TextEdit::singleline(&mut ui_state.dyn_angle)
                                .id(ang_id)
                                .desired_width(48.0),
                        );
                        if first_show {
                            lr.request_focus();
                        }
                        if ui.input(|i| i.key_pressed(egui::Key::Enter))
                            && (lr.lost_focus() || ar.lost_focus())
                        {
                            commit = true;
                        }
                    });
                });
            });
        ui_state.dyn_active = true;
        if commit {
            let cmd = format!(
                "@{}<{}",
                ui_state.dyn_length.trim(),
                ui_state.dyn_angle.trim()
            );
            app.run_command(&cmd);
            ui_state.dyn_active = false;
        }
    } else {
        ui_state.dyn_active = false;
    }
}

pub(super) fn dyn_circle_hud(
    ctx: &egui::Context,
    app: &mut AppState,
    ui_state: &mut UiState,
    origin: egui::Pos2,
) {
    let circle_center = if let Tool::Circle { center: Some(c) } = &app.tool {
        Some(c.to_f64())
    } else {
        None
    };
    if let (true, Some((cx, cy))) = (app.dyn_on, circle_center) {
        let (crx, cry) = app.cursor_world;

        let rad_id = egui::Id::new("dyn_radius");
        let first_show = !ui_state.dyn_circle_active;
        if first_show {
            ui_state.dyn_radius.clear();
        }
        ui_state.dyn_circle_active = true;
        if ctx.input(|i| i.key_pressed(egui::Key::Enter))
            && let Ok(rad) = ui_state.dyn_radius.trim().parse::<f64>()
            && rad > 1e-9
        {
            app.place_tool_point(Point2d::from_f64(cx + rad, cy));
            ui_state.dyn_circle_active = false;
            return;
        }

        let cur = app.view.world_to_screen(crx, cry);
        let hud_pos = pos2(
            origin.x + cur.0 as f32 + 18.0,
            origin.y + cur.1 as f32 - 38.0,
        );
        egui::Area::new(egui::Id::new("dyn_circle_hud"))
            .fixed_pos(hud_pos)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                corner_glass_frame().show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("R")
                                .size(12.0)
                                .color(Color32::from_gray(170)),
                        );
                        let rr = ui.add(
                            egui::TextEdit::singleline(&mut ui_state.dyn_radius)
                                .id(rad_id)
                                .desired_width(58.0)
                                .hint_text("radius"),
                        );
                        let nothing_focused = ui.ctx().memory(|m| m.focused().is_none());
                        if first_show || nothing_focused {
                            rr.request_focus();
                        }
                    });
                });
            });
    } else {
        ui_state.dyn_circle_active = false;
    }
}
pub(super) fn dyn_polygon_hud(
    ctx: &egui::Context,
    app: &mut AppState,
    ui_state: &mut UiState,
    origin: egui::Pos2,
) {
    // `Some(sides)` here is the current Option<usize> count (None until entered).
    let sides = if let Tool::Polygon {
        center: None,
        sides,
    } = &app.tool
    {
        Some(*sides)
    } else {
        None
    };
    if let (true, Some(sides)) = (app.dyn_on, sides) {
        let sid = egui::Id::new("dyn_poly_sides");
        if !ctx.memory(|m| m.has_focus(sid)) {
            ui_state.dyn_poly_sides = sides.map(|n| n.to_string()).unwrap_or_default();
        }

        let (cx, cy) = app.cursor_world;
        let cur = app.view.world_to_screen(cx, cy);
        let hud_pos = pos2(
            origin.x + cur.0 as f32 + 18.0,
            origin.y + cur.1 as f32 - 38.0,
        );
        let first_show = !ui_state.dyn_poly_active;
        egui::Area::new(egui::Id::new("dyn_poly_hud"))
            .fixed_pos(hud_pos)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                corner_glass_frame().show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("Sides")
                                .size(12.0)
                                .color(Color32::from_gray(170)),
                        );
                        let r = ui.add(
                            egui::TextEdit::singleline(&mut ui_state.dyn_poly_sides)
                                .id(sid)
                                .desired_width(40.0)
                                .hint_text("3+"),
                        );
                        let nothing_focused = ui.ctx().memory(|m| m.focused().is_none());
                        if first_show || nothing_focused {
                            r.request_focus();
                        }
                    });
                });
            });
        ui_state.dyn_poly_active = true;
        let parsed = ui_state
            .dyn_poly_sides
            .trim()
            .parse::<usize>()
            .ok()
            .filter(|n| *n >= 3);
        if parsed != sides {
            app.tool = Tool::Polygon {
                center: None,
                sides: parsed,
            };
        }
    } else {
        ui_state.dyn_poly_active = false;
    }
}

pub(super) fn dyn_rect_hud(
    ctx: &egui::Context,
    app: &mut AppState,
    ui_state: &mut UiState,
    origin: egui::Pos2,
) {
    let rect_first = if let Tool::Rectangle { first: Some(f) } = &app.tool {
        Some(f.to_f64())
    } else {
        None
    };
    if let (true, Some((fx, fy))) = (app.dyn_on, rect_first) {
        let (crx, cry) = app.cursor_world;

        let field_id = egui::Id::new("dyn_rect_field");
        let first_show = !ui_state.dyn_rect_active;
        if first_show {
            ui_state.dyn_rect_width.clear();
            ui_state.dyn_rect_height.clear();
            ui_state.dyn_rect_stage_h = false;
        }
        ui_state.dyn_rect_active = true;
        let mut committed = false;
        let mut focus_field = first_show;
        if ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
            if !ui_state.dyn_rect_stage_h {
                if let Ok(w) = ui_state.dyn_rect_width.trim().parse::<f64>()
                    && w.abs() > 1e-9
                {
                    ui_state.dyn_rect_stage_h = true;
                    focus_field = true;
                }
            } else if let Ok(h) = ui_state.dyn_rect_height.trim().parse::<f64>() {
                let w = ui_state.dyn_rect_width.trim().parse::<f64>().unwrap_or(0.0);
                if h.abs() > 1e-9 && w.abs() > 1e-9 {
                    // Grow the rectangle toward whichever quadrant the cursor is
                    // in, so the typed width/height never need a minus sign:
                    // aim up-left and a "10 × 5" box is drawn up and to the left.
                    let sx = if crx >= fx { 1.0 } else { -1.0 };
                    let sy = if cry >= fy { 1.0 } else { -1.0 };
                    app.place_tool_point(Point2d::from_f64(fx + w.abs() * sx, fy + h.abs() * sy));
                    ui_state.dyn_rect_active = false;
                    committed = true;
                }
            }
        }
        if committed {
            return;
        }

        let on_height = ui_state.dyn_rect_stage_h;
        let cur = app.view.world_to_screen(crx, cry);
        let hud_pos = pos2(
            origin.x + cur.0 as f32 + 18.0,
            origin.y + cur.1 as f32 - 38.0,
        );
        egui::Area::new(egui::Id::new("dyn_rect_hud"))
            .fixed_pos(hud_pos)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                corner_glass_frame().show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let (label, buf, hint) = if on_height {
                            ("H", &mut ui_state.dyn_rect_height, "height, Enter")
                        } else {
                            ("W", &mut ui_state.dyn_rect_width, "width, Enter")
                        };
                        ui.label(
                            egui::RichText::new(label)
                                .size(12.0)
                                .color(Color32::from_gray(170)),
                        );
                        let r = ui.add(
                            egui::TextEdit::singleline(buf)
                                .id(field_id)
                                .desired_width(70.0)
                                .hint_text(hint),
                        );
                        let nothing_focused = ui.ctx().memory(|m| m.focused().is_none());
                        if focus_field || nothing_focused {
                            r.request_focus();
                        }
                    });
                });
            });
    } else {
        ui_state.dyn_rect_active = false;
    }
}
pub(super) fn dyn_ellipse_hud(
    ctx: &egui::Context,
    app: &mut AppState,
    ui_state: &mut UiState,
    origin: egui::Pos2,
) {
    let stage = match &app.tool {
        Tool::Ellipse {
            center: Some(c),
            axis_end: None,
        } => Some((c.to_f64(), None)),
        Tool::Ellipse {
            center: Some(c),
            axis_end: Some(a),
        } => Some((c.to_f64(), Some(a.to_f64()))),
        _ => None,
    };
    if let (true, Some((center, axis_end))) = (app.dyn_on, stage) {
        let (crx, cry) = app.cursor_world;
        let first_show = !ui_state.dyn_ell_active;
        if first_show {
            ui_state.dyn_ell_major.clear();
            ui_state.dyn_ell_minor.clear();
        }
        ui_state.dyn_ell_active = true;
        let maj_id = egui::Id::new("dyn_ell_major");
        let min_id = egui::Id::new("dyn_ell_minor");
        let active_id = if axis_end.is_none() { maj_id } else { min_id };
        let tab = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Tab))
            | ctx.input_mut(|i| i.consume_key(egui::Modifiers::SHIFT, egui::Key::Tab));
        if tab {
            ctx.memory_mut(|m| m.request_focus(active_id));
        }

        let mut committed = false;
        if ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
            match axis_end {
                None => {
                    if let Ok(maj) = ui_state.dyn_ell_major.trim().parse::<f64>()
                        && maj.abs() > 1e-9
                    {
                        let dir = (crx - center.0, cry - center.1);
                        let len = (dir.0 * dir.0 + dir.1 * dir.1).sqrt();
                        let (ux, uy) = if len > 1e-9 {
                            (dir.0 / len, dir.1 / len)
                        } else {
                            (1.0, 0.0)
                        };
                        app.place_tool_point(Point2d::from_f64(
                            center.0 + maj * ux,
                            center.1 + maj * uy,
                        ));
                        ui_state.dyn_ell_active = false;
                        committed = true;
                    }
                }
                Some(a_end) => {
                    if let Ok(minr) = ui_state.dyn_ell_minor.trim().parse::<f64>()
                        && minr.abs() > 1e-9
                    {
                        let dir = (a_end.0 - center.0, a_end.1 - center.1);
                        let len = (dir.0 * dir.0 + dir.1 * dir.1).sqrt().max(1e-12);
                        let (px, py) = (-dir.1 / len, dir.0 / len);
                        app.place_tool_point(Point2d::from_f64(
                            center.0 + minr * px,
                            center.1 + minr * py,
                        ));
                        ui_state.dyn_ell_active = false;
                        committed = true;
                    }
                }
            }
        }
        if committed {
            return;
        }

        let cur = app.view.world_to_screen(crx, cry);
        let hud_pos = pos2(
            origin.x + cur.0 as f32 + 18.0,
            origin.y + cur.1 as f32 - 52.0,
        );
        egui::Area::new(egui::Id::new("dyn_ell_hud"))
            .fixed_pos(hud_pos)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                corner_glass_frame().show(ui, |ui| {
                    if axis_end.is_none() {
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new("A")
                                    .size(12.0)
                                    .color(Color32::from_gray(170)),
                            );
                            let mr = ui.add(
                                egui::TextEdit::singleline(&mut ui_state.dyn_ell_major)
                                    .id(maj_id)
                                    .desired_width(54.0)
                                    .hint_text("major (aim with cursor)"),
                            );
                            let nothing_focused = ui.ctx().memory(|m| m.focused().is_none());
                            if first_show || nothing_focused {
                                mr.request_focus();
                            }
                        });
                    } else {
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new("B")
                                    .size(12.0)
                                    .color(Color32::from_gray(170)),
                            );
                            let br = ui.add(
                                egui::TextEdit::singleline(&mut ui_state.dyn_ell_minor)
                                    .id(min_id)
                                    .desired_width(54.0)
                                    .hint_text("minor"),
                            );
                            let nothing_focused = ui.ctx().memory(|m| m.focused().is_none());
                            if first_show || nothing_focused {
                                br.request_focus();
                            }
                        });
                    }
                });
            });
    } else {
        ui_state.dyn_ell_active = false;
    }
}

pub(super) fn dyn_offset_hud(
    ctx: &egui::Context,
    app: &mut AppState,
    ui_state: &mut UiState,
    origin: egui::Pos2,
) {
    let dist = if let Tool::Offset { dist, .. } = &app.tool {
        Some(*dist)
    } else {
        None
    };
    if let (true, Some(dist)) = (app.dyn_on, dist) {
        let first_show = !ui_state.dyn_offset_active;
        if first_show {
            ui_state.dyn_offset_dist = format!("{:.4}", dist)
                .trim_end_matches('0')
                .trim_end_matches('.')
                .to_string();
        }
        let did = egui::Id::new("dyn_offset_dist");

        let (crx, cry) = app.cursor_world;
        let cur = app.view.world_to_screen(crx, cry);
        let hud_pos = pos2(
            origin.x + cur.0 as f32 + 18.0,
            origin.y + cur.1 as f32 - 38.0,
        );
        egui::Area::new(egui::Id::new("dyn_offset_hud"))
            .fixed_pos(hud_pos)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                corner_glass_frame().show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("Dist")
                                .size(12.0)
                                .color(Color32::from_gray(170)),
                        );
                        let dr = ui.add(
                            egui::TextEdit::singleline(&mut ui_state.dyn_offset_dist)
                                .id(did)
                                .desired_width(58.0)
                                .hint_text("distance"),
                        );
                        let nothing_focused = ui.ctx().memory(|m| m.focused().is_none());
                        if first_show || nothing_focused {
                            dr.request_focus();
                        }
                    });
                });
            });
        ui_state.dyn_offset_active = true;
        if let Ok(d) = ui_state.dyn_offset_dist.trim().parse::<f64>()
            && d > 1e-9
            && let Tool::Offset { source, .. } = &app.tool
        {
            app.tool = Tool::Offset {
                dist: d,
                source: *source,
            };
        }
    } else {
        ui_state.dyn_offset_active = false;
    }
}
/// Radius/distance entry for the Fillet and Chamfer tools — a field at the cursor
/// so the value can be typed before (or between) the two picks, like Offset.
pub(super) fn dyn_corner_hud(
    ctx: &egui::Context,
    app: &mut AppState,
    ui_state: &mut UiState,
    origin: egui::Pos2,
) {
    // (label, current value) for whichever value-then-pick tool is active.
    let info = match &app.tool {
        Tool::Fillet { radius, .. } => Some(("Radius", *radius)),
        Tool::Chamfer { dist, .. } => Some(("Dist", *dist)),
        Tool::CircleTtr { radius, .. } => Some(("Radius", *radius)),
        _ => None,
    };
    let (Some((label, value)), true) = (info, app.dyn_on) else {
        ui_state.dyn_corner_active = false;
        return;
    };

    let first_show = !ui_state.dyn_corner_active;
    if first_show {
        ui_state.dyn_corner_val = format!("{value:.4}")
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string();
    }
    let id = egui::Id::new("dyn_corner_val");
    let (crx, cry) = app.cursor_world;
    let cur = app.view.world_to_screen(crx, cry);
    let hud_pos = pos2(
        origin.x + cur.0 as f32 + 18.0,
        origin.y + cur.1 as f32 - 38.0,
    );
    egui::Area::new(egui::Id::new("dyn_corner_hud"))
        .fixed_pos(hud_pos)
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            corner_glass_frame().show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(label)
                            .size(12.0)
                            .color(Color32::from_gray(170)),
                    );
                    let r = ui.add(
                        egui::TextEdit::singleline(&mut ui_state.dyn_corner_val)
                            .id(id)
                            .desired_width(58.0)
                            .hint_text("value, then pick lines"),
                    );
                    let nothing_focused = ui.ctx().memory(|m| m.focused().is_none());
                    if first_show || nothing_focused {
                        r.request_focus();
                    }
                });
            });
        });
    ui_state.dyn_corner_active = true;
    // Push the typed value back into the tool so the pick uses it.
    if let Ok(v) = ui_state.dyn_corner_val.trim().parse::<f64>()
        && v > 1e-9
    {
        match &app.tool {
            Tool::Fillet { first, .. } => {
                app.tool = Tool::Fillet {
                    radius: v,
                    first: *first,
                }
            }
            Tool::Chamfer { first, .. } => {
                app.tool = Tool::Chamfer {
                    dist: v,
                    first: *first,
                }
            }
            Tool::CircleTtr { first, .. } => {
                app.tool = Tool::CircleTtr {
                    radius: v,
                    first: *first,
                }
            }
            _ => {}
        }
    }
}

pub(super) fn dyn_text_hud(
    ctx: &egui::Context,
    app: &mut AppState,
    ui_state: &mut UiState,
    origin: egui::Pos2,
) {
    let anchor = if let Tool::Text {
        anchor: Some(a), ..
    } = &app.tool
    {
        Some(a.to_f64())
    } else {
        None
    };
    if let Some((ax, ay)) = anchor {
        let first_show = !ui_state.dyn_text_active;
        if first_show {
            ui_state.dyn_text_content.clear();
        }
        let tid = egui::Id::new("dyn_text_field");
        let sp = app.view.world_to_screen(ax, ay);
        let hud_pos = pos2(origin.x + sp.0 as f32, origin.y + sp.1 as f32 - 26.0);
        let mut commit = false;
        let mut cancel = false;
        egui::Area::new(egui::Id::new("dyn_text_hud"))
            .fixed_pos(hud_pos)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let te = ui.add(
                            egui::TextEdit::singleline(&mut ui_state.dyn_text_content)
                                .id(tid)
                                .desired_width(180.0)
                                .hint_text("type text, Enter to place"),
                        );
                        ui.add_space(4.0);
                        super::chrome::font_combo(ui, "dyn_text_font", &mut app.text_font);
                        height_glyph(ui);
                        let mut size = if let Tool::Text { height, .. } = &app.tool {
                            *height
                        } else {
                            2.5
                        };
                        let dv = ui
                            .add(egui::DragValue::new(&mut size).speed(0.05).range(0.1..=1e6))
                            .on_hover_text("Text height");
                        if dv.changed()
                            && let Tool::Text { height, .. } = &mut app.tool
                        {
                            *height = size;
                        }
                        let nothing_focused = ui.ctx().memory(|m| m.focused().is_none());
                        if first_show || nothing_focused {
                            te.request_focus();
                        }
                        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                            cancel = true;
                        } else if (te.lost_focus() || te.has_focus())
                            && ui.input(|i| i.key_pressed(egui::Key::Enter))
                        {
                            commit = true;
                        }
                    });
                });
            });
        ui_state.dyn_text_active = true;
        if commit {
            let content = std::mem::take(&mut ui_state.dyn_text_content);
            app.run_command(&content);
            ui_state.dyn_text_active = false;
        } else if cancel {
            app.tool = Tool::Select;
            ui_state.dyn_text_active = false;
        }
    } else {
        ui_state.dyn_text_active = false;
    }
}
fn height_glyph(ui: &mut egui::Ui) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(11.0, 16.0), egui::Sense::hover());
    let x = rect.center().x;
    let (top, bot) = (rect.top() + 2.0, rect.bottom() - 2.0);
    let s = egui::Stroke::new(1.3, Color32::from_gray(170));
    let p = ui.painter();
    p.line_segment([pos2(x, top), pos2(x, bot)], s);
    for (y, dy) in [(top, 3.5_f32), (bot, -3.5_f32)] {
        p.line_segment([pos2(x, y), pos2(x - 3.0, y + dy)], s);
        p.line_segment([pos2(x, y), pos2(x + 3.0, y + dy)], s);
    }
}

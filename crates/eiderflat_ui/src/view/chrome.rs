use super::UiState;
use crate::command::Command;
use crate::state::AppState;
use crate::tools::Tool;
use egui::{Color32, Context};
use eiderflat_document::{EntityKind, Layer};
use eiderflat_geometry::{Curve, Point2d};
use rfd::FileDialog;

/// Global keyboard shortcuts (formerly handled inside the menu bar) plus the
/// window-title sync. Called once per frame before the chrome is drawn.
pub(super) fn handle_shortcuts(ctx: &Context, app: &mut AppState, ui_state: &mut UiState) {
    let title = app.window_title();
    if ui_state.last_title != title {
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
        ui_state.last_title = title;
    }
    let (ctrl, shift) = ctx.input(|i| (i.modifiers.ctrl, i.modifiers.shift));
    let s_key = ctx.input(|i| i.key_pressed(egui::Key::S));
    let n_key = ctx.input(|i| i.key_pressed(egui::Key::N));
    let o_key = ctx.input(|i| i.key_pressed(egui::Key::O));
    if ctrl && n_key && maybe_save(app) {
        app.new_document();
    }
    if ctrl && o_key && maybe_save(app) {
        file_open(app);
    }
    let save_as_key = ctrl && shift && s_key;
    let save_key = ctrl && !shift && s_key;
    if save_as_key || (save_key && !app.save_file()) {
        file_save_as(app);
    }
    let typing = ctx.memory(|m| m.focused().is_some());
    if !typing {
        let z = ctx.input(|i| i.key_pressed(egui::Key::Z));
        let y = ctx.input(|i| i.key_pressed(egui::Key::Y));
        if ctrl && ((z && shift) || y) {
            app.redo();
        } else if ctrl && z {
            app.undo();
        }
        // Ctrl+A selects every entity.
        if ctrl && ctx.input(|i| i.key_pressed(egui::Key::A)) {
            app.execute(Command::SelectAll);
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Delete)) {
            app.erase_selection();
        }
    }
}

/// Floating "glass" top bar pill: brand, menus, undo/redo, command search and export.
pub(super) fn top_bar(ctx: &Context, app: &mut AppState, canvas_rect: egui::Rect) {
    let margin = 12.0;
    let pos = canvas_rect.left_top() + egui::vec2(margin, margin);
    let width = canvas_rect.width() - 2.0 * margin;
    egui::Area::new(egui::Id::new("top_bar_pill"))
        .fixed_pos(pos)
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            ui.set_width(width);
            crate::theme::glass(crate::theme::tok::R_LG)
                .inner_margin(egui::Margin::symmetric(10, 6))
                .show(ui, |ui| {
                    ui.set_width(width - 20.0);
                    ui.set_height(34.0);
                    ui.horizontal_centered(|ui| {
                        ui.add_space(4.0);
                        // Document name + save-status dot:
                        //   red   = never saved to disk
                        //   amber = saved before, but has unsaved changes
                        //   green = saved, no pending changes
                        ui.label(
                            egui::RichText::new(app.document_label())
                                .size(13.0)
                                .color(crate::theme::TEXT),
                        );
                        {
                            let (dot_color, status) = if app.current_file_path.is_none() {
                                (crate::theme::STATUS_RED, "Not saved yet")
                            } else if app.is_dirty() {
                                (crate::theme::STATUS_AMBER, "Unsaved changes")
                            } else {
                                (crate::theme::STATUS_GREEN, "All changes saved")
                            };
                            let (rect, resp) =
                                ui.allocate_exact_size(egui::vec2(8.0, 8.0), egui::Sense::hover());
                            ui.painter().circle_filled(rect.center(), 3.0, dot_color);
                            resp.on_hover_text(status);
                        }
                        ui.add_space(6.0);
                        menu_items(ui, app);
                        // Undo / redo sit just right of the Help menu (as requested).
                        ui.add_space(2.0);
                        ui.scope(|ui| {
                            ui.spacing_mut().item_spacing.x = 2.0;
                            ui.add_enabled_ui(app.history.can_undo(), |ui| {
                                if crate::icons::icon_button_sized(
                                    ui,
                                    crate::icons::Icon::Undo,
                                    "Undo  (Ctrl+Z)",
                                    false,
                                    30.0,
                                )
                                .clicked()
                                {
                                    app.undo();
                                }
                            });
                            ui.add_enabled_ui(app.history.can_redo(), |ui| {
                                if crate::icons::icon_button_sized(
                                    ui,
                                    crate::icons::Icon::Redo,
                                    "Redo  (Ctrl+Y)",
                                    false,
                                    30.0,
                                )
                                .clicked()
                                {
                                    app.redo();
                                }
                            });
                        });

                        // Right cluster (search · export · avatar), right-aligned.
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.add_space(2.0);
                            if ui
                                .add(export_button())
                                .on_hover_text("Export DXF / SVG")
                                .clicked()
                            {
                                ui.ctx().data_mut(|d| {
                                    d.insert_temp(egui::Id::new("open_export"), true)
                                });
                            }
                            ui.add_space(8.0);
                            if ui.add(search_button()).clicked() {
                                ui.ctx().data_mut(|d| {
                                    d.insert_temp(egui::Id::new("open_palette"), true)
                                });
                            }
                        });
                    });
                });
        });

    export_menu(ctx, app);
}

fn search_button() -> impl egui::Widget {
    move |ui: &mut egui::Ui| {
        let desired = egui::vec2(264.0, 32.0);
        let (rect, resp) = ui.allocate_exact_size(desired, egui::Sense::click());
        let hov = resp.hovered();
        let fill = if hov {
            crate::theme::WIDGET_HOVER
        } else {
            crate::theme::WIDGET_BG
        };
        let p = ui.painter();
        p.rect(
            rect,
            9.0,
            fill,
            egui::Stroke::new(1.0, crate::theme::OUTLINE),
            egui::StrokeKind::Inside,
        );
        p.circle_stroke(
            egui::pos2(rect.left() + 15.0, rect.center().y),
            4.5,
            egui::Stroke::new(1.4, crate::theme::TEXT_DIM),
        );
        p.text(
            egui::pos2(rect.left() + 28.0, rect.center().y),
            egui::Align2::LEFT_CENTER,
            "Search or run a command",
            egui::FontId::proportional(12.5),
            crate::theme::TEXT_DIM,
        );
        // Two separate keycaps on the right — "Ctrl" and "K" — instead of one
        // combined badge.
        let cap = |p: &egui::Painter, right: f32, text: &str| -> f32 {
            let galley = p.layout_no_wrap(
                text.to_string(),
                egui::FontId::monospace(10.0),
                crate::theme::TEXT_DIM,
            );
            let w = galley.size().x + 10.0;
            let kr = egui::Rect::from_min_size(
                egui::pos2(right - w, rect.center().y - 9.0),
                egui::vec2(w, 18.0),
            );
            p.rect(
                kr,
                5.0,
                crate::theme::WIDGET_BG,
                egui::Stroke::new(1.0, crate::theme::OUTLINE),
                egui::StrokeKind::Inside,
            );
            p.text(
                kr.center(),
                egui::Align2::CENTER_CENTER,
                text,
                egui::FontId::monospace(10.0),
                crate::theme::TEXT_DIM,
            );
            kr.left()
        };
        let mut right = rect.right() - 10.0;
        right = cap(p, right, "K") - 4.0;
        cap(p, right, "Ctrl");
        resp
    }
}

fn export_button() -> impl egui::Widget {
    move |ui: &mut egui::Ui| {
        let desired = egui::vec2(86.0, 30.0);
        let (rect, resp) = ui.allocate_exact_size(desired, egui::Sense::click());
        let fill = if resp.hovered() {
            crate::theme::ACCENT_BRIGHT
        } else {
            crate::theme::ACCENT
        };
        let p = ui.painter();
        p.rect_filled(rect, 9.0, fill);
        p.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "Export",
            egui::FontId::proportional(12.5),
            Color32::WHITE,
        );
        resp
    }
}

fn export_menu(ctx: &Context, app: &mut AppState) {
    if !ctx.data(|d| {
        d.get_temp::<bool>(egui::Id::new("open_export"))
            .unwrap_or(false)
    }) {
        return;
    }
    let mut open = true;
    egui::Window::new("Export")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .open(&mut open)
        .show(ctx, |ui| {
            ui.set_width(220.0);
            if ui.button("Export DXF…").clicked() {
                if let Some(path) = FileDialog::new().add_filter("DXF", &["dxf"]).save_file() {
                    let content = eiderflat_io::export_dxf(&app.document);
                    if let Err(e) = std::fs::write(&path, content) {
                        app.command_log.push(format!("DXF export failed: {e}"));
                    }
                }
                ctx.data_mut(|d| d.insert_temp(egui::Id::new("open_export"), false));
            }
            if ui.button("Export SVG…").clicked() {
                if let Some(path) = FileDialog::new()
                    .add_filter("SVG image", &["svg"])
                    .save_file()
                {
                    let content = eiderflat_io::export_svg(&app.document);
                    if let Err(e) = std::fs::write(&path, content) {
                        app.command_log.push(format!("SVG export failed: {e}"));
                    }
                }
                ctx.data_mut(|d| d.insert_temp(egui::Id::new("open_export"), false));
            }
        });
    if !open {
        ctx.data_mut(|d| d.insert_temp(egui::Id::new("open_export"), false));
    }
}

fn menu_items(ui: &mut egui::Ui, app: &mut AppState) {
    ui.spacing_mut().item_spacing.x = 12.0;
    ui.menu_button("File", |ui| {
        if ui
            .add(egui::Button::new("New").shortcut_text("Ctrl+N"))
            .clicked()
        {
            if maybe_save(app) {
                app.new_document();
            }
            ui.close();
        }
        if ui
            .add(egui::Button::new("Open…").shortcut_text("Ctrl+O"))
            .clicked()
        {
            if maybe_save(app) {
                file_open(app);
            }
            ui.close();
        }
        if ui
            .add(egui::Button::new("Save").shortcut_text("Ctrl+S"))
            .clicked()
        {
            if !app.save_file() {
                file_save_as(app);
            }
            ui.close();
        }
        if ui
            .add(egui::Button::new("Save As…").shortcut_text("Ctrl+Shift+S"))
            .clicked()
        {
            file_save_as(app);
            ui.close();
        }
        ui.separator();
        if ui.button("Export DXF…").clicked() {
            if let Some(path) = FileDialog::new().add_filter("DXF", &["dxf"]).save_file() {
                let content = eiderflat_io::export_dxf(&app.document);
                if let Err(e) = std::fs::write(&path, content) {
                    app.command_log.push(format!("DXF export failed: {e}"));
                }
            }
            ui.close();
        }
        if ui.button("Export SVG…").clicked() {
            if let Some(path) = FileDialog::new()
                .add_filter("SVG image", &["svg"])
                .save_file()
            {
                let content = eiderflat_io::export_svg(&app.document);
                if let Err(e) = std::fs::write(&path, content) {
                    app.command_log.push(format!("SVG export failed: {e}"));
                }
            }
            ui.close();
        }
    });
    ui.menu_button("Edit", |ui| {
        if ui
            .add_enabled(
                app.history.can_undo(),
                egui::Button::new("Undo").shortcut_text("Ctrl+Z"),
            )
            .clicked()
        {
            app.undo();
        }
        if ui
            .add_enabled(
                app.history.can_redo(),
                egui::Button::new("Redo").shortcut_text("Ctrl+Y"),
            )
            .clicked()
        {
            app.redo();
        }
        ui.separator();
        if ui
            .add(egui::Button::new("Erase").shortcut_text("Del"))
            .clicked()
        {
            app.erase_selection();
        }
        if ui.button("Select All").clicked() {
            app.execute(Command::SelectAll);
        }
        ui.separator();
        if ui
            .add(egui::Button::new("Command Palette…").shortcut_text("Ctrl+F"))
            .clicked()
        {
            ui.ctx()
                .data_mut(|d| d.insert_temp(egui::Id::new("open_palette"), true));
        }
    });
    ui.menu_button("View", |ui| {
        if ui
            .add(egui::Button::new("Zoom Extents").shortcut_text("Z"))
            .clicked()
        {
            app.zoom_extents();
        }
        ui.separator();
        ui.checkbox(&mut app.grid_on, "Grid  (F7)");
        ui.checkbox(&mut app.grid_snap_on, "Snap to Grid");
        ui.checkbox(&mut app.snap_on, "Object Snap  (F9)");
    });
    ui.menu_button("Draw", |ui| {
        tool_menu_item(ui, app, "Select", Tool::Select);
        ui.separator();
        tool_menu_item(ui, app, "Line", Tool::Line { last: None });
        tool_menu_item(ui, app, "Circle", Tool::Circle { center: None });
        tool_menu_item(
            ui,
            app,
            "Ellipse",
            Tool::Ellipse {
                center: None,
                axis_end: None,
            },
        );
        tool_menu_item(ui, app, "Arc", Tool::Arc3 { pts: vec![] });
        tool_menu_item(ui, app, "Rectangle", Tool::Rectangle { first: None });
        tool_menu_item(
            ui,
            app,
            "Polygon",
            Tool::Polygon {
                center: None,
                sides: None,
            },
        );
        tool_menu_item(ui, app, "Spline", Tool::Spline { pts: vec![] });
        tool_menu_item(ui, app, "Polyline", Tool::Polyline { pts: vec![] });
        tool_menu_item(
            ui,
            app,
            "Text",
            Tool::Text {
                anchor: None,
                height: 2.5,
            },
        );
    });
    ui.menu_button("Modify", |ui| {
        tool_menu_item(
            ui,
            app,
            "Move",
            Tool::Move {
                base: None,
                ids: vec![],
            },
        );
        tool_menu_item(
            ui,
            app,
            "Copy",
            Tool::Copy {
                base: None,
                ids: vec![],
            },
        );
        tool_menu_item(
            ui,
            app,
            "Rotate",
            Tool::Rotate {
                base: None,
                ids: vec![],
            },
        );
        tool_menu_item(
            ui,
            app,
            "Scale",
            Tool::Scale {
                base: None,
                reference: None,
                ids: vec![],
            },
        );
        tool_menu_item(
            ui,
            app,
            "Mirror",
            Tool::Mirror {
                first: None,
                ids: vec![],
            },
        );
        tool_menu_item(
            ui,
            app,
            "Stretch",
            Tool::Stretch {
                c1: None,
                c2: None,
                base: None,
                ids: vec![],
            },
        );
        ui.separator();
        tool_menu_item(
            ui,
            app,
            "Offset",
            Tool::Offset {
                dist: 1.0,
                source: None,
            },
        );
        tool_menu_item(ui, app, "Trim", Tool::Trim);
        tool_menu_item(ui, app, "Extend", Tool::Extend);
        tool_menu_item(
            ui,
            app,
            "Fillet",
            Tool::Fillet {
                radius: 1.0,
                first: None,
            },
        );
        tool_menu_item(
            ui,
            app,
            "Chamfer",
            Tool::Chamfer {
                dist: 1.0,
                first: None,
            },
        );
        ui.separator();
        if ui
            .add(egui::Button::new("Disjoint").shortcut_text("X"))
            .clicked()
        {
            app.explode_selection();
            ui.close();
        }
        if ui
            .add(egui::Button::new("Join").shortcut_text("J"))
            .clicked()
        {
            app.join_selection();
            ui.close();
        }
        if ui
            .add(egui::Button::new("Hatch").shortcut_text("H"))
            .clicked()
        {
            app.execute(Command::Hatch);
            ui.close();
        }
    });
    ui.menu_button("Help", |ui| {
        if ui.button("About eiderFLAT").clicked() {
            ui.ctx()
                .data_mut(|d| d.insert_temp(egui::Id::new("open_about"), true));
            ui.close();
        }
    });
}

pub(super) fn about_window(ctx: &Context, ui_state: &mut UiState) {
    // Pick up the open request flagged by the Help menu.
    if ctx.data(|d| {
        d.get_temp::<bool>(egui::Id::new("open_about"))
            .unwrap_or(false)
    }) {
        ctx.data_mut(|d| d.insert_temp(egui::Id::new("open_about"), false));
        ui_state.about_open = true;
    }
    if !ui_state.about_open {
        return;
    }

    let backdrop = egui::Area::new(egui::Id::new("about_backdrop"))
        .order(egui::Order::Middle)
        .fixed_pos(ctx.content_rect().min)
        .show(ctx, |ui| {
            let r = ctx.content_rect();
            ui.painter()
                .rect_filled(r, 0.0, egui::Color32::from_black_alpha(160));
            ui.allocate_rect(r, egui::Sense::click())
        });
    let mut close = backdrop.inner.clicked();

    egui::Window::new("about_dialog")
        .title_bar(false)
        .collapsible(false)
        .resizable(false)
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .fixed_size(egui::vec2(360.0, 0.0))
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(14.0);
                if let Some(tex) = crate::icons::logo_texture(ui.ctx()) {
                    let s = tex.size_vec2();
                    let w = 280.0;
                    let size = egui::vec2(w, w * s.y / s.x);
                    ui.image(egui::load::SizedTexture::new(tex.id(), size));
                }
                ui.add_space(10.0);
                ui.label(
                    egui::RichText::new(concat!("Version ", env!("CARGO_PKG_VERSION")))
                        .size(12.0)
                        .color(crate::theme::TEXT_DIM),
                );
                ui.add_space(2.0);
                ui.label(
                    egui::RichText::new("Exact, robust 2D CAD")
                        .size(11.0)
                        .color(crate::theme::TEXT_DIM),
                );
                ui.add_space(14.0);
                if ui.button("Close").clicked() {
                    close = true;
                }
                ui.add_space(6.0);
            });
        });

    if close || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        ui_state.about_open = false;
    }
}

pub(super) fn font_combo(ui: &mut egui::Ui, salt: &str, font: &mut Option<String>) -> bool {
    let families = crate::fonts::system_families();
    let default_label = match crate::fonts::default_family_label() {
        Some(name) => format!("Default ({name})"),
        None => "Default".to_string(),
    };
    let label = font.clone().unwrap_or_else(|| default_label.clone());
    let mut changed = false;
    egui::ComboBox::from_id_salt(salt)
        .selected_text(label)
        .width(150.0)
        .show_ui(ui, |ui| {
            if ui.selectable_label(font.is_none(), &default_label).clicked() && font.is_some() {
                *font = None;
                changed = true;
            }
            for fam in &families {
                if ui
                    .selectable_label(font.as_deref() == Some(fam), fam)
                    .clicked()
                    && font.as_deref() != Some(fam)
                {
                    *font = Some(fam.clone());
                    changed = true;
                }
            }
        });
    changed
}
fn tool_hotkey(tool: &Tool) -> &'static str {
    match tool {
        Tool::Select => "SE",
        Tool::Line { .. } => "L",
        Tool::Polyline { .. } => "PL",
        Tool::Circle { .. } => "C",
        Tool::Ellipse { .. } => "EL",
        Tool::Arc3 { .. } => "A",
        Tool::Rectangle { .. } => "REC",
        Tool::Polygon { .. } => "POL",
        Tool::Spline { .. } => "SPL",
        Tool::Text { .. } => "T",
        Tool::Move { .. } => "M",
        Tool::Copy { .. } => "CO",
        Tool::Rotate { .. } => "RO",
        Tool::Scale { .. } => "SC",
        Tool::Mirror { .. } => "MI",
        Tool::Offset { .. } => "O",
        Tool::Trim => "TR",
        Tool::Extend => "EX",
        Tool::Fillet { .. } => "F",
        Tool::Chamfer { .. } => "CHA",
        Tool::Stretch { .. } => "S",
        Tool::Hatch => "H",
    }
}

fn tool_menu_item(ui: &mut egui::Ui, app: &mut AppState, label: &str, tool: Tool) {
    let hotkey = tool_hotkey(&tool);
    if ui
        .add(egui::Button::new(label).shortcut_text(hotkey))
        .clicked()
    {
        app.execute(Command::Activate(tool));
        ui.close();
    }
}

enum Act {
    Tool(Tool),
    Cmd(Command),
}

fn run_act(app: &mut AppState, act: &Act) {
    match act {
        Act::Tool(t) => app.execute(Command::Activate(t.clone())),
        Act::Cmd(c) => app.execute(c.clone()),
    }
}

fn draw_entries() -> Vec<(crate::icons::Icon, &'static str, Act)> {
    use crate::icons::Icon;
    vec![
        (Icon::Select, "Select  (SE)", Act::Tool(Tool::Select)),
        (
            Icon::Line,
            "Line  (L)",
            Act::Tool(Tool::Line { last: None }),
        ),
        (
            Icon::Polyline,
            "Polyline  (PL)",
            Act::Tool(Tool::Polyline { pts: vec![] }),
        ),
        (
            Icon::Circle,
            "Circle  (C)",
            Act::Tool(Tool::Circle { center: None }),
        ),
        (
            Icon::Ellipse,
            "Ellipse  (EL) — center, axis end, then minor axis",
            Act::Tool(Tool::Ellipse {
                center: None,
                axis_end: None,
            }),
        ),
        (
            Icon::Arc,
            "Arc — 3 points  (A)",
            Act::Tool(Tool::Arc3 { pts: vec![] }),
        ),
        (
            Icon::Rectangle,
            "Rectangle  (REC)",
            Act::Tool(Tool::Rectangle { first: None }),
        ),
        (
            Icon::Polygon,
            "Polygon  (POL)",
            Act::Tool(Tool::Polygon {
                center: None,
                sides: None,
            }),
        ),
        (
            Icon::Spline,
            "Spline  (SPL)",
            Act::Tool(Tool::Spline { pts: vec![] }),
        ),
        (
            Icon::Text,
            "Text  (T)",
            Act::Tool(Tool::Text {
                anchor: None,
                height: 2.5,
            }),
        ),
    ]
}

fn modify_entries() -> Vec<(crate::icons::Icon, &'static str, Act)> {
    use crate::icons::Icon;
    vec![
        (
            Icon::Move,
            "Move selection  (M)",
            Act::Tool(Tool::Move {
                base: None,
                ids: vec![],
            }),
        ),
        (
            Icon::Copy,
            "Copy selection  (CO)",
            Act::Tool(Tool::Copy {
                base: None,
                ids: vec![],
            }),
        ),
        (
            Icon::Rotate,
            "Rotate selection  (RO)",
            Act::Tool(Tool::Rotate {
                base: None,
                ids: vec![],
            }),
        ),
        (
            Icon::Scale,
            "Scale selection  (SC)",
            Act::Tool(Tool::Scale {
                base: None,
                reference: None,
                ids: vec![],
            }),
        ),
        (
            Icon::Mirror,
            "Mirror selection  (MI)",
            Act::Tool(Tool::Mirror {
                first: None,
                ids: vec![],
            }),
        ),
        (
            Icon::Offset,
            "Offset  (O) — type a distance, click curve, click side",
            Act::Tool(Tool::Offset {
                dist: 1.0,
                source: None,
            }),
        ),
        (
            Icon::Trim,
            "Trim  (TR) — click the piece to cut",
            Act::Tool(Tool::Trim),
        ),
        (
            Icon::Extend,
            "Extend  (EX) — click the end to lengthen",
            Act::Tool(Tool::Extend),
        ),
        (
            Icon::Fillet,
            "Fillet  (F) — type radius, pick 2 lines",
            Act::Tool(Tool::Fillet {
                radius: 1.0,
                first: None,
            }),
        ),
        (
            Icon::Chamfer,
            "Chamfer  (CHA) — type distance, pick 2 lines",
            Act::Tool(Tool::Chamfer {
                dist: 1.0,
                first: None,
            }),
        ),
        (
            Icon::Stretch,
            "Stretch  (S) — window, then base→destination",
            Act::Tool(Tool::Stretch {
                c1: None,
                c2: None,
                base: None,
                ids: vec![],
            }),
        ),
        (
            Icon::Explode,
            "Disjoint  (X) — break a polyline/polygon/rectangle into lines",
            Act::Cmd(Command::Explode),
        ),
        (
            Icon::Join,
            "Join  (J) — merge selected connected curves",
            Act::Cmd(Command::Join),
        ),
        (
            Icon::Hatch,
            "Hatch  (H) — fill selected boundaries, or click inside an area",
            Act::Cmd(Command::Hatch),
        ),
    ]
}

pub(super) fn ribbon(ctx: &Context, app: &mut AppState, canvas_rect: egui::Rect) {
    let entries = draw_entries();
    let avail = canvas_rect;
    // The dock floats, vertically centred, as a single column glass pill.
    let row_h = 47.0;
    let est_h = entries.len() as f32 * row_h + 24.0;
    let y = (avail.center().y - est_h / 2.0).max(avail.top() + 76.0);

    egui::Area::new(egui::Id::new("tool_ribbon"))
        .fixed_pos(egui::pos2(avail.left() + 12.0, y))
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            crate::theme::glass(crate::theme::tok::R_LG)
                .inner_margin(egui::Margin::same(6))
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(0.0, 4.0);
                    ui.vertical_centered(|ui| {
                        for (i, (icon, tip, act)) in entries.iter().enumerate() {
                            // Divider after Select.
                            if i == 1 {
                                dock_divider(ui);
                            }
                            let active = matches!(act, Act::Tool(t) if app.tool.name() == t.name());
                            if crate::icons::icon_button_sized(ui, *icon, tip, active, 38.0)
                                .clicked()
                            {
                                run_act(app, act);
                            }
                        }
                    });
                });
        });
}

fn dock_divider(ui: &mut egui::Ui) {
    ui.add_space(2.0);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(26.0, 1.0), egui::Sense::hover());
    ui.painter().hline(
        rect.x_range(),
        rect.center().y,
        egui::Stroke::new(1.0, crate::theme::OUTLINE),
    );
    ui.add_space(2.0);
}

/// Bottom-centre floating "glass" status pill. A single row: modify tools,
/// live coordinates, a units dropdown, snap chips (with a stay-open object-snap
/// popup), and zoom controls.
pub(super) fn status_pill(ctx: &Context, app: &mut AppState, canvas_rect: egui::Rect) {
    egui::Area::new(egui::Id::new("status_pill"))
        .anchor(
            egui::Align2::CENTER_BOTTOM,
            egui::vec2(
                0.0,
                -(canvas_rect.bottom() - ctx.content_rect().bottom()) - 16.0,
            ),
        )
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            crate::theme::glass(crate::theme::tok::R_MD)
                .inner_margin(egui::Margin::symmetric(10, 6))
                .show(ui, |ui| {
                    ui.horizontal_centered(|ui| {
                        // ── Modify tools, same size as the left toolbar tools.
                        ui.scope(|ui| {
                            ui.spacing_mut().item_spacing = egui::vec2(2.0, 0.0);
                            for (icon, tip, act) in modify_entries() {
                                let active =
                                    matches!(&act, Act::Tool(t) if app.tool.name() == t.name());
                                if crate::icons::icon_button_sized(ui, icon, tip, active, 38.0)
                                    .clicked()
                                {
                                    run_act(app, &act);
                                }
                            }
                        });
                        pill_sep(ui);

                        // ── Live cursor coordinates: "X  14.20    Y  248.75   mm".
                        // Values sit in fixed-width left-aligned cells so the gaps
                        // stay constant regardless of the number's length.
                        ui.scope(|ui| {
                            ui.spacing_mut().item_spacing.x = 0.0;
                            let (cx, cy) = app.cursor_world;
                            let cell = |ui: &mut egui::Ui, text: String| {
                                let (rect, _) = ui.allocate_exact_size(
                                    egui::vec2(56.0, 18.0),
                                    egui::Sense::hover(),
                                );
                                ui.painter().text(
                                    egui::pos2(rect.left(), rect.center().y),
                                    egui::Align2::LEFT_CENTER,
                                    text,
                                    egui::FontId::monospace(12.5),
                                    crate::theme::ACCENT_BRIGHT,
                                );
                            };
                            ui.label(
                                egui::RichText::new("X")
                                    .size(11.0)
                                    .color(crate::theme::TEXT_DIM),
                            );
                            ui.add_space(8.0);
                            cell(ui, format!("{cx:.2}"));
                            ui.add_space(6.0);
                            ui.label(
                                egui::RichText::new("Y")
                                    .size(11.0)
                                    .color(crate::theme::TEXT_DIM),
                            );
                            ui.add_space(8.0);
                            cell(ui, format!("{cy:.2}"));
                            ui.add_space(6.0);
                            ui.label(
                                egui::RichText::new(app.units_label())
                                    .size(11.0)
                                    .color(crate::theme::TEXT_DIM),
                            );
                        });
                        pill_sep(ui);

                        // ── SNAP master toggle + arrow popup, then the quick chips.
                        snap_master(ui, app);
                        ui.add_space(6.0);
                        snap_chip(ui, &mut app.grid_on, "Grid");
                        let mut polar = app.polar_on;
                        if snap_chip(ui, &mut polar, "Guides") {
                            app.polar_on = polar;
                            if app.polar_on {
                                app.ortho_on = false;
                            }
                        }
                        snap_chip(ui, &mut app.grid_snap_on, "GSnap");
                        snap_chip(ui, &mut app.dyn_on, "Dyn");
                        pill_sep(ui);

                        // ── Zoom: − / 100% / + (plus zoom-extents).
                        let (wx, wy) = app
                            .view
                            .screen_to_world(app.view.width / 2.0, app.view.height / 2.0);
                        if round_btn(ui, "−", "Zoom out") {
                            app.view.zoom_at(wx, wy, 0.8);
                        }
                        ui.label(
                            egui::RichText::new(format!("{:>3.0}%", app.view.zoom_percent()))
                                .monospace()
                                .color(crate::theme::ACCENT_BRIGHT),
                        );
                        if round_btn(ui, "+", "Zoom in") {
                            app.view.zoom_at(wx, wy, 1.25);
                        }
                        ui.add_space(2.0);
                        if crate::icons::icon_button_sized(
                            ui,
                            crate::icons::Icon::ZoomFit,
                            "Zoom extents — fit the whole drawing",
                            false,
                            40.0,
                        )
                        .clicked()
                        {
                            app.zoom_extents();
                        }
                        pill_sep(ui);

                        // ── Units dropdown at the far right (as in the reference).
                        unit_dropdown(ui, app);
                    });
                });
        });
}

/// Units selector: a compact "mm" button with a small chevron that flips up when
/// the menu is open (replaces the old top "Units" menu).
fn unit_dropdown(ui: &mut egui::Ui, app: &mut AppState) {
    use eiderflat_document::Units;
    let open_id = egui::Id::new("unit_menu_open");
    let mut open = ui
        .ctx()
        .data(|d| d.get_temp::<bool>(open_id).unwrap_or(false));

    let label = app.units_label();
    let galley = ui.painter().layout_no_wrap(
        label.to_string(),
        egui::FontId::proportional(12.0),
        crate::theme::TEXT,
    );
    let w = galley.size().x + 28.0;
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, 28.0), egui::Sense::click());
    let fill = if open || resp.hovered() {
        crate::theme::WIDGET_HOVER
    } else {
        crate::theme::WIDGET_BG
    };
    let p = ui.painter();
    p.rect(
        rect,
        8.0,
        fill,
        egui::Stroke::new(1.0, crate::theme::OUTLINE),
        egui::StrokeKind::Inside,
    );
    p.text(
        egui::pos2(rect.left() + 9.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        egui::FontId::proportional(12.0),
        crate::theme::TEXT,
    );
    // Small chevron (▲ when open, ▼ when closed), example-dim colour.
    let cc = egui::pos2(rect.right() - 11.0, rect.center().y);
    let (dx, dy) = (3.2, 2.2);
    let chev = if open {
        [
            egui::pos2(cc.x - dx, cc.y + dy * 0.6),
            egui::pos2(cc.x, cc.y - dy * 0.9),
            egui::pos2(cc.x + dx, cc.y + dy * 0.6),
        ]
    } else {
        [
            egui::pos2(cc.x - dx, cc.y - dy * 0.6),
            egui::pos2(cc.x, cc.y + dy * 0.9),
            egui::pos2(cc.x + dx, cc.y - dy * 0.6),
        ]
    };
    p.add(egui::Shape::line(
        chev.to_vec(),
        egui::Stroke::new(1.3, crate::theme::TEXT_DIM),
    ));
    if resp.clicked() {
        open = !open;
    }

    if open {
        let popup = egui::Area::new(egui::Id::new("unit_menu_popup"))
            .order(egui::Order::Foreground)
            .fixed_pos(rect.center_top() - egui::vec2(0.0, 8.0))
            .pivot(egui::Align2::CENTER_BOTTOM)
            .show(ui.ctx(), |ui| {
                crate::theme::glass(crate::theme::tok::R_MD)
                    .inner_margin(egui::Margin::same(6))
                    .show(ui, |ui| {
                        ui.set_min_width(150.0);
                        for (name, units) in [
                            ("Millimeters (mm)", Units::Millimeters),
                            ("Centimeters (cm)", Units::Centimeters),
                            ("Meters (m)", Units::Meters),
                            ("Kilometers (km)", Units::Kilometers),
                            ("Inches (in)", Units::Inches),
                            ("Feet (ft)", Units::Feet),
                            ("Unitless", Units::Unitless),
                        ] {
                            let selected = app.document.settings.units == units;
                            if ui.selectable_label(selected, name).clicked() {
                                app.document.settings.units = units;
                                app.sync_zoom_limits();
                                open = false;
                            }
                        }
                    });
            });
        if popup.response.clicked_elsewhere() && !resp.hovered() {
            open = false;
        }
    }

    ui.ctx().data_mut(|d| d.insert_temp(open_id, open));
}

/// Small round −/+ button for the zoom control. Returns true when clicked.
fn round_btn(ui: &mut egui::Ui, glyph: &str, tip: &str) -> bool {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(28.0, 28.0), egui::Sense::click());
    if resp.hovered() {
        ui.painter()
            .rect_filled(rect, 8.0, crate::theme::WIDGET_HOVER);
    }
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        glyph,
        egui::FontId::proportional(17.0),
        crate::theme::TEXT,
    );
    resp.on_hover_text(tip).clicked()
}

/// Combined "SNAP | ▲" control: clicking the SNAP half toggles object snap on/off,
/// clicking the arrow half opens the object-snap kinds popup (which stays open
/// while toggling and reflects the current state).
fn snap_master(ui: &mut egui::Ui, app: &mut AppState) {
    let open_id = egui::Id::new("snap_kinds_open");
    let mut open = ui
        .ctx()
        .data(|d| d.get_temp::<bool>(open_id).unwrap_or(false));

    let h = 26.0;
    let saved_sp = ui.spacing().item_spacing.x;
    ui.spacing_mut().item_spacing.x = 0.0;
    let (srect, sresp) = ui.allocate_exact_size(egui::vec2(48.0, h), egui::Sense::click());
    let (arect, aresp) = ui.allocate_exact_size(egui::vec2(24.0, h), egui::Sense::click());
    ui.spacing_mut().item_spacing.x = saved_sp;
    let on = app.snap_on;

    // Shared pill background.
    let union = srect.union(arect);
    let p = ui.painter();
    p.rect(
        union,
        8.0,
        crate::theme::WIDGET_BG,
        egui::Stroke::new(1.0, crate::theme::OUTLINE),
        egui::StrokeKind::Inside,
    );
    // SNAP half highlight when active or hovered.
    if on {
        p.rect_filled(srect.shrink(1.0), 7.0, crate::theme::ACCENT_DIM);
    } else if sresp.hovered() {
        p.rect_filled(srect.shrink(1.0), 7.0, crate::theme::WIDGET_HOVER);
    }
    p.text(
        srect.center(),
        egui::Align2::CENTER_CENTER,
        "SNAP",
        egui::FontId::proportional(11.0),
        if on {
            crate::theme::ACCENT_BRIGHT
        } else {
            crate::theme::TEXT_DIM
        },
    );
    // Divider bar between the two halves.
    p.vline(
        srect.right(),
        (union.top() + 5.0)..=(union.bottom() - 5.0),
        egui::Stroke::new(1.0, crate::theme::OUTLINE),
    );
    // Arrow half.
    if open || aresp.hovered() {
        p.rect_filled(arect.shrink(1.0), 7.0, crate::theme::WIDGET_HOVER);
    }
    p.text(
        arect.center(),
        egui::Align2::CENTER_CENTER,
        "▲",
        egui::FontId::proportional(10.0),
        if open {
            crate::theme::ACCENT_BRIGHT
        } else {
            crate::theme::TEXT_DIM
        },
    );

    if sresp.clicked() {
        app.snap_on = !app.snap_on;
    }
    if aresp.clicked() {
        open = !open;
    }
    let trigger_hovered = sresp.hovered() || aresp.hovered();

    if open {
        let kinds = [
            (eiderflat_cad::SnapKind::Endpoint, "Endpoint"),
            (eiderflat_cad::SnapKind::Midpoint, "Midpoint"),
            (eiderflat_cad::SnapKind::Center, "Center"),
            (eiderflat_cad::SnapKind::Quadrant, "Quadrant"),
            (eiderflat_cad::SnapKind::Intersection, "Intersection"),
            (eiderflat_cad::SnapKind::Perpendicular, "Perpendicular"),
            (eiderflat_cad::SnapKind::Tangent, "Tangent"),
            (eiderflat_cad::SnapKind::Nearest, "Nearest"),
            (eiderflat_cad::SnapKind::Node, "Node"),
            (eiderflat_cad::SnapKind::Insertion, "Insertion"),
        ];
        let popup = egui::Area::new(egui::Id::new("snap_kinds_popup_area"))
            .order(egui::Order::Foreground)
            .fixed_pos(union.left_top() - egui::vec2(0.0, 8.0))
            .pivot(egui::Align2::LEFT_BOTTOM)
            .show(ui.ctx(), |ui| {
                crate::theme::glass(crate::theme::tok::R_MD)
                    .inner_margin(egui::Margin::symmetric(12, 10))
                    .show(ui, |ui| {
                        ui.set_min_width(168.0);
                        ui.label(
                            egui::RichText::new("OBJECT SNAP")
                                .size(10.0)
                                .color(crate::theme::TEXT_DIM)
                                .strong(),
                        );
                        ui.add_space(4.0);
                        for (kind, label) in kinds {
                            let mut enabled = app.snap.enabled.contains(&kind);
                            if ui.checkbox(&mut enabled, label).changed() {
                                if enabled {
                                    if !app.snap.enabled.contains(&kind) {
                                        app.snap.enabled.push(kind);
                                    }
                                } else {
                                    app.snap.enabled.retain(|&k| k != kind);
                                }
                            }
                        }
                    });
            });
        // Close only when clicking outside the popup (and not on the trigger).
        if popup.response.clicked_elsewhere() && !trigger_hovered {
            open = false;
        }
    }

    ui.ctx().data_mut(|d| d.insert_temp(open_id, open));
}

fn pill_sep(ui: &mut egui::Ui) {
    ui.add_space(3.0);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(1.0, 20.0), egui::Sense::hover());
    ui.painter().vline(
        rect.center().x,
        rect.y_range(),
        egui::Stroke::new(1.0, crate::theme::OUTLINE),
    );
    ui.add_space(3.0);
}

/// A small pill toggle chip (mirrors the mockup's SNAP chips). Returns true if toggled.
fn snap_chip(ui: &mut egui::Ui, on: &mut bool, label: &str) -> bool {
    let galley = ui.painter().layout_no_wrap(
        label.to_string(),
        egui::FontId::proportional(11.5),
        crate::theme::TEXT,
    );
    let w = galley.size().x + 18.0;
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, 26.0), egui::Sense::click());
    let (fill, stroke, fg) = if *on {
        // Match the active SNAP master: borderless ACCENT_DIM fill, bright text.
        (
            crate::theme::ACCENT_DIM,
            egui::Stroke::NONE,
            crate::theme::ACCENT_BRIGHT,
        )
    } else if resp.hovered() {
        (
            crate::theme::WIDGET_HOVER,
            egui::Stroke::new(1.0, crate::theme::OUTLINE),
            crate::theme::TEXT,
        )
    } else {
        (
            crate::theme::WIDGET_BG,
            egui::Stroke::new(1.0, crate::theme::OUTLINE),
            crate::theme::TEXT_DIM,
        )
    };
    let p = ui.painter();
    p.rect(rect, 9.0, fill, stroke, egui::StrokeKind::Inside);
    p.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        egui::FontId::proportional(11.5),
        fg,
    );
    if resp.clicked() {
        *on = !*on;
        true
    } else {
        false
    }
}

/// Single combined right "glass" inspector: Properties at the top, Layers below.
pub(super) fn inspector(ctx: &Context, app: &mut AppState, canvas_rect: egui::Rect) {
    const RIGHT_M: f32 = 12.0;
    const WIDTH: f32 = 292.0;
    let screen = ctx.content_rect();
    let top_off = (canvas_rect.top() - screen.top()) + 76.0;
    // Fill from just under the top bar down to just above the status pill.
    let avail_h = (canvas_rect.height() - 76.0 - 80.0).max(160.0);

    egui::Area::new(egui::Id::new("inspector"))
        .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-RIGHT_M, top_off))
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            ui.set_width(WIDTH);
            crate::theme::glass(crate::theme::tok::R_LG)
                .inner_margin(egui::Margin::same(0))
                .show(ui, |ui| {
                    ui.set_width(WIDTH);
                    // Force the panel to span the full available height; the scroll
                    // area inside takes the remaining space and scrolls when the
                    // content (or a short window) doesn't fit.
                    ui.set_height(avail_h);
                    egui::Frame::new()
                        .inner_margin(egui::Margin {
                            left: 20,
                            right: 14,
                            top: 12,
                            bottom: 12,
                        })
                        .show(ui, |ui| inspector_header(ui, app));
                    divider_h(ui);
                    let remaining = ui.available_height();
                    egui::ScrollArea::vertical()
                        .max_height(remaining)
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            egui::Frame::new()
                                .inner_margin(egui::Margin {
                                    left: 20,
                                    right: 14,
                                    top: 10,
                                    bottom: 10,
                                })
                                .show(ui, |ui| {
                                    ui.set_width(WIDTH - 34.0);
                                    selection_properties(ui, app);
                                    ui.add_space(12.0);
                                    layers_section(ui, app);
                                });
                        });
                });
        });
}

fn inspector_header(ui: &mut egui::Ui, app: &AppState) {
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("PROPERTIES")
                .size(11.0)
                .color(crate::theme::TEXT_DIM)
                .strong(),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let n = app.selection.len();
            let txt = if n == 1 {
                "1 selected".to_string()
            } else {
                format!("{n} selected")
            };
            let galley = ui.painter().layout_no_wrap(
                txt.clone(),
                egui::FontId::monospace(11.0),
                crate::theme::ACCENT_BRIGHT,
            );
            let (rect, _) = ui.allocate_exact_size(
                egui::vec2(galley.size().x + 14.0, 20.0),
                egui::Sense::hover(),
            );
            ui.painter().rect(
                rect,
                6.0,
                crate::theme::ACCENT_DIM,
                egui::Stroke::new(1.0, crate::theme::ACCENT),
                egui::StrokeKind::Inside,
            );
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                txt,
                egui::FontId::monospace(11.0),
                crate::theme::ACCENT_BRIGHT,
            );
        });
    });
}

pub(super) fn divider_h(ui: &mut egui::Ui) {
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 1.0), egui::Sense::hover());
    ui.painter().hline(
        rect.x_range(),
        rect.center().y,
        egui::Stroke::new(1.0, crate::theme::OUTLINE),
    );
}

fn layers_section(ui: &mut egui::Ui, app: &mut AppState) {
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("LAYERS")
                .size(10.0)
                .color(crate::theme::TEXT_DIM)
                .strong(),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if crate::icons::icon_button_sized(
                ui,
                crate::icons::Icon::AddLayer,
                "New Layer",
                false,
                38.0,
            )
            .clicked()
            {
                let n = app.document.layers.layers.len();
                app.document.layers.add(Layer::new(format!("Layer{}", n)));
            }
        });
    });
    ui.add_space(4.0);

    let current = app.document.layers.current;
    let n_layers = app.document.layers.layers.len();
    let mut counts = vec![0usize; n_layers];
    for e in app.document.iter() {
        if e.layer < n_layers {
            counts[e.layer] += 1;
        }
    }
    let rows: Vec<(usize, String, [u8; 3], bool, usize)> = app
        .document
        .layers
        .layers
        .iter()
        .enumerate()
        .map(|(i, l)| {
            (
                i,
                l.name.clone(),
                [l.color.0, l.color.1, l.color.2],
                l.on,
                counts[i],
            )
        })
        .collect();

    let mut delete_layer: Option<usize> = None;
    for (i, name, rgb, on, count) in rows {
        let is_cur = i == current;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 7.0;
            ui.set_height(38.0);

            // Current-layer indicator bar (click to set current).
            let (dr, dresp) = ui.allocate_exact_size(egui::vec2(5.0, 18.0), egui::Sense::click());
            let bar = egui::Rect::from_center_size(dr.center(), egui::vec2(3.0, 16.0));
            let col = if is_cur {
                crate::theme::ACCENT
            } else if dresp.hovered() {
                crate::theme::TEXT_DIM
            } else {
                crate::theme::OUTLINE
            };
            ui.painter().rect_filled(bar, 2.0, col);
            if dresp
                .on_hover_text("Set as the current drawing layer")
                .clicked()
            {
                app.document.layers.current = i;
            }

            // Small colour swatch (opens the colour picker).
            let mut c = rgb;
            let changed = ui
                .scope(|ui| {
                    ui.spacing_mut().interact_size = egui::vec2(14.0, 14.0);
                    ui.color_edit_button_srgb(&mut c).changed()
                })
                .inner;
            if changed && let Some(l) = app.document.layers.get_mut(i) {
                l.color = (c[0], c[1], c[2]);
            }

            // Right cluster: count · eye · trash.
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                use crate::icons::{Icon, icon_button_sized};
                let deletable = i != 0 && i != current;
                ui.add_enabled_ui(deletable, |ui| {
                    let tip = if deletable {
                        "Delete this layer (its objects move to layer 0)"
                    } else {
                        "Layer 0 and the current layer can't be deleted"
                    };
                    if icon_button_sized(ui, Icon::Delete, tip, false, 36.0).clicked() {
                        delete_layer = Some(i);
                    }
                });
                let icon = if on { Icon::Eye } else { Icon::EyeOff };
                if icon_button_sized(ui, icon, "Show / hide this layer", false, 36.0).clicked()
                    && let Some(l) = app.document.layers.get_mut(i)
                {
                    l.on = !on;
                }
                ui.add_space(10.0);
                ui.label(
                    egui::RichText::new(format!("{count:>2}"))
                        .monospace()
                        .size(11.0)
                        .color(crate::theme::TEXT_DIM),
                );
                ui.add_space(4.0);
                // Editable layer name fills the gap between swatch and the cluster.
                let mut buf = name.clone();
                let name_col = if is_cur {
                    crate::theme::TEXT
                } else {
                    crate::theme::TEXT_DIM
                };
                let resp = ui.add_sized(
                    [ui.available_width(), 22.0],
                    egui::TextEdit::singleline(&mut buf)
                        .frame(egui::Frame::NONE)
                        .text_color(name_col)
                        .font(egui::TextStyle::Monospace),
                );
                if resp.changed()
                    && let Some(l) = app.document.layers.get_mut(i)
                {
                    l.name = buf;
                }
            });
        });
    }
    if let Some(idx) = delete_layer
        && idx != 0
        && idx != app.document.layers.current
    {
        let lname = app.document.layers.layers[idx].name.clone();
        app.history.snapshot(&app.document);
        let ids: Vec<_> = app.document.iter().map(|e| e.id).collect();
        for id in ids {
            if let Some(e) = app.document.get_mut(id) {
                if e.layer == idx {
                    e.layer = 0;
                } else if e.layer > idx {
                    e.layer -= 1;
                }
            }
        }
        let _ = app.document.layers.delete(&lname);
    }
}

/// Floating contextual toolbar shown just above a single selected entity.
pub(super) fn contextual_toolbar(ctx: &Context, app: &mut AppState, canvas_rect: egui::Rect) {
    if !matches!(app.tool, Tool::Select) || app.selection.len() != 1 {
        return;
    }
    let id = app.selection[0];
    let Some(bbox) = app.document.get(id).and_then(|e| e.bounding_box()) else {
        return;
    };
    // Anchor above the bbox top-centre, in screen space.
    let cxw = (bbox.min.x + bbox.max.x) * 0.5;
    let topw = bbox.max.y;
    let (sx, sy) = app.view.world_to_screen(cxw, topw);
    let anchor = canvas_rect.min + egui::vec2(sx as f32, sy as f32) - egui::vec2(0.0, 50.0);
    // Keep it inside the canvas.
    let anchor = egui::pos2(
        anchor
            .x
            .clamp(canvas_rect.left() + 90.0, canvas_rect.right() - 200.0),
        anchor
            .y
            .clamp(canvas_rect.top() + 70.0, canvas_rect.bottom() - 60.0),
    );

    egui::Area::new(egui::Id::new("contextual_toolbar"))
        .fixed_pos(anchor)
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            crate::theme::glass(crate::theme::tok::R_MD)
                .inner_margin(egui::Margin::same(5))
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(2.0, 0.0);
                    ui.horizontal(|ui| {
                        use crate::icons::{Icon, icon_button_sized};
                        if icon_button_sized(ui, Icon::Copy, "Duplicate  (CO)", false, 38.0)
                            .clicked()
                        {
                            app.execute(Command::Activate(Tool::Copy {
                                base: None,
                                ids: vec![],
                            }));
                        }
                        if icon_button_sized(ui, Icon::Mirror, "Mirror  (MI)", false, 38.0)
                            .clicked()
                        {
                            app.execute(Command::Activate(Tool::Mirror {
                                first: None,
                                ids: vec![],
                            }));
                        }
                        if icon_button_sized(ui, Icon::Rotate, "Rotate  (RO)", false, 38.0)
                            .clicked()
                        {
                            app.execute(Command::Activate(Tool::Rotate {
                                base: None,
                                ids: vec![],
                            }));
                        }
                        if icon_button_sized(ui, Icon::Offset, "Offset  (O)", false, 38.0).clicked()
                        {
                            app.execute(Command::Activate(Tool::Offset {
                                dist: 1.0,
                                source: None,
                            }));
                        }
                        pill_sep(ui);
                        if icon_button_sized(ui, Icon::Delete, "Delete  (Del)", false, 38.0)
                            .clicked()
                        {
                            app.erase_selection();
                        }
                    });
                });
        });
}

fn prop_section(ui: &mut egui::Ui, title: &str) {
    ui.add_space(15.0);
    ui.add(egui::Label::new(
        egui::RichText::new(title)
            .size(10.0)
            .color(crate::theme::TEXT_DIM)
            .strong(),
    ));
    ui.add_space(7.0);
}

fn prop_caption(ui: &mut egui::Ui, text: &str) {
    ui.add(
        egui::Label::new(
            egui::RichText::new(text)
                .size(10.0)
                .color(crate::theme::TEXT_DIM),
        )
        .truncate(),
    );
}

/// Apply the rounded "value box" widget styling used across the inspector fields.
fn style_value_box(ui: &mut egui::Ui) {
    let r = egui::CornerRadius::same(9);
    let v = ui.visuals_mut();
    v.widgets.inactive.bg_fill = crate::theme::WIDGET_BG;
    v.widgets.inactive.weak_bg_fill = crate::theme::WIDGET_BG;
    v.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, crate::theme::OUTLINE);
    v.widgets.inactive.corner_radius = r;
    v.widgets.hovered.bg_fill = crate::theme::WIDGET_HOVER;
    v.widgets.hovered.weak_bg_fill = crate::theme::WIDGET_HOVER;
    v.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, crate::theme::ACCENT_DIM);
    v.widgets.hovered.corner_radius = r;
    v.widgets.active.bg_fill = crate::theme::WIDGET_HOVER;
    v.widgets.active.weak_bg_fill = crate::theme::WIDGET_HOVER;
    v.widgets.active.bg_stroke = egui::Stroke::new(1.0, crate::theme::ACCENT);
    v.widgets.active.corner_radius = r;
}

fn num_field(ui: &mut egui::Ui, caption: &str, v: &mut f64, speed: f64) -> bool {
    ui.scope(|ui| {
        prop_caption(ui, caption);
        ui.add_space(4.0);
        style_value_box(ui);
        ui.add_sized(
            [ui.available_width(), 30.0],
            egui::DragValue::new(v).speed(speed).max_decimals(4),
        )
        .changed()
    })
    .inner
}

fn xy_fields(ui: &mut egui::Ui, ca: &str, a: &mut f64, cb: &str, b: &mut f64, speed: f64) -> bool {
    let mut changed = false;
    ui.columns(2, |c| {
        changed |= num_field(&mut c[0], ca, a, speed);
        changed |= num_field(&mut c[1], cb, b, speed);
    });
    changed
}

fn metric_field(ui: &mut egui::Ui, caption: &str, value: f64) {
    ui.scope(|ui| {
        prop_caption(ui, caption);
        ui.add_space(4.0);
        style_value_box(ui);
        let mut v = value;
        ui.add_enabled_ui(false, |ui| {
            ui.add_sized(
                [ui.available_width(), 30.0],
                egui::DragValue::new(&mut v).max_decimals(4),
            );
        });
    });
}

fn kind_label(kind: &EntityKind) -> &'static str {
    match kind {
        EntityKind::Curve(Curve::Line(_)) => "Line",
        EntityKind::Curve(Curve::Arc(a)) => {
            let span = (a.end_angle - a.start_angle).abs();
            if (span - std::f64::consts::TAU).abs() < 1e-9 {
                "Circle"
            } else {
                "Arc"
            }
        }
        EntityKind::Curve(Curve::Ellipse(_)) => "Ellipse",
        EntityKind::Curve(Curve::Bezier(_)) => "Bézier",
        EntityKind::Curve(Curve::Poly(_)) => "Polyline",
        EntityKind::Curve(Curve::Rational(_)) | EntityKind::Curve(Curve::Nurbs(_)) => "Spline",
        EntityKind::Point(_) => "Point",
        EntityKind::Text { .. } => "Text",
        EntityKind::XLine { .. } => "Construction line",
        EntityKind::Ray { .. } => "Ray",
        EntityKind::Insert { .. } => "Block insert",
        EntityKind::Hatch { .. } => "Hatch",
    }
}

fn kind_icon(kind: &EntityKind) -> crate::icons::Icon {
    use crate::icons::Icon;
    match kind {
        EntityKind::Curve(Curve::Line(_)) | EntityKind::XLine { .. } | EntityKind::Ray { .. } => {
            Icon::Line
        }
        EntityKind::Curve(Curve::Arc(a)) => {
            let span = (a.end_angle - a.start_angle).abs();
            if (span - std::f64::consts::TAU).abs() < 1e-9 {
                Icon::Circle
            } else {
                Icon::Arc
            }
        }
        EntityKind::Curve(Curve::Ellipse(_)) => Icon::Ellipse,
        EntityKind::Curve(Curve::Poly(_)) => Icon::Polyline,
        EntityKind::Curve(Curve::Bezier(_))
        | EntityKind::Curve(Curve::Rational(_))
        | EntityKind::Curve(Curve::Nurbs(_)) => Icon::Spline,
        EntityKind::Text { .. } => Icon::Text,
        EntityKind::Hatch { .. } => Icon::Hatch,
        _ => Icon::Select,
    }
}

/// Selected-object header: an icon chip, the type name, and a subtitle.
fn object_header(ui: &mut egui::Ui, name: &str, subtitle: &str, icon: crate::icons::Icon) {
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(38.0, 38.0), egui::Sense::hover());
        ui.painter().rect(
            rect,
            10.0,
            crate::theme::ACCENT_DIM,
            egui::Stroke::new(1.0, crate::theme::ACCENT),
            egui::StrokeKind::Inside,
        );
        crate::icons::paint_icon(
            &ui.painter_at(rect),
            ui.ctx(),
            icon,
            rect.shrink(10.0),
            crate::theme::ACCENT_BRIGHT,
        );
        ui.add_space(4.0);
        ui.vertical(|ui| {
            ui.add_space(2.0);
            ui.label(
                egui::RichText::new(name)
                    .size(14.0)
                    .strong()
                    .color(crate::theme::TEXT),
            );
            ui.label(
                egui::RichText::new(subtitle)
                    .size(11.5)
                    .monospace()
                    .color(crate::theme::TEXT_DIM),
            );
        });
    });
}

fn measurements(ui: &mut egui::Ui, kind: &EntityKind) {
    use eiderflat_geometry::CurveSegment;
    let EntityKind::Curve(c) = kind else {
        return;
    };
    prop_section(ui, "MEASUREMENTS");
    match c {
        Curve::Line(l) => {
            let (dx, dy) = (l.p1.x - l.p0.x, l.p1.y - l.p0.y);
            let len = (dx * dx + dy * dy).sqrt();
            let ang = dy.atan2(dx).to_degrees();
            ui.columns(2, |c| {
                metric_field(&mut c[0], "Length", len);
                metric_field(&mut c[1], "Angle °", ang);
            });
        }
        Curve::Arc(a) => {
            let span = (a.end_angle - a.start_angle).abs();
            let is_circle = (span - std::f64::consts::TAU).abs() < 1e-9;
            if is_circle {
                ui.columns(2, |c| {
                    metric_field(&mut c[0], "Circumference", std::f64::consts::TAU * a.radius);
                    metric_field(
                        &mut c[1],
                        "Area",
                        std::f64::consts::PI * a.radius * a.radius,
                    );
                });
            } else {
                ui.columns(2, |c| {
                    metric_field(&mut c[0], "Arc length", a.radius * span);
                    metric_field(&mut c[1], "Sweep °", span.to_degrees());
                });
            }
        }
        other => metric_field(ui, "Length", other.arc_length()),
    }
}

/// A full-width rounded Appearance row: a left label and a right value that is a
/// borderless dropdown (optionally with a leading swatch or a line sample).
fn appearance_row(
    ui: &mut egui::Ui,
    label: &str,
    value: String,
    swatch: Option<egui::Color32>,
    line_sample: bool,
    add_options: impl FnOnce(&mut egui::Ui),
) {
    let id = ui.make_persistent_id(("appearance_row", label));
    // Whole row is the control: paint the value (no inner button) and make the
    // entire frame clickable so a click anywhere opens the dropdown.
    let inner = egui::Frame::new()
        .fill(crate::theme::WIDGET_BG)
        .stroke(egui::Stroke::new(1.0, crate::theme::OUTLINE))
        .corner_radius(egui::CornerRadius::same(9))
        .inner_margin(egui::Margin::symmetric(11, 5))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.set_height(22.0);
                ui.label(
                    egui::RichText::new(label)
                        .size(12.5)
                        .color(crate::theme::TEXT_DIM),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Trailing chevron marks the row as a dropdown.
                    let (cr, _) =
                        ui.allocate_exact_size(egui::vec2(12.0, 22.0), egui::Sense::hover());
                    let cc = cr.center();
                    let (dx, dy) = (3.0, 2.0);
                    ui.painter().add(egui::Shape::line(
                        vec![
                            egui::pos2(cc.x - dx, cc.y - dy * 0.6),
                            egui::pos2(cc.x, cc.y + dy * 0.9),
                            egui::pos2(cc.x + dx, cc.y - dy * 0.6),
                        ],
                        egui::Stroke::new(1.3, crate::theme::TEXT_DIM),
                    ));
                    ui.add_space(2.0);
                    ui.label(
                        egui::RichText::new(value)
                            .size(12.5)
                            .color(crate::theme::TEXT),
                    );
                    if let Some(c) = swatch {
                        let (r, _) =
                            ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
                        ui.painter().rect_filled(r, 3.0, c);
                    }
                    if line_sample {
                        let (r, _) =
                            ui.allocate_exact_size(egui::vec2(34.0, 12.0), egui::Sense::hover());
                        ui.painter().hline(
                            r.x_range(),
                            r.center().y,
                            egui::Stroke::new(1.6, crate::theme::TEXT),
                        );
                    }
                });
            });
        });

    let rect = inner.response.rect;
    let resp = ui.interact(rect, id, egui::Sense::click());
    if resp.hovered() {
        // Brighten the whole row outline to read as a single hoverable control.
        ui.painter().rect_stroke(
            rect,
            egui::CornerRadius::same(9),
            egui::Stroke::new(1.0, crate::theme::ACCENT_DIM),
            egui::StrokeKind::Inside,
        );
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    egui::Popup::menu(&resp).show(add_options);
    ui.add_space(8.0);
}

fn appearance_section(ui: &mut egui::Ui, app: &mut AppState, sel: &[eiderflat_document::EntityId]) {
    use eiderflat_document::{LineTypeRef, LineWeight};
    prop_section(ui, "APPEARANCE");

    // ── Line weight ───────────────────────────────────────────────────────
    let first_lw = sel
        .first()
        .and_then(|&id| app.document.get(id))
        .map(|e| e.line_weight.clone());
    let lw_label = match &first_lw {
        Some(LineWeight::ByBlock) => "By block".to_string(),
        Some(LineWeight::Hundredths(h)) => format!("{:.2} mm", *h as f64 / 100.0),
        _ => "By layer".to_string(),
    };
    appearance_row(ui, "Line weight", lw_label, None, false, |ui| {
        for (lbl, val) in [
            ("By layer", LineWeight::ByLayer),
            ("0.13 mm", LineWeight::Hundredths(13)),
            ("0.25 mm", LineWeight::Hundredths(25)),
            ("0.35 mm", LineWeight::Hundredths(35)),
            ("0.50 mm", LineWeight::Hundredths(50)),
            ("0.70 mm", LineWeight::Hundredths(70)),
            ("1.00 mm", LineWeight::Hundredths(100)),
        ] {
            if ui
                .selectable_label(first_lw.as_ref() == Some(&val), lbl)
                .clicked()
            {
                app.history.snapshot(&app.document);
                for &id in sel {
                    if let Some(e) = app.document.get_mut(id) {
                        e.line_weight = val.clone();
                    }
                }
                ui.close();
            }
        }
    });

    // ── Line type ─────────────────────────────────────────────────────────
    let first_lt = sel
        .first()
        .and_then(|&id| app.document.get(id))
        .map(|e| e.line_type.clone());
    let lt_label = match &first_lt {
        Some(LineTypeRef::ByBlock) => "By block".to_string(),
        Some(LineTypeRef::Named(n)) if n == "Continuous" => "Solid".to_string(),
        Some(LineTypeRef::Named(n)) => n.clone(),
        _ => "By layer".to_string(),
    };
    appearance_row(ui, "Line type", lt_label, None, true, |ui| {
        for (lbl, val) in [
            ("By layer", LineTypeRef::ByLayer),
            ("Solid", LineTypeRef::Named("Continuous".into())),
            ("Dashed", LineTypeRef::Named("Dashed".into())),
            ("Center", LineTypeRef::Named("Center".into())),
        ] {
            if ui
                .selectable_label(first_lt.as_ref() == Some(&val), lbl)
                .clicked()
            {
                app.history.snapshot(&app.document);
                for &id in sel {
                    if let Some(e) = app.document.get_mut(id) {
                        e.line_type = val.clone();
                    }
                }
                ui.close();
            }
        }
    });

    // ── Layer ─────────────────────────────────────────────────────────────
    let layer_names: Vec<String> = app
        .document
        .layers
        .layers
        .iter()
        .map(|l| l.name.clone())
        .collect();
    let first_layer = sel
        .first()
        .and_then(|&id| app.document.get(id))
        .map(|e| e.layer)
        .unwrap_or(0);
    let mixed = sel
        .iter()
        .any(|&id| app.document.get(id).map(|e| e.layer) != Some(first_layer));
    let layer_value = if mixed {
        "(mixed)".to_string()
    } else {
        layer_names.get(first_layer).cloned().unwrap_or_default()
    };
    let swatch = app
        .document
        .layers
        .get(first_layer)
        .map(|l| egui::Color32::from_rgb(l.color.0, l.color.1, l.color.2));
    appearance_row(ui, "Layer", layer_value, swatch, false, |ui| {
        for (i, name) in layer_names.iter().enumerate() {
            if ui.selectable_label(i == first_layer, name).clicked() {
                app.history.snapshot(&app.document);
                for &id in sel {
                    if let Some(e) = app.document.get_mut(id) {
                        e.layer = i;
                    }
                }
                ui.close();
            }
        }
    });
}

fn selection_properties(ui: &mut egui::Ui, app: &mut AppState) {
    let sel: Vec<_> = app.selection.clone();
    if sel.is_empty() {
        ui.add(egui::Label::new(
            egui::RichText::new("Nothing selected").color(crate::theme::TEXT_DIM),
        ));
        ui.add(egui::Label::new(
            egui::RichText::new(format!("{} objects in drawing", app.document.len()))
                .size(11.0)
                .color(crate::theme::TEXT_DIM),
        ));
        return;
    }

    if sel.len() == 1 {
        let id = sel[0];
        if let Some(kind) = app.document.get(id).map(|e| e.kind.clone()) {
            let layer_idx = app.document.get(id).map(|e| e.layer).unwrap_or(0);
            let layer_name = app
                .document
                .layers
                .get(layer_idx)
                .map(|l| l.name.clone())
                .unwrap_or_default();
            object_header(
                ui,
                kind_label(&kind),
                &format!("Layer {layer_name}"),
                kind_icon(&kind),
            );
            edit_entity_geometry(ui, app, id);
            if let Some(e) = app.document.get(id) {
                measurements(ui, &e.kind);
            }
        }
    } else {
        ui.add(egui::Label::new(
            egui::RichText::new(format!("{} objects selected", sel.len()))
                .size(14.0)
                .strong(),
        ));
    }

    appearance_section(ui, app, &sel);
}

fn edit_entity_geometry(ui: &mut egui::Ui, app: &mut AppState, id: eiderflat_document::EntityId) {
    let entity = match app.document.get(id) {
        Some(e) => e.clone(),
        None => return,
    };
    match &entity.kind {
        EntityKind::Curve(Curve::Line(line)) => {
            prop_section(ui, "GEOMETRY");
            let mut p0x = line.p0.x;
            let mut p0y = line.p0.y;
            let mut p1x = line.p1.x;
            let mut p1y = line.p1.y;

            let mut changed = false;
            prop_caption(ui, "Start");
            changed |= xy_fields(ui, "X", &mut p0x, "Y", &mut p0y, 0.01);
            ui.add_space(4.0);
            prop_caption(ui, "End");
            changed |= xy_fields(ui, "X", &mut p1x, "Y", &mut p1y, 0.01);

            if changed {
                app.history.snapshot(&app.document);
                if let Some(e) = app.document.get_mut(id)
                    && let EntityKind::Curve(Curve::Line(ref mut l)) = e.kind
                {
                    l.p0 = Point2d::from_f64(p0x, p0y);
                    l.p1 = Point2d::from_f64(p1x, p1y);
                }
            }
        }
        EntityKind::Curve(Curve::Arc(arc)) => {
            let span = (arc.end_angle - arc.start_angle).abs();
            let is_circle = (span - 2.0 * std::f64::consts::PI).abs() < 1e-9;
            prop_section(ui, "GEOMETRY");

            let mut cx = arc.center.x;
            let mut cy = arc.center.y;
            let mut r = arc.radius;
            let mut sa = arc.start_angle.to_degrees();
            let mut ea = arc.end_angle.to_degrees();

            let mut changed = false;
            prop_caption(ui, "Centre");
            changed |= xy_fields(ui, "X", &mut cx, "Y", &mut cy, 0.01);
            ui.add_space(4.0);
            changed |= num_field(ui, "Radius", &mut r, 0.01);

            if !is_circle {
                ui.add_space(4.0);
                ui.columns(2, |c| {
                    changed |= num_field(&mut c[0], "Start °", &mut sa, 0.5);
                    changed |= num_field(&mut c[1], "End °", &mut ea, 0.5);
                });
            }

            if changed {
                app.history.snapshot(&app.document);
                if let Some(e) = app.document.get_mut(id)
                    && let EntityKind::Curve(Curve::Arc(ref mut a)) = e.kind
                {
                    a.center = Point2d::from_f64(cx, cy);
                    a.radius = r.max(0.001);
                    if !is_circle {
                        a.start_angle = sa.to_radians();
                        a.end_angle = ea.to_radians();
                    }
                }
            }
        }
        EntityKind::Text {
            anchor,
            content,
            height,
            rotation,
            font,
        } => {
            prop_section(ui, "GEOMETRY");
            let mut ax = anchor.x;
            let mut ay = anchor.y;
            let mut h = *height;
            let mut rot = rotation.to_degrees();
            let mut txt = content.clone();
            let mut chosen_font = font.clone();

            let mut changed = false;
            prop_caption(ui, "Font");
            changed |= font_combo(ui, "prop_font", &mut chosen_font);
            ui.add_space(4.0);
            prop_caption(ui, "Anchor");
            changed |= xy_fields(ui, "X", &mut ax, "Y", &mut ay, 0.01);
            ui.add_space(4.0);
            ui.columns(2, |c| {
                changed |= num_field(&mut c[0], "Height", &mut h, 0.01);
                changed |= num_field(&mut c[1], "Rotation °", &mut rot, 0.5);
            });
            ui.add_space(4.0);
            prop_caption(ui, "Content");
            changed |= ui
                .add_sized(
                    [ui.available_width(), 48.0],
                    egui::TextEdit::multiline(&mut txt),
                )
                .changed();

            if changed {
                app.history.snapshot(&app.document);
                if let Some(e) = app.document.get_mut(id)
                    && let EntityKind::Text {
                        anchor: ref mut a,
                        content: ref mut c,
                        height: ref mut ht,
                        rotation: ref mut rot_rad,
                        font: ref mut f,
                    } = e.kind
                {
                    *a = Point2d::from_f64(ax, ay);
                    *c = txt;
                    *ht = h.max(0.1);
                    *rot_rad = rot.to_radians();
                    *f = chosen_font;
                }
            }
        }
        EntityKind::Point(pt) => {
            prop_section(ui, "GEOMETRY");
            let mut px = pt.x;
            let mut py = pt.y;

            let mut changed = false;
            prop_caption(ui, "Position");
            changed |= xy_fields(ui, "X", &mut px, "Y", &mut py, 0.01);

            if changed {
                app.history.snapshot(&app.document);
                if let Some(e) = app.document.get_mut(id)
                    && let EntityKind::Point(ref mut p) = e.kind
                {
                    *p = Point2d::from_f64(px, py);
                }
            }
        }
        EntityKind::Curve(Curve::Ellipse(el)) => {
            let span = (el.end_angle - el.start_angle).abs();
            let is_full = (span - std::f64::consts::TAU).abs() < 1e-9;
            prop_section(ui, "GEOMETRY");

            let mut cx = el.center.x;
            let mut cy = el.center.y;
            let mut major = el.semi_major;
            let mut minor = el.semi_minor;
            let mut rot = el.rotation.to_degrees();
            let mut sa = el.start_angle.to_degrees();
            let mut ea = el.end_angle.to_degrees();

            let mut changed = false;
            prop_caption(ui, "Centre");
            changed |= xy_fields(ui, "X", &mut cx, "Y", &mut cy, 0.01);
            ui.add_space(4.0);
            ui.columns(2, |c| {
                changed |= num_field(&mut c[0], "Semi-major", &mut major, 0.01);
                changed |= num_field(&mut c[1], "Semi-minor", &mut minor, 0.01);
            });
            ui.add_space(4.0);
            changed |= num_field(ui, "Rotation °", &mut rot, 0.5);
            if !is_full {
                ui.add_space(4.0);
                ui.columns(2, |c| {
                    changed |= num_field(&mut c[0], "Start °", &mut sa, 0.5);
                    changed |= num_field(&mut c[1], "End °", &mut ea, 0.5);
                });
            }

            if changed {
                app.history.snapshot(&app.document);
                if let Some(e) = app.document.get_mut(id)
                    && let EntityKind::Curve(Curve::Ellipse(ref mut a)) = e.kind
                {
                    a.center = Point2d::from_f64(cx, cy);
                    a.semi_major = major.max(0.001);
                    a.semi_minor = minor.max(0.001);
                    a.rotation = rot.to_radians();
                    if !is_full {
                        a.start_angle = sa.to_radians();
                        a.end_angle = ea.to_radians();
                    }
                }
            }
        }
        EntityKind::Curve(Curve::Poly(pc)) => {
            use eiderflat_geometry::CurveSegment;
            prop_section(ui, "GEOMETRY");
            let segs = &pc.segments;
            let all_lines = !segs.is_empty() && segs.iter().all(|s| matches!(s, Curve::Line(_)));
            if !all_lines {
                ui.add(egui::Label::new(
                    egui::RichText::new(format!("{} segments — edit on canvas", segs.len()))
                        .size(11.0)
                        .italics()
                        .color(crate::theme::TEXT_DIM),
                ));
            } else {
                let mut verts: Vec<(f64, f64)> = Vec::with_capacity(segs.len() + 1);
                let (t0, _) = segs[0].domain();
                verts.push(segs[0].evaluate_f64(t0));
                for s in segs {
                    let (_, t1) = s.domain();
                    verts.push(s.evaluate_f64(t1));
                }
                let closed = {
                    let (a, b) = (verts[0], *verts.last().unwrap());
                    (a.0 - b.0).hypot(a.1 - b.1) < 1e-6
                };
                if closed {
                    verts.pop();
                }

                let mut changed = false;
                egui::ScrollArea::vertical()
                    .max_height(220.0)
                    .show(ui, |ui| {
                        for (k, v) in verts.iter_mut().enumerate() {
                            prop_caption(ui, &format!("Vertex {}", k + 1));
                            changed |= xy_fields(ui, "X", &mut v.0, "Y", &mut v.1, 0.01);
                            ui.add_space(2.0);
                        }
                    });

                if changed {
                    app.history.snapshot(&app.document);
                    let n = verts.len();
                    let limit = if closed { n } else { n - 1 };
                    let mut new_segs: Vec<Curve> = Vec::with_capacity(limit);
                    for i in 0..limit {
                        let a = verts[i];
                        let b = verts[(i + 1) % n];
                        new_segs.push(Curve::Line(eiderflat_geometry::LineSeg::from_endpoints(
                            Point2d::from_f64(a.0, a.1),
                            Point2d::from_f64(b.0, b.1),
                        )));
                    }
                    if let Some(e) = app.document.get_mut(id)
                        && let EntityKind::Curve(Curve::Poly(ref mut p)) = e.kind
                    {
                        **p = eiderflat_geometry::PolyCurve::new(new_segs);
                    }
                }
            }
        }
        EntityKind::Hatch { pattern, .. } => {
            prop_section(ui, "PATTERN");
            let mut pat = *pattern;
            if hatch_pattern_editor(ui, &mut pat) {
                app.history.snapshot(&app.document);
                if let Some(e) = app.document.get_mut(id)
                    && let EntityKind::Hatch {
                        pattern: ref mut p, ..
                    } = e.kind
                {
                    *p = pat;
                }
                app.hatch_pattern = pat;
            }
        }
        _ => {
            prop_section(ui, "GEOMETRY");
            ui.add(egui::Label::new(
                egui::RichText::new("Not editable here")
                    .size(11.0)
                    .italics()
                    .color(crate::theme::TEXT_DIM),
            ));
        }
    }
}
fn hatch_pattern_editor(ui: &mut egui::Ui, pattern: &mut eiderflat_document::HatchPattern) -> bool {
    use eiderflat_document::HatchPattern as HP;
    let mut changed = false;
    // Current kind label.
    let kind = match pattern {
        HP::Solid => "Solid",
        HP::Lines { .. } => "Lines",
        HP::Cross { .. } => "Cross-hatch",
        HP::Dots { .. } => "Dots",
    };
    egui::ComboBox::from_id_salt("hatch_pat")
        .selected_text(kind)
        .show_ui(ui, |ui| {
            let (a, s) = match *pattern {
                HP::Lines { angle_deg, spacing } | HP::Cross { angle_deg, spacing } => {
                    (angle_deg, spacing)
                }
                HP::Dots { spacing } => (45.0, spacing),
                HP::Solid => (45.0, 1.0),
            };
            for (label, cand) in [
                ("Solid", HP::Solid),
                (
                    "Lines",
                    HP::Lines {
                        angle_deg: a,
                        spacing: s.max(0.1),
                    },
                ),
                (
                    "Cross-hatch",
                    HP::Cross {
                        angle_deg: a,
                        spacing: s.max(0.1),
                    },
                ),
                (
                    "Dots",
                    HP::Dots {
                        spacing: s.max(0.1),
                    },
                ),
            ] {
                let selected = std::mem::discriminant(pattern) == std::mem::discriminant(&cand);
                if ui.selectable_label(selected, label).clicked() && !selected {
                    *pattern = cand;
                    changed = true;
                }
            }
        });
    match pattern {
        HP::Lines { angle_deg, spacing } | HP::Cross { angle_deg, spacing } => {
            ui.add_space(4.0);
            ui.columns(2, |c| {
                changed |= num_field(&mut c[0], "Angle °", &mut *angle_deg, 1.0);
                changed |= num_field(&mut c[1], "Spacing", &mut *spacing, 0.05);
            });
            *spacing = spacing.max(0.05);
        }
        HP::Dots { spacing } => {
            ui.add_space(4.0);
            changed |= num_field(ui, "Spacing", &mut *spacing, 0.05);
            *spacing = spacing.max(0.05);
        }
        HP::Solid => {}
    }
    changed
}

fn maybe_save(app: &mut AppState) -> bool {
    if !app.is_dirty() {
        return true;
    }
    let name = app
        .current_file_path
        .as_ref()
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Untitled".to_string());
    let res = rfd::MessageDialog::new()
        .set_level(rfd::MessageLevel::Warning)
        .set_title("Unsaved changes")
        .set_description(format!("Save changes to \"{name}\" before continuing?"))
        .set_buttons(rfd::MessageButtons::YesNoCancel)
        .show();
    match res {
        rfd::MessageDialogResult::Yes => {
            if !app.save_file() {
                file_save_as(app);
            }
            !app.is_dirty()
        }
        rfd::MessageDialogResult::No => true,
        _ => false,
    }
}

fn file_open(app: &mut AppState) {
    if let Some(path) = FileDialog::new()
        .add_filter("All supported (e2d, dxf, svg)", &["e2d", "dxf", "svg"])
        .add_filter("eiderFLAT drawing", &["e2d"])
        .add_filter("DXF (ASCII)", &["dxf"])
        .add_filter("SVG", &["svg"])
        .pick_file()
    {
        app.open_file(path);
    }
}

fn file_save_as(app: &mut AppState) {
    let suggested = app
        .current_file_path
        .as_ref()
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "untitled.e2d".to_string());
    if let Some(path) = FileDialog::new()
        .add_filter("eiderFLAT drawing", &["e2d"])
        .add_filter("DXF (ASCII)", &["dxf"])
        .add_filter("SVG", &["svg"])
        .set_file_name(&suggested)
        .save_file()
    {
        app.save_file_to(path);
    }
}

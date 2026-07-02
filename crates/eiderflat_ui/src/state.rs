use eiderflat_cad::{
    Grip, Guide, SnapPoint, SnapSettings, apply_grip, best_snap, edit, find_snaps_excluding,
    grips_for, infer_axis, pick_at,
};
use eiderflat_document::{Document, Entity, EntityId, EntityKind, Layer, LineTypeRef, LineWeight};
use eiderflat_geometry::{Curve, MinTracker, Point2d};

use crate::command::{Command, CoordInput, parse_command, parse_coordinate};
use crate::history::History;
use crate::tools::{Tool, ToolEvent};
use crate::view_transform::ViewTransform;

mod modify;
pub use modify::TrimExtendPreview;
mod contextual;

pub use contextual::{CornerAction, CornerGeom, CornerKind, fillet_arc};

pub struct AppState {
    pub document: Document,
    pub view: ViewTransform,
    pub tool: Tool,
    pub selection: Vec<EntityId>,
    pub snap: SnapSettings,
    pub snap_on: bool,
    pub grid_on: bool,
    pub grid_snap_on: bool,
    pub ortho_on: bool,
    pub polar_on: bool,
    pub track_on: bool,
    pub dyn_on: bool,
    pub last_command: Option<String>,
    pub history: History,
    pub command_log: Vec<String>,
    pub cursor_world: (f64, f64),
    pub active_snap: Option<SnapPoint>,
    pub click_count: u32,
    pub origin_id: EntityId,
    pub interaction: InteractionState,
    pub current_file_path: Option<std::path::PathBuf>,
    pub text_font: Option<String>,
    pub hatch_pattern: eiderflat_document::HatchPattern,
    pub saved_depth: usize,
    pub zoom_target: Option<(f64, f64, f64)>,
    pub default_line_type: LineTypeRef,
    pub default_line_weight: LineWeight,
    pub comb_on: bool,
    pub comb_scale: f64,
    pub snap_px: f64,
    pub polar_step: f64,
    pub zoom_speed: f64,
    pub zoom_to_cursor: bool,
    pub invert_zoom: bool,
    pub crosshair: bool,
    pub pick_box: f64,
    pub show_lineweights: bool,
    pub lineweight_scale: f64,
    pub grid_dots: bool,
    pub grid_major_every: u32,
    pub grid_minor_rgb: (u8, u8, u8),
    pub grid_major_rgb: (u8, u8, u8),
    pub clipboard: Vec<Entity>,
    pub hint_tool: Option<Tool>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiPrefs {
    pub snap_on: bool,
    pub grid_on: bool,
    pub grid_snap_on: bool,
    pub ortho_on: bool,
    pub polar_on: bool,
    pub track_on: bool,
    pub dyn_on: bool,
    pub comb_on: bool,
    pub comb_scale: f64,
    pub snap_px: f64,
    pub polar_step: f64,
    pub zoom_speed: f64,
    pub zoom_to_cursor: bool,
    pub invert_zoom: bool,
    pub crosshair: bool,
    pub pick_box: f64,
    pub show_lineweights: bool,
    pub lineweight_scale: f64,
    pub grid_dots: bool,
    pub grid_major_every: u32,
    pub grid_minor_rgb: (u8, u8, u8),
    pub grid_major_rgb: (u8, u8, u8),
    pub text_font: Option<String>,
}

impl Default for UiPrefs {
    fn default() -> Self {
        UiPrefs {
            snap_on: true,
            grid_on: true,
            grid_snap_on: false,
            ortho_on: false,
            polar_on: true,
            track_on: true,
            dyn_on: true,
            comb_on: false,
            comb_scale: 5.0,
            snap_px: 12.0,
            polar_step: 45.0,
            zoom_speed: 1.0,
            zoom_to_cursor: true,
            invert_zoom: false,
            crosshair: true,
            pick_box: 11.0,
            show_lineweights: true,
            lineweight_scale: 5.0,
            grid_dots: false,
            grid_major_every: 5,
            grid_minor_rgb: (24, 28, 36),
            grid_major_rgb: (33, 39, 49),
            text_font: None,
        }
    }
}

fn parse_rgb(s: &str) -> Option<(u8, u8, u8)> {
    let p: Vec<u8> = s.split(',').filter_map(|v| v.trim().parse().ok()).collect();
    (p.len() == 3).then(|| (p[0], p[1], p[2]))
}

impl UiPrefs {
    pub fn serialize(&self) -> String {
        let b = |v: bool| if v { "1" } else { "0" };
        let rgb = |c: (u8, u8, u8)| format!("{},{},{}", c.0, c.1, c.2);
        let font = self.text_font.as_deref().unwrap_or("");
        format!(
            "snap={}\ngrid={}\ngsnap={}\northo={}\npolar={}\ntrack={}\ndyn={}\ncomb={}\ncomb_scale={}\nsnap_px={}\npolar_step={}\nzoom_speed={}\nzoom_cursor={}\ninvert_zoom={}\ncrosshair={}\npick_box={}\nlw_show={}\nlw_scale={}\ngrid_dots={}\ngrid_major={}\ngrid_minor={}\ngrid_majorc={}\nfont={}\n",
            b(self.snap_on),
            b(self.grid_on),
            b(self.grid_snap_on),
            b(self.ortho_on),
            b(self.polar_on),
            b(self.track_on),
            b(self.dyn_on),
            b(self.comb_on),
            self.comb_scale,
            self.snap_px,
            self.polar_step,
            self.zoom_speed,
            b(self.zoom_to_cursor),
            b(self.invert_zoom),
            b(self.crosshair),
            self.pick_box,
            b(self.show_lineweights),
            self.lineweight_scale,
            b(self.grid_dots),
            self.grid_major_every,
            rgb(self.grid_minor_rgb),
            rgb(self.grid_major_rgb),
            font,
        )
    }

    pub fn deserialize(s: &str) -> Self {
        let mut p = UiPrefs::default();
        for line in s.lines() {
            let Some((k, v)) = line.split_once('=') else {
                continue;
            };
            let on = v == "1";
            match k.trim() {
                "snap" => p.snap_on = on,
                "grid" => p.grid_on = on,
                "gsnap" => p.grid_snap_on = on,
                "ortho" => p.ortho_on = on,
                "polar" => p.polar_on = on,
                "track" => p.track_on = on,
                "dyn" => p.dyn_on = on,
                "comb" => p.comb_on = on,
                "comb_scale" => {
                    if let Ok(f) = v.trim().parse::<f64>() {
                        p.comb_scale = f;
                    }
                }
                "snap_px" => {
                    if let Ok(f) = v.trim().parse::<f64>() {
                        p.snap_px = f.clamp(2.0, 40.0);
                    }
                }
                "polar_step" => {
                    if let Ok(f) = v.trim().parse::<f64>() {
                        p.polar_step = f.clamp(1.0, 90.0);
                    }
                }
                "zoom_speed" => {
                    if let Ok(f) = v.trim().parse::<f64>() {
                        p.zoom_speed = f.clamp(0.25, 4.0);
                    }
                }
                "zoom_cursor" => p.zoom_to_cursor = on,
                "invert_zoom" => p.invert_zoom = on,
                "crosshair" => p.crosshair = on,
                "pick_box" => {
                    if let Ok(f) = v.trim().parse::<f64>() {
                        p.pick_box = f.clamp(5.0, 30.0);
                    }
                }
                "lw_show" => p.show_lineweights = on,
                "lw_scale" => {
                    if let Ok(f) = v.trim().parse::<f64>() {
                        p.lineweight_scale = f.clamp(1.0, 20.0);
                    }
                }
                "grid_dots" => p.grid_dots = on,
                "grid_major" => {
                    if let Ok(n) = v.trim().parse::<u32>() {
                        p.grid_major_every = n.clamp(2, 20);
                    }
                }
                "grid_minor" => {
                    if let Some(c) = parse_rgb(v) {
                        p.grid_minor_rgb = c;
                    }
                }
                "grid_majorc" => {
                    if let Some(c) = parse_rgb(v) {
                        p.grid_major_rgb = c;
                    }
                }
                "font" => p.text_font = (!v.is_empty()).then(|| v.to_string()),
                _ => {}
            }
        }
        p
    }
}

impl AppState {
    pub fn ui_prefs(&self) -> UiPrefs {
        UiPrefs {
            snap_on: self.snap_on,
            grid_on: self.grid_on,
            grid_snap_on: self.grid_snap_on,
            ortho_on: self.ortho_on,
            polar_on: self.polar_on,
            track_on: self.track_on,
            dyn_on: self.dyn_on,
            comb_on: self.comb_on,
            comb_scale: self.comb_scale,
            snap_px: self.snap_px,
            polar_step: self.polar_step,
            zoom_speed: self.zoom_speed,
            zoom_to_cursor: self.zoom_to_cursor,
            invert_zoom: self.invert_zoom,
            crosshair: self.crosshair,
            pick_box: self.pick_box,
            show_lineweights: self.show_lineweights,
            lineweight_scale: self.lineweight_scale,
            grid_dots: self.grid_dots,
            grid_major_every: self.grid_major_every,
            grid_minor_rgb: self.grid_minor_rgb,
            grid_major_rgb: self.grid_major_rgb,
            text_font: self.text_font.clone(),
        }
    }

    pub fn apply_prefs(&mut self, p: &UiPrefs) {
        self.snap_on = p.snap_on;
        self.grid_on = p.grid_on;
        self.grid_snap_on = p.grid_snap_on;
        self.ortho_on = p.ortho_on;
        self.polar_on = p.polar_on;
        self.track_on = p.track_on;
        self.dyn_on = p.dyn_on;
        self.comb_on = p.comb_on;
        self.comb_scale = p.comb_scale;
        self.snap_px = p.snap_px;
        self.polar_step = p.polar_step;
        self.zoom_speed = p.zoom_speed;
        self.zoom_to_cursor = p.zoom_to_cursor;
        self.invert_zoom = p.invert_zoom;
        self.crosshair = p.crosshair;
        self.pick_box = p.pick_box;
        self.show_lineweights = p.show_lineweights;
        self.lineweight_scale = p.lineweight_scale;
        self.grid_dots = p.grid_dots;
        self.grid_major_every = p.grid_major_every;
        self.grid_minor_rgb = p.grid_minor_rgb;
        self.grid_major_rgb = p.grid_major_rgb;
        self.text_font = p.text_font.clone();
        if self.ortho_on {
            self.polar_on = false;
        }
    }
}

#[derive(Default)]
pub struct InteractionState {
    pub grip_drag: Option<GripDrag>,
    pub bbox_drag: Option<BboxDrag>,
    pub corner_action: Option<CornerAction>,
    pub active_guide: Option<((f64, f64), f64)>,
    pub active_guides: Vec<Guide>,
}

#[derive(Clone, Debug)]
pub struct GripDrag {
    pub entity_id: EntityId,
    pub grip: Grip,
    pub start_kind: EntityKind,
}

#[derive(Clone, Debug)]
pub struct BboxDrag {
    pub handle: BboxHandle,
    pub bbox_start: eiderflat_geometry::BoundingBox,
    pub cursor_start: (f64, f64),
    pub originals: Vec<(EntityId, EntityKind)>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum BboxHandle {
    Body,
    CornerNW,
    CornerNE,
    CornerSW,
    CornerSE,
    RotateNW,
    RotateNE,
    RotateSW,
    RotateSE,
}

fn seed_default_layers(doc: &mut eiderflat_document::Document) {
    use eiderflat_document::Layer;
    for layer in [
        Layer::new("Dimensions").with_color(46, 204, 113),
        Layer::new("Centerlines")
            .with_color(232, 134, 108)
            .with_line_type("Center"),
        Layer::new("Construction")
            .with_color(169, 140, 255)
            .with_line_type("Dotted"),
        Layer::new("Hidden")
            .with_color(150, 160, 178)
            .with_line_type("Dashed"),
    ] {
        doc.layers.add(layer);
    }
}

fn line_endpoints(kind: &EntityKind) -> Option<((f64, f64), (f64, f64))> {
    match kind {
        EntityKind::Curve(Curve::Line(l)) => Some((l.p0.to_f64(), l.p1.to_f64())),
        _ => None,
    }
}

impl AppState {
    pub fn new(canvas_w: f64, canvas_h: f64) -> Self {
        let mut document = Document::new();
        seed_default_layers(&mut document);
        let origin_id = document.add(EntityKind::Point(Point2d::from_i64(0, 0)));

        AppState {
            document,
            view: ViewTransform::new(canvas_w, canvas_h),
            tool: Tool::Select,
            selection: Vec::new(),
            snap: SnapSettings::default(),
            snap_on: true,
            grid_on: true,
            grid_snap_on: false,
            ortho_on: false,
            polar_on: true,
            track_on: true,
            dyn_on: true,
            last_command: None,
            history: History::new(),
            command_log: Vec::new(),
            cursor_world: (0.0, 0.0),
            active_snap: None,
            click_count: 0,
            origin_id,
            interaction: InteractionState::default(),

            current_file_path: None,
            text_font: None,
            hatch_pattern: eiderflat_document::HatchPattern::Solid,
            saved_depth: 0,
            zoom_target: None,
            default_line_type: LineTypeRef::ByLayer,
            default_line_weight: LineWeight::ByLayer,
            comb_on: false,
            comb_scale: 5.0,
            snap_px: 12.0,
            polar_step: 45.0,
            zoom_speed: 1.0,
            zoom_to_cursor: true,
            invert_zoom: false,
            crosshair: true,
            pick_box: 11.0,
            show_lineweights: true,
            lineweight_scale: 5.0,
            grid_dots: false,
            grid_major_every: 5,
            grid_minor_rgb: (24, 28, 36),
            grid_major_rgb: (33, 39, 49),
            clipboard: Vec::new(),
            hint_tool: None,
        }
    }

    pub fn has_selection(&self) -> bool {
        self.selection.iter().any(|&id| id != self.origin_id)
    }

    pub fn clipboard_copy(&mut self) -> usize {
        let items: Vec<Entity> = self
            .selection
            .iter()
            .filter(|&&id| id != self.origin_id)
            .filter_map(|&id| self.document.get(id).cloned())
            .collect();
        let n = items.len();
        if n > 0 {
            self.clipboard = items;
        }
        n
    }

    pub fn clipboard_cut(&mut self) {
        if self.clipboard_copy() > 0 {
            self.erase_selection();
        }
    }

    pub fn clipboard_paste(&mut self) {
        if self.clipboard.is_empty() {
            return;
        }
        let bbox = self
            .clipboard
            .iter()
            .filter_map(|e| e.bounding_box())
            .reduce(|a, b| a.union(&b));
        let (dx, dy) = match bbox {
            Some(bb) => {
                let cx = (bb.min.x + bb.max.x) * 0.5;
                let cy = (bb.min.y + bb.max.y) * 0.5;
                (self.cursor_world.0 - cx, self.cursor_world.1 - cy)
            }
            None => (0.0, 0.0),
        };
        let t = eiderflat_geometry::Transform2d::translation(dx, dy);
        self.history.snapshot(&self.document);
        let mut pasted = Vec::with_capacity(self.clipboard.len());
        for e in &self.clipboard {
            let mut copy = e.clone();
            copy.transform(&t);
            pasted.push(self.document.add_entity(copy));
        }
        self.selection = pasted;
        self.tool = Tool::Select;
    }

    fn apply_new_entity_defaults(&mut self, id: EntityId) {
        let (lt, lw) = (
            self.default_line_type.clone(),
            self.default_line_weight.clone(),
        );
        let is_dim = matches!(
            self.document.get(id).map(|e| &e.kind),
            Some(
                eiderflat_document::EntityKind::Dimension { .. }
                    | eiderflat_document::EntityKind::OrthoDim { .. }
                    | eiderflat_document::EntityKind::AngularDim { .. }
                    | eiderflat_document::EntityKind::RadialDim { .. }
            )
        );
        let dim_layer = is_dim.then(|| {
            self.document.layers.add(
                eiderflat_document::Layer::new(eiderflat_document::DIMENSION_LAYER)
                    .with_color(46, 204, 113),
            )
        });
        if let Some(e) = self.document.get_mut(id) {
            e.line_type = lt;
            e.line_weight = lw;
            if let Some(layer) = dim_layer {
                e.layer = layer;
            }
        }
    }

    pub fn pointer_moved(&mut self, sx: f64, sy: f64) {
        let (wx, wy) = self.view.screen_to_world(sx, sy);

        let dragged_entity = self.interaction.grip_drag.as_ref().map(|d| d.entity_id);
        let allow_snap = self.tool.wants_point_snap() || dragged_entity.is_some();

        self.active_snap = if self.snap_on && allow_snap {
            let mut s = self.snap.clone();
            s.tolerance = self.view.pixel_world_size() * self.snap_px;
            let ref_pt = self.tool.reference_point().map(|p| p.to_f64());
            let doc_snap = match dragged_entity {
                Some(ex) => find_snaps_excluding(&self.document, (wx, wy), &s, ref_pt, Some(ex))
                    .into_iter()
                    .next(),
                None => best_snap(&self.document, (wx, wy), &s, ref_pt),
            };
            let self_snap = self.nearest_self_snap((wx, wy), s.tolerance);
            match (doc_snap, self_snap) {
                (Some(a), Some(b)) => {
                    let da = (a.pos.0 - wx).hypot(a.pos.1 - wy);
                    let db = (b.pos.0 - wx).hypot(b.pos.1 - wy);
                    Some(if db < da { b } else { a })
                }
                (a, b) => a.or(b),
            }
        } else {
            None
        };

        self.interaction.active_guide = None;
        self.interaction.active_guides.clear();

        if let Some(ref sp) = self.active_snap {
            self.cursor_world = sp.pos;
        } else if self.grid_snap_on && allow_snap {
            self.cursor_world = self.view.snap_to_grid(wx, wy);
        } else if self.ortho_on {
            if let Some(ref_pt) = self.tool.reference_point() {
                let (rx, ry) = ref_pt.to_f64();
                let dx = wx - rx;
                let dy = wy - ry;
                let angle_rad = if dx.abs() >= dy.abs() {
                    self.cursor_world = (wx, ry);
                    if wx >= rx { 0.0 } else { std::f64::consts::PI }
                } else {
                    self.cursor_world = (rx, wy);
                    if wy >= ry {
                        std::f64::consts::FRAC_PI_2
                    } else {
                        -std::f64::consts::FRAC_PI_2
                    }
                };
                self.interaction.active_guide = Some(((rx, ry), angle_rad));
            } else {
                self.cursor_world = (wx, wy);
            }
        } else {
            if let Some(ref_pt) = self.tool.reference_point() {
                let (rx, ry) = ref_pt.to_f64();
                let dx = wx - rx;
                let dy = wy - ry;
                let dist = (dx * dx + dy * dy).sqrt();
                if self.polar_on && dist > 1e-4 {
                    let angle_rad = dy.atan2(dx);
                    let angle_deg_wrapped = eiderflat_geometry::wrap_deg360(angle_rad.to_degrees());
                    let step = self.polar_step.max(1.0);
                    let nearest = (angle_deg_wrapped / step).round() * step;
                    let diff = (angle_deg_wrapped - nearest).abs();
                    let diff = diff.min(360.0 - diff);

                    if diff <= 3.0 {
                        let snapped_rad = nearest.to_radians();
                        self.cursor_world =
                            (rx + dist * snapped_rad.cos(), ry + dist * snapped_rad.sin());
                        self.interaction.active_guide = Some(((rx, ry), snapped_rad));
                    } else {
                        self.cursor_world = (wx, wy);
                    }
                } else {
                    self.cursor_world = (wx, wy);
                }
            } else {
                self.cursor_world = (wx, wy);
            }
        }

        if self.track_on
            && self.active_snap.is_none()
            && let Some(drag) = self.interaction.grip_drag.as_ref()
            && let Some((a, b)) = line_endpoints(&drag.start_kind)
        {
            let tol = self.view.pixel_world_size() * 10.0;
            if let Some(res) = infer_axis(a, b, (wx, wy), tol) {
                self.cursor_world = res.point;
                self.interaction.active_guides = res.guides;
            }
        }
    }

    fn nearest_self_snap(&self, cursor: (f64, f64), tol: f64) -> Option<SnapPoint> {
        if !self
            .snap
            .enabled
            .contains(&eiderflat_cad::SnapKind::Endpoint)
        {
            return None;
        }
        let mut best = MinTracker::new();
        for p in self.tool.in_progress_points() {
            let (px, py) = p.to_f64();
            let d = (px - cursor.0).hypot(py - cursor.1);
            if d <= tol {
                best.offer(d, (px, py));
            }
        }
        best.value().map(|pos| SnapPoint {
            kind: eiderflat_cad::SnapKind::Endpoint,
            pos,
            entity: self.origin_id,
        })
    }

    pub fn resolved_point(&self) -> Point2d {
        match &self.active_snap {
            Some(sp) => Point2d::from_f64(sp.pos.0, sp.pos.1),
            None => Point2d::from_f64(self.cursor_world.0, self.cursor_world.1),
        }
    }

    pub fn canvas_click(&mut self, sx: f64, sy: f64) {
        self.click_count = self.click_count.wrapping_add(1);
        self.pointer_moved(sx, sy);
        let p = self.resolved_point();

        if self.handle_modify_click(&p) {
            return;
        }

        if let Tool::Text { anchor, height } = &self.tool {
            let height = *height;
            let need_anchor = anchor.is_none();
            if need_anchor {
                self.tool = Tool::Text {
                    anchor: Some(p),
                    height,
                };
            }
            return;
        }

        if matches!(self.tool, Tool::Select) {
            if let Some(id) = pick_at(&self.document, p.x, p.y, self.view.pixel_world_size() * 6.0)
            {
                self.toggle_selection(id);
            } else {
                self.selection.clear();
            }
            return;
        }

        if self.try_close_on_start(p) {
            return;
        }

        let ev = self.tool.on_point(p);
        self.apply_tool_event(ev);
    }

    pub fn place_tool_point(&mut self, p: Point2d) {
        if self.try_close_on_start(p) {
            return;
        }
        let ev = self.tool.on_point(p);
        self.apply_tool_event(ev);
    }

    /// Commits the pending polygon (center + radius both picked, popup
    /// showing) with whatever side count is currently set. No-op outside
    /// that pending state.
    pub fn confirm_pending_polygon(&mut self) {
        if !matches!(
            self.tool,
            Tool::Polygon {
                center: Some(_),
                radius_point: Some(_),
                ..
            }
        ) {
            return;
        }
        let ev = self.tool.commit();
        self.apply_tool_event(ev);
    }

    /// Drops the pending polygon pick (both clicks made, popup showing)
    /// without committing, returning to "click the center point" — the last
    /// side count used is kept for next time.
    pub fn cancel_pending_polygon(&mut self) {
        if let Tool::Polygon { sides, .. } = self.tool.clone() {
            self.tool = Tool::Polygon {
                center: None,
                radius_point: None,
                sides,
            };
        }
    }

    fn try_close_on_start(&mut self, p: Point2d) -> bool {
        let close = match &self.tool {
            Tool::Polyline { pts } | Tool::Spline { pts } => {
                pts.len() >= 3 && pts[0].dist_f64(&p) <= self.view.pixel_world_size() * self.snap_px
            }
            _ => false,
        };
        if close {
            let ev = self.tool.close_and_commit();
            self.apply_tool_event(ev);
        }
        close
    }

    fn apply_tool_event(&mut self, ev: ToolEvent) {
        match ev {
            ToolEvent::Pending => {}
            ToolEvent::Create(kinds) => {
                self.history.snapshot(&self.document);
                for k in kinds {
                    let id = self.document.add(k);
                    self.apply_new_entity_defaults(id);
                }
            }
            ToolEvent::Transform { ids, t } => {
                self.history.snapshot(&self.document);
                let mut moved = Vec::new();
                for id in ids {
                    if id != self.origin_id
                        && let Some(e) = self.document.get_mut(id)
                    {
                        e.transform(&t);
                        moved.push(id);
                    }
                }
                self.selection = moved;
                self.tool = Tool::Select;
            }
            ToolEvent::CopyOf { ids, t } => {
                self.history.snapshot(&self.document);
                let mut new_ids = Vec::new();
                for id in ids {
                    if id != self.origin_id
                        && let Some(e) = self.document.get(id)
                    {
                        let copy = e.transformed(&t);
                        new_ids.push(self.document.add_entity(copy));
                    }
                }
                self.selection = new_ids;
                self.tool = Tool::Select;
            }
        }
    }

    fn toggle_selection(&mut self, id: EntityId) {
        if id == self.origin_id {
            return;
        }
        if let Some(pos) = self.selection.iter().position(|&s| s == id) {
            self.selection.remove(pos);
        } else {
            self.selection.push(id);
        }
    }

    pub fn run_command(&mut self, text: &str) {
        let trimmed = text.trim();

        if let Tool::Text {
            anchor: Some(p),
            height,
        } = self.tool.clone()
        {
            if !trimmed.is_empty() {
                self.history.snapshot(&self.document);
                self.document.add(EntityKind::Text {
                    anchor: p,
                    content: trimmed.replace("\\n", "\n"),
                    height,
                    rotation: 0.0,
                    font: self.text_font.clone(),
                });
            }
            self.tool = Tool::Select;
            self.command_log.push(trimmed.to_string());
            return;
        }

        if matches!(self.tool, Tool::Polyline { .. } | Tool::Spline { .. }) {
            if trimmed.is_empty() {
                let ev = self.tool.commit();
                self.apply_tool_event(ev);
                self.tool = Tool::Select;
                return;
            }
            let upper = trimmed.to_ascii_uppercase();
            if upper == "C" || upper == "CLOSE" {
                let ev = self.tool.close_and_commit();
                self.apply_tool_event(ev);
                self.tool = Tool::Select;
                self.command_log.push(trimmed.to_string());
                return;
            }
        }

        if let Tool::Polygon { center: None, .. } = self.tool
            && let Ok(n) = trimmed.parse::<usize>()
            && n >= 3
        {
            self.tool = Tool::Polygon {
                center: None,
                radius_point: None,
                sides: Some(n),
            };
            self.command_log.push(trimmed.to_string());
            return;
        }

        if let Ok(v) = trimmed.parse::<f64>()
            && v > 0.0
        {
            match &self.tool {
                Tool::Offset { source, .. } => {
                    self.tool = Tool::Offset {
                        dist: v,
                        source: *source,
                    };
                    self.command_log.push(trimmed.to_string());
                    return;
                }
                Tool::Fillet { first, .. } => {
                    self.tool = Tool::Fillet {
                        radius: v,
                        first: *first,
                    };
                    self.command_log.push(trimmed.to_string());
                    return;
                }
                Tool::Chamfer { first, .. } => {
                    self.tool = Tool::Chamfer {
                        dist: v,
                        first: *first,
                    };
                    self.command_log.push(trimmed.to_string());
                    return;
                }
                Tool::Blend {
                    continuity,
                    first,
                    second,
                    ..
                } => {
                    self.tool = Tool::Blend {
                        continuity: *continuity,
                        tension: v,
                        first: *first,
                        second: *second,
                    };
                    self.command_log.push(trimmed.to_string());
                    return;
                }
                Tool::CircleTtr { first, .. } => {
                    self.tool = Tool::CircleTtr {
                        radius: v,
                        first: *first,
                    };
                    self.command_log.push(trimmed.to_string());
                    return;
                }
                _ => {}
            }
        }

        if let Ok(dist) = trimmed.parse::<f64>()
            && let Some(ref_pt) = self.tool.reference_point()
        {
            let (rx, ry) = ref_pt.to_f64();
            let (cx, cy) = self.cursor_world;
            let dx = cx - rx;
            let dy = cy - ry;
            let len = (dx * dx + dy * dy).sqrt();
            let (ux, uy) = if len > 1e-9 {
                (dx / len, dy / len)
            } else if let Some((_, angle_rad)) = self.interaction.active_guide {
                (angle_rad.cos(), angle_rad.sin())
            } else {
                (1.0, 0.0)
            };
            let target_pt = Point2d::from_f64(rx + dist * ux, ry + dist * uy);
            let ev = self.tool.on_point(target_pt);
            self.apply_tool_event(ev);
            self.command_log.push(trimmed.to_string());
            return;
        }

        if let Some(coord) = parse_coordinate(trimmed) {
            let (rx, ry) = self
                .tool
                .reference_point()
                .map(|p| p.to_f64())
                .unwrap_or((0.0, 0.0));
            let (x, y) = match coord {
                CoordInput::Absolute(x, y) => (x, y),
                CoordInput::Relative(dx, dy) => (rx + dx, ry + dy),
                CoordInput::PolarAbsolute { dist, angle_deg } => {
                    let a = angle_deg.to_radians();
                    (dist * a.cos(), dist * a.sin())
                }
                CoordInput::PolarRelative { dist, angle_deg } => {
                    let a = angle_deg.to_radians();
                    (rx + dist * a.cos(), ry + dist * a.sin())
                }
            };
            let ev = self.tool.on_point(Point2d::from_f64(x, y));
            self.apply_tool_event(ev);
            self.command_log.push(trimmed.to_string());
            return;
        }

        let cmd = parse_command(text);
        self.command_log.push(text.trim().to_string());
        if !matches!(cmd, Command::Cancel | Command::Unknown(_)) {
            self.last_command = Some(trimmed.to_string());
        }
        self.execute(cmd);
    }

    pub fn repeat_last_command(&mut self) {
        if let Some(cmd) = self.last_command.clone() {
            self.run_command(&cmd);
        }
    }

    pub fn execute(&mut self, cmd: Command) {
        match cmd {
            Command::Activate(mut tool) => {
                match &mut tool {
                    Tool::Move { ids, .. }
                    | Tool::Copy { ids, .. }
                    | Tool::Rotate { ids, .. }
                    | Tool::Scale { ids, .. }
                    | Tool::Mirror { ids, .. }
                    | Tool::Stretch { ids, .. } => *ids = self.selection.clone(),
                    _ => {}
                }
                self.tool = tool;
            }
            Command::Cancel => {
                self.tool.reset();
                if matches!(self.tool, Tool::Select) {
                    self.selection.clear();
                }
                self.tool = Tool::Select;
            }
            Command::Undo => self.undo(),
            Command::Redo => self.redo(),
            Command::Erase => self.erase_selection(),
            Command::Explode => self.explode_selection(),
            Command::Join => self.join_selection(),
            Command::Hatch => {
                if self.selection.is_empty() {
                    self.tool = Tool::Hatch;
                } else {
                    self.hatch_selection();
                }
            }
            Command::SelectAll => {
                self.selection = self
                    .document
                    .iter()
                    .map(|e| e.id)
                    .filter(|&id| id != self.origin_id)
                    .collect();
            }
            Command::ZoomExtents => self.zoom_extents(),
            Command::ZoomScale(s) => {
                self.view.zoom = s.clamp(1e-9, 1e12);
            }
            Command::LayerSet(name) => {
                self.document.layers.set_current(&name);
            }
            Command::LayerNew(name) => {
                let idx = self.document.layers.add(Layer::new(name));
                self.document.layers.current = idx;
            }
            Command::Unknown(_) => {}
        }
    }

    pub fn undo(&mut self) {
        if let Some(prev) = self.history.undo(&self.document) {
            self.document = prev;
            self.selection.clear();
        }
    }

    pub fn redo(&mut self) {
        if let Some(next) = self.history.redo(&self.document) {
            self.document = next;
            self.selection.clear();
        }
    }

    pub fn erase_selection(&mut self) {
        if self.selection.is_empty() {
            return;
        }
        self.history.snapshot(&self.document);
        for id in std::mem::take(&mut self.selection) {
            if id != self.origin_id {
                self.document.remove(id);
            }
        }
    }

    pub fn explode_selection(&mut self) {
        if self.selection.is_empty() {
            return;
        }
        self.history.snapshot(&self.document);
        let ids: Vec<_> = std::mem::take(&mut self.selection)
            .into_iter()
            .filter(|&id| id != self.origin_id)
            .collect();
        let new_ids = edit::explode(&mut self.document, &ids);
        let survived: Vec<_> = ids
            .into_iter()
            .filter(|&id| self.document.get(id).is_some())
            .collect();
        self.selection = survived.into_iter().chain(new_ids).collect();
    }

    pub fn hatch_selection(&mut self) {
        if self.selection.is_empty() {
            return;
        }
        let fill = self.document.layers.current_layer().color;
        let loops: Vec<Vec<Curve>> = self
            .selection
            .iter()
            .filter(|&&id| id != self.origin_id)
            .filter_map(|&id| self.document.get(id).and_then(eiderflat_cad::boundary_loop))
            .collect();
        if loops.is_empty() {
            self.command_log.push(
                "HATCH: select a closed boundary, or run HATCH and click inside an area".into(),
            );
            return;
        }
        self.history.snapshot(&self.document);
        self.selection = loops
            .into_iter()
            .map(|b| {
                self.document.add(EntityKind::Hatch {
                    boundary: b,
                    holes: Vec::new(),
                    fill,
                    pattern: self.hatch_pattern,
                })
            })
            .collect();
    }

    pub fn hatch_at_point(&mut self, x: f64, y: f64) -> bool {
        let (boundary, holes) = match eiderflat_cad::trace_pick_region(&self.document, x, y) {
            Ok(r) => r,
            Err(eiderflat_cad::PickRegionError::TooComplex) => {
                self.command_log
                    .push("HATCH: boundary too complex to trace (over 4000 segments)".into());
                return false;
            }
            Err(eiderflat_cad::PickRegionError::NotFound) => {
                self.command_log
                    .push("HATCH: no enclosed area found at that point".into());
                return false;
            }
        };
        let fill = self.document.layers.current_layer().color;
        self.history.snapshot(&self.document);
        let id = self.document.add(EntityKind::Hatch {
            boundary,
            holes,
            fill,
            pattern: self.hatch_pattern,
        });
        self.selection = vec![id];
        true
    }

    pub fn join_selection(&mut self) {
        if self.selection.is_empty() {
            return;
        }
        self.history.snapshot(&self.document);
        let ids: Vec<_> = std::mem::take(&mut self.selection)
            .into_iter()
            .filter(|&id| id != self.origin_id)
            .collect();
        let new_ids = edit::join(&mut self.document, &ids);
        if new_ids.is_empty() {
            self.selection = ids;
            self.history.discard_last();
            return;
        }
        let survived: Vec<_> = ids
            .into_iter()
            .filter(|&id| self.document.get(id).is_some())
            .collect();
        self.selection = survived.into_iter().chain(new_ids).collect();
    }

    pub fn zoom_extents(&mut self) {
        if let Some(bb) = self.document.extents() {
            let (x0, y0) = bb.min.to_f64();
            let (x1, y1) = bb.max.to_f64();
            let mut target = self.view.clone();
            target.zoom_to_bounds(x0, y0, x1, y1);
            self.zoom_target = Some((target.center.0, target.center.1, target.zoom));
        }
    }

    pub fn tick_zoom_anim(&mut self) -> bool {
        let Some((tx, ty, tz)) = self.zoom_target else {
            return false;
        };
        let k = 0.25;
        self.view.center.0 += (tx - self.view.center.0) * k;
        self.view.center.1 += (ty - self.view.center.1) * k;
        self.view.zoom = (self.view.zoom.ln() + (tz.ln() - self.view.zoom.ln()) * k).exp();
        let dc = (tx - self.view.center.0).hypot(ty - self.view.center.1) * self.view.zoom;
        let dz = (tz / self.view.zoom).ln().abs();
        if dc < 0.5 && dz < 2e-3 {
            self.view.center = (tx, ty);
            self.view.zoom = tz;
            self.zoom_target = None;
            return false;
        }
        true
    }

    pub fn add_entity(&mut self, kind: EntityKind) -> EntityId {
        self.history.snapshot(&self.document);
        self.document.add(kind)
    }

    pub fn selected_nurbs(&self) -> Option<(EntityId, Vec<Point2d>, Vec<f64>)> {
        if self.selection.len() != 1 {
            return None;
        }
        let id = self.selection[0];
        if let EntityKind::Curve(Curve::Nurbs(nc)) = &self.document.get(id)?.kind {
            Some((id, nc.control.clone(), nc.weights.clone()))
        } else {
            None
        }
    }

    pub fn selected_nurbs_all(&self) -> Vec<(EntityId, Vec<Point2d>, Vec<f64>)> {
        self.selection
            .iter()
            .filter_map(|&id| match &self.document.get(id)?.kind {
                EntityKind::Curve(Curve::Nurbs(nc)) => {
                    Some((id, nc.control.clone(), nc.weights.clone()))
                }
                _ => None,
            })
            .collect()
    }

    pub fn begin_edit(&mut self) {
        self.history.snapshot(&self.document);
    }

    pub fn dim_override(&self, id: EntityId) -> Option<String> {
        match &self.document.get(id)?.kind {
            EntityKind::Dimension { override_text, .. }
            | EntityKind::OrthoDim { override_text, .. }
            | EntityKind::AngularDim { override_text, .. }
            | EntityKind::RadialDim { override_text, .. } => override_text.clone(),
            _ => None,
        }
    }

    pub fn set_dim_override(&mut self, id: EntityId, text: Option<String>) {
        let text = text.filter(|t| !t.trim().is_empty());
        if let Some(e) = self.document.get_mut(id) {
            match &mut e.kind {
                EntityKind::Dimension { override_text, .. }
                | EntityKind::OrthoDim { override_text, .. }
                | EntityKind::AngularDim { override_text, .. }
                | EntityKind::RadialDim { override_text, .. } => *override_text = text,
                _ => {}
            }
        }
    }

    pub fn commit_text_edit(
        &mut self,
        id: EntityId,
        content: String,
        font: Option<String>,
        size: f64,
    ) {
        let size = size.max(0.1);
        let changed = matches!(self.document.get(id).map(|e| &e.kind),
            Some(EntityKind::Text { content: c, font: f, height: h, .. })
                if *c != content || *f != font || (*h - size).abs() > 1e-9);
        if !changed {
            return;
        }
        self.history.snapshot(&self.document);
        if let Some(EntityKind::Text {
            content: c,
            font: f,
            height: h,
            ..
        }) = self.document.get_mut(id).map(|e| &mut e.kind)
        {
            *c = content;
            *f = font.clone();
            *h = size;
        }
        self.text_font = font;
    }

    pub fn outline_text_selection(&mut self) {
        let texts: Vec<EntityId> = self
            .selection
            .iter()
            .copied()
            .filter(|&id| {
                matches!(
                    self.document.get(id).map(|e| &e.kind),
                    Some(EntityKind::Text { .. })
                )
            })
            .collect();
        if texts.is_empty() {
            return;
        }
        self.history.snapshot(&self.document);
        let mut new_ids = Vec::new();
        for id in texts {
            let info = match self.document.get(id) {
                Some(e) => match &e.kind {
                    EntityKind::Text {
                        content,
                        font,
                        height,
                        anchor,
                        rotation,
                    } => Some((
                        content.clone(),
                        font.clone(),
                        *height,
                        *anchor,
                        *rotation,
                        e.layer,
                        e.color.clone(),
                    )),
                    _ => None,
                },
                None => None,
            };
            let Some((content, font, height, anchor, rotation, layer, color)) = info else {
                continue;
            };
            let curves =
                crate::fonts::outline_text(&content, font.as_deref(), height, anchor, rotation);
            if curves.is_empty() {
                continue;
            }

            for c in curves {
                let cid = self.document.add_on_layer(EntityKind::Curve(c), layer);
                if let Some(e) = self.document.get_mut(cid) {
                    e.color = color.clone();
                }
                new_ids.push(cid);
            }
            self.document.remove(id);
        }
        if !new_ids.is_empty() {
            self.selection = new_ids;
        }
    }

    pub fn set_nurbs_control(&mut self, id: EntityId, index: usize, p: Point2d) {
        if let Some(e) = self.document.get_mut(id)
            && let EntityKind::Curve(Curve::Nurbs(nc)) = &mut e.kind
            && index < nc.control.len()
        {
            nc.control[index] = p;
        }
    }

    pub fn adjust_nurbs_weight(&mut self, id: EntityId, index: usize, factor: f64) -> bool {
        let ok = matches!(self.document.get(id).map(|e| &e.kind),
            Some(EntityKind::Curve(Curve::Nurbs(nc))) if index < nc.weights.len());
        if !ok {
            return false;
        }
        self.history.snapshot(&self.document);
        if let Some(EntityKind::Curve(Curve::Nurbs(nc))) =
            self.document.get_mut(id).map(|e| &mut e.kind)
        {
            nc.weights[index] = (nc.weights[index] * factor).clamp(0.05, 20.0);
        }
        true
    }

    pub fn new_document(&mut self) {
        self.document = Document::new();
        seed_default_layers(&mut self.document);
        self.origin_id = self
            .document
            .add(EntityKind::Point(Point2d::from_i64(0, 0)));
        self.selection.clear();
        self.history = History::new();
        self.tool = Tool::Select;
        self.current_file_path = None;
        self.saved_depth = 0;
    }

    pub fn open_file(&mut self, path: std::path::PathBuf) {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let result = match ext.as_str() {
            "dxf" => std::fs::read_to_string(&path)
                .map(|t| eiderflat_io::import_dxf(&t))
                .map_err(|e| e.to_string()),
            "svg" => std::fs::read_to_string(&path)
                .map(|t| eiderflat_io::import_svg(&t))
                .map_err(|e| e.to_string()),
            "dwg" => Err("DWG is a proprietary binary format eiderFLAT can't read. \
                          Re-export it as DXF from your CAD app, then open the .dxf."
                .to_string()),
            _ => eiderflat_io::load_native(&path).map_err(|e| e.to_string()),
        };
        match result {
            Ok(mut doc) => {
                let origin_id = doc.add(EntityKind::Point(Point2d::from_i64(0, 0)));
                self.document = doc;
                self.origin_id = origin_id;
                self.selection.clear();
                self.history = History::new();
                self.tool = Tool::Select;
                self.current_file_path = Some(path);
                self.saved_depth = 0;
            }
            Err(e) => self.command_log.push(format!("Cannot open: {e}")),
        }
    }

    pub fn save_file(&mut self) -> bool {
        if let Some(path) = self.current_file_path.clone() {
            self.save_file_to(path)
        } else {
            false
        }
    }

    pub fn save_file_to(&mut self, path: std::path::PathBuf) -> bool {
        let mut save_doc = self.document.clone();
        save_doc.remove(self.origin_id);
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let result = match ext.as_str() {
            "dxf" => std::fs::write(&path, eiderflat_io::export_dxf(&save_doc))
                .map_err(|e| e.to_string()),
            "svg" => std::fs::write(&path, eiderflat_io::export_svg(&save_doc))
                .map_err(|e| e.to_string()),
            "dwg" => Err("eiderFLAT can't write DWG (proprietary binary). \
                          Save as DXF for CAD interchange."
                .to_string()),
            _ => eiderflat_io::save_native(&save_doc, &path).map_err(|e| e.to_string()),
        };
        match result {
            Ok(()) => {
                self.current_file_path = Some(path);
                self.saved_depth = self.history.undo_depth();
                true
            }
            Err(e) => {
                self.command_log.push(format!("Save failed: {e}"));
                false
            }
        }
    }

    pub fn is_dirty(&self) -> bool {
        self.history.undo_depth() != self.saved_depth
    }

    pub fn window_title(&self) -> String {
        let name = self
            .current_file_path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Untitled".to_string());
        let star = if self.is_dirty() { "*" } else { "" };
        format!("eiderFLAT — {name}{star}")
    }

    pub fn document_label(&self) -> String {
        let name = self
            .current_file_path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Untitled".to_string());
        let star = if self.is_dirty() { "*" } else { "" };
        format!("{name}{star}")
    }

    pub fn coord_readout(&self) -> String {
        format!("{:.4}, {:.4}", self.cursor_world.0, self.cursor_world.1)
    }

    pub fn current_layer_name(&self) -> &str {
        &self.document.layers.current_layer().name
    }

    pub fn units_label(&self) -> &'static str {
        match self.document.settings.units.short_name() {
            "" => "none",
            s => s,
        }
    }

    pub fn sync_zoom_limits(&mut self) {
        let (mn, mx) = self.document.settings.units.visible_range();
        self.view.set_visible_range(mn, mx);
    }

    pub fn begin_bbox_drag(&mut self, handle: BboxHandle, cursor: (f64, f64)) {
        if self.selection.is_empty() {
            return;
        }

        let mut bbox: Option<eiderflat_geometry::BoundingBox> = None;
        for &id in &self.selection {
            if let Some(e) = self.document.get(id)
                && let Some(b) = e.bounding_box()
            {
                bbox = Some(if let Some(existing) = bbox {
                    existing.union(&b)
                } else {
                    b
                });
            }
        }

        if let Some(bbox_start) = bbox {
            let originals: Vec<(EntityId, EntityKind)> = self
                .selection
                .iter()
                .filter_map(|&id| self.document.get(id).map(|e| (id, e.kind.clone())))
                .collect();
            self.interaction.bbox_drag = Some(BboxDrag {
                handle,
                bbox_start,
                cursor_start: cursor,
                originals,
            });
            self.history.snapshot(&self.document);
        }
    }

    pub fn end_bbox_drag(&mut self) {
        self.interaction.bbox_drag = None;
    }

    pub fn apply_bbox_drag_transform(&mut self, cursor: (f64, f64)) {
        let Some(drag) = self.interaction.bbox_drag.as_ref() else {
            return;
        };
        let ids: Vec<_> = self.selection.clone();

        for (id, kind) in &drag.originals {
            if let Some(e) = self.document.get_mut(*id) {
                e.kind = kind.clone();
            }
        }

        let (cx, cy) = cursor;
        let (sx, sy) = drag.cursor_start;
        let (dx, dy) = (cx - sx, cy - sy);

        let bbox = drag.bbox_start;
        let (bx0, by0) = (bbox.min.x, bbox.min.y);
        let (bx1, by1) = (bbox.max.x, bbox.max.y);

        match drag.handle {
            BboxHandle::Body => {
                edit::move_by(&mut self.document, &ids, dx, dy);
            }
            BboxHandle::CornerNW => {
                self.scale_bbox_from_opposite(&ids, bbox, cursor, (bx1, by1));
            }
            BboxHandle::CornerNE => {
                self.scale_bbox_from_opposite(&ids, bbox, cursor, (bx0, by1));
            }
            BboxHandle::CornerSW => {
                self.scale_bbox_from_opposite(&ids, bbox, cursor, (bx1, by0));
            }
            BboxHandle::CornerSE => {
                self.scale_bbox_from_opposite(&ids, bbox, cursor, (bx0, by0));
            }
            BboxHandle::RotateNW
            | BboxHandle::RotateNE
            | BboxHandle::RotateSW
            | BboxHandle::RotateSE => {
                let center = Point2d::from_f64((bx0 + bx1) / 2.0, (by0 + by1) / 2.0);
                let angle_start = (sy - center.y).atan2(sx - center.x);
                let angle_current = (cy - center.y).atan2(cx - center.x);
                let angle = angle_current - angle_start;

                if angle.abs() > 1e-9 {
                    edit::rotate(&mut self.document, &ids, &center, angle);
                }
            }
        }
    }

    fn scale_bbox_from_opposite(
        &mut self,
        ids: &[EntityId],
        bbox: eiderflat_geometry::BoundingBox,
        cursor: (f64, f64),
        opposite: (f64, f64),
    ) {
        let (cx, cy) = cursor;
        let (ox, oy) = opposite;
        let w = (cx - ox).abs();
        let h = (cy - oy).abs();
        let orig_w = (bbox.max.x - bbox.min.x).abs();
        let orig_h = (bbox.max.y - bbox.min.y).abs();

        if orig_w > 1e-9 && orig_h > 1e-9 {
            let sx = w / orig_w;
            let sy = h / orig_h;
            let s = sx.max(sy);

            if (s - 1.0).abs() > 1e-6 {
                let base = Point2d::from_f64(ox, oy);
                edit::scale(&mut self.document, ids, &base, s);
            }
        }
    }

    pub fn begin_grip_drag(&mut self, id: EntityId, grip: Grip) {
        if let Some(e) = self.document.get(id) {
            self.history.snapshot(&self.document);
            self.interaction.grip_drag = Some(GripDrag {
                entity_id: id,
                grip,
                start_kind: e.kind.clone(),
            });
        }
    }

    pub fn apply_grip_drag(&mut self, cursor: (f64, f64)) {
        let Some(drag) = self.interaction.grip_drag.as_ref() else {
            return;
        };
        let to = Point2d::from_f64(cursor.0, cursor.1);
        let edited = apply_grip(&drag.start_kind, &drag.grip, to);
        let id = drag.entity_id;
        if let Some(e) = self.document.get_mut(id) {
            e.kind = edited;
        }
        self.reconstrain_tangency(id);
    }

    fn reconstrain_tangency(&mut self, id: EntityId) {
        let Some(e) = self.document.get(id) else {
            return;
        };
        if e.tangents.is_empty() {
            return;
        }
        let Some(Curve::Arc(arc)) = e.as_curve() else {
            return;
        };
        let (center, radius) = (arc.center, arc.radius);
        let tangents = e.tangents.clone();
        let curves: Vec<Curve> = tangents
            .iter()
            .filter_map(|tr| {
                self.document
                    .get(tr.target)
                    .and_then(|t| t.as_curve())
                    .cloned()
            })
            .collect();
        if curves.len() != tangents.len() {
            return;
        }
        let solved = match curves.len() {
            3 => eiderflat_geometry::tangent_circle_ttt(&curves[0], &curves[1], &curves[2], center),
            2 => eiderflat_geometry::tangent_circle_ttr(&curves[0], &curves[1], radius, center),
            1 => {
                let r = eiderflat_geometry::point_to_curve_distance(&curves[0], center.x, center.y);
                (r > 1e-9).then_some((center, r))
            }
            _ => None,
        };
        if let Some((c, r)) = solved
            && r > 1e-9
            && let Some(e) = self.document.get_mut(id)
        {
            e.kind = EntityKind::Curve(Curve::Arc(eiderflat_geometry::CircularArc::new(
                c,
                r,
                0.0,
                std::f64::consts::TAU,
            )));
        }
    }

    pub fn remove_tangent(&mut self, id: EntityId, which: usize) {
        self.history.snapshot(&self.document);
        if let Some(e) = self.document.get_mut(id)
            && which < e.tangents.len()
        {
            e.tangents.remove(which);
        }
    }

    pub fn tangent_markers(&self, id: EntityId) -> Vec<(usize, Point2d)> {
        let Some(e) = self.document.get(id) else {
            return vec![];
        };
        let Some(Curve::Arc(arc)) = e.as_curve() else {
            return vec![];
        };
        e.tangents
            .iter()
            .enumerate()
            .filter_map(|(i, tr)| {
                let target = self.document.get(tr.target)?.as_curve()?;
                let (cx, cy) = arc.center.to_f64();
                let foot = eiderflat_geometry::project_point_onto_curve(target, cx, cy).point;
                let (fx, fy) = foot;
                let (dx, dy) = (fx - cx, fy - cy);
                let len = (dx * dx + dy * dy).sqrt();
                let tp = if len > 1e-9 {
                    Point2d::from_f64(cx + dx / len * arc.radius, cy + dy / len * arc.radius)
                } else {
                    Point2d::from_f64(fx, fy)
                };
                Some((i, tp))
            })
            .collect()
    }

    pub fn end_grip_drag(&mut self) {
        self.interaction.grip_drag = None;
    }

    pub fn cancel_grip_drag(&mut self) {
        if let Some(drag) = self.interaction.grip_drag.take() {
            if let Some(e) = self.document.get_mut(drag.entity_id) {
                e.kind = drag.start_kind;
            }
            self.history.discard_last();
        }
    }

    pub fn grip_editing(&self) -> bool {
        self.interaction.grip_drag.is_some()
    }

    pub fn grip_role(&self) -> Option<eiderflat_cad::GripRole> {
        self.interaction.grip_drag.as_ref().map(|d| d.grip.role)
    }

    pub fn commit_grip_value(&mut self, value: f64) {
        let Some(drag) = self.interaction.grip_drag.as_ref() else {
            return;
        };
        let to = Point2d::from_f64(self.cursor_world.0, self.cursor_world.1);
        let edited = eiderflat_cad::apply_grip_value(&drag.start_kind, &drag.grip, value, to);
        let id = drag.entity_id;
        if let Some(e) = self.document.get_mut(id) {
            e.kind = edited;
        }
        self.reconstrain_tangency(id);
        self.interaction.grip_drag = None;
    }

    pub fn selection_grips(&self) -> Vec<(EntityId, Grip)> {
        if !matches!(self.tool, Tool::Select) || self.interaction.corner_action.is_some() {
            return Vec::new();
        }
        let mut out = Vec::new();
        for &id in &self.selection {
            if let Some(e) = self.document.get(id) {
                for g in grips_for(&e.kind) {
                    out.push((id, g));
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eiderflat_geometry::{Curve, LineSeg};

    fn pt(x: i64, y: i64) -> Point2d {
        Point2d::from_i64(x, y)
    }

    fn app() -> AppState {
        AppState::new(800.0, 600.0)
    }

    fn line(x0: i64, y0: i64, x1: i64, y1: i64) -> EntityKind {
        EntityKind::Curve(Curve::Line(LineSeg::from_endpoints(pt(x0, y0), pt(x1, y1))))
    }

    #[test]
    fn polyline_closes_when_clicking_start_vertex() {
        let mut a = app();
        a.tool = Tool::Polyline { pts: Vec::new() };
        a.place_tool_point(pt(0, 0));
        a.place_tool_point(pt(10, 0));
        a.place_tool_point(pt(5, 8));
        a.place_tool_point(pt(0, 0));

        let poly = a
            .document
            .iter()
            .find_map(|e| match &e.kind {
                EntityKind::Curve(Curve::Poly(p)) => Some(p.clone()),
                _ => None,
            })
            .expect("a closed polycurve should have been created");
        assert_eq!(poly.segments.len(), 3);
        let first = poly.segments.first().unwrap().as_line().unwrap();
        let last = poly.segments.last().unwrap().as_line().unwrap();
        assert!(
            first.p0.dist_f64(&last.p1) < 1e-9,
            "ends must coincide (welded)"
        );
        assert!(matches!(a.tool, Tool::Polyline { ref pts } if pts.is_empty()));
    }

    #[test]
    fn polyline_does_not_close_on_a_non_start_point() {
        let mut a = app();
        a.tool = Tool::Polyline { pts: Vec::new() };
        a.place_tool_point(pt(0, 0));
        a.place_tool_point(pt(10, 0));
        a.place_tool_point(pt(5, 8));
        a.place_tool_point(pt(12, 8));
        assert!(matches!(a.tool, Tool::Polyline { ref pts } if pts.len() == 4));
        assert!(
            !a.document
                .iter()
                .any(|e| matches!(&e.kind, EntityKind::Curve(Curve::Poly(_))))
        );
    }

    #[test]
    fn tangent_markers_and_removal() {
        use eiderflat_geometry::CircularArc;
        let mut a = app();
        a.snap_on = false;
        let l1 = a.document.add(line(0, 0, 10, 0));
        let l2 = a.document.add(line(0, 0, 0, 10));
        let cid = a
            .document
            .add(EntityKind::Curve(Curve::Arc(CircularArc::new(
                Point2d::from_f64(2.0, 2.0),
                2.0,
                0.0,
                std::f64::consts::TAU,
            ))));
        if let Some(e) = a.document.get_mut(cid) {
            e.tangents = vec![
                eiderflat_document::TangentRef {
                    target: l1,
                    near: Point2d::from_f64(2.0, 0.0),
                },
                eiderflat_document::TangentRef {
                    target: l2,
                    near: Point2d::from_f64(0.0, 2.0),
                },
            ];
        }
        a.selection = vec![cid];
        let markers = a.tangent_markers(cid);
        assert_eq!(markers.len(), 2);
        for (_, p) in &markers {
            assert!((p.dist_f64(&Point2d::from_f64(2.0, 2.0)) - 2.0).abs() < 1e-6);
        }
        a.remove_tangent(cid, 0);
        assert_eq!(a.tangent_markers(cid).len(), 1);
    }

    #[test]
    fn clipboard_copy_paste_duplicates_at_cursor() {
        let mut a = app();
        let id = a.document.add(line(0, 0, 10, 0));
        a.selection = vec![id];
        assert_eq!(a.clipboard_copy(), 1);

        a.cursor_world = (50.0, 20.0);
        let before = a.document.len();
        a.clipboard_paste();
        assert_eq!(a.document.len(), before + 1);
        assert_eq!(a.selection.len(), 1, "pasted entity becomes the selection");

        let pasted = a.document.get(a.selection[0]).unwrap();
        if let EntityKind::Curve(Curve::Line(l)) = &pasted.kind {
            assert!((l.p0.x - 45.0).abs() < 1e-9 && (l.p0.y - 20.0).abs() < 1e-9);
            assert!((l.p1.x - 55.0).abs() < 1e-9 && (l.p1.y - 20.0).abs() < 1e-9);
        } else {
            panic!("expected a pasted line");
        }
    }

    #[test]
    fn clipboard_cut_removes_then_pastes() {
        let mut a = app();
        let id = a.document.add(line(0, 0, 2, 2));
        a.selection = vec![id];
        let with_entity = a.document.len();
        a.clipboard_cut();
        assert_eq!(a.document.len(), with_entity - 1, "cut removes the entity");
        a.cursor_world = (0.0, 0.0);
        a.clipboard_paste();
        assert_eq!(a.document.len(), with_entity, "paste restores one entity");
    }

    #[test]
    fn paste_with_empty_clipboard_is_noop() {
        let mut a = app();
        let before = a.document.len();
        a.clipboard_paste();
        assert_eq!(a.document.len(), before);
    }

    #[test]
    fn ui_prefs_round_trip() {
        let p = UiPrefs {
            snap_on: false,
            grid_on: true,
            grid_snap_on: true,
            ortho_on: true,
            polar_on: false,
            track_on: false,
            dyn_on: true,
            comb_on: true,
            comb_scale: 7.5,
            snap_px: 8.0,
            polar_step: 30.0,
            zoom_speed: 1.5,
            zoom_to_cursor: false,
            invert_zoom: true,
            crosshair: false,
            pick_box: 14.0,
            show_lineweights: false,
            lineweight_scale: 3.0,
            grid_dots: true,
            grid_major_every: 4,
            grid_minor_rgb: (20, 30, 40),
            grid_major_rgb: (50, 60, 70),
            text_font: Some("Arial".into()),
        };
        assert_eq!(UiPrefs::deserialize(&p.serialize()), p);
        let q = UiPrefs {
            text_font: None,
            ..Default::default()
        };
        assert_eq!(UiPrefs::deserialize(&q.serialize()).text_font, None);
    }

    #[test]
    fn dimension_lands_on_dimension_layer() {
        let mut a = app();
        a.tool = crate::tools::Tool::Dimension { p1: None, p2: None };
        a.place_tool_point(Point2d::from_f64(0.0, 0.0));
        a.place_tool_point(Point2d::from_f64(10.0, 0.0));
        a.place_tool_point(Point2d::from_f64(0.0, 3.0));
        let dim = a
            .document
            .iter()
            .find(|e| matches!(e.kind, eiderflat_document::EntityKind::Dimension { .. }))
            .expect("a dimension entity");
        let layer = a.document.layers.get(dim.layer).expect("its layer");
        assert_eq!(layer.name, eiderflat_document::DIMENSION_LAYER);
    }

    #[test]
    fn radial_dimension_tool_dimensions_a_circle() {
        let mut a = app();
        a.snap_on = false;
        let circle = a.document.add(EntityKind::Curve(Curve::Arc(
            eiderflat_geometry::CircularArc::new(
                Point2d::from_f64(0.0, 0.0),
                5.0,
                0.0,
                std::f64::consts::TAU,
            ),
        )));
        a.tool = crate::tools::Tool::DimRadial {
            diameter: false,
            center: None,
            radius: 0.0,
        };
        let (sx, sy) = a.view.world_to_screen(5.0, 0.0);
        a.canvas_click(sx, sy);
        assert!(
            matches!(a.tool, crate::tools::Tool::DimRadial { center: Some(_), radius, .. } if (radius - 5.0).abs() < 1e-9),
            "circle pick set centre+radius"
        );
        let (lx, ly) = a.view.world_to_screen(0.0, 6.0);
        a.canvas_click(lx, ly);
        let made = a
            .document
            .iter()
            .any(|e| matches!(&e.kind, EntityKind::RadialDim { center, .. } if *center == Point2d::from_f64(0.0, 0.0)));
        assert!(made, "radial dimension created on the circle");
        let _ = circle;
    }

    #[test]
    fn angular_from_two_lines_creates_dim_at_intersection() {
        let mut a = app();
        a.snap_on = false;
        a.document
            .add(EntityKind::Curve(Curve::Line(LineSeg::from_endpoints(
                Point2d::from_f64(0.0, 0.0),
                Point2d::from_f64(10.0, 0.0),
            ))));
        a.document
            .add(EntityKind::Curve(Curve::Line(LineSeg::from_endpoints(
                Point2d::from_f64(0.0, 0.0),
                Point2d::from_f64(0.0, 10.0),
            ))));
        a.tool = crate::tools::Tool::DimAngularLines {
            a: None,
            geom: None,
        };
        let (s1x, s1y) = a.view.world_to_screen(5.0, 0.0);
        a.canvas_click(s1x, s1y);
        let (s2x, s2y) = a.view.world_to_screen(0.0, 5.0);
        a.canvas_click(s2x, s2y);
        assert!(
            matches!(
                a.tool,
                crate::tools::Tool::DimAngularLines { geom: Some(_), .. }
            ),
            "two line picks produced the angle geometry"
        );
        let (lx, ly) = a.view.world_to_screen(3.0, 3.0);
        a.canvas_click(lx, ly);
        let dim = a
            .document
            .iter()
            .find_map(|e| match &e.kind {
                EntityKind::AngularDim { center, .. } => Some(*center),
                _ => None,
            })
            .expect("an angular dimension");
        assert!(
            dim.dist_f64(&Point2d::from_f64(0.0, 0.0)) < 1e-6,
            "vertex at intersection"
        );
    }

    #[test]
    fn apply_prefs_keeps_ortho_polar_exclusive() {
        let mut a = app();
        let p = UiPrefs {
            ortho_on: true,
            polar_on: true,
            ..Default::default()
        };
        a.apply_prefs(&p);
        assert!(a.ortho_on && !a.polar_on, "ortho should win over polar");
    }

    #[test]
    fn save_open_dispatches_by_extension() {
        for ext in ["e2d", "dxf", "svg"] {
            let mut a = app();
            a.document
                .add(EntityKind::Curve(Curve::Line(LineSeg::from_endpoints(
                    pt(0, 0),
                    pt(10, 5),
                ))));
            a.document.add(EntityKind::Curve(Curve::Arc(
                eiderflat_geometry::CircularArc::new(pt(3, 4), 5.0, 0.0, std::f64::consts::TAU),
            )));
            let want = a.document.iter().filter(|e| e.id != a.origin_id).count();

            let path = std::env::temp_dir()
                .join(format!("e2d_io_test_{}_{ext}.{ext}", std::process::id()));
            assert!(a.save_file_to(path.clone()), "save .{ext} should succeed");

            let mut b = app();
            b.open_file(path.clone());
            let got = b.document.iter().filter(|e| e.id != b.origin_id).count();
            assert_eq!(
                got, want,
                ".{ext} round-trip lost entities: {want} -> {got}"
            );
            let _ = std::fs::remove_file(path);
        }
    }

    #[test]
    fn line_command_then_two_clicks_creates_segment() {
        let mut a = app();
        a.run_command("LINE");
        assert_eq!(a.tool.name(), "LINE");
        let (s1x, s1y) = a.view.world_to_screen(0.0, 0.0);
        let (s2x, s2y) = a.view.world_to_screen(5.0, 0.0);
        a.snap_on = false;
        a.canvas_click(s1x, s1y);
        assert_eq!(a.document.len(), 1);
        a.canvas_click(s2x, s2y);
        assert_eq!(a.document.len(), 2);
    }

    #[test]
    fn undo_redo_through_state() {
        let mut a = app();
        a.add_entity(EntityKind::Curve(Curve::Line(LineSeg::from_endpoints(
            pt(0, 0),
            pt(1, 1),
        ))));
        assert_eq!(a.document.len(), 2);
        a.undo();
        assert_eq!(a.document.len(), 1);
        a.redo();
        assert_eq!(a.document.len(), 2);
    }

    #[test]
    fn erase_removes_selection() {
        let mut a = app();
        let id = a.add_entity(EntityKind::Curve(Curve::Line(LineSeg::from_endpoints(
            pt(0, 0),
            pt(2, 2),
        ))));
        a.selection = vec![id];
        a.run_command("ERASE");
        assert_eq!(a.document.len(), 1);
    }

    #[test]
    fn select_all_then_erase() {
        let mut a = app();
        a.add_entity(EntityKind::Curve(Curve::Line(LineSeg::from_endpoints(
            pt(0, 0),
            pt(1, 0),
        ))));
        a.add_entity(EntityKind::Curve(Curve::Line(LineSeg::from_endpoints(
            pt(0, 0),
            pt(0, 1),
        ))));
        a.run_command("ALL");
        assert_eq!(a.selection.len(), 2);
        a.run_command("ERASE");
        assert_eq!(a.document.len(), 1);
    }

    #[test]
    fn layer_commands() {
        let mut a = app();
        a.run_command("LAYER NEW walls");
        assert_eq!(a.current_layer_name(), "walls");
        a.run_command("LAYER SET 0");
        assert_eq!(a.current_layer_name(), "0");
    }

    #[test]
    fn move_command_uses_selection() {
        let mut a = app();
        let id = a.add_entity(EntityKind::Curve(Curve::Line(LineSeg::from_endpoints(
            pt(0, 0),
            pt(2, 0),
        ))));
        a.selection = vec![id];
        a.run_command("MOVE");
        a.snap_on = false;
        let (b1x, b1y) = a.view.world_to_screen(0.0, 0.0);
        let (b2x, b2y) = a.view.world_to_screen(10.0, 5.0);
        a.canvas_click(b1x, b1y);
        a.canvas_click(b2x, b2y);
        if let Some(Curve::Line(l)) = a.document.get(id).unwrap().as_curve() {
            assert!((l.p0.x - 10.0).abs() < 1e-4);
            assert!((l.p0.y - 5.0).abs() < 1e-4);
        } else {
            panic!()
        }
    }

    #[test]
    fn zoom_extents_frames_geometry() {
        let mut a = app();
        a.add_entity(EntityKind::Curve(Curve::Line(LineSeg::from_endpoints(
            pt(0, 0),
            pt(100, 80),
        ))));
        a.run_command("ZOOM E");
        for _ in 0..200 {
            if !a.tick_zoom_anim() {
                break;
            }
        }
        let (x0, y0, x1, y1) = a.view.visible_bounds();
        assert!(x0 <= 0.0 && x1 >= 100.0 && y0 <= 0.0 && y1 >= 80.0);
    }

    #[test]
    fn coord_readout_tracks_cursor() {
        let mut a = app();
        let (sx, sy) = a.view.world_to_screen(3.0, 7.0);
        a.pointer_moved(sx, sy);
        let r = a.coord_readout();
        assert!(r.starts_with("3.0000, 7.0000"));
    }

    #[test]
    fn perpendicular_snapping_uses_tool_reference_point() {
        let mut a = app();
        a.add_entity(EntityKind::Curve(Curve::Line(LineSeg::from_endpoints(
            pt(0, 0),
            pt(10, 0),
        ))));
        a.snap.enabled = vec![eiderflat_cad::SnapKind::Perpendicular];
        a.snap_on = true;

        a.run_command("LINE");

        let (s1x, s1y) = a.view.world_to_screen(3.0, 5.0);
        a.canvas_click(s1x, s1y);

        let (s2x, s2y) = a.view.world_to_screen(3.1, 0.1);
        a.pointer_moved(s2x, s2y);

        assert!(a.active_snap.is_some());
        let sp = a.active_snap.as_ref().unwrap();
        assert_eq!(sp.kind, eiderflat_cad::SnapKind::Perpendicular);
        assert!((sp.pos.0 - 3.0).abs() < 1e-4);
        assert!(sp.pos.1.abs() < 1e-4);
    }

    #[test]
    fn grid_snap_locks_cursor_to_grid_intersection() {
        let mut a = app();
        a.snap_on = false;
        a.grid_snap_on = true;
        a.run_command("LINE");

        let g = a.view.grid_spacing();
        let (sx, sy) = a.view.world_to_screen(2.0 * g + g * 0.2, -g - g * 0.1);
        a.pointer_moved(sx, sy);
        assert!(
            (a.cursor_world.0 - 2.0 * g).abs() < 1e-6,
            "x={}",
            a.cursor_world.0
        );
        assert!(
            (a.cursor_world.1 - (-g)).abs() < 1e-6,
            "y={}",
            a.cursor_world.1
        );
    }

    #[test]
    fn grip_drag_snaps_to_other_entity() {
        let mut a = app();
        a.snap_on = true;
        let l1 = a.add_entity(EntityKind::Curve(Curve::Line(LineSeg::from_endpoints(
            pt(0, 0),
            pt(10, 0),
        ))));
        a.add_entity(EntityKind::Curve(Curve::Line(LineSeg::from_endpoints(
            pt(5, 5),
            pt(20, 5),
        ))));
        a.selection = vec![l1];
        let grip = a
            .selection_grips()
            .into_iter()
            .find(|(id, _)| *id == l1)
            .map(|(_, g)| g)
            .expect("line should expose grips");
        a.begin_grip_drag(l1, grip);
        let (sx, sy) = a.view.world_to_screen(5.0, 5.0);
        a.pointer_moved(sx, sy);
        assert!(
            a.active_snap.is_some(),
            "expected a snap while grip-dragging"
        );
        assert!(
            (a.cursor_world.0 - 5.0).abs() < 1e-6 && (a.cursor_world.1 - 5.0).abs() < 1e-6,
            "cursor did not snap to the other entity: {:?}",
            a.cursor_world
        );
    }

    #[test]
    fn ortho_mode_constrains_cursor_to_axis() {
        let mut a = app();
        a.snap_on = false;
        a.ortho_on = true;

        a.run_command("LINE");
        let (s1x, s1y) = a.view.world_to_screen(0.0, 0.0);
        a.canvas_click(s1x, s1y);

        let (s2x, s2y) = a.view.world_to_screen(8.0, 3.0);
        a.pointer_moved(s2x, s2y);
        assert!((a.cursor_world.0 - 8.0).abs() < 1e-4);
        assert!(a.cursor_world.1.abs() < 1e-4);

        let (s3x, s3y) = a.view.world_to_screen(2.0, 9.0);
        a.pointer_moved(s3x, s3y);
        assert!(a.cursor_world.0.abs() < 1e-4);
        assert!((a.cursor_world.1 - 9.0).abs() < 1e-4);
    }

    #[test]
    fn perpendicular_snapping_triggers_anywhere_near_line() {
        let mut a = app();
        a.add_entity(EntityKind::Curve(Curve::Line(LineSeg::from_endpoints(
            pt(0, 0),
            pt(10, 0),
        ))));
        a.snap.enabled = vec![eiderflat_cad::SnapKind::Perpendicular];
        a.snap_on = true;

        a.run_command("LINE");
        let (s1x, s1y) = a.view.world_to_screen(5.0, 5.0);
        a.canvas_click(s1x, s1y);

        let (s2x, s2y) = a.view.world_to_screen(5.3, 0.1);
        a.pointer_moved(s2x, s2y);

        assert!(a.active_snap.is_some());
        let sp = a.active_snap.as_ref().unwrap();
        assert_eq!(sp.kind, eiderflat_cad::SnapKind::Perpendicular);
        assert!((sp.pos.0 - 5.0).abs() < 1e-4);
        assert!(sp.pos.1.abs() < 1e-4);
    }

    #[test]
    fn direct_distance_entry_projects_along_cursor() {
        let mut a = app();
        a.snap_on = false;
        a.run_command("LINE");

        let (s1x, s1y) = a.view.world_to_screen(0.0, 0.0);
        a.canvas_click(s1x, s1y);

        let (s2x, s2y) = a.view.world_to_screen(3.0, 4.0);
        a.pointer_moved(s2x, s2y);

        a.run_command("10.0");

        assert_eq!(a.document.len(), 2);
        let first = a.document.iter().find(|e| e.id != a.origin_id).unwrap();
        if let EntityKind::Curve(Curve::Line(l)) = &first.kind {
            assert!((l.p0.x - 0.0).abs() < 1e-4);
            assert!((l.p0.y - 0.0).abs() < 1e-4);
            assert!((l.p1.x - 6.0).abs() < 1e-4);
            assert!((l.p1.y - 8.0).abs() < 1e-4);
        } else {
            panic!("expected line");
        }
    }

    #[test]
    fn typed_coordinates_build_a_line() {
        let mut a = app();
        a.snap_on = false;
        a.run_command("LINE");
        a.run_command("0,0");
        a.run_command("@10,0");

        assert_eq!(a.document.len(), 2);
        let line = a.document.iter().find(|e| e.id != a.origin_id).unwrap();
        if let EntityKind::Curve(Curve::Line(l)) = &line.kind {
            assert!((l.p0.x).abs() < 1e-9 && (l.p0.y).abs() < 1e-9);
            assert!((l.p1.x - 10.0).abs() < 1e-9 && (l.p1.y).abs() < 1e-9);
        } else {
            panic!("expected line");
        }
    }

    #[test]
    fn relative_polar_coordinate_places_point() {
        let mut a = app();
        a.snap_on = false;
        a.run_command("LINE");
        a.run_command("0,0");
        a.run_command("@5<90");

        let line = a.document.iter().find(|e| e.id != a.origin_id).unwrap();
        if let EntityKind::Curve(Curve::Line(l)) = &line.kind {
            assert!((l.p1.x).abs() < 1e-6, "x should be ~0, got {}", l.p1.x);
            assert!(
                (l.p1.y - 5.0).abs() < 1e-6,
                "y should be ~5, got {}",
                l.p1.y
            );
        } else {
            panic!("expected line");
        }
    }

    #[test]
    fn right_click_repeat_reactivates_last_command() {
        let mut a = app();
        a.run_command("CIRCLE");
        assert!(matches!(a.tool, Tool::Circle { .. }));
        assert_eq!(a.last_command.as_deref(), Some("CIRCLE"));
        a.run_command("");
        assert!(matches!(a.tool, Tool::Select));
        a.repeat_last_command();
        assert!(matches!(a.tool, Tool::Circle { .. }));
    }

    #[test]
    fn polygon_command_allows_side_update() {
        let mut a = app();
        a.run_command("POLYGON");
        assert!(matches!(
            a.tool,
            Tool::Polygon {
                center: None,
                radius_point: None,
                sides: None
            }
        ));

        a.run_command("6");
        assert!(matches!(
            a.tool,
            Tool::Polygon {
                center: None,
                radius_point: None,
                sides: Some(6)
            }
        ));

        let (s1x, s1y) = a.view.world_to_screen(0.0, 0.0);
        a.canvas_click(s1x, s1y);

        let (s2x, s2y) = a.view.world_to_screen(10.0, 0.0);
        a.canvas_click(s2x, s2y);

        // Both clicks placed (center + radius), but the side-count popup is
        // now pending confirmation — nothing is created yet.
        assert_eq!(a.document.len(), 1);
        assert!(matches!(
            a.tool,
            Tool::Polygon {
                center: Some(_),
                radius_point: Some(_),
                sides: Some(6)
            }
        ));

        a.confirm_pending_polygon();
        assert_eq!(a.document.len(), 2);
        assert!(matches!(
            a.tool,
            Tool::Polygon {
                center: None,
                radius_point: None,
                ..
            }
        ));
    }

    #[test]
    fn polygon_cancel_pending_drops_without_committing() {
        let mut a = app();
        a.run_command("POLYGON");
        let (s1x, s1y) = a.view.world_to_screen(0.0, 0.0);
        a.canvas_click(s1x, s1y);
        let (s2x, s2y) = a.view.world_to_screen(10.0, 0.0);
        a.canvas_click(s2x, s2y);
        assert_eq!(a.document.len(), 1);

        a.cancel_pending_polygon();
        assert_eq!(a.document.len(), 1, "cancel must not create anything");
        assert!(matches!(
            a.tool,
            Tool::Polygon {
                center: None,
                radius_point: None,
                sides: Some(6)
            }
        ));
    }

    #[test]
    fn polyline_command_commits_on_empty_command() {
        let mut a = app();
        a.run_command("PL");
        assert!(matches!(a.tool, Tool::Polyline { .. }));

        let (s1x, s1y) = a.view.world_to_screen(0.0, 0.0);
        a.canvas_click(s1x, s1y);
        let (s2x, s2y) = a.view.world_to_screen(5.0, 5.0);
        a.canvas_click(s2x, s2y);
        let (s3x, s3y) = a.view.world_to_screen(10.0, 0.0);
        a.canvas_click(s3x, s3y);

        a.run_command("");
        assert!(matches!(a.tool, Tool::Select));
        assert_eq!(a.document.len(), 2);
    }

    #[test]
    fn cv_spline_command_commits_to_nurbs() {
        let mut a = app();
        a.run_command("SPLINE");
        assert!(matches!(a.tool, Tool::Spline { .. }));

        for (wx, wy) in [(0.0, 0.0), (5.0, 8.0), (10.0, -4.0), (15.0, 0.0)] {
            let (sx, sy) = a.view.world_to_screen(wx, wy);
            a.canvas_click(sx, sy);
        }
        a.run_command("");
        assert!(matches!(a.tool, Tool::Select));
        assert_eq!(a.document.len(), 2);

        let entity = a.document.iter().find(|e| e.id != a.origin_id).unwrap();
        match &entity.kind {
            EntityKind::Curve(Curve::Nurbs(nc)) => assert_eq!(nc.control.len(), 4),
            other => panic!("expected a NURBS curve, got {:?}", other),
        }
    }

    #[test]
    fn nurbs_grip_edit_moves_control_and_weight() {
        let mut a = app();
        let nc = eiderflat_geometry::NurbsCurve::uniform(vec![
            Point2d::from_i64(0, 0),
            Point2d::from_i64(2, 4),
            Point2d::from_i64(6, 4),
            Point2d::from_i64(8, 0),
            Point2d::from_i64(10, 4),
        ]);
        let id = a.add_entity(EntityKind::Curve(Curve::Nurbs(nc)));
        a.selection = vec![id];

        let (sid, control, weights) = a.selected_nurbs().expect("a NURBS is selected");
        assert_eq!(sid, id);
        assert_eq!(control.len(), 5);
        assert!(weights.iter().all(|&w| w == 1.0));

        a.begin_edit();
        a.set_nurbs_control(id, 2, Point2d::from_f64(6.0, 9.0));
        let weight_at = |a: &AppState, i: usize| {
            if let EntityKind::Curve(Curve::Nurbs(nc)) = &a.document.get(id).unwrap().kind {
                (nc.control[i], nc.weights[i])
            } else {
                panic!("expected NURBS")
            }
        };
        assert_eq!(weight_at(&a, 2).0, Point2d::from_f64(6.0, 9.0));
        assert!(a.adjust_nurbs_weight(id, 2, 5.0));
        assert!((weight_at(&a, 2).1 - 5.0).abs() < 1e-9);
        a.adjust_nurbs_weight(id, 2, 100.0);
        assert!(weight_at(&a, 2).1 <= 20.0 + 1e-9);
        a.undo();
        assert!(
            (weight_at(&a, 2).1 - 5.0).abs() < 1e-9,
            "undo restores the prior weight"
        );
    }

    #[test]
    fn polyline_command_closes_on_c_command() {
        let mut a = app();
        a.run_command("PL");

        let (s1x, s1y) = a.view.world_to_screen(0.0, 0.0);
        a.canvas_click(s1x, s1y);
        let (s2x, s2y) = a.view.world_to_screen(5.0, 5.0);
        a.canvas_click(s2x, s2y);
        let (s3x, s3y) = a.view.world_to_screen(10.0, 0.0);
        a.canvas_click(s3x, s3y);

        a.run_command("c");
        assert!(matches!(a.tool, Tool::Select));
        assert_eq!(a.document.len(), 2);

        let entity = a.document.iter().find(|e| e.id != a.origin_id).unwrap();
        if let EntityKind::Curve(Curve::Poly(poly)) = &entity.kind {
            assert_eq!(poly.segments.len(), 3);
        } else {
            panic!("expected PolyCurve");
        }
    }

    #[test]
    fn fixed_origin_test() {
        let mut a = app();
        if let Some(EntityKind::Point(p)) = a.document.get(a.origin_id).map(|e| &e.kind) {
            assert_eq!(p.to_f64(), (0.0, 0.0));
        } else {
            panic!("expected origin point");
        }

        a.toggle_selection(a.origin_id);
        assert!(!a.selection.contains(&a.origin_id));

        a.selection = vec![a.origin_id];
        a.erase_selection();
        assert!(a.document.get(a.origin_id).is_some());

        let t = eiderflat_geometry::Transform2d::translation(10.0, 10.0);
        let ev = ToolEvent::Transform {
            ids: vec![a.origin_id],
            t,
        };
        a.apply_tool_event(ev);
        if let Some(EntityKind::Point(p)) = a.document.get(a.origin_id).map(|e| &e.kind) {
            assert_eq!(p.to_f64(), (0.0, 0.0));
        } else {
            panic!("expected origin point");
        }
    }

    #[test]
    fn text_tool_places_text_entity() {
        let mut a = app();
        a.run_command("TEXT");
        assert!(matches!(a.tool, Tool::Text { anchor: None, .. }));
        let (sx, sy) = a.view.world_to_screen(2.0, 3.0);
        a.canvas_click(sx, sy);
        assert!(matches!(
            a.tool,
            Tool::Text {
                anchor: Some(_),
                ..
            }
        ));
        a.run_command("Hello\\nWorld");
        assert!(matches!(a.tool, Tool::Select));
        let content = a
            .document
            .iter()
            .find_map(|e| match &e.kind {
                EntityKind::Text { content, .. } => Some(content.clone()),
                _ => None,
            })
            .expect("a Text entity should be created");
        assert_eq!(
            content, "Hello\nWorld",
            "single unified tool handles multi-line via \\n"
        );
    }

    #[test]
    fn reconstrain_tangency_tolerates_deleted_target() {
        use eiderflat_document::TangentRef;
        use eiderflat_geometry::CircularArc;
        let mut a = app();
        let t1 = a.document.add(line(0, 0, 10, 0));
        let t2 = a.document.add(line(0, 10, 10, 10));
        let arc = a
            .document
            .add(EntityKind::Curve(Curve::Arc(CircularArc::new(
                pt(5, 5),
                1.0,
                0.0,
                std::f64::consts::TAU,
            ))));
        if let Some(e) = a.document.get_mut(arc) {
            e.tangents = vec![
                TangentRef {
                    target: t1,
                    near: pt(5, 0),
                },
                TangentRef {
                    target: t2,
                    near: pt(5, 10),
                },
            ];
        }
        // Drop one tangent target, leaving a dangling reference.
        a.document.remove(t2);
        // Must not panic, and must leave the arc untouched (cannot re-solve).
        a.reconstrain_tangency(arc);
        assert!(matches!(
            a.document.get(arc).and_then(|e| e.as_curve()),
            Some(Curve::Arc(_))
        ));
    }
}

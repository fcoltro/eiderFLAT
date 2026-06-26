use eiderflat_document::{
    Color, Document, Entity, EntityKind, HatchPattern, Layer, LineTypeDef, LineTypeRef, Units,
};
use eiderflat_geometry::{
    CircularArc, CubicBezier, Curve, EllipticalArc, LineSeg, NurbsCurve, Point2d, PolyCurve,
    RationalBezier,
};
use std::io::Write;

const TAU: f64 = std::f64::consts::TAU;
pub const MAGIC: &str = "E2D";
pub const VERSION: u32 = 1;

pub fn to_string(doc: &Document) -> String {
    let mut s = String::new();
    s.push_str(&format!("{} {}\n", MAGIC, VERSION));
    s.push_str(&format!("UNITS {}\n", units_name(doc.settings.units)));
    s.push_str(&format!("GRID {}\n", doc.settings.grid_spacing));
    s.push_str(&format!("SNAP {}\n", doc.settings.snap_spacing));
    let ds = &doc.settings.dim_style;
    s.push_str(&format!(
        "DIMSTYLE {} {} {} {}\n",
        ds.text_height,
        ds.arrow_size,
        ds.font.as_deref().map(esc).unwrap_or_else(|| "-".into()),
        ds.precision
    ));

    for lt in &doc.line_types {
        let pat: Vec<String> = lt.pattern.iter().map(|p| p.to_string()).collect();
        s.push_str(&format!("LT {} {}\n", esc(&lt.name), pat.join(",")));
    }
    for l in &doc.layers.layers {
        s.push_str(&format!(
            "LAYER {} {},{},{} {} {} {} {}\n",
            esc(&l.name),
            l.color.0,
            l.color.1,
            l.color.2,
            l.on as u8,
            l.frozen as u8,
            l.locked as u8,
            esc(&linetype_name(&l.line_type))
        ));
    }
    for e in doc.iter() {
        write_entity(&mut s, e);
    }
    s
}

pub fn save(doc: &Document, path: &std::path::Path) -> std::io::Result<()> {
    let data = to_string(doc);
    let tmp = path.with_extension("e2d.tmp");
    {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(data.as_bytes())?;
        f.sync_all()?;
    }
    std::fs::rename(&tmp, path)
}

fn write_entity(s: &mut String, e: &Entity) {
    let layer = e.layer;
    let color = color_str(&e.color);
    match &e.kind {
        EntityKind::Curve(Curve::Line(l)) => s.push_str(&format!(
            "E LINE {} {} {} {}\n",
            layer,
            color,
            pt(&l.p0),
            pt(&l.p1)
        )),
        EntityKind::Curve(Curve::Arc(a)) => {
            let (cx, cy) = (rat(a.center.x), rat(a.center.y));
            s.push_str(&format!(
                "E ARC {} {} {};{} {} {} {}\n",
                layer,
                color,
                cx,
                cy,
                rat(a.radius),
                a.start_angle,
                a.end_angle
            ));
        }
        EntityKind::Curve(Curve::Ellipse(el)) => s.push_str(&format!(
            "E ELLIPSE {} {} {};{} {} {} {} {} {}\n",
            layer,
            color,
            rat(el.center.x),
            rat(el.center.y),
            rat(el.semi_major),
            rat(el.semi_minor),
            el.rotation,
            el.start_angle,
            el.end_angle
        )),
        EntityKind::Curve(Curve::Bezier(b)) => s.push_str(&format!(
            "E BEZIER {} {} {} {} {} {}\n",
            layer,
            color,
            pt(&b.p0),
            pt(&b.p1),
            pt(&b.p2),
            pt(&b.p3)
        )),
        EntityKind::Curve(Curve::Rational(rb)) => s.push_str(&format!(
            "E RATIONAL {} {} {}\n",
            layer,
            color,
            control_fields(&rb.points, &rb.weights)
        )),
        EntityKind::Curve(Curve::Nurbs(nc)) => s.push_str(&format!(
            "E NURBS {} {} {}\n",
            layer,
            color,
            control_fields(&nc.control, &nc.weights)
        )),
        EntityKind::Curve(Curve::Poly(pc)) => {
            s.push_str(&format!(
                "E POLY {} {} {}\n",
                layer,
                color,
                pc.segments.len()
            ));
            for seg in &pc.segments {
                write_segment(s, seg);
            }
        }
        EntityKind::Hatch {
            boundary,
            holes,
            fill,
            pattern,
        } => {
            s.push_str(&format!(
                "E HATCH {} {} {},{},{} {} {} {}\n",
                layer,
                color,
                fill.0,
                fill.1,
                fill.2,
                boundary.len(),
                holes.len(),
                pattern_str(pattern)
            ));
            for seg in boundary {
                write_segment(s, seg);
            }
            for hole in holes {
                s.push_str(&format!("HOLE {}\n", hole.len()));
                for seg in hole {
                    write_segment(s, seg);
                }
            }
        }
        EntityKind::Point(p) => s.push_str(&format!("E POINT {} {} {}\n", layer, color, pt(p))),
        EntityKind::Text {
            anchor,
            content,
            height,
            rotation,
            font,
        } => s.push_str(&format!(
            "E TEXT {} {} {} {} {} {} {}\n",
            layer,
            color,
            pt(anchor),
            height,
            rotation,
            esc(content),
            font.as_deref().map(esc).unwrap_or_else(|| "-".into())
        )),
        EntityKind::Dimension {
            p1,
            p2,
            line,
            height,
            override_text,
        } => s.push_str(&format!(
            "E DIM {} {} {} {} {} {} {}\n",
            layer,
            color,
            pt(p1),
            pt(p2),
            pt(line),
            height,
            dim_override(override_text)
        )),
        EntityKind::OrthoDim {
            p1,
            p2,
            line,
            vertical,
            height,
            override_text,
        } => s.push_str(&format!(
            "E DIMORTHO {} {} {} {} {} {} {} {}\n",
            layer,
            color,
            pt(p1),
            pt(p2),
            pt(line),
            *vertical as u8,
            height,
            dim_override(override_text)
        )),
        EntityKind::AngularDim {
            center,
            p1,
            p2,
            line,
            height,
            override_text,
        } => s.push_str(&format!(
            "E DIMANG {} {} {} {} {} {} {} {}\n",
            layer,
            color,
            pt(center),
            pt(p1),
            pt(p2),
            pt(line),
            height,
            dim_override(override_text)
        )),
        EntityKind::RadialDim {
            center,
            edge,
            diameter,
            height,
            override_text,
        } => s.push_str(&format!(
            "E DIMRAD {} {} {} {} {} {} {}\n",
            layer,
            color,
            pt(center),
            pt(edge),
            *diameter as u8,
            height,
            dim_override(override_text)
        )),
        _ => {}
    }
}

fn write_segment(s: &mut String, seg: &Curve) {
    match seg {
        Curve::Line(l) => s.push_str(&format!("SEG LINE {} {}\n", pt(&l.p0), pt(&l.p1))),
        Curve::Arc(a) => s.push_str(&format!(
            "SEG ARC {};{} {} {} {}\n",
            rat(a.center.x),
            rat(a.center.y),
            rat(a.radius),
            a.start_angle,
            a.end_angle
        )),
        Curve::Bezier(b) => s.push_str(&format!(
            "SEG BEZIER {} {} {} {}\n",
            pt(&b.p0),
            pt(&b.p1),
            pt(&b.p2),
            pt(&b.p3)
        )),
        Curve::Rational(rb) => s.push_str(&format!(
            "SEG RATIONAL {}\n",
            control_fields(&rb.points, &rb.weights)
        )),
        Curve::Ellipse(e) => s.push_str(&format!(
            "SEG ELLIPSE {};{} {} {} {} {} {}\n",
            rat(e.center.x),
            rat(e.center.y),
            rat(e.semi_major),
            rat(e.semi_minor),
            e.rotation,
            e.start_angle,
            e.end_angle
        )),
        _ => s.push_str("SEG LINE 0;0 0;0\n"),
    }
}

fn control_fields(points: &[Point2d], weights: &[f64]) -> String {
    let mut out = points.len().to_string();
    for (p, w) in points.iter().zip(weights) {
        out.push_str(&format!(" {} {}", pt(p), rat(*w)));
    }
    out
}

pub fn from_string(text: &str) -> Result<Document, String> {
    let mut lines = text.lines().peekable();
    let header = lines.next().ok_or("empty file")?;
    let mut hp = header.split_whitespace();
    if hp.next() != Some(MAGIC) {
        return Err("not an E2D file".into());
    }
    let ver: u32 = hp
        .next()
        .and_then(|v| v.parse().ok())
        .ok_or("missing version")?;
    if ver > VERSION {
        return Err(format!("unsupported version {}", ver));
    }

    let mut doc = Document::new();
    doc.layers.layers.clear();
    doc.line_types.clear();

    while let Some(line) = lines.next() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut tok = line.split_whitespace();
        match tok.next() {
            Some("UNITS") => doc.settings.units = parse_units(tok.next().unwrap_or("")),
            Some("GRID") => {
                doc.settings.grid_spacing = tok.next().and_then(|v| v.parse().ok()).unwrap_or(10.0)
            }
            Some("SNAP") => {
                doc.settings.snap_spacing = tok.next().and_then(|v| v.parse().ok()).unwrap_or(1.0)
            }
            Some("DIMSTYLE") => {
                let ds = &mut doc.settings.dim_style;
                ds.text_height = tok.next().and_then(|v| v.parse().ok()).unwrap_or(2.5);
                ds.arrow_size = tok.next().and_then(|v| v.parse().ok()).unwrap_or(2.5);
                ds.font = match tok.next() {
                    None | Some("-") => None,
                    Some(t) => Some(unesc(t)),
                };
                // Precision is optional (absent in older files → keep the default).
                if let Some(p) = tok.next().and_then(|v| v.parse().ok()) {
                    ds.precision = p;
                }
            }
            Some("LT") => {
                if let Some(lt) = parse_lt(&mut tok) {
                    doc.line_types.push(lt);
                }
            }
            Some("LAYER") => {
                if let Some(l) = parse_layer(&mut tok) {
                    doc.layers.layers.push(l);
                }
            }
            Some("E") => {
                parse_entity(&mut tok, &mut lines, &mut doc);
            }
            _ => {}
        }
    }

    if doc.layers.layers.is_empty() {
        doc.layers.layers.push(Layer::new("0"));
    }
    Ok(doc)
}

/// Load from a file.
pub fn load(path: &std::path::Path) -> std::io::Result<Document> {
    let text = std::fs::read_to_string(path)?;
    from_string(&text).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

fn parse_entity<'a>(
    tok: &mut impl Iterator<Item = &'a str>,
    lines: &mut std::iter::Peekable<std::str::Lines>,
    doc: &mut Document,
) {
    let etype = match tok.next() {
        Some(t) => t,
        None => return,
    };
    let layer: usize = tok.next().and_then(|v| v.parse().ok()).unwrap_or(0);
    let color = parse_color(tok.next().unwrap_or("bylayer"));

    let kind = match etype {
        "LINE" => {
            let p0 = parse_pt(tok.next());
            let p1 = parse_pt(tok.next());
            Some(EntityKind::Curve(Curve::Line(LineSeg::from_endpoints(
                p0, p1,
            ))))
        }
        "ARC" => {
            let c = parse_pt(tok.next());
            let r = parse_num(tok.next().unwrap_or("1"));
            let start = tok.next().and_then(|v| v.parse().ok()).unwrap_or(0.0);
            let end = tok.next().and_then(|v| v.parse().ok()).unwrap_or(TAU);
            Some(EntityKind::Curve(Curve::Arc(CircularArc::new(
                c, r, start, end,
            ))))
        }
        "ELLIPSE" => {
            let c = parse_pt(tok.next());
            let major = parse_num(tok.next().unwrap_or("1"));
            let minor = parse_num(tok.next().unwrap_or("1"));
            let rot = tok.next().and_then(|v| v.parse().ok()).unwrap_or(0.0);
            let start = tok.next().and_then(|v| v.parse().ok()).unwrap_or(0.0);
            let end = tok.next().and_then(|v| v.parse().ok()).unwrap_or(TAU);
            Some(EntityKind::Curve(Curve::Ellipse(EllipticalArc::new(
                c, major, minor, rot, start, end,
            ))))
        }
        "BEZIER" => {
            let p0 = parse_pt(tok.next());
            let p1 = parse_pt(tok.next());
            let p2 = parse_pt(tok.next());
            let p3 = parse_pt(tok.next());
            Some(EntityKind::Curve(Curve::Bezier(CubicBezier::new(
                p0, p1, p2, p3,
            ))))
        }
        "RATIONAL" => parse_control_data(tok)
            .map(|(p, w)| EntityKind::Curve(Curve::Rational(RationalBezier::new(p, w)))),
        "NURBS" => parse_control_data(tok)
            .map(|(c, w)| EntityKind::Curve(Curve::Nurbs(NurbsCurve::new(c, w)))),
        "POINT" => Some(EntityKind::Point(parse_pt(tok.next()))),
        "TEXT" => {
            let anchor = parse_pt(tok.next());
            let height = tok.next().and_then(|v| v.parse().ok()).unwrap_or(1.0);
            let rotation = tok.next().and_then(|v| v.parse().ok()).unwrap_or(0.0);
            let content = unesc(tok.next().unwrap_or(""));
            // Optional trailing font token (absent in older files, "-" = default font).
            let font = match tok.next() {
                None | Some("-") => None,
                Some(t) => Some(unesc(t)),
            };
            Some(EntityKind::Text {
                anchor,
                content,
                height,
                rotation,
                font,
            })
        }
        "DIM" => {
            let p1 = parse_pt(tok.next());
            let p2 = parse_pt(tok.next());
            let line = parse_pt(tok.next());
            let height = tok.next().and_then(|v| v.parse().ok()).unwrap_or(2.5);
            let override_text = parse_dim_override(tok);
            Some(EntityKind::Dimension {
                p1,
                p2,
                line,
                height,
                override_text,
            })
        }
        "DIMORTHO" => {
            let p1 = parse_pt(tok.next());
            let p2 = parse_pt(tok.next());
            let line = parse_pt(tok.next());
            let vertical = tok.next().map(|v| v == "1").unwrap_or(false);
            let height = tok.next().and_then(|v| v.parse().ok()).unwrap_or(2.5);
            let override_text = parse_dim_override(tok);
            Some(EntityKind::OrthoDim {
                p1,
                p2,
                line,
                vertical,
                height,
                override_text,
            })
        }
        "DIMANG" => {
            let center = parse_pt(tok.next());
            let p1 = parse_pt(tok.next());
            let p2 = parse_pt(tok.next());
            let line = parse_pt(tok.next());
            let height = tok.next().and_then(|v| v.parse().ok()).unwrap_or(2.5);
            let override_text = parse_dim_override(tok);
            Some(EntityKind::AngularDim {
                center,
                p1,
                p2,
                line,
                height,
                override_text,
            })
        }
        "DIMRAD" => {
            let center = parse_pt(tok.next());
            let edge = parse_pt(tok.next());
            let diameter = tok.next().map(|v| v == "1").unwrap_or(false);
            let height = tok.next().and_then(|v| v.parse().ok()).unwrap_or(2.5);
            let override_text = parse_dim_override(tok);
            Some(EntityKind::RadialDim {
                center,
                edge,
                diameter,
                height,
                override_text,
            })
        }
        "POLY" => {
            let count: usize = tok.next().and_then(|v| v.parse().ok()).unwrap_or(0);
            let mut segs = Vec::new();
            for _ in 0..count {
                // Stop at end-of-input rather than spinning `count` times on a
                // truncated or corrupt file (a huge count must not hang the load).
                let Some(segline) = lines.next() else { break };
                if let Some(seg) = parse_segment(segline.trim()) {
                    segs.push(seg);
                }
            }
            Some(EntityKind::Curve(Curve::Poly(Box::new(PolyCurve::new(
                segs,
            )))))
        }
        "HATCH" => {
            let fill = parse_rgb_triple(tok.next().unwrap_or(""));
            let nb: usize = tok.next().and_then(|v| v.parse().ok()).unwrap_or(0);
            let nh: usize = tok.next().and_then(|v| v.parse().ok()).unwrap_or(0);
            let read_segs = |lines: &mut std::iter::Peekable<std::str::Lines>, n: usize| {
                let mut v = Vec::new();
                for _ in 0..n {
                    // Stop at end-of-input so a corrupt segment count can't hang.
                    let Some(line) = lines.next() else { break };
                    if let Some(seg) = parse_segment(line.trim()) {
                        v.push(seg);
                    }
                }
                v
            };
            let pattern = parse_pattern(tok.next());
            let boundary = read_segs(lines, nb);
            let mut holes = Vec::new();
            for _ in 0..nh {
                // each hole begins with a `HOLE <count>` line.
                let ns = lines
                    .next()
                    .and_then(|l| {
                        l.trim()
                            .strip_prefix("HOLE ")
                            .and_then(|c| c.trim().parse().ok())
                    })
                    .unwrap_or(0);
                holes.push(read_segs(lines, ns));
            }
            Some(EntityKind::Hatch {
                boundary,
                holes,
                fill,
                pattern,
            })
        }
        _ => None,
    };

    if let Some(k) = kind {
        let id = doc.add_on_layer(k, layer.min(doc.layers.layers.len().saturating_sub(1)));
        if let Some(e) = doc.get_mut(id) {
            e.color = color;
        }
    }
}

fn parse_segment(line: &str) -> Option<Curve> {
    let mut tok = line.split_whitespace();
    if tok.next() != Some("SEG") {
        return None;
    }
    match tok.next()? {
        "LINE" => Some(Curve::Line(LineSeg::from_endpoints(
            parse_pt(tok.next()),
            parse_pt(tok.next()),
        ))),
        "ARC" => {
            let c = parse_pt(tok.next());
            let r = parse_num(tok.next().unwrap_or("1"));
            let start = tok.next().and_then(|v| v.parse().ok()).unwrap_or(0.0);
            let end = tok.next().and_then(|v| v.parse().ok()).unwrap_or(TAU);
            Some(Curve::Arc(CircularArc::new(c, r, start, end)))
        }
        "BEZIER" => Some(Curve::Bezier(CubicBezier::new(
            parse_pt(tok.next()),
            parse_pt(tok.next()),
            parse_pt(tok.next()),
            parse_pt(tok.next()),
        ))),
        "RATIONAL" => {
            parse_control_data(&mut tok).map(|(p, w)| Curve::Rational(RationalBezier::new(p, w)))
        }
        "ELLIPSE" => {
            let c = parse_pt(tok.next());
            let major = parse_num(tok.next().unwrap_or("1"));
            let minor = parse_num(tok.next().unwrap_or("1"));
            let rot = tok.next().and_then(|v| v.parse().ok()).unwrap_or(0.0);
            let start = tok.next().and_then(|v| v.parse().ok()).unwrap_or(0.0);
            let end = tok.next().and_then(|v| v.parse().ok()).unwrap_or(TAU);
            Some(Curve::Ellipse(EllipticalArc::new(
                c, major, minor, rot, start, end,
            )))
        }
        _ => None,
    }
}

fn parse_control_data<'a, I: Iterator<Item = &'a str>>(
    tok: &mut I,
) -> Option<(Vec<Point2d>, Vec<f64>)> {
    let n: usize = tok.next().and_then(|v| v.parse().ok())?;
    // Cap the pre-allocation: a corrupt count must not trigger a huge allocation.
    // The real data is bounded by the tokens present on the line, so we stop as
    // soon as a control point token is missing.
    let mut points = Vec::with_capacity(n.min(1024));
    let mut weights = Vec::with_capacity(n.min(1024));
    for _ in 0..n {
        let Some(p) = tok.next() else { break };
        points.push(parse_pt(Some(p)));
        weights.push(parse_num(tok.next().unwrap_or("1")));
    }
    (points.len() >= 2 && points.len() == weights.len() && weights.iter().all(|&w| w > 0.0))
        .then_some((points, weights))
}

fn rat(v: f64) -> String {
    format!("{}", v)
}
fn pt(p: &Point2d) -> String {
    format!("{};{}", rat(p.x), rat(p.y))
}

fn parse_num(s: &str) -> f64 {
    if let Some((n, d)) = s.split_once('/') {
        let n: f64 = n.parse().unwrap_or(0.0);
        let d: f64 = d.parse().unwrap_or(1.0);
        if d != 0.0 { n / d } else { 0.0 }
    } else {
        s.parse().unwrap_or(0.0)
    }
}

fn parse_rgb_triple(s: &str) -> (u8, u8, u8) {
    let p: Vec<u8> = s.split(',').filter_map(|v| v.parse().ok()).collect();
    if p.len() == 3 {
        (p[0], p[1], p[2])
    } else {
        (128, 128, 128)
    }
}

fn parse_pt(s: Option<&str>) -> Point2d {
    let s = s.unwrap_or("0;0");
    let (x, y) = s.split_once(';').unwrap_or(("0", "0"));
    Point2d::new(parse_num(x), parse_num(y))
}

fn color_str(c: &Color) -> String {
    match c {
        Color::ByLayer => "bylayer".into(),
        Color::ByBlock => "byblock".into(),
        Color::Rgb(r, g, b) => format!("rgb:{}:{}:{}", r, g, b),
    }
}
fn parse_color(s: &str) -> Color {
    match s {
        "bylayer" => Color::ByLayer,
        "byblock" => Color::ByBlock,
        other => {
            if let Some(rest) = other.strip_prefix("rgb:") {
                let p: Vec<u8> = rest.split(':').filter_map(|v| v.parse().ok()).collect();
                if p.len() == 3 {
                    return Color::Rgb(p[0], p[1], p[2]);
                }
            }
            Color::ByLayer
        }
    }
}

fn parse_layer<'a>(tok: &mut impl Iterator<Item = &'a str>) -> Option<Layer> {
    let name = unesc(tok.next()?);
    let rgb: Vec<u8> = tok
        .next()?
        .split(',')
        .filter_map(|v| v.parse().ok())
        .collect();
    let on = tok.next()? == "1";
    let frozen = tok.next()? == "1";
    let locked = tok.next()? == "1";
    let lt = unesc(tok.next().unwrap_or("Continuous"));
    let mut l = Layer::new(name);
    if rgb.len() == 3 {
        l.color = (rgb[0], rgb[1], rgb[2]);
    }
    l.on = on;
    l.frozen = frozen;
    l.locked = locked;
    l.line_type = LineTypeRef::Named(lt);
    Some(l)
}

fn parse_lt<'a>(tok: &mut impl Iterator<Item = &'a str>) -> Option<LineTypeDef> {
    let name = unesc(tok.next()?);
    let pat: Vec<f64> = tok
        .next()
        .map(|s| s.split(',').filter_map(|v| v.parse().ok()).collect())
        .unwrap_or_default();
    Some(LineTypeDef {
        name,
        description: String::new(),
        pattern: pat,
    })
}

fn linetype_name(lt: &LineTypeRef) -> String {
    match lt {
        LineTypeRef::Named(n) => n.clone(),
        LineTypeRef::ByLayer => "ByLayer".into(),
        LineTypeRef::ByBlock => "ByBlock".into(),
    }
}

fn units_name(u: Units) -> &'static str {
    match u {
        Units::Unitless => "Unitless",
        Units::Millimeters => "Millimeters",
        Units::Centimeters => "Centimeters",
        Units::Meters => "Meters",
        Units::Kilometers => "Kilometers",
        Units::Inches => "Inches",
        Units::Feet => "Feet",
    }
}
fn parse_units(s: &str) -> Units {
    match s {
        "Centimeters" => Units::Centimeters,
        "Meters" => Units::Meters,
        "Kilometers" => Units::Kilometers,
        "Inches" => Units::Inches,
        "Feet" => Units::Feet,
        "Unitless" => Units::Unitless,
        _ => Units::Millimeters,
    }
}

fn pattern_str(p: &HatchPattern) -> String {
    match p {
        HatchPattern::Solid => "solid".into(),
        HatchPattern::Lines { angle_deg, spacing } => format!("lines:{angle_deg}:{spacing}"),
        HatchPattern::Cross { angle_deg, spacing } => format!("cross:{angle_deg}:{spacing}"),
        HatchPattern::Dots { spacing } => format!("dots:{spacing}"),
    }
}

fn parse_pattern(tok: Option<&str>) -> HatchPattern {
    let s = match tok {
        Some(s) => s,
        None => return HatchPattern::Solid,
    };
    let mut it = s.split(':');
    match it.next() {
        Some("lines") => {
            let a = it.next().and_then(|v| v.parse().ok()).unwrap_or(45.0);
            let sp = it.next().and_then(|v| v.parse().ok()).unwrap_or(1.0);
            HatchPattern::Lines {
                angle_deg: a,
                spacing: sp,
            }
        }
        Some("cross") => {
            let a = it.next().and_then(|v| v.parse().ok()).unwrap_or(45.0);
            let sp = it.next().and_then(|v| v.parse().ok()).unwrap_or(1.0);
            HatchPattern::Cross {
                angle_deg: a,
                spacing: sp,
            }
        }
        Some("dots") => {
            let sp = it.next().and_then(|v| v.parse().ok()).unwrap_or(1.0);
            HatchPattern::Dots { spacing: sp }
        }
        _ => HatchPattern::Solid,
    }
}

fn esc(s: &str) -> String {
    if s.is_empty() {
        return "_".into();
    }
    s.replace('\\', "\\\\").replace(' ', "\\s")
}

/// Serialise an optional dimension text override: `-` for none, else escaped.
fn dim_override(o: &Option<String>) -> String {
    match o {
        None => "-".into(),
        Some(t) => esc(t),
    }
}

/// Parse a trailing dimension override token (`-`/absent → none).
fn parse_dim_override<'a>(tok: &mut impl Iterator<Item = &'a str>) -> Option<String> {
    match tok.next() {
        None | Some("-") => None,
        Some(t) => Some(unesc(t)),
    }
}
fn unesc(s: &str) -> String {
    if s == "_" {
        return String::new();
    }
    s.replace("\\s", " ").replace("\\\\", "\\")
}

#[cfg(test)]
mod tests {
    use super::*;
    use eiderflat_document::{EntityKind, HatchPattern};

    fn pt_i(x: i64, y: i64) -> Point2d {
        Point2d::from_i64(x, y)
    }

    #[test]
    fn roundtrip_f64_is_lossless() {
        let mut doc = Document::new();
        let third = 1.0 / 3.0;
        doc.add(EntityKind::Curve(Curve::Line(LineSeg::from_endpoints(
            Point2d::new(third, 0.0),
            Point2d::new(2.0, third),
        ))));

        let text = to_string(&doc);
        let doc2 = from_string(&text).unwrap();
        let es: Vec<_> = doc2.iter().collect();
        if let Some(Curve::Line(l)) = es[0].as_curve() {
            assert_eq!(l.p0.x, third);
            assert_eq!(l.p1.y, third);
        } else {
            panic!()
        }
    }

    #[test]
    fn roundtrip_layers_and_settings() {
        let mut doc = Document::new();
        doc.settings.units = Units::Inches;
        doc.layers.add(Layer::new("walls").with_color(255, 0, 0));
        let mut frozen = Layer::new("hidden");
        frozen.frozen = true;
        doc.layers.add(frozen);

        let doc2 = from_string(&to_string(&doc)).unwrap();
        assert_eq!(doc2.settings.units, Units::Inches);
        let w = doc2
            .layers
            .get(doc2.layers.index_of("walls").unwrap())
            .unwrap();
        assert_eq!(w.color, (255, 0, 0));
        assert!(
            doc2.layers
                .get(doc2.layers.index_of("hidden").unwrap())
                .unwrap()
                .frozen
        );
    }

    #[test]
    fn roundtrip_all_entity_types() {
        let mut doc = Document::new();
        doc.add(EntityKind::Curve(Curve::Line(LineSeg::from_endpoints(
            pt_i(0, 0),
            pt_i(5, 5),
        ))));
        doc.add(EntityKind::Curve(Curve::Arc(CircularArc::new(
            pt_i(3, 4),
            5.0,
            0.0,
            TAU,
        ))));
        doc.add(EntityKind::Curve(Curve::Bezier(CubicBezier::new(
            pt_i(0, 0),
            pt_i(1, 2),
            pt_i(3, 2),
            pt_i(4, 0),
        ))));
        doc.add(EntityKind::Point(pt_i(7, 8)));
        doc.add(EntityKind::Text {
            anchor: pt_i(1, 1),
            content: "hello world".into(),
            height: 2.5,
            rotation: 0.0,
            font: Some("Arial".into()),
        });

        let doc2 = from_string(&to_string(&doc)).unwrap();
        assert_eq!(doc2.len(), 5);
        let has_text = doc2.iter().any(
            |e| matches!(&e.kind, EntityKind::Text { content, .. } if content == "hello world"),
        );
        assert!(has_text);
    }

    #[test]
    fn roundtrip_dim_style() {
        let mut doc = Document::new();
        doc.settings.dim_style.text_height = 3.5;
        doc.settings.dim_style.arrow_size = 1.25;
        doc.settings.dim_style.font = Some("Arial".into());
        let doc2 = from_string(&to_string(&doc)).unwrap();
        assert!((doc2.settings.dim_style.text_height - 3.5).abs() < 1e-9);
        assert!((doc2.settings.dim_style.arrow_size - 1.25).abs() < 1e-9);
        assert_eq!(doc2.settings.dim_style.font.as_deref(), Some("Arial"));
    }

    #[test]
    fn roundtrip_dimension() {
        let mut doc = Document::new();
        doc.add(EntityKind::Dimension {
            p1: pt_i(0, 0),
            p2: pt_i(10, 0),
            line: pt_i(0, 3),
            height: 2.5,
            override_text: None,
        });
        let doc2 = from_string(&to_string(&doc)).unwrap();
        let e = doc2.iter().next().expect("one entity");
        match &e.kind {
            EntityKind::Dimension {
                p1,
                p2,
                line,
                height,
                ..
            } => {
                assert_eq!(*p1, pt_i(0, 0));
                assert_eq!(*p2, pt_i(10, 0));
                assert_eq!(*line, pt_i(0, 3));
                assert!((*height - 2.5).abs() < 1e-9);
            }
            other => panic!("expected a Dimension, got {other:?}"),
        }
    }

    #[test]
    fn roundtrip_angular_and_radial_dimensions() {
        let mut doc = Document::new();
        doc.add(EntityKind::AngularDim {
            center: pt_i(0, 0),
            p1: pt_i(10, 0),
            p2: pt_i(0, 10),
            line: pt_i(5, 5),
            height: 2.5,
            override_text: None,
        });
        doc.add(EntityKind::RadialDim {
            center: pt_i(3, 4),
            edge: pt_i(8, 4),
            diameter: true,
            height: 2.5,
            override_text: None,
        });
        let doc2 = from_string(&to_string(&doc)).unwrap();
        assert_eq!(doc2.len(), 2);
        let ang = doc2
            .iter()
            .any(|e| matches!(&e.kind, EntityKind::AngularDim { center, .. } if *center == pt_i(0, 0)));
        let rad = doc2.iter().any(
            |e| matches!(&e.kind, EntityKind::RadialDim { diameter, edge, .. } if *diameter && *edge == pt_i(8, 4)),
        );
        assert!(ang, "angular dimension survived the round-trip");
        assert!(rad, "radial (diameter) dimension survived the round-trip");
    }

    #[test]
    fn roundtrip_dimension_text_override() {
        let mut doc = Document::new();
        doc.add(EntityKind::Dimension {
            p1: pt_i(0, 0),
            p2: pt_i(10, 0),
            line: pt_i(0, 3),
            height: 2.5,
            override_text: Some("≈ 10 cm".into()),
        });
        let doc2 = from_string(&to_string(&doc)).unwrap();
        let ovr = doc2.iter().find_map(|e| match &e.kind {
            EntityKind::Dimension { override_text, .. } => override_text.clone(),
            _ => None,
        });
        assert_eq!(ovr.as_deref(), Some("≈ 10 cm"), "override text survives with spaces");
    }

    #[test]
    fn roundtrip_dimstyle_precision() {
        let mut doc = Document::new();
        doc.settings.dim_style.precision = 4;
        let doc2 = from_string(&to_string(&doc)).unwrap();
        assert_eq!(doc2.settings.dim_style.precision, 4);
    }

    #[test]
    fn roundtrip_rational_is_lossless() {
        let mut doc = Document::new();
        let rb = RationalBezier::new(
            vec![pt_i(0, 0), pt_i(2, 4), pt_i(6, 4), pt_i(8, 0)],
            vec![1.0, 2.0, 0.5, 1.0],
        );
        doc.add(EntityKind::Curve(Curve::Rational(rb.clone())));

        let doc2 = from_string(&to_string(&doc)).unwrap();
        let e = doc2.iter().next().expect("one entity");
        if let EntityKind::Curve(Curve::Rational(r2)) = &e.kind {
            assert_eq!(r2.points, rb.points, "control points must survive exactly");
            assert_eq!(r2.weights, rb.weights, "weights must survive exactly");
        } else {
            panic!("expected a Rational curve after round-trip");
        }
    }

    #[test]
    fn roundtrip_polycurve_of_rational_segments() {
        let seg = || {
            RationalBezier::new(
                vec![pt_i(0, 0), pt_i(1, 2), pt_i(3, 2), pt_i(4, 0)],
                vec![1.0, 1.0, 1.0, 1.0],
            )
        };
        let mut doc = Document::new();
        doc.add(EntityKind::Curve(Curve::Poly(Box::new(PolyCurve::new(
            vec![Curve::Rational(seg()), Curve::Rational(seg())],
        )))));

        let doc2 = from_string(&to_string(&doc)).unwrap();
        let e = doc2.iter().next().expect("one entity");
        if let EntityKind::Curve(Curve::Poly(pc)) = &e.kind {
            assert_eq!(pc.segments.len(), 2);
            assert!(pc.segments.iter().all(|s| matches!(s, Curve::Rational(_))));
        } else {
            panic!("expected a PolyCurve of rational segments");
        }
    }

    #[test]
    fn roundtrip_nurbs_is_lossless() {
        let mut doc = Document::new();
        let nc = NurbsCurve::new(
            vec![pt_i(0, 0), pt_i(2, 5), pt_i(6, 5), pt_i(9, 0), pt_i(12, 4)],
            vec![1.0, 2.0, 0.5, 1.0, 3.0],
        );
        doc.add(EntityKind::Curve(Curve::Nurbs(nc.clone())));

        let doc2 = from_string(&to_string(&doc)).unwrap();
        let e = doc2.iter().next().expect("one entity");
        if let EntityKind::Curve(Curve::Nurbs(n2)) = &e.kind {
            assert_eq!(
                n2.control, nc.control,
                "control vertices must survive exactly"
            );
            assert_eq!(n2.weights, nc.weights, "weights must survive exactly");
        } else {
            panic!("expected a NURBS curve after round-trip");
        }
    }

    #[test]
    fn roundtrip_polycurve() {
        let mut doc = Document::new();
        let segs = vec![
            Curve::Line(LineSeg::from_endpoints(pt_i(0, 0), pt_i(4, 0))),
            Curve::Arc(CircularArc::new(
                pt_i(4, 2),
                2.0,
                -std::f64::consts::FRAC_PI_2,
                std::f64::consts::FRAC_PI_2,
            )),
        ];
        doc.add(EntityKind::Curve(Curve::Poly(Box::new(PolyCurve::new(
            segs,
        )))));
        let doc2 = from_string(&to_string(&doc)).unwrap();
        let es: Vec<_> = doc2.iter().collect();
        if let Some(Curve::Poly(pc)) = es[0].as_curve() {
            assert_eq!(pc.segments.len(), 2);
        } else {
            panic!()
        }
    }

    #[test]
    fn roundtrip_hatch() {
        let mut doc = Document::new();
        doc.add(EntityKind::Hatch {
            boundary: vec![
                Curve::Line(LineSeg::from_endpoints(pt_i(0, 0), pt_i(6, 0))),
                Curve::Line(LineSeg::from_endpoints(pt_i(6, 0), pt_i(6, 6))),
                Curve::Line(LineSeg::from_endpoints(pt_i(6, 6), pt_i(0, 6))),
                Curve::Line(LineSeg::from_endpoints(pt_i(0, 6), pt_i(0, 0))),
            ],
            holes: vec![vec![
                Curve::Line(LineSeg::from_endpoints(pt_i(2, 2), pt_i(4, 2))),
                Curve::Line(LineSeg::from_endpoints(pt_i(4, 2), pt_i(4, 4))),
                Curve::Line(LineSeg::from_endpoints(pt_i(4, 4), pt_i(2, 4))),
                Curve::Line(LineSeg::from_endpoints(pt_i(2, 4), pt_i(2, 2))),
            ]],
            fill: (10, 200, 150),
            pattern: HatchPattern::Lines {
                angle_deg: 30.0,
                spacing: 1.5,
            },
        });
        let doc2 = from_string(&to_string(&doc)).unwrap();
        let es: Vec<_> = doc2.iter().collect();
        match &es[0].kind {
            EntityKind::Hatch {
                boundary,
                holes,
                fill,
                pattern,
            } => {
                assert_eq!(boundary.len(), 4);
                assert_eq!(holes.len(), 1);
                assert_eq!(holes[0].len(), 4);
                assert_eq!(*fill, (10, 200, 150));
                assert_eq!(
                    *pattern,
                    HatchPattern::Lines {
                        angle_deg: 30.0,
                        spacing: 1.5
                    }
                );
            }
            other => panic!("expected Hatch, got {other:?}"),
        }
    }

    #[test]
    fn corrupt_counts_do_not_hang_or_oom() {
        // A control-point count far larger than the data present must not trigger a
        // huge allocation or spin: parsing stops at the end of the available tokens.
        let nurbs = format!("{} {}\nE NURBS 0 bylayer 100000000 0;0 1 5;5 1\n", MAGIC, VERSION);
        let doc = from_string(&nurbs).expect("loads without hanging");
        assert_eq!(doc.len(), 1);

        // A POLY/HATCH segment count larger than the lines present must likewise
        // stop at end-of-input rather than looping the full declared count.
        let poly = format!("{} {}\nE POLY 0 bylayer 100000000\nSEG LINE 0;0 4;0\n", MAGIC, VERSION);
        let doc = from_string(&poly).expect("loads without hanging");
        assert_eq!(doc.len(), 1);
    }

    #[test]
    fn rejects_bad_header() {
        assert!(from_string("NOPE 1\n").is_err());
        assert!(from_string("").is_err());
    }

    #[test]
    fn save_and_load_file_atomic() {
        let mut doc = Document::new();
        doc.add(EntityKind::Curve(Curve::Line(LineSeg::from_endpoints(
            pt_i(1, 2),
            pt_i(3, 4),
        ))));
        let dir = std::env::temp_dir();
        let path = dir.join("eiderflat_native_test.e2d");
        save(&doc, &path).unwrap();
        assert!(path.exists());
        let doc2 = load(&path).unwrap();
        assert_eq!(doc2.len(), 1);
        let _ = std::fs::remove_file(&path);
    }
}

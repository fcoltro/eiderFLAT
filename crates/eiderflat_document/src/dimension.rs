//! Shared dimension geometry and labelling — the single source of truth for the
//! computations that the on-canvas renderer and the DXF/SVG exporters both need.
//! Keeping the angular-sweep selection and the measured-value/label logic here
//! avoids the two diverging (they previously each reimplemented it).

use crate::{DimStyle, EntityKind, Units};
use eiderflat_geometry::{Point2d, wrap_pi};

/// The resolved arc of an angular dimension: where the dimension arc starts, how
/// far it sweeps (signed), and its radius. The sector containing `line` is the
/// one that gets labelled, so reflex angles work.
#[derive(Clone, Copy, Debug)]
pub struct AngularSweep {
    pub start: f64,
    pub sweep: f64,
    pub radius: f64,
}

/// Compute the labelled sector for an angular dimension with vertex `center`,
/// ray points `p1`/`p2`, and arc-placement point `line`.
pub fn angular_sweep(center: Point2d, p1: Point2d, p2: Point2d, line: Point2d) -> AngularSweep {
    let (cx, cy) = center.to_f64();
    let ang = |p: Point2d| {
        let (x, y) = p.to_f64();
        (y - cy).atan2(x - cx)
    };
    let start = ang(p1);
    let mut sweep = wrap_pi(ang(p2) - start);
    // If `line` sits in the opposite (reflex) sector, label that one instead.
    let d = wrap_pi(ang(line) - start);
    let within =
        (sweep >= 0.0 && (0.0..=sweep).contains(&d)) || (sweep < 0.0 && (sweep..=0.0).contains(&d));
    if !within {
        sweep = if sweep >= 0.0 {
            sweep - std::f64::consts::TAU
        } else {
            sweep + std::f64::consts::TAU
        };
    }
    let radius = line.dist_f64(&center).max(1e-6);
    AngularSweep {
        start,
        sweep,
        radius,
    }
}

/// The numeric quantity a dimension measures: length for linear/ortho, degrees
/// for angular, radius for radial, diameter for a radial flagged `diameter`.
/// `None` for non-dimension entities.
pub fn measured_value(kind: &EntityKind) -> Option<f64> {
    Some(match kind {
        EntityKind::Dimension { p1, p2, .. } => p1.dist_f64(p2),
        EntityKind::OrthoDim {
            p1, p2, vertical, ..
        } => {
            let (a, b) = (p1.to_f64(), p2.to_f64());
            if *vertical {
                (b.1 - a.1).abs()
            } else {
                (b.0 - a.0).abs()
            }
        }
        EntityKind::AngularDim {
            center, p1, p2, line, ..
        } => angular_sweep(*center, *p1, *p2, *line).sweep.abs().to_degrees(),
        EntityKind::RadialDim {
            center,
            edge,
            diameter,
            ..
        } => {
            let r = center.dist_f64(edge);
            if *diameter { 2.0 * r } else { r }
        }
        _ => return None,
    })
}

/// The text a dimension should display: its user override when set, otherwise the
/// measured value formatted with the document units and the style's precision
/// (radial values carry an `R`/`⌀` prefix, angular a `°` suffix).
pub fn label_text(kind: &EntityKind, style: &DimStyle, units: Units) -> Option<String> {
    let ovr = override_text(kind);
    if let Some(t) = ovr {
        return Some(t.to_string());
    }
    let value = measured_value(kind)?;
    Some(match kind {
        EntityKind::AngularDim { .. } => format!("{value:.*}\u{00b0}", style.precision),
        EntityKind::RadialDim { diameter, .. } => {
            let prefix = if *diameter { "\u{2300}" } else { "R" };
            format!("{prefix}{}", units.format_measure(value, style.precision))
        }
        _ => units.format_measure(value, style.precision),
    })
}

/// Borrow a dimension's text override, if any.
pub fn override_text(kind: &EntityKind) -> Option<&str> {
    match kind {
        EntityKind::Dimension { override_text, .. }
        | EntityKind::OrthoDim { override_text, .. }
        | EntityKind::AngularDim { override_text, .. }
        | EntityKind::RadialDim { override_text, .. } => override_text.as_deref(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> Point2d {
        Point2d::from_f64(x, y)
    }

    #[test]
    fn right_angle_sweep_is_90_degrees() {
        // Rays +X and +Y, arc placed in the first quadrant → 90°.
        let s = angular_sweep(p(0.0, 0.0), p(10.0, 0.0), p(0.0, 10.0), p(3.0, 3.0));
        assert!((s.sweep.abs().to_degrees() - 90.0).abs() < 1e-6);
    }

    #[test]
    fn reflex_side_selects_270() {
        // Same rays, but arc placed in the opposite sector → 270°.
        let s = angular_sweep(p(0.0, 0.0), p(10.0, 0.0), p(0.0, 10.0), p(-3.0, -3.0));
        assert!((s.sweep.abs().to_degrees() - 270.0).abs() < 1e-6);
    }

    #[test]
    fn label_prefers_override() {
        let kind = EntityKind::RadialDim {
            center: p(0.0, 0.0),
            edge: p(5.0, 0.0),
            diameter: true,
            height: 2.5,
            override_text: Some("custom".into()),
        };
        assert_eq!(
            label_text(&kind, &DimStyle::default(), Units::Millimeters).as_deref(),
            Some("custom")
        );
    }

    #[test]
    fn radial_label_has_prefix_and_units() {
        let kind = EntityKind::RadialDim {
            center: p(0.0, 0.0),
            edge: p(5.0, 0.0),
            diameter: false,
            height: 2.5,
            override_text: None,
        };
        let label = label_text(&kind, &DimStyle::default(), Units::Millimeters).unwrap();
        assert!(label.starts_with('R') && label.contains("mm"));
    }
}

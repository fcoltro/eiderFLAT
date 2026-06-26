pub(crate) mod dim;
pub mod dxf;
pub mod native;
pub mod svg;

pub use dxf::{export_dxf, import_dxf};
pub use native::{
    from_string as from_e2d, load as load_native, save as save_native, to_string as to_e2d,
};
pub use svg::{export_svg, import_svg};

use eiderflat_geometry::{Curve, CurveSegment, Point2d, tessellate_curve};

pub(crate) fn flatten_for_export(c: &Curve) -> Vec<Point2d> {
    let bb = c.bounding_box();
    let diag = ((bb.max.x - bb.min.x).powi(2) + (bb.max.y - bb.min.y).powi(2)).sqrt();
    let tol = (diag * 1e-3).max(1e-6);
    tessellate_curve(c, tol)
}

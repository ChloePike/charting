//! Sub-panes under the price plot: the drawing primitives every pane uses (legend row, value axis, lines, zero line,
//! range), so an app's own panes (CVD, open interest, funding, scripts ...) look like the built-in ones.
use crate::coords::{padded_range, BarAxis};
use crate::style::{dim, lerp, line as hairline, mono, mono_m, mu, prop_m};
use egui::{pos2, Align2, Color32, Painter, Pos2, Rect, Stroke};

/// Height of the legend row at the top of a pane (lines never run through it).
pub const LEGEND_H: f32 = 18.0;

/// The part of pane `r` values are drawn in: under the legend row, 4 points above the bottom.
pub fn inner(r: Rect) -> Rect { Rect::from_min_max(pos2(r.left(), r.top() + LEGEND_H), pos2(r.right(), r.bottom() - 4.0)) }

/// y of `v` in `inner` showing `lo..hi`.
pub fn y_of(v: f64, lo: f64, hi: f64, inner: Rect) -> f32 { inner.bottom() - ((v - lo) / (hi - lo)) as f32 * inner.height() }

/// The value axis' label colour (between the dim and muted greys).
pub fn axis_color() -> Color32 { lerp(dim(), mu(), 0.5) }

/// The pane's name at the left of its legend row; returns where the next legend item starts.
pub fn title(p: &Painter, r: Rect, name: &str) -> f32 { p.text(pos2(r.left() + 6.0, r.top() + 10.0), Align2::LEFT_CENTER, name, prop_m(12.0), mu()).right() + 8.0 }

/// A legend value at `x` in the legend row; returns where the next one starts.
pub fn label(p: &Painter, r: Rect, s: String, col: Color32, x: f32) -> f32 { p.text(pos2(x, r.top() + 10.0), Align2::LEFT_CENTER, s, mono_m(11.5), col).right() + 12.0 }

/// The value axis: `hi` at the top and `lo` at the bottom of `inner`, right of the pane.
pub fn axis(p: &Painter, r: Rect, lo: f64, hi: f64, fmt: &dyn Fn(f64) -> String) {
    let inner = inner(r);
    p.text(pos2(r.right() + 8.0, inner.top() + 4.0), Align2::LEFT_CENTER, fmt(hi), mono(11.0), axis_color());
    p.text(pos2(r.right() + 8.0, inner.bottom() - 4.0), Align2::LEFT_CENTER, fmt(lo), mono(11.0), axis_color());
}

/// A line through (bar index from `i0`, value) points; `step` draws it as steps (a value holds until the next).
#[allow(clippy::too_many_arguments)]
pub fn line(p: &Painter, axis: &BarAxis, i0: usize, inner: Rect, vals: Vec<(usize, f64)>, lo: f64, hi: f64, col: Color32, step: bool) {
    let mut pts: Vec<Pos2> = vec![];
    for (k, v) in vals {
        let pt = pos2(axis.x((i0 + k) as f32), y_of(v, lo, hi, inner));
        if step && let Some(prev) = pts.last().copied() { pts.push(pos2(pt.x, prev.y)); }
        pts.push(pt);
    }
    if pts.len() > 1 { p.add(egui::Shape::line(pts, Stroke::new(1.3, col))); }
}

/// A hairline at zero when the range crosses it.
pub fn zero_line(p: &Painter, r: Rect, lo: f64, hi: f64) {
    if lo < 0.0 && hi > 0.0 { p.hline(r.left()..=r.right(), y_of(0.0, lo, hi, inner(r)), Stroke::new(1.0, hairline())); }
}

/// The range a pane shows for `vals`: padded 8%, including zero when `zero`.
pub fn range(vals: &[f64], zero: bool) -> (f64, f64) { padded_range(vals.iter().copied(), 0.08, zero) }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_map_into_the_area_under_the_legend() {
        let r = Rect::from_min_size(pos2(0.0, 100.0), egui::vec2(200.0, 80.0));
        let i = inner(r);
        assert_eq!((i.top(), i.bottom()), (118.0, 176.0));
        assert_eq!((y_of(10.0, 0.0, 10.0, i), y_of(0.0, 0.0, 10.0, i)), (118.0, 176.0));
        let (lo, hi) = range(&[1.0, 2.0], true);
        assert!(lo < 0.0 + 1e-9 && hi > 2.0);
    }
}

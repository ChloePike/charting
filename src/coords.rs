//! Price and time arithmetic of a chart: nice steps, grid values, padded ranges, interval buckets.

/// 1, 2, 2.5, 5 x 10^k at or above x (x is floored at 1e-12).
pub fn nice(x: f64) -> f64 {
    let x = x.max(1e-12);
    let m = 10f64.powf(x.log10().floor());
    [1.0, 2.0, 2.5, 5.0, 10.0].iter().map(|k| k * m).find(|v| *v >= x).unwrap_or(10.0 * m)
}

/// Finite min/max of `vals`, widened to at least ±0.1% of its magnitude (a single bar's high and
/// low can differ only by float noise), then padded by `pad` of the span.
pub fn padded_range(vals: impl Iterator<Item = f64>, pad: f64, zero: bool) -> (f64, f64) {
    let (mut lo, mut hi) = vals.filter(|v| v.is_finite()).fold((f64::MAX, f64::MIN), |(l, h), v| (l.min(v), h.max(v)));
    if lo > hi { lo = 0.0; hi = 1.0; }
    if zero { lo = lo.min(0.0); hi = hi.max(0.0); }
    let min_span = (lo.abs().max(hi.abs()) * 2e-3).max(1e-9);
    if hi - lo < min_span { let m = (hi + lo) / 2.0; lo = m - min_span / 2.0; hi = m + min_span / 2.0; }
    let p = (hi - lo) * pad;
    (lo - p, hi + p)
}

/// Grid values between lo and hi at a "nice" step, about `target` lines, never more than 50.
/// (A `while v < hi { v += step }` loop never ends once step is below v's float resolution.)
pub fn grid(lo: f64, hi: f64, target: f64) -> Vec<f64> {
    let step = nice((hi - lo) / target);
    let first = (lo / step).ceil();
    (0..50).map(|i| (first + i as f64) * step).take_while(|v| *v < hi).collect()
}

/// Bucket start for an interval of `tf` minutes: weeks start on Monday (the epoch was a Thursday), the rest on
/// UTC multiples of the interval.
pub fn bucket(t: i64, tf: i64) -> i64 {
    let ms = tf * 60_000;
    let off = if tf == 10080 { 4 * 86_400_000 } else { 0 };
    (t - off).div_euclid(ms) * ms + off
}

use egui::Rect;

/// Price <-> y on a plot `r` showing `lo..hi`; a log scale maps ln(price).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PriceScale { pub lo: f64, pub hi: f64, pub log: bool, pub r: Rect }

impl PriceScale {
    fn f(&self, v: f64) -> f64 { if self.log { v.max(1e-12).ln() } else { v } }
    /// y of price `v`
    pub fn y(&self, v: f64) -> f32 { let (a, b) = (self.f(self.lo), self.f(self.hi)); self.r.bottom() - ((self.f(v) - a) / (b - a)) as f32 * self.r.height() }
    /// price at `y`
    pub fn v(&self, y: f32) -> f64 {
        let (a, b) = (self.f(self.lo), self.f(self.hi));
        let x = a + (self.r.bottom() - y) as f64 / self.r.height() as f64 * (b - a);
        if self.log { x.exp() } else { x }
    }
}

/// Bar index <-> x: `bar_w` points per bar, fractional index `right_edge` at the right of `plot` (the newest bar's
/// centre half a bar in), `first` the index at its left edge.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BarAxis { pub first: f32, pub right_edge: f32, pub bar_w: f32, pub plot: Rect }

impl BarAxis {
    /// x of the centre of (fractional) bar `i`
    pub fn x(&self, i: f32) -> f32 { self.plot.right() - (self.right_edge - i) * self.bar_w - self.bar_w / 2.0 }
    /// fractional bar index at `x`
    pub fn idx(&self, x: f32) -> f32 { self.right_edge - (self.plot.right() - x - self.bar_w / 2.0) / self.bar_w }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_is_bounded_and_nice() {
        assert_eq!(nice(0.0), 1e-12);
        assert_eq!(nice(3.0), 5.0);
        assert_eq!(grid(0.0, 100.0, 4.0), vec![0.0, 25.0, 50.0, 75.0]);
        // a float-noise range never loops forever: at most 50 lines
        assert!(grid(1e9, 1e9 + 1e-3, 5.0).len() <= 50);
        let (lo, hi) = padded_range([5.0, 5.0].into_iter(), 0.1, false);
        assert!(lo < 5.0 && hi > 5.0);
        // weeks start on Monday (2026-11-09 00:00 UTC), other intervals on UTC multiples
        assert_eq!(bucket(1_794_700_000_000, 10080), 1_794_182_400_000);
        assert_eq!(bucket(1_794_700_000_000, 60), 1_794_700_000_000 / 3_600_000 * 3_600_000);
    }

    #[test]
    fn scales_round_trip() {
        let r = Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(100.0, 200.0));
        for log in [false, true] {
            let s = PriceScale { lo: 100.0, hi: 400.0, log, r };
            assert_eq!(s.y(100.0), 200.0);
            assert_eq!(s.y(400.0), 0.0);
            assert!((s.v(s.y(250.0)) - 250.0).abs() < 1e-6);
        }
        let a = BarAxis { first: 0.0, right_edge: 10.0, bar_w: 10.0, plot: r };
        assert_eq!(a.x(10.0), 95.0);
        assert!((a.idx(a.x(3.25)) - 3.25).abs() < 1e-5);
    }
}

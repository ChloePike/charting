//! Time x price heat of levels that live between two times (estimated liquidation levels, open interest at price,
//! any "this much rests here from t0 until t1"), drawn as one opaque magma texture.
use egui::{Color32, TextureHandle, TextureId};

/// One level's size at `px` from `t0` until `t1` (ms; `i64::MAX` while open).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Segment { pub px: f64, pub t0: i64, pub t1: i64, pub q: f32 }

/// Size per cell: `cols` columns of `ms` from `t0` (each shows the state at its end), `rows` rows from the bottom;
/// `row` maps a price to its (fractional, unclamped) row, any scale. A level fills the rows of its whole `bin`, its
/// size spread over them, so neighbours join into bands and a zoomed-in bin is not brighter than a small one.
pub fn grid(segs: &[Segment], bin: f64, t0: i64, ms: i64, cols: usize, rows: usize, row: impl Fn(f64) -> f32) -> Vec<f32> {
    let mut g = vec![0f32; cols * rows];
    if cols == 0 || rows == 0 { return g; }
    let span = cols as i64 * ms;
    for s in segs {
        let (a, b) = (row(s.px - bin / 2.0), row(s.px + bin / 2.0));
        let (a, b) = (a.min(b), a.max(b));
        if b < 0.0 || a >= rows as f32 { continue; }
        let (r0, r1) = (a.max(0.0) as usize, (b.ceil() as usize).clamp(1, rows) - 1);
        let q = s.q / (r1 - r0 + 1) as f32;
        // column c (end te = t0 + (c+1) ms - 1) shows the segment when t0_seg <= te < t1_seg
        let c0 = ((s.t0 - t0 + 1).max(0) + ms - 1) / ms - 1;
        let c1 = (s.t1.min(t0 + span) - t0).div_euclid(ms) - 1;
        for c in c0.max(0)..=c1.min(cols as i64 - 1) { for r in r0..=r1.max(r0) { g[c as usize * rows + r] += q; } }
    }
    g
}

/// Magma-like ramp (deep purple -> magenta -> orange -> pale yellow), opaque: an empty cell is the darkest stop, so the
/// map reads as one surface with levels on it (as Hyperdash draws it), not lines on a hole.
pub fn magma(t: f32) -> Color32 {
    const S: [(f32, [u8; 3]); 6] = [(0.0, [0x1e, 0x12, 0x33]), (0.3, [0x5a, 0x1d, 0x5e]), (0.55, [0xa3, 0x30, 0x6a]),
        (0.75, [0xe0, 0x5a, 0x3a]), (0.9, [0xf5, 0xa0, 0x3a]), (1.0, [0xfd, 0xe8, 0x8a])];
    let t = t.clamp(0.0, 1.0);
    let k = S.iter().position(|s| s.0 >= t).unwrap_or(S.len() - 1).max(1);
    let ((t0, a), (t1, b)) = (S[k - 1], S[k]);
    let c = |x: [u8; 3]| Color32::from_rgb(x[0], x[1], x[2]);
    crate::style::lerp(c(a), c(b), (t - t0) / (t1 - t0))
}

/// The texture of a segment set for one view, rebuilt only when the data `version` or the view changes.
#[derive(Default)]
pub struct SegmentHeat { tex: Option<(ViewKey, TextureHandle, f32)> }

/// What a texture was drawn for: data version, first time, bar length, columns, price range bits, scale, rows.
type ViewKey = (u64, i64, i64, usize, u64, u64, usize);

impl SegmentHeat {
    /// The map as an opaque texture to paint over the plot, and the size of its brightest colour (the 99.5th
    /// percentile cell, for the legend's scale). `view` identifies the price mapping (range bits, scale) for the cache.
    /// None past the GPU's texture side (zoomed far out) or with nothing to show.
    #[allow(clippy::too_many_arguments)]
    pub fn texture(&mut self, ctx: &egui::Context, segs: &[Segment], bin: f64, version: u64, t0: i64, ms: i64, cols: usize, rows: usize, view: (u64, u64, u8), row: impl Fn(f64) -> f32) -> Option<(TextureId, f32)> {
        let k = (version, t0, ms, cols, view.0, view.1 ^ ((view.2 as u64) << 60), rows);
        if let Some((kk, t, top)) = &self.tex && *kk == k { return Some((t.id(), *top)); }
        if cols.max(rows) > ctx.input(|i| i.max_texture_side) { return None; }
        let g = grid(segs, bin, t0, ms, cols, rows, row);
        let mut nz: Vec<f32> = g.iter().copied().filter(|v| *v > 0.0).collect();
        if nz.is_empty() { self.tex = None; return None; }
        let p = (nz.len() * 995 / 1000).min(nz.len() - 1);
        let top = *nz.select_nth_unstable_by(p, f32::total_cmp).1;
        let mut img = egui::ColorImage::filled([cols, rows], magma(0.0));
        for c in 0..cols {
            for r in 0..rows {
                let v = g[c * rows + r];
                // linear: most of the map stays deep purple, only the real clusters run to orange and yellow
                if v > 0.0 { img.pixels[(rows - 1 - r) * cols + c] = magma((v / top).min(1.0)); }
            }
        }
        let tex = match self.tex.take() { Some((_, mut t, _)) => { t.set(img, egui::TextureOptions::NEAREST); t } None => ctx.load_texture("segheat", img, egui::TextureOptions::NEAREST) };
        let id = tex.id();
        self.tex = Some((k, tex, top));
        Some((id, top))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn magma_runs_dark_to_light() {
        let l = |c: Color32| c.r() as u32 + c.g() as u32 + c.b() as u32;
        let v: Vec<u32> = (0..=10).map(|i| l(magma(i as f32 / 10.0))).collect();
        assert!(v.windows(2).all(|w| w[0] <= w[1]), "{v:?}");
        assert_eq!(magma(1.0), Color32::from_rgb(0xfd, 0xe8, 0x8a));
    }

    #[test]
    fn a_swept_level_ends_its_columns() {
        // open from minute 0, swept at minute 1: column 0 holds it, column 1 not
        let s = [Segment { px: 100.0, t0: 59_999, t1: 60_000, q: 4.0 }, Segment { px: 100.0, t0: 59_999, t1: i64::MAX, q: 1.0 }];
        let g = grid(&s, 1.0, 0, 60_000, 2, 1, |_| 0.5);
        assert_eq!(g, vec![5.0, 1.0]);
        let ctx = egui::Context::default();
        let mut h = SegmentHeat::default();
        assert!(h.texture(&ctx, &s, 1.0, 1, 0, 60_000, 2, 4, (0, 0, 0), |_| 1.5).is_some());
    }
}

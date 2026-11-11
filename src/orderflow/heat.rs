//! Book heatmap: resting size per price bin over time, folded per chart bar and drawn as one texture.
use egui::Color32;

/// Book heatmap aggregated per chart bar, drawn as one texture: feed it resting-size columns (`update`), paint its
/// `texture` as one quad (`TexGeom` says where).
/// A book read for the depth profile: (when read, bin, [(price, size, ask)]).
pub type Depth = (std::time::Instant, f64, Vec<(f64, f32, bool)>);

#[derive(Default)]
pub struct BookHeat {
    /// what the cached bars were folded for (the app's series identity, e.g. interval and market): a change starts over
    key: u64, bar_ms: i64, last_ts: i64,
    /// price of bin index i is i * bin
    pub bin: f64,
    /// the source's version: a change (columns the source replaced after they were folded) starts over
    src_ver: u64,
    /// per bar: (open time, lowest bin, summed qty per bin, samples); drawn as the time-weighted
    /// mean, so liquidity that was pulled fades instead of staying lit for the whole bar
    bars: Vec<(i64, i64, Vec<f32>, u32)>,
    /// bumped whenever `bars` changes; the GPU texture is rebuilt only then (about once a second)
    ver: u64,
    tex: Option<(u64, egui::TextureHandle, TexGeom)>,
    /// the app's book right now for the depth profile at the right edge
    pub depth: Option<Depth>,
}

/// Where the heatmap texture sits: first bar time, bar width (ms), lowest bin, bin count.
#[derive(Clone, Copy, Debug)]
pub struct TexGeom { pub t0: i64, pub ms: i64, pub lo: i64, pub bins: usize, pub cols: usize }

impl BookHeat {
    /// Fold new columns into per-bar summed rows. `columns_since(t)`: every column after `t`, oldest first, as
    /// (time ms, [(bin index, resting size)]). A new `key` (series), `bin`, `src_ver` or `bar_ms` starts over.
    pub fn update<I: Iterator<Item = (i64, Vec<(i64, f32)>)>>(&mut self, key: u64, bin: f64, src_ver: u64, bar_ms: i64, columns_since: impl FnOnce(i64) -> I) {
        if self.key != key || self.bin != bin || self.src_ver != src_ver || self.bar_ms != bar_ms {
            *self = BookHeat { key, bar_ms, bin, src_ver, last_ts: i64::MIN, ..Default::default() };
        }
        let ms = bar_ms.max(1);
        for (ts, col) in columns_since(self.last_ts) {
            let (ts, col) = (&ts, &col);
            // guard: a column spanning absurdly many bins would allocate gigabytes
            if let (Some(a), Some(b)) = (col.iter().map(|c| c.0).min(), col.iter().map(|c| c.0).max()) && b - a > 20_000 { continue; }
            let t = ts.div_euclid(ms) * ms;
            if self.bars.last().is_none_or(|b| b.0 != t) {
                let lo = col.iter().map(|c| c.0).min().unwrap_or(0) - 50;
                self.bars.push((t, lo, vec![], 0));
            }
            let (_, lo, v, n) = self.bars.last_mut().unwrap();
            if col.iter().any(|c| (c.0 - *lo).abs() > 20_000) { continue; }
            for &(bin, q) in col {
                if bin < *lo {
                    let grow = (*lo - bin) as usize;
                    v.splice(0..0, std::iter::repeat_n(0.0, grow));
                    *lo = bin;
                }
                let i = (bin - *lo) as usize;
                if i >= v.len() { v.resize(i + 1, 0.0); }
                v[i] += q;
            }
            *n += 1;
            self.last_ts = *ts;
            self.ver += 1;
        }
        if self.bars.len() > 2000 { self.bars.drain(..self.bars.len() - 2000); }
    }

    /// The heatmap as one RGBA texture (columns = bars, rows = price bins, top = highest price), re-uploaded only when
    /// new samples arrived. Drawn as a single textured rect per frame. (Thin 2 px lines per bin, as CoinGlass draws its
    /// finer bins, made a comb of BTC's $15-25 bins: nearly every bin holds orders, 2026-11-11.)
    pub fn texture(&mut self, ctx: &egui::Context) -> Option<(egui::TextureId, TexGeom)> {
        if let Some((v, t, g)) = &self.tex && *v == self.ver { return Some((t.id(), *g)); }
        // a texture side over the GPU limit (8192 here) is a wgpu validation panic: keep the newest
        // `side` columns and merge price bins `step` to a row
        let side = ctx.input(|i| i.max_texture_side).max(1);
        let ms = self.bar_ms.max(1);
        let last = self.bars.last()?;
        let t0 = self.bars.iter().map(|b| b.0).find(|t| (last.0 - t) / ms < side as i64)?;
        let bars = || self.bars.iter().filter(move |b| b.0 >= t0);
        let lo = bars().map(|b| b.1).min()?;
        let hi = bars().map(|b| b.1 + b.2.len() as i64).max()?;
        let (cols, bins) = (((last.0 - t0) / ms + 1) as usize, (hi - lo).max(1) as usize);
        let step = bins.div_ceil(side);
        let rows = bins.div_ceil(step);
        if cols * rows > 8_000_000 { return None; }
        // every bar as its time-weighted mean, the forming one too (its single latest sample flickered against the
        // smoother minutes beside it: a bright dashed column at the live bar, 2026-11-11)
        let vals: Vec<(i64, i64, Vec<f32>)> = self.bars.iter().filter(|b| b.0 >= t0)
            .map(|(t, blo, v, n)| (*t, *blo, v.iter().map(|q| q / (*n).max(1) as f32).collect())).collect();
        let mut qs: Vec<f32> = vals.iter().flat_map(|b| b.2.iter().copied().filter(|q| *q > 0.0)).collect();
        if qs.is_empty() { return None; }
        let p = (qs.len() * 995 / 1000).min(qs.len() - 1);
        let qmax = qs.select_nth_unstable_by(p, f32::total_cmp).1.max(1e-9);
        let mut img = egui::ColorImage::new([cols, rows], vec![Color32::TRANSPARENT; cols * rows]);
        for (bt, blo, v) in &vals {
            let x = ((bt - t0) / ms) as usize;
            // each bin against its price neighbours (`local_median`): what stands out of the book around it lights up
            let meds = local_median(v, NEIGHBOURS);
            for (k, q) in v.iter().enumerate() {
                if *q <= 0.0 { continue; }
                let med = meds[k];
                let (i, c) = (((rows - 1 - (blo + k as i64 - lo) as usize / step) * cols + x), heat_color(heat_level(*q, med).max(BASE_MAX * (*q / qmax).min(1.0).sqrt())));
                // merged bins keep the strongest (alpha rises with the level)
                if c.a() >= img.pixels[i].a() { img.pixels[i] = c; }
            }
        }
        let geom = TexGeom { t0, ms, lo, bins: rows * step, cols };
        let t = match self.tex.take() {
            Some((_, mut t, _)) => { t.set(img, egui::TextureOptions::NEAREST); t }
            None => ctx.load_texture("heatmap", img, egui::TextureOptions::NEAREST),
        };
        let id = t.id();
        self.tex = Some((self.ver, t, geom));
        Some((id, geom))
    }
}

/// Every resting bin shows at least this much of the ramp by its size against the map's 99.5th percentile, so the book
/// stays visible (walls alone left the map nearly empty, 2026-11-11); capped so the dense book at the price stays green,
/// not yellow: only standing out of its neighbours (`heat_level`) reaches the top.
pub const BASE_MAX: f32 = 0.45;

/// Bins on each side a bin is compared with (`local_median`).
pub const NEIGHBOURS: usize = 10;

/// Per bin, the median of the resting bins within `w` bins of it (itself included). Against the whole column's median
/// instead, the dense book next to the price stood out of the thin 2% edges: a solid yellow band at the price, 2026-11-11.
pub fn local_median(v: &[f32], w: usize) -> Vec<f32> {
    let mut buf = Vec::with_capacity(2 * w + 1);
    (0..v.len()).map(|k| {
        buf.clear();
        buf.extend(v[k.saturating_sub(w)..(k + w + 1).min(v.len())].iter().copied().filter(|q| *q > 0.0));
        if buf.is_empty() { return 0.0; }
        let m = buf.len() / 2;
        *buf.select_nth_unstable_by(m, f32::total_cmp).1
    }).collect()
}

/// How far a bin stands out of the book around it (`med`: the median of its price neighbours) as 0..1 on a log scale: half the median 0, the
/// median 0.2 (a faint purple wherever orders rest, as CoinGlass shows the book), 3x about 0.5 (still purple: neighbouring
/// bins vary 1-3x minute to minute, and green there read as a checkerboard), WALL_X times it 1. A cut-off at 2x hid the
/// smooth minute-mean history entirely, 2026-11-11. Against the map's top level instead, BTC's $20 bins summed over 12 venues were all near the top: one yellow
/// block, walls invisible (2026-11-11). Fades in, no threshold (a threshold flickered bar to bar).
pub const WALL_X: f32 = 16.0;
pub fn heat_level(q: f32, med: f32) -> f32 { ((2.0 * q / med.max(1e-9)).ln() / (2.0 * WALL_X).ln()).clamp(0.0, 1.0) }

/// Ramp for liquidity, no blue and no purple (the liquidation map under it is purple): transparent -> teal green ->
/// yellow green -> yellow.
pub fn heat_color(s: f32) -> Color32 {
    const S: [(f32, [u8; 4]); 5] = [(0.0, [30, 120, 90, 0]), (0.3, [35, 150, 105, 70]), (0.6, [45, 185, 110, 170]),
        (0.85, [175, 220, 60, 235]), (1.0, [255, 236, 90, 255])];
    let s = s.clamp(0.0, 1.0);
    let k = S.iter().position(|x| x.0 >= s).unwrap_or(S.len() - 1).max(1);
    let ((a, ca), (b, cb)) = (S[k - 1], S[k]);
    let t = (s - a) / (b - a);
    let m = |i: usize| (ca[i] as f32 + (cb[i] as f32 - ca[i] as f32) * t).round() as u8;
    Color32::from_rgba_unmultiplied(m(0), m(1), m(2), m(3))
}


#[cfg(test)]
mod tests {
    use super::*;

    fn feed(c: &mut BookHeat, cols: Vec<(i64, Vec<(i64, f32)>)>) {
        c.update(1, 1.0, 0, 60_000, move |since| cols.into_iter().filter(move |c| c.0 > since));
    }

    #[test]
    fn a_pulled_wall_fades_as_the_bar_mean() {
        // a wall at bin 100 for 1 of 4 samples of bar 0
        let mut c = BookHeat::default();
        feed(&mut c, [(1, 40.0), (15_000, 0.0), (30_000, 0.0), (45_000, 0.0)].into_iter().map(|(t, q)| (t, vec![(100, q), (101, 1.0)])).collect());
        let (_, lo, v, n) = &c.bars[0];
        assert_eq!(v[(100 - lo) as usize] / *n as f32, 10.0);
        // a new source version starts over
        c.update(1, 1.0, 1, 60_000, |_| vec![(60_000, vec![(100, 2.0)])].into_iter());
        assert_eq!(c.bars.len(), 1);
    }

    #[test]
    fn texture_fits_the_gpu_side() {
        // price walked 15,000 bins across bars and the bars span a long gap in time: both sides
        // past the limit crashed wgpu (Dimension Y value 8884 exceeds the limit of 8192)
        let cols: Vec<_> = (0..1500i64).map(|i| (i * 60_000 + i.min(1) * 600_000_000, vec![(i * 10, 5.0f32), (i * 10 + 1, 1.0)])).collect();
        let mut c = BookHeat::default();
        feed(&mut c, cols);
        let ctx = egui::Context::default();
        let side = ctx.input(|i| i.max_texture_side);
        let (_, g) = c.texture(&ctx).unwrap();
        let [w, hh] = c.tex.as_ref().unwrap().1.size();
        assert!(w <= side && hh <= side, "{w} x {hh} over {side}");
        assert!(g.lo + g.bins as i64 > 1499 * 10 && g.cols == w);
    }

    #[test]
    fn a_dense_book_at_the_price_does_not_light_up_only_a_wall_does() {
        // thin edges, a dense block in the middle (the book at the price), one wall 10x inside it
        let mut v = vec![1.0f32; 60];
        for x in v.iter_mut().take(40).skip(20) { *x = 10.0; }
        v[30] = 100.0;
        let m = local_median(&v, NEIGHBOURS);
        assert!(heat_level(v[25], m[25]) < 0.3, "the dense book stays dark: {}", heat_level(v[25], m[25]));
        assert!(heat_level(v[30], m[30]) > 0.75, "the wall lights up: {}", heat_level(v[30], m[30]));
    }

    #[test]
    fn heat_scale_fades_small_and_tops_out() {
        // the column's typical level faint, its 1-3x noise still purple (under the green from 0.55), a wall 16x the brightest
        assert_eq!(heat_level(0.0, 10.0), 0.0);
        let med = heat_level(10.0, 10.0);
        assert!(med > 0.15 && med < 0.25 && heat_color(med).a() > 0, "{med}");
        assert!(heat_level(30.0, 10.0) < 0.55, "{}", heat_level(30.0, 10.0));
        assert!((heat_level(160.0, 10.0) - 1.0).abs() < 1e-5 && heat_level(500.0, 10.0) == 1.0);
        assert_eq!(heat_color(0.0).a(), 0);
        assert!(heat_color(0.5).a() > heat_color(0.2).a() && heat_color(1.0).a() == 255);
        // no blue on top: the ramp ends yellow
        let c = heat_color(1.0);
        assert!(c.r() > c.b() && c.g() > c.b());
    }

}

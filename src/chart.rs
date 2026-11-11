//! A ready chart widget built from the crate's parts: candles drawn the TradingView way, grid, price and time axes,
//! last-price tag, crosshair, pan / glide / zoom, eased auto-fit. Everything else (indicators, order lines, heat maps,
//! drawings) goes on top through `Overlay`, at the depth it belongs.
use crate::candle::{paint_candle, tv_body_px};
use crate::coords::{grid, padded_range, BarAxis, PriceScale};
use crate::interact::{pan_zoom, price_range, TimeView, BAR_W};
use crate::layout::place;
use crate::style::{cross_bg, dim, dn, label_bg, line, mono, mu, up};
use egui::{pos2, vec2, Align2, Color32, Painter, Rect, Response, Sense, Stroke, Ui};

/// One bar: open time (ms), open, high, low, close, volume.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Candle { pub t: i64, pub o: f64, pub h: f64, pub l: f64, pub c: f64, pub v: f64 }

/// What a chart shows this frame: candles oldest first (borrowed, never kept), the bar length, and a key naming the
/// series (symbol, interval ...): a new key snaps the auto-fit instead of easing toward it.
pub struct ChartData<'a> { pub candles: &'a [Candle], pub bar_ms: i64, pub key: &'a str }

/// Where an overlay paints: under the grid, between the grid and the candles, or over the candles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Z { Back, BelowCandles, AboveCandles }

/// One frame of the chart as an overlay sees it: the painter clipped to the plot, the coordinate transforms, the
/// visible bars, the pointer, and the price-axis tag layout (tags placed through `tag` never overlap).
pub struct Frame<'a> {
    pub painter: Painter,
    /// the whole chart (axes included), for anything outside the plot
    pub full: Painter,
    pub ui: &'a Ui,
    pub response: &'a Response,
    pub plot: Rect,
    pub axis: BarAxis,
    pub scale: PriceScale,
    pub candles: &'a [Candle],
    /// visible bar indices, inclusive
    pub i0: usize, pub i1: usize,
    /// (bar index, price) under the pointer
    pub hover: Option<(usize, f64)>,
    tags: Vec<Rect>,
    axis_w: f32,
}

impl Frame<'_> {
    /// x of bar `i`'s centre
    pub fn x(&self, i: usize) -> f32 { self.axis.x(i as f32) }
    /// y of `price`
    pub fn y(&self, price: f64) -> f32 { self.scale.y(price) }
    /// A filled tag on the price axis at `price` (moved until it overlaps no other); None off screen.
    pub fn tag(&mut self, price: f64, text: &str, fill: Color32) -> Option<Rect> {
        let y = self.y(price);
        if y < self.plot.top() || y > self.plot.bottom() { return None; }
        let r = place(Rect::from_min_size(pos2(self.plot.right() + 2.0, y - 9.0), vec2(self.axis_w - 4.0, 18.0)), &mut self.tags);
        self.full.rect_filled(r, 2, fill);
        self.full.text(r.left_center() + vec2(5.0, 0.0), Align2::LEFT_CENTER, text, mono(11.0), Color32::WHITE);
        Some(r)
    }
}

/// The extension point: paint at each depth. A closure `FnMut(Z, &mut Frame)` is one.
pub trait Overlay { fn paint(&mut self, z: Z, f: &mut Frame); }
impl<F: FnMut(Z, &mut Frame)> Overlay for F { fn paint(&mut self, z: Z, f: &mut Frame) { self(z, f) } }

/// No overlay.
pub struct Nothing;
impl Overlay for Nothing { fn paint(&mut self, _: Z, _: &mut Frame) {} }

/// The chart's view state (keep it between frames; `Default` starts at the live edge, default zoom, auto-fit).
pub struct Chart {
    pub view: TimeView,
    /// log price scale
    pub log: bool,
    /// manual price range (wheel / axis drag); None = auto-fit
    pub y_range: Option<(f64, f64)>,
    eased: Option<(String, f64, f64)>,
    /// price format; None = decimals from the grid step
    pub fmt_price: Option<fn(f64) -> String>,
}

impl Default for Chart {
    fn default() -> Self { Chart { view: TimeView { right: 0.0, bar_w: BAR_W, fling: 0.0 }, log: false, y_range: None, eased: None, fmt_price: None } }
}

/// Price axis width and time axis height, points.
pub const AXIS_W: f32 = 70.0;
pub const TIME_H: f32 = 22.0;

impl Chart {
    /// Draws the chart in the space left in `ui` and handles its pointer input; `overlay` paints at each `Z`.
    pub fn show(&mut self, ui: &mut Ui, data: &ChartData, overlay: &mut dyn Overlay) -> Response {
        let (rect, resp) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
        let main = Rect::from_min_max(rect.min, pos2(rect.right() - AXIS_W, rect.bottom() - TIME_H));
        let full = ui.painter_at(rect);
        let n = data.candles.len();
        if n == 0 || main.width() < 20.0 || main.height() < 20.0 { return resp; }
        let on_axis = resp.interact_pointer_pos().is_some_and(|p| p.x > main.right() && p.y < main.bottom());
        let (dt, y_wheel, reset) = pan_zoom(ui, &resp, main, n, &mut self.view, 60.0, false, on_axis, true);
        if reset { self.y_range = None; }
        // the live edge sits 3 bars in from the right; never panned past either end
        self.view.right = self.view.right.clamp(-(main.width() / self.view.bar_w) * 0.8, n as f32);
        let right_edge = n as f32 + 2.0 - self.view.right;
        let axis = BarAxis { first: right_edge - main.width() / self.view.bar_w, right_edge, bar_w: self.view.bar_w, plot: main };
        let i0 = axis.first.floor().max(0.0) as usize;
        let i1 = (right_edge.ceil() as usize).min(n - 1);
        if i0 > i1 { return resp; }
        let vis = &data.candles[i0..=i1];
        let target = padded_range(vis.iter().flat_map(|b| [b.l, b.h]), 0.06, false);
        let (lo, hi) = price_range(ui, &resp, main, target, data.key.to_string(), &mut self.eased, &mut self.y_range, on_axis, false, dt, y_wheel);
        let scale = PriceScale { lo, hi, log: self.log && lo > 0.0, r: main };
        let hover = resp.hover_pos().filter(|p| main.contains(*p)).map(|p| ((axis.idx(p.x).round().max(0.0) as usize).min(n - 1), scale.v(p.y)));
        let mut f = Frame { painter: ui.painter_at(main), full: full.clone(), ui, response: &resp, plot: main, axis, scale, candles: data.candles, i0, i1, hover, tags: vec![], axis_w: AXIS_W };
        let gv = grid(lo, hi, (main.height() as f64 / 60.0).max(2.0));
        let step = gv.windows(2).map(|w| w[1] - w[0]).fold(f64::MAX, f64::min);
        let dp = if step.is_finite() && step > 0.0 { (-step.log10().floor()).max(0.0) as usize } else { 2 };
        let fmt = |v: f64| self.fmt_price.map_or_else(|| format!("{v:.dp$}"), |p| p(v));

        overlay.paint(Z::Back, &mut f);
        for v in &gv {
            let y = scale.y(*v);
            f.painter.hline(main.x_range(), y, Stroke::new(1.0, line()));
            full.text(pos2(main.right() + 6.0, y), Align2::LEFT_CENTER, fmt(*v), mono(11.0), mu());
        }
        overlay.paint(Z::BelowCandles, &mut f);
        let ppp = ui.ctx().pixels_per_point();
        let bw = tv_body_px(self.view.bar_w, ppp);
        for (k, b) in vis.iter().enumerate() {
            let col = if b.c >= b.o { up() } else { dn() };
            paint_candle(&f.painter, axis.x((i0 + k) as f32), bw, self.view.bar_w, ppp, [scale.y(b.o), scale.y(b.h), scale.y(b.l), scale.y(b.c)], col);
        }
        overlay.paint(Z::AboveCandles, &mut f);
        // last price: a dotted line and its tag (reserved before nothing else: overlays placed theirs already)
        let last = data.candles[n - 1];
        let ly = scale.y(last.c);
        let col = if last.c >= last.o { up() } else { dn() };
        f.painter.extend(egui::Shape::dashed_line(&[pos2(main.left(), ly), pos2(main.right(), ly)], Stroke::new(1.0, col.linear_multiply(0.7)), 2.0, 3.0));
        f.tag(last.c, &fmt(last.c), col);
        // time axis: a label every ~100 points
        let every = ((100.0 / self.view.bar_w).ceil() as usize).max(1);
        for i in (i0..=i1).filter(|i| i % every == 0) {
            let t = data.candles[i].t / 1000;
            let (h, m) = (t.rem_euclid(86_400) / 3600, t.rem_euclid(3600) / 60);
            full.text(pos2(axis.x(i as f32), main.bottom() + TIME_H / 2.0), Align2::CENTER_CENTER, format!("{h:02}:{m:02}"), mono(11.0), mu());
        }
        // crosshair with its price tag
        if let Some((i, p)) = hover {
            let pos = resp.hover_pos().unwrap_or_default();
            let s = Stroke::new(1.0, dim());
            f.painter.extend(egui::Shape::dashed_line(&[pos2(axis.x(i as f32), main.top()), pos2(axis.x(i as f32), main.bottom())], s, 3.0, 3.0));
            f.painter.extend(egui::Shape::dashed_line(&[pos2(main.left(), pos.y), pos2(main.right(), pos.y)], s, 3.0, 3.0));
            let r = Rect::from_min_size(pos2(main.right() + 2.0, pos.y - 9.0), vec2(AXIS_W - 4.0, 18.0));
            full.rect_filled(r, 2, cross_bg());
            full.text(r.left_center() + vec2(5.0, 0.0), Align2::LEFT_CENTER, fmt(p), mono(11.0), Color32::WHITE);
        }
        full.rect_stroke(rect, 0, Stroke::new(1.0, label_bg()), egui::StrokeKind::Inside);
        resp
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shows_candles_and_calls_every_overlay_depth() {
        let candles: Vec<Candle> = (0..300).map(|i| { let c = 100.0 + (i as f64 * 0.1).sin() * 5.0; Candle { t: i * 60_000, o: c - 0.5, h: c + 1.0, l: c - 1.0, c, v: 1.0 } }).collect();
        let ctx = egui::Context::default();
        let mut chart = Chart::default();
        let mut seen = vec![];
        for _ in 0..2 {
            let mut out = ctx.run_ui(egui::RawInput { screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(800.0, 500.0))), ..Default::default() }, |ui| {
                chart.show(ui, &ChartData { candles: &candles, bar_ms: 60_000, key: "X" }, &mut |z: Z, f: &mut Frame| {
                    seen.push(z);
                    // the visible bars end at the newest, and the transforms agree with the plot
                    assert_eq!(f.i1, 299);
                    assert!(f.x(f.i1) < f.plot.right() && f.y(100.0) > f.plot.top() && f.y(100.0) < f.plot.bottom());
                });
            });
            out.textures_delta.clear();
        }
        assert_eq!(&seen[..3], &[Z::Back, Z::BelowCandles, Z::AboveCandles]);
    }
}

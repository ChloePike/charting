//! A ready chart widget built from the crate's parts: candles drawn the TradingView way, grid, price and time axes,
//! last-price tag, crosshair, pan / glide / zoom, eased auto-fit, linear / log / percent scales, a legend inset and
//! sub-panes. Everything else (indicators, order lines, heat maps, drawings, panes' content) goes on top through
//! `Overlay`, at the depth it belongs.
use crate::candle::{paint_candle, tv_body_px};
use crate::coords::{grid, padded_range, BarAxis, PriceScale};
use crate::fmt::{fmt_big, step_dp};
use crate::interact::{pan_zoom, price_range, TimeView, BAR_W};
use crate::layout::{place, try_place};
use crate::pane::inner;
use crate::style::{cross_bg, dim, dn, label_bg, line, mono, mono_m, mu, up};
use egui::{pos2, vec2, Align2, Color32, Painter, Pos2, Rect, Response, Sense, Stroke, Ui};

/// One bar: open time (ms), open, high, low, close, volume.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Candle { pub t: i64, pub o: f64, pub h: f64, pub l: f64, pub c: f64, pub v: f64 }

/// What a chart shows this frame: candles oldest first (borrowed, never kept), the bar length, and a key naming the
/// series (symbol, interval ...): a new key snaps the auto-fit instead of easing toward it.
pub struct ChartData<'a> { pub candles: &'a [Candle], pub bar_ms: i64, pub key: &'a str }

/// Where an overlay paints: under the grid, between the grid and the candles, over the candles, or over everything
/// (axes, panes and crosshair included: the axis column, toasts). Panes paint through `Overlay::pane`, between
/// `AboveCandles` and `Top`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Z { Back, BelowCandles, AboveCandles, Top }

/// The price scale: linear, log, or percent (linear, labelled relative to the first visible close).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Scale { #[default] Linear, Log, Percent }

/// One frame of the chart as an overlay sees it: the painter clipped to the price area (plot and legend inset), the
/// coordinate transforms, the visible bars, the pointer, and the tag layout (tags placed through it never overlap).
pub struct Frame<'a> {
    pub painter: Painter,
    /// the whole chart (axes and panes included), for anything outside the plot
    pub full: Painter,
    pub ui: &'a Ui,
    pub response: &'a Response,
    /// the whole widget
    pub rect: Rect,
    /// the price plot, under the legend inset
    pub plot: Rect,
    /// the legend inset above the plot (`Chart::inset_top` high), where the app writes its rows
    pub legend: Rect,
    /// sub-panes (id, rect left of the price axis), top first
    pub panes: Vec<(u32, Rect)>,
    pub axis: BarAxis,
    pub scale: PriceScale,
    pub candles: &'a [Candle],
    /// visible bar indices, inclusive
    pub i0: usize, pub i1: usize,
    /// (bar index, price) under the pointer over the plot
    pub hover: Option<(usize, f64)>,
    /// bar index under the pointer over the plot, legend or a pane (the legend shows it, else the newest)
    pub hover_i: Option<usize>,
    /// decimals of the price grid's step
    pub dp: usize,
    /// percent scale: the price at 0% (the first visible close)
    pub base: Option<f64>,
    fmt_price: Option<fn(f64, usize) -> String>,
    tags: Vec<Rect>,
    axis_w: f32,
}

impl Frame<'_> {
    /// x of bar `i`'s centre
    pub fn x(&self, i: usize) -> f32 { self.axis.x(i as f32) }
    /// y of `price`
    pub fn y(&self, price: f64) -> f32 { self.scale.y(price) }
    /// `v` in the app's price format (`Chart::fmt_price`), else with the grid's decimals.
    pub fn price(&self, v: f64) -> String { self.fmt_price.map_or_else(|| format!("{v:.dp$}", dp = self.dp), |f| f(v, self.dp)) }
    /// `v` as the price axis labels it: percent from `base` on a percent scale, else `price`.
    pub fn label(&self, v: f64) -> String { self.base.map_or_else(|| self.price(v), |b| format!("{:+.2}%", (v / b - 1.0) * 100.0)) }
    /// A slot `h` high on the price axis, centred at `y`.
    pub fn axis_slot(&self, y: f32, h: f32) -> Rect { Rect::from_min_size(pos2(self.plot.right() + 2.0, y - h / 2.0), vec2(self.axis_w - 4.0, h)) }
    /// Takes `r` as it is: later tags move around it (reserve the important ones first: last price, bid / ask, mark).
    pub fn reserve(&mut self, r: Rect) { self.tags.push(r) }
    /// `r` moved up or down until it overlaps no tag (`layout::place`).
    pub fn place(&mut self, r: Rect) -> Rect { place(r, &mut self.tags) }
    /// `r` slid half a tag or a tag at most, None when that is not enough (`layout::try_place`): context levels.
    pub fn try_place(&mut self, r: Rect) -> Option<Rect> { try_place(r, &mut self.tags) }
    /// A filled tag on the price axis at `price` (moved until it overlaps no other); None off screen.
    pub fn tag(&mut self, price: f64, text: &str, fill: Color32) -> Option<Rect> {
        let r = self.on_plot(price)?;
        let r = self.place(r);
        self.paint_tag(r, text, fill);
        Some(r)
    }
    /// A context tag: like `tag`, but it may only slide a little (`try_place`); None when dropped or off screen.
    pub fn context_tag(&mut self, price: f64, text: &str, fill: Color32) -> Option<Rect> {
        let r = self.on_plot(price)?;
        let r = self.try_place(r)?;
        self.paint_tag(r, text, fill);
        Some(r)
    }
    fn on_plot(&self, price: f64) -> Option<Rect> {
        let y = self.y(price);
        (y >= self.plot.top() && y <= self.plot.bottom()).then(|| self.axis_slot(y, 18.0))
    }
    fn paint_tag(&self, r: Rect, text: &str, fill: Color32) {
        self.full.rect_filled(r, 2, fill);
        self.full.text(r.left_center() + vec2(5.0, 0.0), Align2::LEFT_CENTER, text, mono(11.0), Color32::WHITE);
    }
}

/// A sub-pane as `Overlay::pane` sees it. Draw with the `pane` module's primitives.
pub struct PaneFrame<'a> {
    /// clipped to the pane's row, price axis column included (`pane::axis` writes there)
    pub painter: Painter,
    pub ui: &'a Ui,
    pub response: &'a Response,
    /// the pane left of the price axis; values go in `pane::inner(rect)`
    pub rect: Rect,
    pub axis: BarAxis,
    pub candles: &'a [Candle],
    pub i0: usize, pub i1: usize,
    pub hover_i: Option<usize>,
    /// the value range drawn, set by the overlay: the crosshair then shows the value under the pointer
    pub range: Option<(f64, f64)>,
}

impl PaneFrame<'_> {
    /// x of bar `i`'s centre
    pub fn x(&self, i: usize) -> f32 { self.axis.x(i as f32) }
}

/// The extension point: paint at each depth and own some of the input. A closure `FnMut(Z, &mut Frame)` is one.
pub trait Overlay {
    fn paint(&mut self, z: Z, f: &mut Frame);
    /// Paints sub-pane `id` (see `Chart::panes`).
    fn pane(&mut self, _id: u32, _p: &mut PaneFrame) {}
    /// A drag starts at `pos` (`on_axis`: on the price axis). True takes it (the overlay's own line or drawing, hit-tested
    /// last frame): until it ends the chart neither pans, glides nor shifts prices; the overlay reads the drag itself.
    fn claims_drag(&mut self, _pos: Pos2, _on_axis: bool) -> bool { false }
    /// A double-click at `pos` resets the view unless this is false (it hit the overlay's own object).
    fn allows_reset(&mut self, _pos: Pos2) -> bool { true }
}
impl<F: FnMut(Z, &mut Frame)> Overlay for F { fn paint(&mut self, z: Z, f: &mut Frame) { self(z, f) } }

/// No overlay.
pub struct Nothing;
impl Overlay for Nothing { fn paint(&mut self, _: Z, _: &mut Frame) {} }

/// What `Chart::show` reports besides the egui response (which it derefs to).
pub struct ChartResponse {
    pub response: Response,
    /// the view reached the oldest bar: true once per oldest bar (load an older page)
    pub reached_start: bool,
    /// (bar index, its open time, the price when over the plot) under the pointer
    pub hover: Option<(usize, i64, Option<f64>)>,
    /// the view left its reset state (panned, zoomed or a manual price range): show a Reset button, call `Chart::reset`
    pub moved: bool,
}

impl std::ops::Deref for ChartResponse {
    type Target = Response;
    fn deref(&self) -> &Response { &self.response }
}

/// The chart's view state and layout options (keep it between frames; `Default` starts at the live edge, default
/// zoom, auto-fit, linear, no inset, no panes).
pub struct Chart {
    pub view: TimeView,
    pub scale: Scale,
    /// manual price range (wheel / axis drag); None = auto-fit
    pub y_range: Option<(f64, f64)>,
    eased: Option<(String, f64, f64)>,
    /// price format (value, the grid step's decimals); None = those decimals
    pub fmt_price: Option<fn(f64, usize) -> String>,
    /// time format (ms, full: the crosshair's label, else an axis label); None = `fmt::time` (UTC)
    pub fmt_time: Option<fn(i64, bool) -> String>,
    /// height of the legend inset above the plot (points)
    pub inset_top: f32,
    /// sub-panes under the plot, top first: (id, height); `Overlay::pane` paints each
    pub panes: Vec<(u32, f32)>,
    /// price axis width, time axis height (points)
    pub axis_w: f32,
    pub time_h: f32,
    /// widest zoom, points per bar (raise it for footprints)
    pub max_bar_w: f32,
    /// the built-in last-price line and tag (off when the app draws its own)
    pub last_price: bool,
    /// the built-in crosshair (off when the app draws its own, e.g. snapping to OHLC, in `Z::Top`)
    pub crosshair: bool,
    claimed: bool,
    oldest: Option<(String, i64)>,
}

impl Default for Chart {
    fn default() -> Self {
        Chart { view: TimeView { right: 0.0, bar_w: BAR_W, fling: 0.0 }, scale: Scale::Linear, y_range: None, eased: None, fmt_price: None, fmt_time: None,
            inset_top: 0.0, panes: vec![], axis_w: AXIS_W, time_h: TIME_H, max_bar_w: 60.0, last_price: true, crosshair: true, claimed: false, oldest: None }
    }
}

/// Default price axis width and time axis height, points.
pub const AXIS_W: f32 = 70.0;
pub const TIME_H: f32 = 22.0;

impl Chart {
    /// Back to the live edge, default zoom and auto-fit (what a double-click does).
    pub fn reset(&mut self) { self.view = Chart::default().view; self.y_range = None; }

    fn moved(&self) -> bool { self.view.right != 0.0 || self.view.bar_w != BAR_W || self.y_range.is_some() }

    /// Draws the chart in the space left in `ui` and handles its pointer input; `overlay` paints at each `Z` and each pane.
    pub fn show(&mut self, ui: &mut Ui, data: &ChartData, overlay: &mut dyn Overlay) -> ChartResponse {
        let (rect, resp) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
        let mut out = ChartResponse { response: resp.clone(), reached_start: false, hover: None, moved: self.moved() };
        let panes_h: f32 = self.panes.iter().map(|p| p.1).sum();
        // the price area: legend inset on top of the plot, left of the price axis, above the panes
        let area = Rect::from_min_max(rect.min, pos2(rect.right() - self.axis_w, rect.bottom() - self.time_h - panes_h));
        let main = Rect::from_min_max(pos2(area.left(), area.top() + self.inset_top), area.max);
        let full = ui.painter_at(rect);
        let n = data.candles.len();
        if n == 0 || main.width() < 20.0 || main.height() < 20.0 { return out; }
        let on_axis = resp.interact_pointer_pos().is_some_and(|p| p.x > area.right() && p.y < area.bottom());
        if resp.drag_started() {
            let at = ui.input(|i| i.pointer.press_origin()).or(resp.interact_pointer_pos());
            self.claimed = at.is_some_and(|p| overlay.claims_drag(p, on_axis));
        }
        let reset_ok = !resp.double_clicked() || ui.input(|i| i.pointer.interact_pos()).is_none_or(|p| overlay.allows_reset(p));
        let (dt, y_wheel, reset) = pan_zoom(ui, &resp, area, n, &mut self.view, self.max_bar_w, self.claimed, on_axis, reset_ok);
        if reset { self.y_range = None; }
        // the live edge sits 3 bars in from the right; never panned past either end
        self.view.right = self.view.right.clamp(-(main.width() / self.view.bar_w) * 0.8, n as f32);
        let right_edge = n as f32 + 2.0 - self.view.right;
        let axis = BarAxis { first: right_edge - main.width() / self.view.bar_w, right_edge, bar_w: self.view.bar_w, plot: main };
        // the oldest bar in view: once per (series, oldest bar) until the view leaves it
        if axis.first >= 1.0 { self.oldest = None; } else {
            let k = (data.key.to_string(), data.candles[0].t);
            if self.oldest.as_ref() != Some(&k) { out.reached_start = true; self.oldest = Some(k); }
        }
        let i0 = axis.first.floor().max(0.0) as usize;
        let i1 = (right_edge.ceil() as usize).min(n - 1);
        if i0 > i1 { return out; }
        let vis = &data.candles[i0..=i1];
        let target = padded_range(vis.iter().flat_map(|b| [b.l, b.h]), 0.06, false);
        let (lo, hi) = price_range(ui, &resp, main, target, data.key.to_string(), &mut self.eased, &mut self.y_range, on_axis, self.claimed, dt, y_wheel);
        if resp.drag_stopped() { self.claimed = false; }
        out.moved = self.moved();
        let scale = PriceScale { lo, hi, log: self.scale == Scale::Log && lo > 0.0, r: main };
        let base = (self.scale == Scale::Percent).then_some(vis[0].c).filter(|b| *b > 0.0);

        // panes stacked under the price area, the time axis under them
        let mut top = area.bottom();
        let panes: Vec<(u32, Rect)> = self.panes.iter().map(|&(id, h)| { let r = Rect::from_min_size(pos2(rect.left(), top), vec2(area.width(), h)); top += h; (id, r) }).collect();
        let hp = resp.hover_pos();
        let hover_i = hp.filter(|p| p.x < area.right() && p.y >= area.top() && p.y < top).map(|p| axis.idx(p.x).round().clamp(i0 as f32, i1 as f32) as usize);
        let hover = hp.filter(|p| main.contains(*p)).zip(hover_i).map(|(p, i)| (i, scale.v(p.y)));
        out.hover = hover_i.map(|i| (i, data.candles[i].t, hover.map(|h| h.1)));

        // grid: price values (their step sets the decimals), or round percents from the base on a percent scale
        let n_lines = (main.height() as f64 / 60.0).max(2.0);
        let step = |g: &[f64]| { let s = g.windows(2).map(|w| w[1] - w[0]).fold(f64::MAX, f64::min); if s.is_finite() && s > 0.0 { step_dp(s) } else { 2 } };
        let gv = grid(lo, hi, n_lines);
        let dp = step(&gv);
        let mut f = Frame { painter: ui.painter_at(area), full: full.clone(), ui, response: &resp, rect, plot: main, legend: Rect::from_min_max(area.min, pos2(area.right(), main.top())),
            panes: panes.clone(), axis, scale, candles: data.candles, i0, i1, hover, hover_i, dp, base, fmt_price: self.fmt_price, tags: vec![], axis_w: self.axis_w };
        let grid_lines: Vec<(f64, String)> = match base {
            Some(b) => { let pg = grid((lo / b - 1.0) * 100.0, (hi / b - 1.0) * 100.0, n_lines); let pdp = step(&pg); pg.iter().map(|p| (b * (1.0 + p / 100.0), format!("{p:+.pdp$}%"))).collect() }
            None => gv.iter().map(|v| (*v, f.price(*v))).collect(),
        };
        // the last price's tag is reserved before any overlay places one
        let last = data.candles[n - 1];
        let last_tag = (self.last_price && scale.y(last.c) >= main.top() && scale.y(last.c) <= main.bottom()).then(|| { let r = f.axis_slot(scale.y(last.c), 18.0); f.reserve(r); r });

        overlay.paint(Z::Back, &mut f);
        for (v, _) in &grid_lines { f.painter.hline(main.x_range(), scale.y(*v), Stroke::new(1.0, line())); }
        overlay.paint(Z::BelowCandles, &mut f);
        let ppp = ui.ctx().pixels_per_point();
        let bw = tv_body_px(self.view.bar_w, ppp);
        for (k, b) in vis.iter().enumerate() {
            let col = if b.c >= b.o { up() } else { dn() };
            paint_candle(&f.painter, axis.x((i0 + k) as f32), bw, self.view.bar_w, ppp, [scale.y(b.o), scale.y(b.h), scale.y(b.l), scale.y(b.c)], col);
        }
        overlay.paint(Z::AboveCandles, &mut f);
        if self.last_price {
            let (ly, col) = (scale.y(last.c), if last.c >= last.o { up() } else { dn() });
            f.painter.extend(egui::Shape::dashed_line(&[pos2(main.left(), ly), pos2(main.right(), ly)], Stroke::new(1.0, col.linear_multiply(0.7)), 2.0, 3.0));
            if let Some(r) = last_tag { f.paint_tag(r, &f.label(last.c), col); }
        }

        // panes: a hairline on top of each, then the overlay
        let mut ranges = vec![];
        for &(id, r) in &panes {
            full.hline(rect.x_range(), r.top(), Stroke::new(1.0, line()));
            let mut pf = PaneFrame { painter: ui.painter_at(Rect::from_min_max(r.min, pos2(rect.right(), r.bottom()))), ui, response: &resp, rect: r, axis, candles: data.candles, i0, i1, hover_i, range: None };
            overlay.pane(id, &mut pf);
            ranges.push((r, pf.range));
        }
        // time axis: a label every ~100 points
        let ty = top + self.time_h / 2.0;
        let fmt_time = self.fmt_time.unwrap_or(crate::fmt::time);
        let every = ((100.0 / self.view.bar_w).ceil() as usize).max(1);
        for i in (i0..=i1).filter(|i| i % every == 0) {
            full.text(pos2(axis.x(i as f32), ty), Align2::CENTER_CENTER, fmt_time(data.candles[i].t, false), mono(11.0), mu());
        }
        // price labels last: one an axis tag covers is skipped, not half hidden
        for (v, s) in &grid_lines {
            let y = scale.y(*v);
            if f.tags.iter().any(|t| t.left() > main.right() && t.y_range().intersects(egui::Rangef::new(y - 6.0, y + 6.0))) { continue; }
            full.text(pos2(main.right() + 6.0, y), Align2::LEFT_CENTER, s, mono(11.0), mu());
        }
        // crosshair through the plot and the panes, its price / value tag and the time label
        if let (Some(i), Some(p), true) = (hover_i, hp, self.crosshair) {
            let (s, x) = (Stroke::new(1.0, dim()), axis.x(i as f32));
            full.extend(egui::Shape::dashed_line(&[pos2(x, area.top()), pos2(x, top)], s, 3.0, 3.0));
            let value = if main.contains(p) { Some((main, f.label(scale.v(p.y)))) } else {
                ranges.iter().find_map(|(r, rg)| { let (lo, hi) = (*rg)?; let ir = inner(*r); (ir.y_range().contains(p.y)).then(|| (*r, fmt_big(lo + (ir.bottom() - p.y) as f64 / ir.height() as f64 * (hi - lo)))) })
            };
            if let Some((r, text)) = value {
                full.extend(egui::Shape::dashed_line(&[pos2(r.left(), p.y), pos2(r.right(), p.y)], s, 3.0, 3.0));
                f.paint_tag(f.axis_slot(p.y, 18.0), &text, cross_bg());
            }
            let g = full.layout_no_wrap(fmt_time(data.candles[i].t, true), mono_m(11.0), Color32::WHITE);
            let tl = Rect::from_center_size(pos2(x, ty), vec2(g.size().x + 12.0, self.time_h - 2.0));
            full.rect_filled(tl, 2, cross_bg());
            full.galley(tl.center() - g.size() / 2.0, g, Color32::WHITE);
        }
        overlay.paint(Z::Top, &mut f);
        full.rect_stroke(rect, 0, Stroke::new(1.0, label_bg()), egui::StrokeKind::Inside);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Event, PointerButton, RawInput};

    fn walk(n: i64) -> Vec<Candle> {
        (0..n).map(|i| { let c = 100.0 + (i as f64 * 0.1).sin() * 5.0; Candle { t: i * 60_000, o: c - 0.5, h: c + 1.0, l: c - 1.0, c, v: 1.0 } }).collect()
    }

    /// Runs one 800 x 500 frame per event batch; returns the last response's fields.
    fn run(chart: &mut Chart, candles: &[Candle], frames: Vec<Vec<Event>>, ov: &mut dyn Overlay) -> Vec<(bool, bool)> {
        let ctx = egui::Context::default();
        let mut res = vec![];
        for ev in frames {
            let input = RawInput { events: ev, screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(800.0, 500.0))), ..Default::default() };
            let mut out = ctx.run_ui(input, |ui| {
                let r = chart.show(ui, &ChartData { candles, bar_ms: 60_000, key: "X" }, ov);
                res.push((r.reached_start, r.moved));
            });
            // no renderer: the texture uploads are dropped on purpose
            out.textures_delta.clear();
        }
        res
    }

    fn press(x: f32, down: bool) -> Event { Event::PointerButton { pos: pos2(x, 100.0), button: PointerButton::Primary, pressed: down, modifiers: Default::default() } }
    fn drag(x0: f32, x1: f32) -> Vec<Vec<Event>> {
        let mv = |x| vec![Event::PointerMoved(pos2(x, 100.0))];
        vec![mv(x0), vec![press(x0, true)], mv((x0 + x1) / 2.0), mv(x1), vec![press(x1, false)], vec![]]
    }

    /// Records the order of calls and what it was given.
    #[derive(Default)]
    struct Rec { seen: Vec<String>, claim: bool, plot: Option<Rect>, legend: Option<Rect>, panes: Vec<(u32, Rect)>, labels: Vec<String>, log_mid: f32 }
    impl Overlay for Rec {
        fn paint(&mut self, z: Z, f: &mut Frame) {
            self.seen.push(format!("{z:?}"));
            (self.plot, self.legend, self.panes) = (Some(f.plot), Some(f.legend), f.panes.clone());
            if let Some(b) = f.base { self.labels = vec![f.label(b), f.label(b * 1.05)]; }
            self.log_mid = f.y((f.scale.lo * f.scale.hi).sqrt());
        }
        fn pane(&mut self, id: u32, p: &mut PaneFrame) {
            self.seen.push(format!("pane{id}"));
            assert_eq!(Some(p.rect), self.panes.iter().find(|x| x.0 == id).map(|x| x.1));
            p.range = Some((0.0, 1.0));
        }
        fn claims_drag(&mut self, _: Pos2, _: bool) -> bool { self.claim }
    }

    #[test]
    fn shows_candles_and_calls_every_overlay_depth() {
        let candles = walk(300);
        let mut chart = Chart::default();
        let mut seen = vec![];
        run(&mut chart, &candles, vec![vec![], vec![]], &mut |z: Z, f: &mut Frame| {
            seen.push(z);
            // the visible bars end at the newest, and the transforms agree with the plot
            assert_eq!(f.i1, 299);
            assert!(f.x(f.i1) < f.plot.right() && f.y(100.0) > f.plot.top() && f.y(100.0) < f.plot.bottom());
        });
        assert_eq!(&seen[..4], &[Z::Back, Z::BelowCandles, Z::AboveCandles, Z::Top]);
    }

    #[test]
    fn layout_inset_panes_and_depth_order() {
        let mut chart = Chart { inset_top: 40.0, panes: vec![(1, 80.0), (2, 60.0)], axis_w: 80.0, time_h: 30.0, ..Default::default() };
        let mut rec = Rec::default();
        run(&mut chart, &walk(300), vec![vec![]], &mut rec);
        // 500 high: 30 time axis, 140 of panes, the price area above them with the legend on top
        assert_eq!(rec.legend.unwrap(), Rect::from_min_max(pos2(0.0, 0.0), pos2(720.0, 40.0)));
        assert_eq!(rec.plot.unwrap(), Rect::from_min_max(pos2(0.0, 40.0), pos2(720.0, 330.0)));
        assert_eq!(rec.panes, vec![(1, Rect::from_min_max(pos2(0.0, 330.0), pos2(720.0, 410.0))), (2, Rect::from_min_max(pos2(0.0, 410.0), pos2(720.0, 470.0)))]);
        assert_eq!(rec.seen, ["Back", "BelowCandles", "AboveCandles", "pane1", "pane2", "Top"]);
    }

    #[test]
    fn percent_and_log_scales() {
        let candles = walk(300);
        let mut rec = Rec::default();
        run(&mut Chart { scale: Scale::Percent, ..Default::default() }, &candles, vec![vec![]], &mut rec);
        assert_eq!(rec.labels, ["+0.00%", "+5.00%"]);
        // log: the geometric mean of the range sits mid-plot
        run(&mut Chart { scale: Scale::Log, ..Default::default() }, &candles, vec![vec![]], &mut rec);
        assert!((rec.log_mid - rec.plot.unwrap().center().y).abs() < 0.5, "{} {:?}", rec.log_mid, rec.plot);
    }

    #[test]
    fn a_claimed_drag_does_not_pan() {
        let candles = walk(300);
        let mut chart = Chart::default();
        run(&mut chart, &candles, drag(100.0, 300.0), &mut Rec { claim: true, ..Default::default() });
        assert_eq!(chart.view.right, 0.0);
        let r = run(&mut chart, &candles, drag(100.0, 300.0), &mut Rec::default());
        assert!(chart.view.right > 10.0 && r.last().unwrap().1, "{:?}", chart.view);
    }

    #[test]
    fn reached_start_fires_once_per_oldest_bar() {
        // 50 bars fit the plot: the oldest is in view from the first frame
        let mut chart = Chart::default();
        let mut candles = walk(50);
        let r = run(&mut chart, &candles, vec![vec![], vec![], vec![]], &mut Nothing);
        assert_eq!(r.iter().map(|x| x.0).collect::<Vec<_>>(), [true, false, false]);
        // an older page arrived: a new oldest bar fires again
        candles.splice(0..0, walk(20).into_iter().map(|c| Candle { t: c.t - 20 * 60_000, ..c }));
        let r = run(&mut chart, &candles, vec![vec![], vec![]], &mut Nothing);
        assert_eq!(r.iter().map(|x| x.0).collect::<Vec<_>>(), [true, false]);
    }
}

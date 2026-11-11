//! Drawings anchored in (bar time, price), so they survive interval changes: the tools, the model and its geometry
//! (trend / ray levels, channel parallels, the position tool's size).
use crate::fmt::span;
use crate::style::{accent, dn, fg, label_bg, mono, mu, panel2, prop, text, up, warn, Text};
use egui::{pos2, vec2, Align2, Color32, Painter, Pos2, Rect, Stroke};

fn rgb(c: u32) -> Color32 { Color32::from_rgb((c >> 16) as u8, (c >> 8) as u8, c as u8) }

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Tool { HLine, Trend, Ray, Fib, Rect, Channel, Text, Measure, Position }

impl Tool {
    /// clicks needed to place it
    pub fn points(self) -> usize { match self { Tool::HLine | Tool::Text => 1, Tool::Channel => 3, _ => 2 } }
}

/// `hot` part of a locked drawing: right-click opens its menu, nothing moves it.
pub const LOCKED: u8 = 9;

/// What the right-click menu is for: a drawing (index, focus its text field), or the empty plot (point).
#[derive(Clone, Copy, Debug)]
pub enum Menu { Drawing(usize, bool), Chart((i64, f64)) }

/// Fibonacci retracement levels drawn between a drawing's two points (0 at the second point).
pub const FIB: [f64; 7] = [0.0, 0.236, 0.382, 0.5, 0.618, 0.786, 1.0];

/// A user drawing anchored in (bar time ms, price), so it survives timeframe changes.
/// `col` 0xRRGGBB (0 = the accent), `dash` 0 solid / 1 dashed / 2 dotted; `c` is a channel's third
/// point (on its parallel line), `text` a note's text; `meta` is the app's own, kept with the drawing (Depth: the sound
/// of an armed price alert; stored under the key "alert", the name settings have always used).
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Drawing {
    pub tool: Tool, pub a: (i64, f64), pub b: (i64, f64),
    #[cfg_attr(feature = "serde", serde(default))] pub col: u32,
    #[cfg_attr(feature = "serde", serde(default))] pub dash: u8,
    #[cfg_attr(feature = "serde", serde(default))] pub c: (i64, f64),
    #[cfg_attr(feature = "serde", serde(default))] pub text: String,
    #[cfg_attr(feature = "serde", serde(default))] pub locked: bool,
    #[cfg_attr(feature = "serde", serde(default, rename = "alert"))] pub meta: Option<String>,
}

impl Drawing {
    pub fn new(tool: Tool, a: (i64, f64), b: (i64, f64)) -> Self { Drawing { tool, a, b, col: 0, dash: 0, c: a, text: String::new(), locked: false, meta: None } }
    pub fn color(&self) -> Color32 { if self.col == 0 { accent() } else { rgb(self.col) } }
    /// the a-b line's price at time t (None when a and b share a time)
    pub fn line_at(&self, t: i64) -> Option<f64> {
        let (ta, tb) = (self.a.0, self.b.0);
        (ta != tb).then(|| self.a.1 + (self.b.1 - self.a.1) * (t - ta) as f64 / (tb - ta) as f64)
    }
    /// a channel's parallel line, as a price offset from the a-b line
    pub fn offset(&self) -> f64 { self.c.1 - self.line_at(self.c.0).unwrap_or(self.a.1) }
    /// third point stored halfway along the parallel line, so its handle sits there
    pub fn with_c(mut self, c: (i64, f64)) -> Self {
        let off = c.1 - self.line_at(c.0).unwrap_or(self.a.1);
        let mt = (self.a.0 + self.b.0) / 2;
        self.c = (mt, self.line_at(mt).unwrap_or(self.a.1) + off);
        self
    }
    /// A drawing from its placed points: a channel's third point goes on its parallel line, a
    /// position tool (entry, stop) gets a 2R target.
    pub fn placed(tool: Tool, pts: &[(i64, f64)]) -> Self {
        let mut d = Drawing::new(tool, pts[0], *pts.get(1).unwrap_or(&pts[0]));
        if let Some(c) = pts.get(2) { d = d.with_c(*c); }
        if tool == Tool::Position { d.c = (d.a.0, d.a.1 * 3.0 - d.b.1 * 2.0); }
        d
    }
    /// a level an alert can watch (horizontal line, trend line, ray)
    pub fn alertable(&self) -> bool { matches!(self.tool, Tool::HLine | Tool::Trend | Tool::Ray) }
    /// the line's price at time t, for alerts: a trend line only within its two points, a ray from its start on
    pub fn level_at(&self, t: i64) -> Option<f64> {
        match self.tool {
            Tool::HLine => Some(self.a.1),
            Tool::Trend if t >= self.a.0.min(self.b.0) && t <= self.a.0.max(self.b.0) => self.line_at(t),
            Tool::Ray if (t - self.a.0).signum() * (self.b.0 - self.a.0).signum() >= 0 => self.line_at(t),
            _ => None,
        }
    }
}

/// Swatches of the drawing context menu (TradingView's default palette row).
pub const DRAW_COLORS: [u32; 8] = [0x2962ff, 0x089981, 0xf23645, 0xff9800, 0xffeb3b, 0x9c27b0, 0x00bcd4, 0x787b86];

/// Distance from `p` to the segment a-b.
pub fn dist_to_segment(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let ab = b - a;
    let t = if ab.length_sq() > 0.0 { ((p - a).dot(ab) / ab.length_sq()).clamp(0.0, 1.0) } else { 0.0 };
    (a + ab * t - p).length()
}

/// How drawings meet the screen: a (time, price) point's position, a price's y, and the plot (a horizontal line
/// spans it).
pub struct Geom<'a> { pub to_screen: &'a dyn Fn((i64, f64)) -> Pos2, pub y: &'a dyn Fn(f64) -> f32, pub main: Rect }

impl Drawing {
    /// a channel's middle handle: halfway along its parallel line
    pub fn mid_c(&self) -> (i64, f64) { ((self.a.0 + self.b.0) / 2, (self.a.1 + self.b.1) / 2.0 + self.offset()) }

    /// Moves the grabbed part by `mv` (a point to where the pointer moved it): 0 / 1 the first / second point, 3 the
    /// third (a channel's parallel, a position's target), anything else the whole drawing.
    pub fn drag_part(&mut self, part: u8, mv: impl Fn((i64, f64)) -> (i64, f64)) {
        match part {
            0 => self.a = mv(self.a),
            1 => self.b = mv(self.b),
            3 if self.tool == Tool::Position => self.c.1 = mv((self.a.0, self.c.1)).1,
            3 => { let m = self.mid_c(); self.c = (m.0, mv(m).1); }
            _ => { self.a = mv(self.a); self.b = mv(self.b); if matches!(self.tool, Tool::Channel | Tool::Position) { self.c = mv(self.c); } }
        }
        if self.tool == Tool::Position { self.c.0 = self.a.0; }
        if matches!(self.tool, Tool::HLine | Tool::Text) { self.b = self.a; }
    }
}

/// A position tool's horizontal extent on screen (at least 120 points wide).
pub fn pos_x(d: &Drawing, g: &Geom) -> (f32, f32) {
    let (x0, x1) = ((g.to_screen)(d.a).x, (g.to_screen)(d.b).x);
    (x0.min(x1), x0.max(x1).max(x0.min(x1) + 120.0))
}

/// A drawing's screen segments, for painting and hit-testing.
pub fn segs(d: &Drawing, g: &Geom) -> Vec<(Pos2, Pos2)> {
    let (ts, y) = (g.to_screen, g.y);
    let (a, b) = (ts(d.a), ts(d.b));
    match d.tool {
        Tool::HLine => vec![(pos2(g.main.left(), a.y), pos2(g.main.right(), a.y))],
        Tool::Trend | Tool::Measure => vec![(a, b)],
        Tool::Ray => { let dir = b - a; let k = if dir.length() > 0.0 { 4000.0 / dir.length() } else { 0.0 }; vec![(a, a + dir * k)] }
        Tool::Rect => { let r = Rect::from_two_pos(a, b); vec![(r.left_top(), r.right_top()), (r.right_top(), r.right_bottom()), (r.right_bottom(), r.left_bottom()), (r.left_bottom(), r.left_top())] }
        Tool::Fib => FIB.iter().map(|f| { let yy = y(d.b.1 + (d.a.1 - d.b.1) * f); (pos2(a.x.min(b.x), yy), pos2(a.x.max(b.x), yy)) }).collect(),
        Tool::Channel => { let o = d.offset(); vec![(a, b), (ts((d.a.0, d.a.1 + o)), ts((d.b.0, d.b.1 + o)))] }
        Tool::Text => vec![],
        Tool::Position => { let (x0, x1) = pos_x(d, g); [d.a.1, d.b.1, d.c.1].iter().map(|p| (pos2(x0, y(*p)), pos2(x1, y(*p)))).collect() }
    }
}

/// A drawing's drag handles: (part, position).
pub fn handles(d: &Drawing, g: &Geom) -> Vec<(u8, Pos2)> {
    let ts = g.to_screen;
    match d.tool {
        Tool::HLine | Tool::Text => vec![],
        Tool::Channel => vec![(0, ts(d.a)), (1, ts(d.b)), (3, ts(d.mid_c()))],
        Tool::Position => { let x1 = pos_x(d, g).1; vec![(0, ts(d.a)), (1, pos2(x1, (g.y)(d.b.1))), (3, pos2(ts(d.a).x, (g.y)(d.c.1)))] }
        _ => vec![(0, ts(d.a)), (1, ts(d.b))],
    }
}

/// The part under `p`: a handle of an unlocked drawing first (newest on top), then the nearest line or a box's inside;
/// a locked drawing answers with `LOCKED` (its menu only). `note_rect`: a text note's box.
pub fn hit(list: &[Drawing], p: Pos2, g: &Geom, note_rect: &dyn Fn(&Drawing) -> Rect) -> Option<(usize, u8)> {
    for (i, d) in list.iter().enumerate().rev().filter(|(_, d)| !d.locked) {
        for (k, q) in handles(d, g) { if (q - p).length() < 7.0 { return Some((i, k)); } }
    }
    list.iter().enumerate().filter_map(|(i, d)| {
        let dist = segs(d, g).iter().map(|(a, b)| dist_to_segment(p, *a, *b)).fold(f32::MAX, f32::min);
        let inside = match d.tool {
            Tool::Rect | Tool::Measure => Rect::from_two_pos((g.to_screen)(d.a), (g.to_screen)(d.b)).contains(p),
            Tool::Text => note_rect(d).contains(p),
            Tool::Position => { let (x0, x1) = pos_x(d, g); let y = g.y; Rect::from_x_y_ranges(x0..=x1, y(d.b.1).min(y(d.c.1))..=y(d.b.1).max(y(d.c.1))).contains(p) }
            _ => false,
        };
        (dist < 6.0 || inside).then_some((i, if inside { 5.9 } else { dist }))
    }).min_by(|x, z| x.1.total_cmp(&z.1)).map(|(i, _)| (i, if list[i].locked { LOCKED } else { 2 }))
}

/// What painting a drawing needs from the app: painters (clipped to the plot, and whole for the price-axis tag), the
/// geometry, the axis width, the bar length (ms), the base asset's name, the position tool's equity and risk, and the
/// app's price / size / USD formats.
pub struct PaintCx<'a> {
    pub clip: &'a Painter, pub painter: &'a Painter, pub plot: Rect, pub axis_w: f32, pub ms: i64, pub base: &'a str,
    pub equity: Option<f64>, pub risk_pct: f64,
    pub fmt_px: &'a dyn Fn(f64) -> String, pub fmt_qty: &'a dyn Fn(f64) -> String, pub fmt_usd: &'a dyn Fn(f64) -> String,
}

/// A text note's layout (its text, or the hint while empty).
pub fn note(p: &Painter, d: &Drawing) -> std::sync::Arc<egui::Galley> {
    p.layout_no_wrap(if d.text.is_empty() { text(Text::TextHint) } else { d.text.clone() }, prop(13.0), d.color())
}

/// A text note's box on screen.
pub fn note_rect(d: &Drawing, g: &Geom, gl: &egui::Galley) -> Rect {
    let a = (g.to_screen)(d.a);
    Rect::from_min_size(pos2(a.x, a.y - gl.size().y), gl.size()).expand(4.0)
}

/// Paints one drawing (TradingView's look per tool: boxes, Fibonacci bands, channel, measure label, position boxes
/// with target / stop / size tags); `emph` when hovered or selected (thicker, handles), `alpha` for previews.
pub fn paint(cx: &PaintCx, g: &Geom, d: &Drawing, emph: bool, alpha: f32) {
    let (clip, painter, plot, ms) = (cx.clip, cx.painter, cx.plot, cx.ms);
    let (to_screen, y) = (g.to_screen, g.y);
    let (equity, risk_pct) = (cx.equity, cx.risk_pct);
        let col = d.color().linear_multiply(alpha);
        let s = Stroke::new(if emph { 2.0 } else { 1.2 }, col);
        let seg = |p0: Pos2, p1: Pos2, s: Stroke| match d.dash {
            0 => { clip.line_segment([p0, p1], s); }
            k => { let (on, off) = if k == 1 { (6.0, 4.0) } else { (1.5, 3.0) }; clip.extend(egui::Shape::dashed_line(&[p0, p1], s, on, off)); }
        };
        let (a, b) = (to_screen(d.a), to_screen(d.b));
        match d.tool {
            Tool::Rect => {
                let r = Rect::from_two_pos(a, b);
                clip.rect_filled(r, 0, col.linear_multiply(0.12));
                for (p0, p1) in segs(d, g) { seg(p0, p1, s); }
            }
            Tool::Fib => {
                let (x0, x1) = (a.x.min(b.x), a.x.max(b.x));
                let px = |f: f64| d.b.1 + (d.a.1 - d.b.1) * f;
                let fib_col = |f: f64| if f == 0.5 { fg().linear_multiply(0.7) } else if f == 0.618 { warn() } else if f == 0.0 || f == 1.0 { mu() } else { accent() };
                for w in FIB.windows(2) {
                    clip.rect_filled(Rect::from_two_pos(pos2(x0, y(px(w[0]))), pos2(x1, y(px(w[1])))), 0, fib_col(w[1]).linear_multiply(0.06 * alpha));
                }
                for f in FIB {
                    let yy = y(px(f));
                    seg(pos2(x0, yy), pos2(x1, yy), Stroke::new(s.width, fib_col(f).linear_multiply(alpha)));
                    clip.text(pos2(x0 + 3.0, yy - 1.0), Align2::LEFT_BOTTOM, format!("{f} ({})", (cx.fmt_px)(px(f))), mono(10.0), fib_col(f).linear_multiply(alpha));
                }
                clip.extend(egui::Shape::dashed_line(&[a, b], Stroke::new(1.0, mu().linear_multiply(0.6 * alpha)), 4.0, 4.0));
            }
            Tool::Channel => {
                let l = segs(d, g);
                let (p, q) = (l[0], l[1]);
                clip.add(egui::Shape::convex_polygon(vec![p.0, p.1, q.1, q.0], col.linear_multiply(0.08), Stroke::NONE));
                seg(p.0, p.1, s);
                seg(q.0, q.1, s);
                clip.extend(egui::Shape::dashed_line(&[p.0 + (q.0 - p.0) / 2.0, p.1 + (q.1 - p.1) / 2.0], Stroke::new(1.0, col.linear_multiply(0.6)), 4.0, 4.0));
            }
            Tool::Measure => {
                // TradingView's measure: change, percent, bars and time between the two points
                let r = Rect::from_two_pos(a, b);
                let dp = d.b.1 - d.a.1;
                let mc = if dp >= 0.0 { up() } else { dn() }.linear_multiply(alpha);
                clip.rect_filled(r, 0, mc.linear_multiply(0.15));
                clip.extend([egui::Shape::line_segment([pos2(r.center().x, a.y), pos2(r.center().x, b.y)], Stroke::new(1.2, mc)),
                             egui::Shape::line_segment([pos2(a.x, r.center().y), pos2(b.x, r.center().y)], Stroke::new(1.2, mc))]);
                let n = ((d.b.0 - d.a.0) as f64 / ms as f64).round() as i64;
                let pct = if d.a.1 != 0.0 { dp / d.a.1 * 100.0 } else { 0.0 };
                let txt = format!("{}{} ({pct:+.2}%)\n{n} {} · {}", if dp >= 0.0 { "+" } else { "" }, (cx.fmt_px)(dp), text(Text::Bars), span(n.abs() * ms));
                let g = painter.layout(txt, mono(11.0), Color32::WHITE, 260.0);
                let below = b.y >= a.y;
                let lr = Rect::from_min_size(pos2(r.center().x - g.size().x / 2.0 - 6.0, if below { r.bottom() + 6.0 } else { r.top() - g.size().y - 14.0 }), g.size() + vec2(12.0, 8.0));
                // kept inside the plot so neither line is clipped
                let lr = lr.translate(vec2(0.0, (plot.top() + 2.0 - lr.top()).max(0.0) + (plot.bottom() - 2.0 - lr.bottom()).min(0.0)));
                clip.rect_filled(lr, 4, mc.linear_multiply(0.9));
                clip.galley(lr.min + vec2(6.0, 4.0), g, Color32::WHITE);
            }
            Tool::Position => {
                // TradingView's long / short position: reward box to the target, risk box to the
                // stop, and the size that risks `risk_pct` of equity between entry and stop
                let (x0, x1) = pos_x(d, g);
                let (ye, ys, yt) = (y(d.a.1), y(d.b.1), y(d.c.1));
                clip.rect_filled(Rect::from_x_y_ranges(x0..=x1, ye.min(yt)..=ye.max(yt)), 0, up().linear_multiply(0.16 * alpha));
                clip.rect_filled(Rect::from_x_y_ranges(x0..=x1, ye.min(ys)..=ye.max(ys)), 0, dn().linear_multiply(0.16 * alpha));
                for (yy, c) in [(ye, fg()), (ys, dn()), (yt, up())] { clip.hline(x0..=x1, yy, Stroke::new(1.2, c.linear_multiply(alpha))); }
                let pct = |p: f64| if d.a.1 != 0.0 { (p / d.a.1 - 1.0) * 100.0 } else { 0.0 };
                let risk = (d.a.1 - d.b.1).abs();
                let rr = if risk > 0.0 { (d.c.1 - d.a.1).abs() / risk } else { 0.0 };
                let long = d.c.1 > d.a.1;
                let size = match position_qty(equity, risk_pct, d.a.1, d.b.1) {
                    Some(q) => format!("{} {} · {risk_pct}% = ${}", (cx.fmt_qty)(q), cx.base, (cx.fmt_usd)(q * risk)),
                    None => format!("{risk_pct}%"),
                };
                let tag = |yy: f64, txt: String, c: Color32, below: bool| {
                    let g = painter.layout_no_wrap(txt, mono(10.5), Color32::WHITE);
                    let r = Rect::from_min_size(pos2(x0 + 4.0, if below { yy as f32 + 3.0 } else { yy as f32 - g.size().y - 7.0 }), g.size() + vec2(8.0, 4.0));
                    clip.rect_filled(r, 3, c.linear_multiply(0.9 * alpha));
                    clip.galley(r.min + vec2(4.0, 2.0), g, Color32::WHITE);
                };
                tag(yt as f64, format!("{} {} ({:+.2}%)  R:R {rr:.2}", text(Text::PosTarget), (cx.fmt_px)(d.c.1), pct(d.c.1)), up(), !long);
                tag(ys as f64, format!("{} {} ({:+.2}%)", text(Text::PosStop), (cx.fmt_px)(d.b.1), pct(d.b.1)), dn(), long);
                tag(ye as f64, format!("{} {} {} · {size}", if long { text(Text::PosLong) } else { text(Text::PosShort) }, (cx.fmt_px)(d.a.1), text(Text::PosQty)), label_bg(), !long);
            }
            Tool::Text => {
                let gl = note(painter, d);
                let r = note_rect(d, g, &gl);
                if emph { clip.rect_stroke(r, 3, Stroke::new(1.0, col.linear_multiply(0.6)), egui::StrokeKind::Inside); }
                clip.galley(r.min + vec2(4.0, 4.0), gl, col);
            }
            _ => for (p0, p1) in segs(d, g) { seg(p0, p1, s); },
        }
    if d.tool == Tool::HLine {
        let a = to_screen(d.a);
        let lab = Rect::from_min_size(pos2(g.main.right() + 1.0, a.y - 9.0), vec2(cx.axis_w - 2.0, 18.0));
        painter.rect_filled(lab, 2, d.color());
        painter.text(lab.left_center() + vec2(5.0, 0.0), Align2::LEFT_CENTER, (cx.fmt_px)(d.a.1), mono(11.0), Color32::BLACK);
    } else if emph && !d.locked {
        // handles show on hover: hollow, as in TradingView
        for (_, p) in handles(d, g) { clip.circle(p, 4.0, panel2(), Stroke::new(1.5, d.color())); }
    }
}

/// Position tool size: `risk_pct` of `equity` risked over the entry-stop distance.
pub fn position_qty(equity: Option<f64>, risk_pct: f64, entry: f64, stop: f64) -> Option<f64> {
    let risk = (entry - stop).abs();
    equity.filter(|_| risk > 0.0).map(|eq| eq * risk_pct / 100.0 / risk)
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn position_qty_matches_label() {
        // 1 % of 10,000 is 100 USDT risked over a 5 USDT stop distance: 20 coins, either side
        assert_eq!(position_qty(Some(10_000.0), 1.0, 100.0, 95.0), Some(20.0));
        assert_eq!(position_qty(Some(10_000.0), 0.5, 100.0, 104.0), Some(12.5));
        assert_eq!(position_qty(None, 1.0, 100.0, 95.0), None);
        assert_eq!(position_qty(Some(10_000.0), 1.0, 100.0, 100.0), None);
    }

    #[test]
    fn levels_and_channels() {
        // a ray from its start on, a trend line only between its points
        let r = Drawing::new(Tool::Ray, (1000, 10.0), (2000, 20.0));
        assert_eq!((r.level_at(500), r.level_at(3000)), (None, Some(30.0)));
        let t = Drawing { tool: Tool::Trend, ..r.clone() };
        assert_eq!((t.level_at(1500), t.level_at(3000)), (Some(15.0), None));
        // a channel keeps its third point halfway along the parallel line
        let ch = Drawing::new(Tool::Channel, (0, 10.0), (100, 20.0)).with_c((100, 25.0));
        assert_eq!((ch.c, ch.offset()), ((50, 20.0), 5.0));
    }

    #[test]
    fn hit_prefers_handles_then_lines_and_drags_parts() {
        // time ms = x, price = 100 - y: a 1:1 screen
        let ts = |p: (i64, f64)| pos2(p.0 as f32, (100.0 - p.1) as f32);
        let y = |v: f64| (100.0 - v) as f32;
        let g = Geom { to_screen: &ts, y: &y, main: Rect::from_min_max(pos2(0.0, 0.0), pos2(200.0, 100.0)) };
        let list = vec![Drawing::new(Tool::Trend, (10, 50.0), (90, 50.0)), Drawing::new(Tool::HLine, (0, 20.0), (0, 20.0))];
        assert_eq!(hit(&list, pos2(11.0, 50.0), &g, &|_| Rect::NOTHING), Some((0, 0)));
        assert_eq!(hit(&list, pos2(50.0, 52.0), &g, &|_| Rect::NOTHING), Some((0, 2)));
        assert_eq!(hit(&list, pos2(150.0, 80.0), &g, &|_| Rect::NOTHING), Some((1, 2)));
        assert_eq!(hit(&list, pos2(150.0, 40.0), &g, &|_| Rect::NOTHING), None);
        // dragging the second point moves only it; a horizontal line keeps b on a
        let mut d = list[0].clone();
        d.drag_part(1, |p| (p.0 + 5, p.1 + 1.0));
        assert_eq!((d.a, d.b), ((10, 50.0), (95, 51.0)));
        let mut h = list[1].clone();
        h.drag_part(2, |p| (p.0, p.1 + 3.0));
        assert_eq!((h.a, h.b), ((0, 23.0), (0, 23.0)));
    }

    #[cfg(feature = "serde")]
    #[test]
    fn the_app_field_keeps_its_settings_key() {
        let mut d = Drawing::new(Tool::HLine, (0, 1.0), (0, 1.0));
        d.meta = Some("depth:chime".into());
        let j = serde_json::to_value(&d).unwrap();
        assert_eq!(j["alert"], "depth:chime");
        let back: Drawing = serde_json::from_value(j).unwrap();
        assert_eq!(back, d);
    }
}

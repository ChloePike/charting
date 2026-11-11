//! Candle geometry on device pixels, the way TradingView's lightweight-charts draws it.
use egui::{pos2, Color32, Painter, Rect};

/// A candle's body and wick spans (x, in points) on whole device pixels: the wick one point wide (whole pixels), the body `bw` wide (at most
/// the spacing) rounded down to the wick's parity so the wick sits in its exact middle.
pub struct CandlePx { pub body: (f32, f32), pub wick: (f32, f32) }

/// TradingView's candle body width (lightweight-charts `optimalCandlestickWidth`), in points: nearly the whole spacing
/// when bars are dense, easing to 80% as they spread (1 - 0.2 * atan(spacing - 4) / (pi / 2)), 3 px between 2.5 and 4,
/// no gap kept, so dense candles touch as they do there (a fixed 80% with a 1 px gap read loose next to it).
pub fn tv_body_px(spacing: f32, ppp: f32) -> f32 {
    if (2.5..=4.0).contains(&spacing) { return (3.0 * ppp).floor() / ppp; }
    let coeff = 1.0 - 0.2 * (spacing.max(4.0) - 4.0).atan() / std::f32::consts::FRAC_PI_2;
    (spacing * coeff * ppp).floor().min((spacing * ppp).floor()).max(ppp.floor()) / ppp
}

pub fn candle_px(x: f32, bw: f32, spacing: f32, ppp: f32) -> CandlePx {
    let wick = ppp.floor().max(1.0);
    let mut body = (bw * ppp).floor().min((spacing * ppp).floor()).max(wick);
    if (body - wick) % 2.0 != 0.0 { body -= 1.0; }
    let body = body.max(wick);
    let c = (x * ppp - wick / 2.0).round();
    let l = c - (body - wick) / 2.0;
    CandlePx { body: (l / ppp, (l + body) / ppp), wick: (c / ppp, (c + wick) / ppp) }
}

/// One candle centred on `x`: `bw` body width and `spacing` between bars (points), `ppp` pixels per point, `y` the
/// screen y of open, high, low and close. Wick and body on whole device pixels, both at least one pixel tall.
pub fn paint_candle(p: &Painter, x: f32, bw: f32, spacing: f32, ppp: f32, y: [f32; 4], col: Color32) {
    let snap = |v: f32| (v * ppp).round() / ppp;
    let [yo, yh, yl, yc] = y.map(snap);
    let c = candle_px(x, bw, spacing, ppp);
    p.rect_filled(Rect::from_min_max(pos2(c.wick.0, yh.min(yl)), pos2(c.wick.1, yh.max(yl).max(yh.min(yl) + 1.0 / ppp))), 0, col);
    let (top, bot) = (yo.min(yc), yo.max(yc));
    p.rect_filled(Rect::from_min_max(pos2(c.body.0, top), pos2(c.body.1, bot.max(top + 1.0 / ppp))), 0, col);
}

#[cfg(test)]
mod candle_tests {
    use super::candle_px;

    #[test]
    fn body_width_follows_tradingview() {
        // dense bars nearly fill the spacing, wide ones ease to 80%
        assert_eq!(super::tv_body_px(3.0, 2.0), 3.0);
        assert!(super::tv_body_px(7.0, 2.0) >= 5.5, "{}", super::tv_body_px(7.0, 2.0));
        let w = super::tv_body_px(42.0, 1.0);
        assert!((33.0..=34.0).contains(&w), "{w}");
    }

    #[test]
    fn candles_sit_on_pixels_with_a_centred_wick() {
        for ppp in [1.0f32, 2.0] {
            for spacing in [2.0f32, 3.3, 7.0, 10.6, 25.0] {
                for x in [100.0f32, 100.37, 101.5] {
                    let c = candle_px(x, super::tv_body_px(spacing, ppp), spacing, ppp);
                    let px = |v: f32| v * ppp;
                    for v in [c.body.0, c.body.1, c.wick.0, c.wick.1] { assert!((px(v) - px(v).round()).abs() < 1e-3, "{ppp} {spacing} {x}: {v}"); }
                    // the wick in the exact middle of the body
                    assert!(((c.wick.0 - c.body.0) - (c.body.1 - c.wick.1)).abs() < 1e-4, "{ppp} {spacing} {x}");
                    // never wider than the spacing (neighbours may touch, as on TradingView)
                    assert!(px(c.body.1 - c.body.0) <= (spacing * ppp).floor(), "{ppp} {spacing}");
                }
            }
        }
    }
}

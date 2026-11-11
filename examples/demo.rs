//! `cargo run -p charting --example demo`: a random walk, with a moving average and a level drawn through `Overlay`.
use charting::{egui, ta, Candle, Chart, ChartData, Frame, Z};

fn main() -> eframe::Result {
    // a deterministic random walk (xorshift), one bar a minute
    let mut s = 0x2545_f491_4f6c_dd1du64;
    let mut rnd = move || { s ^= s << 13; s ^= s >> 7; s ^= s << 17; (s >> 11) as f64 / (1u64 << 53) as f64 - 0.5 };
    let mut c = 100.0;
    let candles: Vec<Candle> = (0..2000).map(|i| {
        let o = c;
        c += rnd() * 1.2;
        let (h, l) = (o.max(c) + rnd().abs() * 0.6, o.min(c) - rnd().abs() * 0.6);
        Candle { t: 1_700_000_000_000 + i * 60_000, o, h, l, c, v: 1.0 + rnd().abs() * 10.0 }
    }).collect();
    let ma = ta::sma(&candles.iter().map(|b| b.c).collect::<Vec<_>>(), 20);
    let level = candles.iter().map(|b| b.h).fold(f64::MIN, f64::max);
    let mut chart = Chart::default();
    eframe::run_ui_native("charting demo", eframe::NativeOptions::default(), move |ui, _| {
        chart.show(ui, &ChartData { candles: &candles, bar_ms: 60_000, key: "demo" }, &mut |z: Z, f: &mut Frame| {
            if z != Z::AboveCandles { return; }
            // a 20-bar average over the visible bars
            let pts: Vec<egui::Pos2> = (f.i0..=f.i1).filter_map(|i| Some(egui::pos2(f.x(i), f.y(ma[i]?)))).collect();
            f.painter.add(egui::Shape::line(pts, egui::Stroke::new(1.3, egui::Color32::from_rgb(0xf5, 0x8a, 0x3d))));
            // the session high as a dashed level with its own axis tag
            let y = f.y(level);
            f.painter.extend(egui::Shape::dashed_line(&[egui::pos2(f.plot.left(), y), egui::pos2(f.plot.right(), y)], egui::Stroke::new(1.0, egui::Color32::GRAY), 5.0, 4.0));
            f.tag(level, &format!("{level:.2}"), egui::Color32::DARK_GRAY);
        });
    })
}

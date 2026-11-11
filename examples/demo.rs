//! `cargo run -p charting --example demo`: a random walk with a moving average and a level drawn through `Overlay`,
//! a volume pane, and the scale switched with the S key (linear, log, percent).
use charting::{egui, pane, ta, Candle, Chart, ChartData, Frame, Overlay, PaneFrame, Scale, Z};

struct Demo { ma: Vec<Option<f64>>, level: f64, scale: Scale }

impl Overlay for Demo {
    fn paint(&mut self, z: Z, f: &mut Frame) {
        match z {
            Z::AboveCandles => {
                // a 20-bar average over the visible bars
                let pts: Vec<egui::Pos2> = (f.i0..=f.i1).filter_map(|i| Some(egui::pos2(f.x(i), f.y(self.ma[i]?)))).collect();
                f.painter.add(egui::Shape::line(pts, egui::Stroke::new(1.3, egui::Color32::from_rgb(0xf5, 0x8a, 0x3d))));
                // the session high as a dashed level with its own axis tag
                let y = f.y(self.level);
                f.painter.extend(egui::Shape::dashed_line(&[egui::pos2(f.plot.left(), y), egui::pos2(f.plot.right(), y)], egui::Stroke::new(1.0, egui::Color32::GRAY), 5.0, 4.0));
                let text = f.label(self.level);
                f.tag(self.level, &text, egui::Color32::DARK_GRAY);
            }
            // the legend inset: the hovered bar's close (else the newest) and the scale
            Z::Top => {
                let i = f.hover_i.unwrap_or(f.candles.len() - 1);
                let s = format!("C {}   scale {:?} (S)", f.price(f.candles[i].c), self.scale);
                f.full.text(f.legend.left_center() + egui::vec2(6.0, 0.0), egui::Align2::LEFT_CENTER, s, egui::FontId::monospace(12.0), egui::Color32::GRAY);
            }
            _ => {}
        }
    }

    fn pane(&mut self, _id: u32, p: &mut PaneFrame) {
        let vals: Vec<f64> = p.candles[p.i0..=p.i1].iter().map(|b| b.v).collect();
        let (lo, hi) = pane::range(&vals, true);
        pane::title(&p.painter, p.rect, "Volume");
        pane::axis(&p.painter, p.rect, lo, hi, &|v| format!("{v:.1}"));
        pane::line(&p.painter, &p.axis, p.i0, pane::inner(p.rect), vals.into_iter().enumerate().collect(), lo, hi, egui::Color32::from_rgb(0x4c, 0x9a, 0xff), true);
        p.range = Some((lo, hi));
    }
}

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
    let mut demo = Demo { ma, level, scale: Scale::Percent };
    let mut chart = Chart::default();
    (chart.inset_top, chart.panes) = (24.0, vec![(0, 90.0)]);
    eframe::run_ui_native("charting demo", eframe::NativeOptions::default(), move |ui, _| {
        if ui.input(|i| i.key_pressed(egui::Key::S)) {
            demo.scale = match demo.scale { Scale::Linear => Scale::Log, Scale::Log => Scale::Percent, Scale::Percent => Scale::Linear };
        }
        chart.scale = demo.scale;
        chart.show(ui, &ChartData { candles: &candles, bar_ms: 60_000, key: "demo" }, &mut demo);
    })
}

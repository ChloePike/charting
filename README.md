# charting

[![crates.io](https://img.shields.io/crates/v/charting.svg)](https://crates.io/crates/charting) [![docs.rs](https://docs.rs/charting/badge.svg)](https://docs.rs/charting)

Financial charts for [egui](https://github.com/emilk/egui): candles drawn the way TradingView draws them, an
overlay API that leaves the drawing to you, drawing tools, sub-panes, and order-flow layers (book heatmap,
liquidation-style time × price maps). Extracted from [Depth](https://github.com/ChloePike/depth), a multi-exchange
crypto trading terminal.

```sh
cargo add charting
```

```rust
use charting::{Chart, ChartData, Frame, Z};

let mut chart = Chart::default();
// every frame:
chart.show(ui, &ChartData { candles: &candles, bar_ms: 60_000, key: "BTCUSDT 1m" }, &mut |z: Z, f: &mut Frame| {
    if z == Z::AboveCandles {
        let y = f.y(level);
        f.painter.hline(f.plot.x_range(), y, (1.0, egui::Color32::GRAY));
        f.tag(level, "level", egui::Color32::DARK_GRAY);
    }
});
```

`cargo run --example demo` shows a random walk with a moving average, a level, a legend row and a volume pane drawn
through the overlay, on a percent scale (S switches linear / log / percent).

## What is in it

- **`Chart`**: candles on device pixels with TradingView's body width, grid, price and time axes, last-price tag,
  crosshair, pan / glide / zoom around the pointer, eased auto-fit, wheel and axis-drag price scaling.
- **`Chart` options**: linear / log / percent scale, the app's price and time formats, a legend inset above the plot,
  sub-panes under it (id, height), axis sizes, the widest zoom, the built-in last-price tag, crosshair, candles and
  grid on or off (an app painting its own footprint or dotted grid; `Frame` carries the grid values and the time
  labels' bars, which sit on round multiples of the bar). `show` returns a
  `ChartResponse`: the view reached the oldest bar (once per bar: load an older page), the hovered bar, whether the
  view moved (`Chart::reset`).
- **`Overlay`**: paint under the grid, under the candles, over them, and over everything (`Z::Top`), each sub-pane
  (`pane`, with the `pane` primitives), with the coordinate transforms, the visible bars, the pointer and price-axis
  tags that never overlap (reserve the important ones first, context tags slide only a little). It can claim a drag
  that starts on its own lines or drawings (no pan meanwhile) and veto a double-click reset. A closure is an overlay.
- **Parts**, for apps that assemble their own chart: `coords`, `candle`, `interact`, `layout`, `pane`, `ta` (EMA,
  RSI, MACD, Bollinger, SMA with TradingView's seeding), `draw` (horizontal line, trend, ray, Fibonacci, rectangle,
  channel, text, measure, position tool: model, geometry, hit-testing, painting), `orderflow` (book heatmap with walls
  standing out of their neighbours, time × price segment heat), `style` (palette, fonts and words, set once per frame;
  light and dark), `fmt`.
- Feature `serde` (default): drawings and candles serialize.

egui 0.36. The crate follows egui's minor versions.

## License

GPL-3.0-only. See [LICENSE](LICENSE) and [NOTICE](NOTICE).

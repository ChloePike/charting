//! Financial charts for egui.
//!
//! - [`Chart`]: a ready widget (candles drawn the TradingView way, grid, axes, last-price tag, crosshair, pan / glide /
//!   zoom, eased auto-fit, linear / log / percent [`Scale`], a legend inset, sub-panes) and the [`Overlay`] trait, which
//!   paints at each depth ([`Z`]) with the coordinate transforms, the visible bars and the price-axis tag layout in a
//!   [`Frame`], paints each pane in a [`PaneFrame`] and may claim drags; `show` returns a [`ChartResponse`].
//! - The parts it is built from, for apps that assemble their own: `coords` (price scale, bar axis, grid), `candle`,
//!   `interact` (pan, zoom, price range), `layout` (axis tags that never overlap), `pane` (sub-panes), `ta`
//!   (EMA, RSI, MACD, Bollinger, SMA), `draw` (drawing tools: model, geometry, hit-testing, painting), `orderflow`
//!   (book heatmap, time x price segment heat such as liquidation maps), `style` (palette, fonts, words; set once per
//!   frame) and `fmt`.
//!
//! See `examples/demo.rs`.

pub mod candle;
pub mod chart;
pub mod coords;
pub mod draw;
pub mod fmt;
pub mod interact;
pub mod layout;
pub mod pane;
pub mod orderflow;
pub mod style;
pub mod ta;

pub use chart::{Candle, Chart, ChartData, ChartResponse, Frame, Nothing, Overlay, PaneFrame, Scale, Z};
pub use egui;

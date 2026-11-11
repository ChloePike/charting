//! Colours, fonts and corner radius of the chart. The app sets a `Style` once per frame (`set`), and the chart's
//! drawing code reads it through the functions below (`fg()`, `up()`, `mono(12.0)` ...), so an app re-themes the
//! whole chart (light / dark, its own fonts and colours) without passing a style through every call.
use egui::{Color32, FontFamily, FontId};
use std::borrow::Cow;
use std::cell::RefCell;

/// The chart's colours. `muted` / `dim` are the secondary and tertiary text greys; `line` hairlines and grid;
/// `edge` a stronger hairline; `hl` a hover tint; `label_bg` / `cross_bg` / `popup_bg` the fills under axis tags,
/// crosshair tags and menus.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    pub bg: Color32, pub panel: Color32, pub panel2: Color32,
    pub fg: Color32, pub fg_strong: Color32, pub muted: Color32, pub dim: Color32,
    pub line: Color32, pub edge: Color32, pub hl: Color32,
    pub popup_bg: Color32, pub label_bg: Color32, pub cross_bg: Color32,
    pub up: Color32, pub down: Color32, pub accent: Color32, pub warn: Color32, pub purple: Color32,
}

impl Palette {
    /// A neutral dark palette (the crate's default).
    pub fn dark() -> Self {
        let g = Color32::from_gray;
        let w = |a: u8| Color32::from_rgba_premultiplied(a, a, a, a);
        Palette {
            bg: g(10), panel: g(18), panel2: w(10), fg: g(214), fg_strong: Color32::WHITE, muted: g(146), dim: g(112),
            line: w(20), edge: w(41), hl: w(18), popup_bg: g(29), label_bg: g(38), cross_bg: g(51),
            up: Color32::from_rgb(0x5c, 0xc0, 0x9b), down: Color32::from_rgb(0xd4, 0x4b, 0x62),
            accent: Color32::from_rgb(0xf5, 0x8a, 0x3d), warn: Color32::from_rgb(0xe8, 0xa3, 0x3d), purple: Color32::from_rgb(0x9b, 0x8c, 0xf0),
        }
    }
}

/// Font families for text and for numbers (tabular figures), each in regular, medium and strong weight.
#[derive(Clone, Debug, PartialEq)]
pub struct Fonts { pub text: FontFamily, pub text_m: FontFamily, pub text_s: FontFamily, pub num: FontFamily, pub num_m: FontFamily, pub num_s: FontFamily }

impl Default for Fonts {
    fn default() -> Self {
        Fonts { text: FontFamily::Proportional, text_m: FontFamily::Proportional, text_s: FontFamily::Proportional,
            num: FontFamily::Monospace, num_m: FontFamily::Monospace, num_s: FontFamily::Monospace }
    }
}

/// The words the chart writes itself; `Style::text` gives each in the app's language (`Text::en` by default). New ones
/// are added as the crate grows (non-exhaustive).
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Text { Bars, PosTarget, PosStop, PosLong, PosShort, PosQty, TextHint }

impl Text {
    pub fn en(self) -> &'static str {
        match self {
            Text::Bars => "bars", Text::PosTarget => "Target", Text::PosStop => "Stop", Text::PosLong => "Long",
            Text::PosShort => "Short", Text::PosQty => "size", Text::TextHint => "Text",
        }
    }
}

fn english(t: Text) -> Cow<'static, str> { Cow::Borrowed(t.en()) }

#[derive(Clone, Debug)]
pub struct Style {
    pub palette: Palette,
    pub fonts: Fonts,
    /// corner radius of tags and buttons, points
    pub radius: u8,
    /// a light theme is in effect (a few overlays pick lighter fills)
    pub light: bool,
    /// the chart's words in the app's language
    pub text: fn(Text) -> Cow<'static, str>,
}

/// Equal when it would draw the same: colours, fonts, radius and light (the text function is not compared, function
/// addresses are not unique).
impl PartialEq for Style {
    fn eq(&self, o: &Self) -> bool { self.palette == o.palette && self.fonts == o.fonts && self.radius == o.radius && self.light == o.light }
}

impl Default for Style {
    fn default() -> Self { Style { palette: Palette::dark(), fonts: Fonts::default(), radius: 4, light: false, text: english } }
}

thread_local! { static CURRENT: RefCell<Style> = RefCell::new(Style::default()); }

/// The style the chart draws with from now on (call it once per frame, before the chart; egui draws on one thread).
pub fn set(s: Style) { CURRENT.with(|c| *c.borrow_mut() = s); }

/// The style in effect.
pub fn current() -> Style { CURRENT.with(|c| c.borrow().clone()) }

fn pal() -> Palette { CURRENT.with(|c| c.borrow().palette) }
fn fam(f: impl Fn(&Fonts) -> &FontFamily) -> FontFamily { CURRENT.with(|c| f(&c.borrow().fonts).clone()) }

pub fn bg() -> Color32 { pal().bg }
pub fn panel() -> Color32 { pal().panel }
pub fn panel2() -> Color32 { pal().panel2 }
pub fn fg() -> Color32 { pal().fg }
pub fn fg_strong() -> Color32 { pal().fg_strong }
pub fn mu() -> Color32 { pal().muted }
pub fn dim() -> Color32 { pal().dim }
pub fn line() -> Color32 { pal().line }
pub fn edge() -> Color32 { pal().edge }
pub fn hl() -> Color32 { pal().hl }
pub fn popup_bg() -> Color32 { pal().popup_bg }
pub fn label_bg() -> Color32 { pal().label_bg }
pub fn cross_bg() -> Color32 { pal().cross_bg }
pub fn up() -> Color32 { pal().up }
pub fn dn() -> Color32 { pal().down }
pub fn accent() -> Color32 { pal().accent }
pub fn warn() -> Color32 { pal().warn }
pub fn purple() -> Color32 { pal().purple }
pub fn light() -> bool { CURRENT.with(|c| c.borrow().light) }
pub fn rad() -> u8 { CURRENT.with(|c| c.borrow().radius) }
/// `t` in the app's language
pub fn text(t: Text) -> String { CURRENT.with(|c| (c.borrow().text)(t).into_owned()) }

pub fn prop(size: f32) -> FontId { FontId::new(size, fam(|f| &f.text)) }
pub fn prop_m(size: f32) -> FontId { FontId::new(size, fam(|f| &f.text_m)) }
pub fn prop_s(size: f32) -> FontId { FontId::new(size, fam(|f| &f.text_s)) }
pub fn mono(size: f32) -> FontId { FontId::new(size, fam(|f| &f.num)) }
pub fn mono_m(size: f32) -> FontId { FontId::new(size, fam(|f| &f.num_m)) }
pub fn mono_s(size: f32) -> FontId { FontId::new(size, fam(|f| &f.num_s)) }

/// Linear blend of two colours, alpha included (`t` clamped to 0..1).
pub fn lerp(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgba_premultiplied(m(a.r(), b.r()), m(a.g(), b.g()), m(a.b(), b.b()), m(a.a(), b.a()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_set_style_is_what_the_chart_reads() {
        let mut s = Style::default();
        s.palette.up = Color32::RED;
        s.fonts.num = FontFamily::Name("num".into());
        set(s.clone());
        assert_eq!(up(), Color32::RED);
        assert_eq!(mono(12.0).family, FontFamily::Name("num".into()));
        assert_eq!(current(), s);
        assert_eq!(text(Text::PosStop), "Stop");
        set(Style { text: |t| if t == Text::PosStop { "止损".into() } else { t.en().into() }, ..Style::default() });
        assert_eq!((text(Text::PosStop), text(Text::Bars)), ("止损".to_string(), "bars".to_string()));
        set(Style::default());
        assert_eq!(up(), Palette::dark().up);
    }
}

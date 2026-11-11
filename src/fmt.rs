//! Number and duration labels.

/// Time left until a bar closes: "mm:ss", or "h:mm:ss" from an hour on.
pub fn countdown(ms: i64) -> String {
    let s = (ms / 1000).max(0);
    if s >= 3600 { format!("{}:{:02}:{:02}", s / 3600, s % 3600 / 60, s % 60) } else { format!("{:02}:{:02}", s / 60, s % 60) }
}

/// 1.2K / 3.4M / 1.1B, two decimals under a thousand.
pub fn fmt_big(v: f64) -> String {
    let a = v.abs();
    if a >= 1e9 { format!("{:.2}B", v / 1e9) } else if a >= 1e6 { format!("{:.2}M", v / 1e6) } else if a >= 1e3 { format!("{:.1}K", v / 1e3) } else { format!("{v:.2}") }
}

/// Footprint cell volume: 0.42 / 3.1 / 27 / 1.2K.
pub fn fmt_fp(v: f64) -> String {
    let a = v.abs();
    if a == 0.0 { "0".into() } else if a >= 1e6 { format!("{:.1}M", v / 1e6) } else if a >= 1e3 { format!("{:.1}K", v / 1e3) } else if a >= 100.0 { format!("{v:.0}") } else if a >= 10.0 { format!("{v:.1}") } else { format!("{v:.2}") }
}

pub fn fmt_fp_signed(v: f64) -> String { format!("{}{}", if v >= 0.0 { "+" } else { "" }, fmt_fp(v)) }

/// "3d 4h", "2h 15m", "45m" for a duration in ms.
pub fn span(ms: i64) -> String {
    let m = ms / 60_000;
    let (d, h, mm) = (m / 1440, m / 60 % 24, m % 60);
    if d > 0 { format!("{d}d {h}h") } else if h > 0 { format!("{h}h {mm}m") } else { format!("{mm}m") }
}

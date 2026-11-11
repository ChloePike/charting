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

/// Decimals needed to tell `step`-spaced values apart (a grid step of 2.5 needs 1, 0.0005 needs 4).
pub fn step_dp(step: f64) -> usize {
    (0..10).find(|d| { let x = step * 10f64.powi(*d as i32); (x - x.round()).abs() < 1e-6 * x.max(1.0) }).unwrap_or(10)
}

/// UTC time of `ms`: "HH:MM" (or "MM-DD" at midnight), or "YYYY-MM-DD HH:MM" when `full`.
pub fn time(ms: i64, full: bool) -> String {
    let (s, days) = (ms.div_euclid(1000), ms.div_euclid(86_400_000));
    let (h, m) = (s.rem_euclid(86_400) / 3600, s.rem_euclid(3600) / 60);
    // civil date from days since 1970-01-01 (Howard Hinnant's algorithm)
    let z = days + 719_468;
    let (era, doe) = (z.div_euclid(146_097), z.rem_euclid(146_097));
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let (d, mo) = (doy - (153 * mp + 2) / 5 + 1, if mp < 10 { mp + 3 } else { mp - 9 });
    let y = yoe + era * 400 + i64::from(mo <= 2);
    if full { format!("{y:04}-{mo:02}-{d:02} {h:02}:{m:02}") } else if h == 0 && m == 0 { format!("{mo:02}-{d:02}") } else { format!("{h:02}:{m:02}") }
}

#[cfg(test)]
mod tests {
    #[test]
    fn steps_and_times() {
        assert_eq!((super::step_dp(2.5), super::step_dp(0.0005), super::step_dp(100.0)), (1, 4, 0));
        assert_eq!(super::time(1_794_182_400_000, true), "2026-11-09 00:00");
        assert_eq!(super::time(1_794_182_400_000, false), "11-09");
        assert_eq!(super::time(1_794_182_400_000 + 3_660_000, false), "01:01");
    }
}

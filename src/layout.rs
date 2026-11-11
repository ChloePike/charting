//! Label layout on the price axis: tags slide up or down until they overlap none placed before them.
use egui::{vec2, Rect};

/// `r`, moved up or down until it overlaps none of `taken`, joins them.
pub fn place(r: Rect, taken: &mut Vec<Rect>) -> Rect {
    let h = r.height() + 2.0;
    let cand = (0..12).map(|k| { let d = ((k + 1) / 2) as f32 * h * if k % 2 == 1 { 1.0 } else { -1.0 }; r.translate(vec2(0.0, d)) })
        .find(|c| taken.iter().all(|t| !t.intersects(*c))).unwrap_or(r);
    taken.push(cand);
    cand
}
// context levels (POC / VA / S/R / VWAP / breakouts) may slide one tag height at most; further
// away the tag would name the wrong price, so it is dropped (the line and the legend stay)
pub fn try_place(r: Rect, taken: &mut Vec<Rect>) -> Option<Rect> {
    let h = r.height() + 2.0;
    let cand = [0.0, -h * 0.5, h * 0.5, -h, h].iter().map(|d| r.translate(vec2(0.0, *d))).find(|c| taken.iter().all(|t| !t.intersects(*c)))?;
    taken.push(cand);
    Some(cand)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags_never_overlap_and_far_ones_drop() {
        let r = Rect::from_min_size(egui::pos2(0.0, 100.0), vec2(50.0, 18.0));
        let mut taken = vec![];
        let a = place(r, &mut taken);
        let b = place(r, &mut taken);
        assert!(!a.intersects(b));
        // a context tag may slide one tag height at most; with both sides taken it is dropped
        let mut full = vec![r.translate(vec2(0.0, -20.0)), r, r.translate(vec2(0.0, 20.0)), r.translate(vec2(0.0, -10.0)), r.translate(vec2(0.0, 10.0))];
        assert!(try_place(r, &mut full).is_none());
    }
}

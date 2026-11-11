//! Pointer interaction of a chart: pan, glide, zoom around the pointer, and the price range (eased auto-fit or the
//! manual range the wheel and drags set). The app decides what a drag belongs to (`editing`: a drawing or an app
//! line) and keeps the state; these functions move it.
use egui::{Rect, Response, Ui};

/// The time axis view: the offset from the live edge in bars (`right`), points per bar, glide velocity (bars / s).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TimeView { pub right: f32, pub bar_w: f32, pub fling: f32 }

/// Default zoom (points per bar), as after a double-click.
pub const BAR_W: f32 = 7.0;

/// Drag pans time (not when `editing` or `on_axis`, a drag that started on the price axis); a release while moving
/// glides on (TradingView-style inertia, repainting only while it moves); the wheel or a pinch over the plot zooms time
/// around the pointer (the bar under it stays put) up to `max_bar_w`, horizontal scroll pans; a double-click resets
/// the view when `reset_ok`. `main`: the plot left of the price axis; `n_bars`: the bars loaded.
/// Returns (frame dt, the wheel delta over the price axis for `price_range`, whether the view was reset).
#[allow(clippy::too_many_arguments)]
pub fn pan_zoom(ui: &Ui, resp: &Response, main: Rect, n_bars: usize, v: &mut TimeView, max_bar_w: f32, editing: bool, on_axis: bool, reset_ok: bool) -> (f32, f32, bool) {
    if resp.drag_started() { v.fling = 0.0; }
    let dt = ui.input(|i| i.stable_dt).clamp(0.001, 0.05);
    if resp.dragged() && !on_axis && !editing { v.right += resp.drag_delta().x / v.bar_w; }
    if resp.drag_stopped() {
        let vx = ui.input(|i| i.pointer.velocity().x);
        v.fling = if !editing && !on_axis && vx.abs() > 150.0 { vx / v.bar_w } else { 0.0 };
    } else if !resp.dragged() && v.fling != 0.0 {
        v.right += v.fling * dt;
        v.fling *= (-dt * 4.0).exp();
        // under half a bar per second it stops
        if v.fling.abs() < 0.5 { v.fling = 0.0 } else { ui.ctx().request_repaint(); }
    }
    if resp.hover_pos().is_some_and(|p| p.x > main.right() && p.y < main.bottom()) { ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical); }
    let mut y_wheel = 0.0f32;
    if let (true, Some(hp)) = (resp.hovered(), resp.hover_pos()) {
        let (d, pinch) = ui.input(|i| (i.smooth_scroll_delta, i.zoom_delta()));
        if hp.x > main.right() && hp.y < main.bottom() { y_wheel = d.y; } else {
            let bw = (v.bar_w * (d.y * 0.003).exp() * pinch).clamp(1.5, max_bar_w);
            if bw != v.bar_w {
                let edge = n_bars as f32 + 3.0 - v.right;
                let idx = edge - (main.right() - hp.x - v.bar_w / 2.0) / v.bar_w;
                let edge2 = idx + (main.right() - hp.x - bw / 2.0) / bw;
                v.right = n_bars as f32 + 3.0 - edge2;
                v.bar_w = bw;
            }
            if d.x != 0.0 { v.right += d.x / v.bar_w; v.fling = 0.0; }
        }
    }
    let reset = resp.double_clicked() && reset_ok;
    // a reset also stops a glide (it would carry the view away from the live edge again)
    if reset { v.right = 0.0; v.bar_w = BAR_W; v.fling = 0.0; }
    (dt, y_wheel, reset)
}

/// The price range shown. Auto-fit eases toward `target` (a new high slides the scale instead of snapping it); a new
/// `key` (another series) or a jump larger than the range snaps. A manual range (`y_range`, set by the wheel over the
/// axis, a drag on the axis, or a drag of the plot once manual) wins until a reset clears it.
#[allow(clippy::too_many_arguments)]
pub fn price_range<K: PartialEq>(ui: &Ui, resp: &Response, main: Rect, target: (f64, f64), key: K, eased: &mut Option<(K, f64, f64)>,
    y_range: &mut Option<(f64, f64)>, on_axis: bool, editing: bool, dt: f32, y_wheel: f32) -> (f64, f64) {
    let (tlo, thi) = target;
    let (alo, ahi) = match eased {
        Some((k, lo0, hi0)) if *k == key && (tlo - *lo0).abs().max((thi - *hi0).abs()) < (thi - tlo) => {
            let f = 1.0 - (-dt as f64 * 14.0).exp();
            let (l2, h2) = (*lo0 + (tlo - *lo0) * f, *hi0 + (thi - *hi0) * f);
            // repaint only while still moving visibly
            if ((l2 - tlo).abs() + (h2 - thi).abs()) / (thi - tlo).max(1e-12) > 1e-4 { ui.ctx().request_repaint(); (l2, h2) } else { (tlo, thi) }
        }
        _ => (tlo, thi),
    };
    *eased = Some((key, alo, ahi));
    let (mut lo, mut hi) = y_range.unwrap_or((alo, ahi));
    if y_wheel != 0.0 {
        let (mid, half) = ((lo + hi) / 2.0, (hi - lo) / 2.0 * (-y_wheel as f64 * 0.003).exp());
        (lo, hi) = (mid - half, mid + half);
        *y_range = Some((lo, hi));
    }
    if resp.dragged() {
        let (dy, ph) = (resp.drag_delta().y as f64, (main.height() - 54.0).max(50.0) as f64);
        if on_axis {
            // drag down stretches the range (candles shrink), up compresses it, around the middle
            let (mid, half) = ((lo + hi) / 2.0, (hi - lo) / 2.0 * (dy * 0.006).exp());
            (lo, hi) = (mid - half, mid + half);
            *y_range = Some((lo, hi));
        } else if y_range.is_some() && !editing {
            let shift = dy / ph * (hi - lo);
            (lo, hi) = (lo + shift, hi + shift);
            *y_range = Some((lo, hi));
        }
    }
    if !(hi > lo && lo.is_finite() && hi.is_finite()) { (lo, hi) = (alo, ahi); *y_range = None; }
    (lo, hi)
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{pos2, Event, PointerButton, Pos2, RawInput};

    /// Runs frames of a 400 x 300 chart area fed with `events` (one batch per frame); returns the view after them.
    fn run(frames: Vec<Vec<Event>>, editing: bool) -> (TimeView, bool) {
        let ctx = egui::Context::default();
        let mut v = TimeView { right: 0.0, bar_w: 10.0, fling: 0.0 };
        let mut reset = false;
        for ev in frames {
            let input = RawInput { events: ev, screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(500.0, 400.0))), ..Default::default() };
            let mut out = ctx.run_ui(input, |ui| {
                let r = Rect::from_min_size(pos2(0.0, 0.0), egui::vec2(400.0, 300.0));
                let resp = ui.interact(r, ui.id().with("chart"), egui::Sense::click_and_drag());
                // the price axis is right of x = 350
                let main = Rect::from_min_max(r.min, pos2(350.0, r.bottom()));
                let on_axis = resp.interact_pointer_pos().is_some_and(|p| p.x > main.right());
                let (_, _, rs) = pan_zoom(ui, &resp, main, 100, &mut v, 60.0, editing, on_axis, true);
                reset |= rs;
            });
            // no renderer here: the frame's texture uploads are dropped on purpose
            out.textures_delta.clear();
        }
        (v, reset)
    }

    fn press(x: f32, y: f32, down: bool) -> Event {
        Event::PointerButton { pos: pos2(x, y), button: PointerButton::Primary, pressed: down, modifiers: Default::default() }
    }
    fn drag_frames(x0: f32, x1: f32) -> Vec<Vec<Event>> {
        vec![vec![Event::PointerMoved(pos2(x0, 100.0))], vec![press(x0, 100.0, true)],
             vec![Event::PointerMoved(pos2((x0 + x1) / 2.0, 100.0))], vec![Event::PointerMoved(pos2(x1, 100.0))], vec![press(x1, 100.0, false)], vec![]]
    }

    #[test]
    fn a_drag_pans_time_unless_editing_or_on_the_axis() {
        // 100 points right at 10 points per bar: 10 bars back from the live edge (a glide may add a little)
        let (v, _) = run(drag_frames(100.0, 200.0), false);
        assert!(v.right >= 10.0 - 1e-3, "{v:?}");
        // a drag that belongs to a drawing / app line moves no time
        let (v, _) = run(drag_frames(100.0, 200.0), true);
        assert_eq!(v.right, 0.0);
        // a drag that starts on the price axis scales prices, not time
        let (v, _) = run(drag_frames(370.0, 390.0), false);
        assert_eq!(v.right, 0.0);
    }

    #[test]
    fn a_double_click_resets_the_view() {
        let click = |down| vec![press(100.0, 100.0, down)];
        let (v, reset) = run(vec![vec![Event::PointerMoved(pos2(100.0, 100.0))], click(true), click(false), click(true), click(false), vec![]], false);
        assert!(reset && v.right == 0.0 && v.bar_w == BAR_W, "{v:?} {reset}");
        // a reset right after a flung drag stays at the live edge (the glide stops)
        let mut f = drag_frames(100.0, 300.0);
        f.pop();
        f.extend([click(true), click(false), click(true), click(false), vec![], vec![]]);
        let (v, reset) = run(f, false);
        assert!(reset && v.right == 0.0 && v.fling == 0.0, "{v:?}");
    }
}

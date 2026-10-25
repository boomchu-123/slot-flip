//! The Flip's two analog sticks, for a game that reads them.
//!
//! The input thread that owns the gamepad node writes the raw axes here; the
//! session reads them once per update and hands them to the core. They are
//! kept apart from the button events because a stick is a position, not a
//! press, and a game wants the whole range.

use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};

pub const ABS_X: u16 = 0x00;
pub const ABS_Y: u16 = 0x01;
pub const ABS_RX: u16 = 0x03;
pub const ABS_RY: u16 = 0x04;

/// The full swing of an axis, as the Flip's gamepad node reports it (+-32767).
pub const FULL: i32 = 32_767;

/// How far a stick must be pushed before it counts. The Flip's sticks do not
/// rest at zero (a right stick at rest was measured at about -5100 and 1400),
/// so without this a character would walk on its own.
pub const DEAD_ZONE: i32 = 7_000;

/// How far a stick must be pushed to count as all the way. The Flip's sticks do not
/// reach their nominal 32767: pushed hard against the gate they read anywhere from
/// about 25000 to 31000 (the left one tops out lower than the right) and bounce as
/// they meet it. Mapping only 32767 to full made the end of the travel swing between
/// roughly 75% and 95% of full, and full lock hard to reach, so a firm push to the
/// gate reads as all the way. Pushes short of it are scaled linearly as before.
pub const EDGE: i32 = 26_000;

static AXES: [AtomicI32; 4] = [
    AtomicI32::new(0),
    AtomicI32::new(0),
    AtomicI32::new(0),
    AtomicI32::new(0),
];

/// The furthest each axis went since it was last read. The stick springs back off
/// the gate, so a hard push can be a pulse of one or two samples (16 to 33 ms), and
/// slot reads the stick only about every 22 ms: a pulse that begins and ends between
/// two reads would otherwise never be seen at all.
/// How far the other way, in the stick's own units, counts as a real swing rather than
/// the rebound after a slam (a bit past the dead zone, which the rebound barely clears).
const SWING_MIN: i32 = 8_000;

static PEAKS: [AtomicI32; 4] = [
    AtomicI32::new(0),
    AtomicI32::new(0),
    AtomicI32::new(0),
    AtomicI32::new(0),
];

/// Whether the right stick is the game's, not the frontend's. It stands in for
/// the R2 button otherwise (see `evdev::Stick`), and cannot be both.
static TAKEN: AtomicBool = AtomicBool::new(false);

/// Left x, left y, right x, right y: what a core is handed.
pub type Sticks = [i16; 4];

/// A raw axis event from the gamepad node. Anything that is not a stick is ignored.
pub fn feed(code: u16, value: i32) {
    let at = match code {
        ABS_X => 0,
        ABS_Y => 1,
        ABS_RX => 2,
        ABS_RY => 3,
        _ => return,
    };
    let value = value.clamp(-FULL, FULL);
    AXES[at].store(value, Ordering::Relaxed);
    if value.abs() >= PEAKS[at].load(Ordering::Relaxed).abs() {
        PEAKS[at].store(value, Ordering::Relaxed);
    }
}

/// The sticks as a game should see them, dead zone applied.
pub fn read() -> Sticks {
    // The furthest push since the last read, or where the stick is now if that is
    // further; the next read starts again from where it is.
    let axis = |i: usize| {
        let now = AXES[i].load(Ordering::Relaxed);
        let peak = PEAKS[i].swap(now, Ordering::Relaxed);
        // The furthest push wins over where the stick is now (a slam's rebound is
        // small), unless the stick has really gone the other way: then the end it
        // reached is the news, not the end it left.
        let swung = i64::from(peak) * i64::from(now) < 0 && now.abs() >= SWING_MIN;
        if peak.abs() > now.abs() && !swung {
            peak
        } else {
            now
        }
    };
    let (lx, ly) = dead_zone(axis(0), axis(1));
    let (rx, ry) = dead_zone(axis(2), axis(3));
    [lx, ly, rx, ry]
}

/// How long a push is held once it has been seen, however quickly the stick springs
/// back. A game reads its pad once per frame (every 33 ms at 30 fps), so a pulse
/// shorter than that could fall between two of its reads; this guarantees at least
/// one sees it. It is also the most a release can lag, which is why it is short.
pub const HOLD_MS: u64 = 50;

/// How far the other way a push must go to count as a new push rather than the
/// recoil past centre. A slam's rebound was measured at a few hundred on this scale.
const REVERSAL_MIN: f64 = 8_000.0;

/// Keeps each stick's furthest recent push for `HOLD_MS`, then lets it follow the
/// stick again. Owned by whoever reads the sticks, not shared.
#[derive(Default)]
pub struct Hold {
    held: [i16; 4],
    until: [u64; 2],
}

impl Hold {
    /// `sticks` as `read()` gave them, at `now` milliseconds.
    pub fn filter(&mut self, now: u64, sticks: Sticks) -> Sticks {
        let mut out = sticks;
        for s in 0..2 {
            let (x, y) = (sticks[2 * s], sticks[2 * s + 1]);
            let (hx, hy) = (self.held[2 * s], self.held[2 * s + 1]);
            let mag = |x: i16, y: i16| f64::from(x).hypot(f64::from(y));
            // A real push the other way (not the small recoil past centre after a
            // slam) is a new push, not the old one easing off, so it is never held
            // back for being a little smaller than the one before it. Without this a
            // quick sweep from one end to the other was answered with the first end.
            let opposite = i32::from(x) * i32::from(hx) + i32::from(y) * i32::from(hy) < 0
                && mag(x, y) >= REVERSAL_MIN;
            if opposite || mag(x, y) >= mag(hx, hy) || now >= self.until[s] {
                // As far as before or further, or the hold has run out: follow the
                // stick, and a push starts a fresh hold.
                self.held[2 * s] = x;
                self.held[2 * s + 1] = y;
                if (opposite || mag(x, y) >= mag(hx, hy)) && (x != 0 || y != 0) {
                    self.until[s] = now + HOLD_MS;
                }
            } else {
                out[2 * s] = hx;
                out[2 * s + 1] = hy;
            }
        }
        out
    }

    /// Forget everything, for when the sticks stop being the game's.
    pub fn reset(&mut self) {
        *self = Hold::default();
    }
}

/// Forget where the sticks were, for a core that has just been given none.
pub fn release() {
    for a in AXES.iter().chain(&PEAKS) {
        a.store(0, Ordering::Relaxed);
    }
}

pub fn set_taken(taken: bool) {
    TAKEN.store(taken, Ordering::Relaxed);
}

pub fn taken() -> bool {
    TAKEN.load(Ordering::Relaxed)
}

/// A circular dead zone: nothing inside it, and the rest of the range stretched
/// back over the full swing so a stick pushed to the edge still reads as the edge.
/// Circular rather than per axis, so a diagonal is not dead on one axis and live
/// on the other.
pub fn dead_zone(x: i32, y: i32) -> (i16, i16) {
    let (fx, fy) = (f64::from(x), f64::from(y));
    let mag = fx.hypot(fy);
    let dead = f64::from(DEAD_ZONE);
    if mag <= dead {
        return (0, 0);
    }
    let full = f64::from(FULL);
    let edge = f64::from(EDGE);
    let scaled = ((mag - dead) / (edge - dead)).min(1.0) * full;
    let k = scaled / mag;
    ((fx * k).round() as i16, (fy * k).round() as i16)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// The axes are global, so the tests that write them take turns.
    static AXES_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn a_stick_at_rest_reads_as_still() {
        assert_eq!(dead_zone(0, 0), (0, 0));
        // What the Flip's right stick measured at rest.
        assert_eq!(dead_zone(-5096, 1404), (0, 0));
    }

    #[test]
    fn the_dead_zone_is_round_not_square() {
        // Each axis alone is inside the zone; together they are past it.
        let edge = (f64::from(DEAD_ZONE) * 0.8) as i32;
        let (x, y) = dead_zone(edge, edge);
        assert!(x > 0 && y > 0, "{x} {y}");
        // And the same push along one axis is not.
        assert_eq!(dead_zone(edge, 0), (0, 0));
    }

    #[test]
    fn the_edge_of_the_stick_is_still_the_edge() {
        assert_eq!(dead_zone(FULL, 0), (i16::MAX, 0));
        assert_eq!(dead_zone(0, -FULL), (0, -i16::MAX));
        let (x, y) = dead_zone(FULL, FULL);
        let mag = f64::from(x).hypot(f64::from(y));
        assert!((mag - f64::from(FULL)).abs() < 2.0, "{mag}");
    }

    #[test]
    fn a_push_just_past_the_zone_barely_moves_and_one_more_moves_further() {
        let (a, _) = dead_zone(DEAD_ZONE + 1, 0);
        let (b, _) = dead_zone(DEAD_ZONE + 5_000, 0);
        let (c, _) = dead_zone(DEAD_ZONE + 15_000, 0);
        assert!(0 < a && a < 10, "{a}");
        assert!(a < b && b < c, "{a} {b} {c}");
    }

    #[test]
    fn everything_from_a_firm_push_to_the_gate_reads_as_all_the_way() {
        // What the left stick was measured doing when pushed hard and held: it
        // wanders over this band as it bounces against the gate.
        for push in [EDGE, 27_000, 29_000, 31_000, FULL] {
            assert_eq!(dead_zone(push, 0), (i16::MAX, 0), "{push}");
            assert_eq!(dead_zone(0, -push), (0, -i16::MAX), "{push}");
        }
        // Short of it the response is still graded, not a switch.
        let (half, _) = dead_zone((DEAD_ZONE + EDGE) / 2, 0);
        assert!((16_000..=17_000).contains(&half), "{half}");
    }

    #[test]
    fn a_stick_resting_off_centre_still_reads_as_still_with_the_wider_edge() {
        // Resting readings measured on the device (magnitudes of about 5300 and 4500).
        for (x, y) in [(-5_096, 1_404), (-4_368, 936), (3_000, -4_500)] {
            assert_eq!(dead_zone(x, y), (0, 0), "{x} {y}");
        }
    }

    #[test]
    fn direction_is_kept() {
        let (x, y) = dead_zone(-20_000, 10_000);
        assert!(x < 0 && y > 0);
        // The ratio between the two survives the stretch.
        let want = 10_000.0 / 20_000.0;
        assert!((f64::from(y) / f64::from(-x) - want).abs() < 0.01);
    }

    #[test]
    fn an_axis_event_lands_on_the_stick_it_belongs_to() {
        let _g = AXES_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        release();
        feed(ABS_X, 30_000);
        feed(ABS_RY, -30_000);
        feed(2, 255); // the L2 trigger is not a stick
        let [lx, ly, rx, ry] = read();
        assert!(
            lx > 20_000 && ly == 0 && rx == 0 && ry < -20_000,
            "{lx} {ly} {rx} {ry}"
        );
        release();
        assert_eq!(read(), [0; 4]);
    }

    #[test]
    fn a_value_past_the_range_is_clamped() {
        let _g = AXES_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        release();
        feed(ABS_X, 90_000);
        assert_eq!(read()[0], i16::MAX);
        release();
    }

    #[test]
    fn a_pulse_between_two_reads_is_not_lost() {
        let _g = AXES_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        release();
        // A slam that springs back before slot looks: one sample out, one sample home.
        feed(ABS_X, -29_000);
        feed(ABS_X, 0);
        let [lx, ..] = read();
        assert!(lx < -20_000, "the peak was lost: {lx}");
        // And it is reported once, not forever.
        assert_eq!(read()[0], 0);
        release();
    }

    #[test]
    fn a_sweep_to_the_other_end_inside_one_read_reports_the_end_it_ended_on() {
        let _g = AXES_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        release();
        feed(ABS_X, 30_000);
        feed(ABS_X, -25_000);
        let [lx, ..] = read();
        assert!(
            lx < -15_000,
            "reported the end it left, not the one it reached: {lx}"
        );
        // But a slam's rebound is not a sweep: the peak still wins over a small recoil.
        release();
        feed(ABS_X, -29_000);
        feed(ABS_X, 5_000);
        assert!(read()[0] < -20_000, "the rebound cut the slam short");
        release();
    }

    #[test]
    fn a_stick_that_is_still_pushed_reads_as_pushed() {
        let _g = AXES_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        release();
        feed(ABS_X, -29_000);
        assert!(read()[0] < -20_000);
        assert!(read()[0] < -20_000, "a held push stopped after one read");
        release();
    }

    #[test]
    fn a_push_is_held_for_the_games_next_look_and_then_let_go() {
        let mut h = Hold::default();
        let push = [-i16::MAX, 0, 0, 0];
        assert_eq!(h.filter(1_000, push), push);
        // The stick has already sprung back, but the game has not looked yet.
        assert_eq!(h.filter(1_020, [0; 4]), push, "let go too soon");
        assert_eq!(h.filter(1_049, [0; 4]), push, "let go too soon");
        assert_eq!(h.filter(1_050, [0; 4]), [0; 4], "held too long");
        assert_eq!(h.filter(1_070, [0; 4]), [0; 4]);
    }

    #[test]
    fn a_hold_never_outlives_its_time_or_invents_a_push() {
        let mut h = Hold::default();
        for t in (0..500).step_by(16) {
            assert_eq!(h.filter(t, [0; 4]), [0; 4], "{t}");
        }
        // A push that is still being made is never delayed or cut short.
        let mut now = 0;
        for _ in 0..20 {
            assert_eq!(h.filter(now, [20_000, 0, 0, 0])[0], 20_000);
            now += 16;
        }
    }

    #[test]
    fn a_bigger_push_takes_over_at_once_and_a_smaller_one_waits_its_turn() {
        let mut h = Hold::default();
        h.filter(0, [10_000, 0, 0, 0]);
        // Further: immediate.
        assert_eq!(h.filter(10, [30_000, 0, 0, 0])[0], 30_000);
        // Easing off: the peak is held until its time is up, then it follows.
        assert_eq!(h.filter(20, [12_000, 0, 0, 0])[0], 30_000);
        assert_eq!(h.filter(60, [12_000, 0, 0, 0])[0], 12_000);
    }

    #[test]
    fn a_swing_to_the_far_side_is_followed_at_once_even_if_it_is_a_little_smaller() {
        let mut h = Hold::default();
        assert_eq!(h.filter(0, [30_000, 0, 0, 0])[0], 30_000);
        // The stick sweeps to the other end, not quite as far. That is a new push, not
        // the first one easing off, so the hold must not mask it.
        assert_eq!(h.filter(20, [-25_000, 0, 0, 0])[0], -25_000);
        // And the same up and down, and on the right stick.
        assert_eq!(h.filter(25, [0, 28_000, 0, 0])[1], 28_000);
        assert_eq!(h.filter(30, [0, -20_000, 0, 0])[1], -20_000);
        assert_eq!(h.filter(35, [0, 0, 29_000, 0])[2], 29_000);
        assert_eq!(h.filter(40, [0, 0, -22_000, 0])[2], -22_000);
    }

    #[test]
    fn the_rebound_after_a_slam_is_still_held_back() {
        let mut h = Hold::default();
        h.filter(0, [-30_000, 0, 0, 0]);
        // The recoil past centre is tiny, and must not cut the slam short.
        assert_eq!(h.filter(15, [3_000, 0, 0, 0])[0], -30_000);
        assert_eq!(h.filter(30, [-1_000, 0, 0, 0])[0], -30_000);
    }

    #[test]
    fn the_two_sticks_hold_independently() {
        let mut h = Hold::default();
        h.filter(0, [-i16::MAX, 0, 0, 0]);
        // The right stick moving is not held by the left one's push, and vice versa.
        let out = h.filter(20, [0, 0, 5_000, 0]);
        assert_eq!(out, [-i16::MAX, 0, 5_000, 0]);
    }

    #[test]
    fn resetting_forgets_a_hold() {
        let mut h = Hold::default();
        h.filter(0, [-i16::MAX, 0, 0, 0]);
        h.reset();
        assert_eq!(h.filter(10, [0; 4]), [0; 4]);
    }
}

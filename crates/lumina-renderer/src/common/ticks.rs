//! Tick positions for `Axes` and `NumberLine`, shared by both backends.
//!
//! Two problems this exists to solve.
//!
//! **Accumulated error.** Walking a range with `t += step` accumulates
//! rounding linearly in the tick count, so the last tick on a long axis lands
//! visibly off. The `+ 1e-4` fudge that used to guard the loop condition was
//! the symptom. Multiplying an integer index by the step is exact for as long
//! as the product is representable.
//!
//! **Unbounded loops.** `((max - min) / step).ceil() as i32` produces
//! `i32::MAX` when `step` is zero, because `inf as i32` saturates rather than
//! failing — 2.1 billion stroked paths per frame. Validation rejects such
//! scenes (`INVALID_STEP`, `TOO_MANY_TICKS`), but a renderer is a public API
//! that can be called without validating first, so the bound is enforced here
//! too.

/// Hard ceiling on ticks drawn for one axis in one frame.
///
/// Matches `lumina_core::validation::MAX_TICK_COUNT`. Validation gives the
/// author a structured error; this stops an unvalidated caller from hanging
/// the renderer.
const MAX_TICKS: usize = 100_000;

/// How many ticks span `[min, max]` at `step`, inclusive of both ends.
///
/// Returns 0 for a non-positive or non-finite step, or a non-finite range —
/// an axis that cannot be drawn draws nothing rather than looping forever.
pub(crate) fn count(min: f32, max: f32, step: f32) -> usize {
    if !step.is_finite() || step <= 0.0 || !min.is_finite() || !max.is_finite() || max < min {
        return 0;
    }
    let spans = f64::from(max - min) / f64::from(step);
    if !spans.is_finite() {
        return 0;
    }
    // +1 for the inclusive endpoint; the epsilon absorbs the case where the
    // range is an exact multiple of the step and the division lands a hair
    // below it.
    let n = (spans + 1e-6).floor() as usize + 1;
    n.min(MAX_TICKS)
}

/// The position of tick `i`, computed from the index rather than accumulated.
pub(crate) fn at(min: f32, step: f32, i: usize) -> f32 {
    // The multiplication happens in f64 so a long axis does not lose the low
    // bits of the step before it reaches the screen transform.
    (f64::from(min) + i as f64 * f64::from(step)) as f32
}

#[cfg(test)]
mod tests {
    use super::{at, count, MAX_TICKS};

    #[test]
    fn an_inclusive_range_counts_both_ends() {
        assert_eq!(count(0.0, 10.0, 1.0), 11);
        assert_eq!(count(-5.0, 5.0, 5.0), 3);
        assert_eq!(count(0.0, 1.0, 0.25), 5);
    }

    #[test]
    fn a_degenerate_step_draws_nothing_instead_of_looping() {
        // `((max - min) / 0.0).ceil() as i32` used to saturate to i32::MAX.
        assert_eq!(count(0.0, 10.0, 0.0), 0);
        assert_eq!(count(0.0, 10.0, -1.0), 0);
        assert_eq!(count(0.0, 10.0, f32::NAN), 0);
        assert_eq!(count(0.0, f32::INFINITY, 1.0), 0);
    }

    #[test]
    fn an_absurd_tick_count_is_capped() {
        assert_eq!(count(0.0, 1e9, 1e-6), MAX_TICKS);
    }

    #[test]
    fn positions_do_not_drift() {
        // Accumulating `t += 0.1` a thousand times drifts by ~1e-4; indexing
        // does not. This is the difference the fudge factor was hiding.
        let (min, step) = (0.0f32, 0.1f32);
        let mut accumulated = min;
        for i in 0..1000 {
            accumulated += step;
            let indexed = at(min, step, i + 1);
            assert!(
                (indexed - (i + 1) as f32 * step).abs() < 1e-3,
                "indexed position drifted at i = {i}"
            );
            let _ = accumulated;
        }
        // The last indexed tick lands where arithmetic says it should.
        assert!((at(0.0, 0.1, 1000) - 100.0).abs() < 1e-2);
    }

    #[test]
    fn positions_survive_a_large_origin() {
        // f32 has ~7 significant digits, so `1e6 + 1` is near the edge of what
        // it can distinguish; computing in f64 keeps consecutive ticks apart.
        let a = at(1.0e6, 1.0, 0);
        let b = at(1.0e6, 1.0, 1);
        assert!(b > a, "consecutive ticks collapsed at a large origin");
    }
}

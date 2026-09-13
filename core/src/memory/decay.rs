//! One decay curve, read everywhere. Learning is a write; forgetting is free.

/// Strength now, from strength then. `half_life_days <= 0` turns decay off.
pub fn decayed(value: f64, stamped_at: i64, at: i64, half_life_days: f64) -> f64 {
    if half_life_days <= 0.0 {
        return value;
    }
    let elapsed = (at - stamped_at).max(0) as f64;
    value * 2f64.powf(-elapsed / (half_life_days * 86_400.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAY: i64 = 86_400;

    #[test]
    fn a_half_life_halves_it() {
        assert!((decayed(4.0, 0, 30 * DAY, 30.0) - 2.0).abs() < 1e-9);
        assert!((decayed(4.0, 0, 60 * DAY, 30.0) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn time_never_runs_backwards_and_zero_turns_decay_off() {
        assert_eq!(decayed(4.0, 100, 0, 30.0), 4.0);
        assert_eq!(decayed(4.0, 0, 10_000 * DAY, 0.0), 4.0);
    }
}

//! Spinning-disc speed settings.

/// Seconds per revolution for a setting name ("slow" | "normal" | "fast"; unknown = normal).
pub fn secs_per_turn(setting: &str) -> f32 {
    match setting {
        "slow" => 12.0,
        "fast" => 4.0,
        _ => 8.0,
    }
}

/// Degrees per second at full speed.
pub fn degrees_per_sec(setting: &str) -> f32 {
    360.0 / secs_per_turn(setting)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speeds_per_setting() {
        assert_eq!(degrees_per_sec("slow"), 30.0);
        assert_eq!(degrees_per_sec("normal"), 45.0);
        assert_eq!(degrees_per_sec("fast"), 90.0);
        assert_eq!(degrees_per_sec("bogus"), 45.0);
        // the default is calmer than the old 3 s per turn
        assert!(secs_per_turn(&crate::config::Config::default().disc_speed) >= 8.0);
    }
}

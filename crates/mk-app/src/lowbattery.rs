//! When to show the low-battery notification (SPEC BAT-02): once at the configured threshold and
//! once more at [`CRITICAL`] percent.

/// Second warning level, in percent.
pub const CRITICAL: u8 = 5;
/// The warnings are re-armed once the battery is this far above the threshold (or charging).
const REARM_MARGIN: u8 = 5;

#[derive(Default)]
pub struct LowBattery {
    /// 0 = not warned, 1 = warned at the threshold, 2 = warned at the critical level.
    stage: u8,
}

impl LowBattery {
    /// Feeds the weakest keyboard running on battery (`None` if none is). Returns the percentage
    /// to warn about, if a warning is due now. `threshold` 0 disables the warnings.
    pub fn update(&mut self, lowest: Option<u8>, threshold: u8) -> Option<u8> {
        let percent = match lowest {
            Some(p) if threshold > 0 && p <= threshold.saturating_add(REARM_MARGIN) => p,
            _ => {
                self.stage = 0;
                return None;
            }
        };
        let due = if threshold > CRITICAL && percent <= CRITICAL {
            2
        } else if percent <= threshold {
            1
        } else {
            0
        };
        (due > self.stage).then(|| {
            self.stage = due;
            percent
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn drain(alert: &mut LowBattery, threshold: u8, from: u8, to: u8) -> Vec<u8> {
        (to..=from)
            .rev()
            .filter_map(|p| alert.update(Some(p), threshold))
            .collect()
    }

    #[test]
    fn warns_at_threshold_and_again_at_five_percent() {
        let mut alert = LowBattery::default();
        assert_eq!(drain(&mut alert, 15, 40, 1), vec![15, 5]);
    }

    #[test]
    fn first_reading_below_critical_warns_once() {
        let mut alert = LowBattery::default();
        assert_eq!(alert.update(Some(3), 15), Some(3));
        assert_eq!(alert.update(Some(2), 15), None);
    }

    #[test]
    fn charging_or_recovering_rearms() {
        let mut alert = LowBattery::default();
        assert_eq!(alert.update(Some(14), 15), Some(14));
        assert_eq!(alert.update(Some(18), 15), None, "within the margin");
        assert_eq!(alert.update(Some(14), 15), None);
        assert_eq!(alert.update(None, 15), None, "charging");
        assert_eq!(alert.update(Some(14), 15), Some(14));
        assert_eq!(alert.update(Some(21), 15), None);
        assert_eq!(alert.update(Some(15), 15), Some(15));
    }

    #[test]
    fn threshold_at_or_below_critical_warns_once() {
        let mut alert = LowBattery::default();
        assert_eq!(drain(&mut alert, 5, 20, 1), vec![5]);
    }

    #[test]
    fn zero_threshold_disables_warnings() {
        let mut alert = LowBattery::default();
        assert!(drain(&mut alert, 0, 100, 0).is_empty());
    }
}

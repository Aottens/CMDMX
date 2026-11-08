//! Capacitive touch from the charge time of the fader's touch track (HW-16).

/// Samples between one-step baseline corrections while untouched.
const DRIFT_EVERY: u8 = 8;

pub struct Touch {
    threshold: u16,
    baseline: Option<u16>,
    touched: bool,
    count: u8,
}

impl Touch {
    pub fn new(threshold: u16) -> Self {
        Self {
            threshold,
            baseline: None,
            touched: false,
            count: 0,
        }
    }

    /// Feed one charge-time sample; returns whether the fader is touched.
    pub fn update(&mut self, charge_time: u16) -> bool {
        let base = *self.baseline.get_or_insert(charge_time);
        let limit = if self.touched {
            self.threshold / 2
        } else {
            self.threshold
        };
        self.touched = charge_time > base.saturating_add(limit);
        if !self.touched {
            self.count += 1;
            if self.count >= DRIFT_EVERY {
                self.count = 0;
                self.baseline = Some(match charge_time.cmp(&base) {
                    core::cmp::Ordering::Greater => base.saturating_add(1),
                    core::cmp::Ordering::Less => base.saturating_sub(1),
                    core::cmp::Ordering::Equal => base,
                });
            }
        }
        self.touched
    }
}

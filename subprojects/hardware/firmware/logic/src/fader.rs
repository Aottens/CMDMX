//! Fader filtering: deadband (HW-14) and settled boot position (HW-18).

/// 0.25 % of the 14-bit travel.
pub const DEFAULT_DEADBAND: u16 = 41;
const MAX: u16 = 16383;

/// 12-bit ADC reading to 14 bit; input above 4095 is clamped.
pub fn adc12_to_14(v: u16) -> u16 {
    let v = v.min(4095);
    // top code maps to 16383, mid-scale to exactly 8192
    (v << 2) | if v == 4095 { 3 } else { 0 }
}

pub struct Fader {
    deadband: u16,
    settle: u16,
    seen: u16,
    sum: u32,
    last: Option<u16>,
}

impl Fader {
    pub fn new(deadband: u16, settle_samples: u16) -> Self {
        Self {
            deadband,
            settle: settle_samples.max(1),
            seen: 0,
            sum: 0,
            last: None,
        }
    }

    /// Forget the position: the next settled value is sent again (new USB host).
    pub fn reset(&mut self) {
        self.seen = 0;
        self.sum = 0;
        self.last = None;
    }

    fn snap(&self, raw: u16) -> u16 {
        if raw <= self.deadband {
            0
        } else if raw >= MAX.saturating_sub(self.deadband) {
            MAX
        } else {
            raw
        }
    }

    /// Feed one 14-bit sample; returns a value only when it must be sent.
    pub fn update(&mut self, raw: u16) -> Option<u16> {
        let raw = raw.min(MAX);
        let Some(last) = self.last else {
            self.sum = self.sum.saturating_add(u32::from(raw));
            self.seen = self.seen.saturating_add(1);
            if self.seen < self.settle {
                return None;
            }
            let mean = u16::try_from(self.sum / u32::from(self.settle)).unwrap_or(MAX);
            let v = self.snap(mean);
            self.last = Some(v);
            return Some(v);
        };
        let v = self.snap(raw);
        let end = v == 0 || v == MAX;
        if v.abs_diff(last) > self.deadband || (end && v != last) {
            self.last = Some(v);
            Some(v)
        } else {
            None
        }
    }
}

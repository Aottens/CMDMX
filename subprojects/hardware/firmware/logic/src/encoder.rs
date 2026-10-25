//! Quadrature decoding and relative CC steps (HW-22, CTL-11).

/// Gray-code position of (a, b): 00, 10, 11, 01.
fn pos(a: bool, b: bool) -> u8 {
    match (a, b) {
        (false, false) => 0,
        (true, false) => 1,
        (true, true) => 2,
        (false, true) => 3,
    }
}

pub struct Quadrature {
    prev: u8,
}

impl Quadrature {
    pub fn new(a: bool, b: bool) -> Self {
        Self { prev: pos(a, b) }
    }

    /// +1 (A leading B), -1, or 0 for no change / an invalid double step.
    pub fn update(&mut self, a: bool, b: bool) -> i8 {
        let now = pos(a, b);
        let d = (now + 4 - self.prev) % 4;
        self.prev = now;
        match d {
            1 => 1,
            3 => -1,
            _ => 0,
        }
    }
}

/// Counts to steps, at most 63 per message; the rest carries to the next.
pub struct Accumulator {
    divider: i32,
    counts: i32,
}

impl Accumulator {
    pub fn new(divider: u16) -> Self {
        Self {
            divider: i32::from(divider).max(1),
            counts: 0,
        }
    }

    pub fn add(&mut self, counts: i32) {
        self.counts = self.counts.saturating_add(counts);
    }

    pub fn take(&mut self) -> Option<u8> {
        let steps = (self.counts / self.divider).clamp(-63, 63);
        if steps == 0 {
            return None;
        }
        self.counts -= steps * self.divider;
        Some(twos_complement(steps))
    }
}

/// Relative CC value: 1-63 up, 127-65 down.
pub fn twos_complement(steps: i32) -> u8 {
    let n = steps.clamp(-63, 63);
    u8::try_from(if n < 0 { 128 + n } else { n }).unwrap_or(0)
}

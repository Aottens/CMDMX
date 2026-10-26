//! Key debounce (HW-20): a change counts after `scans` equal readings.

pub struct Debounce {
    scans: u16,
    state: bool,
    run: u16,
}

impl Debounce {
    pub fn new(scans: u16) -> Self {
        Self {
            scans,
            state: false,
            run: 0,
        }
    }

    pub fn update(&mut self, pressed: bool) -> Option<bool> {
        if pressed == self.state {
            self.run = 0;
            return None;
        }
        self.run = self.run.saturating_add(1);
        if self.run >= self.scans {
            self.state = pressed;
            self.run = 0;
            Some(pressed)
        } else {
            None
        }
    }
}

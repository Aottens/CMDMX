//! Motorised fader control: P-drive to a target, off while touched (HW-16), stall timeout (HW-17),
//! no target at power-up (HW-18).

/// H-bridge command; the duty is in permille (0..=1000). `Up` is towards higher fader values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Drive {
    Coast,
    Up(u16),
    Down(u16),
}

#[derive(Debug, Clone, Copy)]
pub struct MotorConfig {
    /// Within this many 14-bit steps of the target counts as arrived.
    pub tolerance: u16,
    /// Duty (permille) per step of error.
    pub kp_permille_per_step: u16,
    /// Lowest duty that still moves the fader.
    pub min_duty: u16,
    pub max_duty: u16,
    /// Give up on a move after this long.
    pub timeout_ms: u32,
}

pub struct Motor {
    cfg: MotorConfig,
    target: Option<u16>,
    start: u32,
    touched: bool,
    stalled: bool,
}

impl Motor {
    pub fn new(cfg: MotorConfig) -> Self {
        Self {
            cfg,
            target: None,
            start: 0,
            touched: false,
            stalled: false,
        }
    }

    /// Ignored while touched. Starts the stall timer.
    pub fn set_target(&mut self, target: u16, now_ms: u32) {
        if self.touched {
            return;
        }
        self.target = Some(target);
        self.start = now_ms;
    }

    /// A touch switches the motor off and drops the target, so a release leaves the fader where the
    /// hand put it.
    pub fn set_touched(&mut self, touched: bool) {
        self.touched = touched;
        if touched {
            self.target = None;
        }
    }

    pub fn update(&mut self, position: u16, now_ms: u32) -> Drive {
        let Some(target) = self.target else {
            return Drive::Coast;
        };
        let error = position.abs_diff(target);
        if error <= self.cfg.tolerance {
            self.target = None;
            return Drive::Coast;
        }
        if now_ms.wrapping_sub(self.start) >= self.cfg.timeout_ms {
            self.target = None;
            self.stalled = true;
            return Drive::Coast;
        }
        let duty = error
            .saturating_mul(self.cfg.kp_permille_per_step)
            .clamp(self.cfg.min_duty, self.cfg.max_duty.max(self.cfg.min_duty));
        if position < target {
            Drive::Up(duty)
        } else {
            Drive::Down(duty)
        }
    }

    /// True once per stall.
    pub fn take_stall(&mut self) -> bool {
        core::mem::take(&mut self.stalled)
    }

    pub fn moving(&self) -> bool {
        self.target.is_some()
    }
}

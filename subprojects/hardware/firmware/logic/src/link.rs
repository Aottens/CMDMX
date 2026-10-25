//! Host link state for the disconnected pattern (HW-32).

#[derive(Default)]
pub struct Link {
    configured: bool,
    feedback: bool,
}

impl Link {
    pub fn new() -> Self {
        Self::default()
    }

    /// Any change of configuration needs new feedback from the host.
    pub fn set_configured(&mut self, configured: bool) {
        self.configured = configured;
        self.feedback = false;
    }

    pub fn feedback_received(&mut self) {
        if self.configured {
            self.feedback = true;
        }
    }

    pub fn disconnected(&self) -> bool {
        !(self.configured && self.feedback)
    }
}

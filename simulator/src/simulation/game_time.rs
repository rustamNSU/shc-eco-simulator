#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GameTime {
    ticks: u64,
}

impl GameTime {
    pub fn new() -> Self {
        Self { ticks: 0 }
    }

    pub fn ticks(&self) -> u64 {
        self.ticks
    }

    pub fn advance(&mut self, delta_ticks: u64) {
        self.ticks = self.ticks.saturating_add(delta_ticks);
    }
}

impl Default for GameTime {
    fn default() -> Self {
        Self::new()
    }
}

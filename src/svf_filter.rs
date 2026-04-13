pub struct SvfFilter {
    low: f32,
    band: f32,
}

impl SvfFilter {
    pub fn new() -> Self {
        Self { low: 0.0, band: 0.0 }
    }

    pub fn process(&mut self, raw_input: f32, user_cutoff: f32, resonance: f32) -> (f32, f32, f32) {
        // exponential curve
        let safe_cutoff = 0.001 + (user_cutoff * user_cutoff * 0.9);

        let q = 1.0 - resonance.max(0.0).min(0.99); 

        let high = raw_input - self.low - q * self.band;
        self.band += safe_cutoff * high;
        self.low += safe_cutoff * self.band;

        (self.low, high, self.band)
    }
}

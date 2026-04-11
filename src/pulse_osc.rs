pub struct PulseOscillator {
    pub sample_rate: u32,
    phase: f32,
    phase_increment: f32,
}

impl PulseOscillator {
    pub fn new(sample_rate: u32) -> PulseOscillator {
        PulseOscillator {
            sample_rate,
            phase: 0.0,
            phase_increment: 0.0,
        }
    }

    pub fn set_freq(&mut self, freq: f32) {
        self.phase_increment = freq / self.sample_rate as f32;
    }

    pub fn get_sample(&mut self, duty: f32) -> f32 {
        self.phase += self.phase_increment;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }

        if self.phase < duty { 1.0 } else { -1.0 }
    }
}

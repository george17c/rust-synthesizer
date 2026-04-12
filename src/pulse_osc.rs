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

pub struct PulseUnison {
    oscs: [PulseOscillator; 5],
    pub active_voices: usize,
}

impl PulseUnison {
    pub fn new(sample_rate: u32) -> Self {
        Self {
            oscs: core::array::from_fn(|_| PulseOscillator::new(sample_rate)),
            active_voices: 1,
        }
    }

    pub fn set_freq_and_detune(&mut self, base_freq: f32, detune: f32) {
        let center = (self.active_voices - 1) as f32 / 2.0;
        for i in 0..self.active_voices {
            let step = i as f32 - center;
            self.oscs[i].set_freq(base_freq * (1.0 + step * detune));
        }
    }

    pub fn get_sample(&mut self, duty: f32) -> f32 {
        let mut sum = 0.0;
        for i in 0..self.active_voices {
            sum += self.oscs[i].get_sample(duty);
        }
        sum / (self.active_voices as f32)
    }
}

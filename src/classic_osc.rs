#[derive(Clone)]
pub struct WtableOscillator {
    sample_rate: u32,
    wave_table: &'static [f32; 128],
    idx: f32,
    idx_increment: f32,
}

impl WtableOscillator {
    pub fn new(sample_rate: u32, wave_table: &'static [f32; 128]) -> WtableOscillator {
        return WtableOscillator {
            sample_rate: sample_rate,
            wave_table: wave_table,
            idx: 0.0, idx_increment: 0.0,
        }
    }

    pub fn set_table(&mut self, new_table: &'static [f32; 128]) {
        self.wave_table = new_table;
    }

    pub fn set_freq(&mut self, freq: f32) {
        self.idx_increment = freq * self.wave_table.len() as f32 / self.sample_rate as f32;
    }

    pub fn get_sample(&mut self) -> f32 {
        let sample = self.lerp();
        self.idx += self.idx_increment;
        self.idx %= self.wave_table.len() as f32;
        return sample;
    }

    fn lerp(&self) -> f32 {
        let truncated_index = self.idx as usize;
        let next_index = (truncated_index + 1) % self.wave_table.len();

        let next_index_weight = self.idx - truncated_index as f32;
        let truncated_index_weight = 1.0 - next_index_weight;

        return truncated_index_weight * self.wave_table[truncated_index] 
               + next_index_weight * self.wave_table[next_index];
    }
}

pub fn wave_sine(i: usize, size: usize) -> f32 {
    let pi = std::f32::consts::PI;
    (2.0 * pi * i as f32 / size as f32).sin()
}

pub fn wave_triangle(i: usize, size: usize) -> f32 {
    let pi = std::f32::consts::PI;
    (2.0 / pi) * (2.0 * pi * i as f32 / size as f32).sin().asin()
}

pub fn wave_saw(i: usize, size: usize) -> f32 {
    2.0 * (i as f32 / size as f32) - 1.0
}

pub fn make_wtable(func: fn(usize, usize) -> f32) -> [f32; 128] {
    let size: usize = 128;
    core::array::from_fn(|i| func(i, size))
}

pub struct WtableUnison {
    oscs: [WtableOscillator; 5],
    pub active_voices: usize,
}

impl WtableUnison {
    pub fn new(sample_rate: u32, table: &'static [f32; 128]) -> Self {
        Self {
            oscs: core::array::from_fn(|_| WtableOscillator::new(sample_rate, table)),
            active_voices: 1,
        }
    }

    pub fn set_table(&mut self, table: &'static [f32; 128]) {
        for osc in self.oscs.iter_mut() { osc.set_table(table); }
    }

    pub fn set_freq_and_detune(&mut self, base_freq: f32, detune: f32) {
        if self.active_voices <= 1 {
            self.oscs[0].set_freq(base_freq);
            return;
        }
        let center = (self.active_voices - 1) as f32 / 2.0;
        for i in 0..self.active_voices {
            let step = i as f32 - center;
            self.oscs[i].set_freq(base_freq * (1.0 + step * detune));
        }
    }

    pub fn get_sample(&mut self) -> f32 {
        let mut sum = 0.0;
        for i in 0..self.active_voices {
            sum += self.oscs[i].get_sample();
        }
        sum / (self.active_voices as f32)
    }
}

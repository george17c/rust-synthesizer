use crate::classic_osc::WtableOscillator;

pub struct DrumEnv {
    value: f32,
    decay: f32,
    active: bool,
}

impl DrumEnv {
    pub fn new(decay: f32) -> Self {
        Self { value: 0.0, decay: decay, active: false }
    }

    pub fn trigger(&mut self) {
        self.value = 1.0;
        self.active = true;
    }
    pub fn tick(&mut self) -> f32 {
        if self.active {
            self.value *= self.decay;
            if self.value < 0.0001 { self.value = 0.0; self.active = false; }
        }
        self.value
    }
}

pub struct KickDrum {
    sine: WtableOscillator,
    env: DrumEnv,
}

impl KickDrum {
    pub fn new(sample_rate: u32, sine_table: &'static [f32; 128]) -> Self {
        let sine = WtableOscillator::new(sample_rate, sine_table);
        Self { sine, env: DrumEnv::new(0.99985) }
    }
    pub fn trigger(&mut self) {
        // eliminate crackling
        self.sine.reset_phase();
        self.env.trigger();
    }
    pub fn get_sample(&mut self) -> f32 {
        let e = self.env.tick();
        if e <= 0.0 { return 0.0; }

        let freq = 40.0 + (110.0 * e);
        self.sine.set_freq(freq);

        2.0 * self.sine.get_sample() * e
    }
}

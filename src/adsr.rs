#[derive(Clone, Copy, PartialEq)]
pub enum AdsrStage {
    Off,
    Attack,
    Decay,
    Sustain,
    Release,
}

pub struct AdsrEnvelope {
    pub stage: AdsrStage,
    value: f32,
    sample_rate: f32,

    attack_time: f32,
    decay_time: f32,
    sustain_level: f32,
    release_time: f32,
}

impl AdsrEnvelope {
    pub fn new(sample_rate: u32) -> Self {
        Self {
            stage: AdsrStage::Off,
            value: 0.0,
            sample_rate: sample_rate as f32,
            attack_time: 0.05,
            decay_time: 0.4,
            sustain_level: 0.8,
            release_time: 0.4,
        }
    }

    pub fn note_on(&mut self) { self.stage = AdsrStage::Attack; }
    pub fn note_off(&mut self) { self.stage = AdsrStage::Release; }

    pub fn tick(&mut self) -> f32 {
        match self.stage {
            AdsrStage::Off => self.value = 0.0,
            AdsrStage::Attack => {
                self.value += 1.0 / (self.attack_time * self.sample_rate);
                if self.value >= 1.0 {
                    self.value = 1.0;
                    self.stage = AdsrStage::Decay;
                }
            }
            AdsrStage::Decay => {
                self.value -= (1.0 - self.sustain_level) / (self.decay_time * self.sample_rate);
                if self.value <= self.sustain_level {
                    self.value = self.sustain_level;
                    self.stage = AdsrStage::Sustain;
                }
            }
            AdsrStage::Sustain => self.value = self.sustain_level,
            AdsrStage::Release => {
                self.value -= 1.0 / (self.release_time * self.sample_rate);
                if self.value <= 0.0 {
                    self.value = 0.0;
                    self.stage = AdsrStage::Off;
                }
            }
        }
        self.value
    }
}

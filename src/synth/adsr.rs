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

    attack_time: f32,
    decay_time: f32,
    sustain_level: f32,
    release_time: f32,
}

impl AdsrEnvelope {
    pub fn new(sample_rate: u32) -> Self {
        let attack_time = 0.05;
        let decay_time = 0.4;
        let sustain_lvl = 0.8;
        let release_time = 0.4;
        let sr = sample_rate as f32;

        Self {
            stage: AdsrStage::Off,
            value: 0.0,
            attack_time: 1.0 / (attack_time * sr),
            decay_time: (1.0 - sustain_lvl) / (decay_time * sr),
            sustain_level: sustain_lvl,
            release_time: 1.0 / (release_time * sr),
        }
    }

    pub fn note_on(&mut self) { self.stage = AdsrStage::Attack; }
    pub fn note_off(&mut self) { self.stage = AdsrStage::Release; }

    pub fn set_params(&mut self, atk: f32, dcy: f32, sus: f32, rel: f32) {
        let sample_rate = 48000.0;
        self.attack_time = 1.0 / (atk.max(0.001) * sample_rate);
        self.decay_time = (1.0 - sus) / (dcy.max(0.001) * sample_rate);
        self.sustain_level = sus.clamp(0.0, 1.0);
        self.release_time = 1.0 / (rel.max(0.001) * sample_rate);
    }

    pub fn tick(&mut self) -> f32 {
        match self.stage {
            AdsrStage::Off => self.value = 0.0,
            AdsrStage::Attack => {
                self.value += self.attack_time;
                if self.value >= 1.0 {
                    self.value = 1.0;
                    self.stage = AdsrStage::Decay;
                }
            }
            AdsrStage::Decay => {
                self.value -= self.decay_time;
                if self.value <= self.sustain_level {
                    self.value = self.sustain_level;
                    self.stage = AdsrStage::Sustain;
                }
            }
            AdsrStage::Sustain => self.value = self.sustain_level,
            AdsrStage::Release => {
                self.value -= self.release_time;
                if self.value <= 0.0001 {
                    self.value = 0.0;
                    self.stage = AdsrStage::Off;
                }
            }
        }
        self.value
    }
}

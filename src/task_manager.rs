use rodio::Source;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::pulse_osc::PulseOscillator;
use crate::classic_osc::WtableOscillator;
use crate::adsr::AdsrEnvelope;
use crate::adsr::AdsrStage;

pub struct TaskManager {
    pub pulse: PulseOscillator,
    pub sine: WtableOscillator,
    pub square: WtableOscillator,
    pub triangle: WtableOscillator,
    pub saw: WtableOscillator,

    pub adsr: AdsrEnvelope,
    pub shared_gate: Arc<AtomicU32>, // 0 or 1

    pub shared_duty_bits: Arc<AtomicU32>, 
    pub shared_freq_bits: Arc<AtomicU32>,
    pub shared_mode: Arc<AtomicU32>,
}

impl Iterator for TaskManager {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        let gate = self.shared_gate.load(Ordering::Relaxed);
        let duty = f32::from_bits(self.shared_duty_bits.load(Ordering::Relaxed));
        let freq = f32::from_bits(self.shared_freq_bits.load(Ordering::Relaxed));
        let mode = self.shared_mode.load(Ordering::Relaxed);

        self.pulse.set_freq(freq);
        self.sine.set_freq(freq);
        self.square.set_freq(freq);
        self.triangle.set_freq(freq);
        self.saw.set_freq(freq);

        let sample = match mode {
            0 => self.pulse.get_sample(duty),
            1 => self.sine.get_sample(),
            2 => self.square.get_sample(),
            3 => self.triangle.get_sample(),
            4 => self.saw.get_sample(),
            _ => 0.0,
        };

        let stage = self.adsr.get_stage();
        if gate == 1 && stage == AdsrStage::Off {
            self.adsr.note_on();
        } else if gate == 0 && stage != AdsrStage::Release && stage != AdsrStage::Off {
            self.adsr.note_off();
        }

        let envelope_volume = self.adsr.tick();

        Some(sample * envelope_volume)
    }
}

impl Source for TaskManager {
    fn channels(&self) -> u16 { 1 }
    fn sample_rate(&self) -> u32 { self.pulse.sample_rate }
    fn current_frame_len(&self) -> Option<usize> { None }
    fn total_duration(&self) -> Option<Duration> { None }
}

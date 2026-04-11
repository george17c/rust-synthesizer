use rodio::Source;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::pulse_osc::PulseOscillator;
use crate::classic_osc::WtableOscillator;

pub struct TaskManager {
    pub pulse: PulseOscillator,
    pub sine: WtableOscillator,
    pub square: WtableOscillator,
    pub triangle: WtableOscillator,
    pub saw: WtableOscillator,

    pub shared_duty_bits: Arc<AtomicU32>, 
    pub shared_freq_bits: Arc<AtomicU32>,
    pub shared_mode: Arc<AtomicU32>,
}

impl Iterator for TaskManager {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        // 1. Citim valorile partajate
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

        Some(sample)
    }
}

impl Source for TaskManager {
    fn channels(&self) -> u16 { 1 }
    fn sample_rate(&self) -> u32 { self.pulse.sample_rate }
    fn current_frame_len(&self) -> Option<usize> { None }
    fn total_duration(&self) -> Option<Duration> { None }
}

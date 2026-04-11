use rodio::Source;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::pulse_osc::PulseOscillator;
use crate::classic_osc::WtableOscillator;
use crate::adsr::AdsrEnvelope;
use crate::adsr::AdsrStage;

const FREQS: [f32; 28] = [
    164.81, 174.61, 185.00, 196.00, 207.65, 220.00, 233.08, 246.94, 
    261.63, 277.18, 293.66, 311.13, 329.63, 349.23, 369.99, 392.00, 415.30, 440.00, 466.16, 493.88, 523.25,
    554.37, 587.33, 622.25, 659.25, 698.46, 739.99, 783.99
];

pub struct Voice {
    pub pulse: PulseOscillator,
    pub sine: WtableOscillator,
    pub square: WtableOscillator,
    pub triangle: WtableOscillator,
    pub saw: WtableOscillator,
    pub adsr: AdsrEnvelope,
    pub active_freq: f32,
}

impl Voice {
    pub fn set_freq(&mut self, freq: f32) {
        self.active_freq = freq;
        self.pulse.set_freq(freq);
        self.sine.set_freq(freq);
        self.square.set_freq(freq);
        self.triangle.set_freq(freq);
        self.saw.set_freq(freq);
    }
}

pub struct TaskManager {
    pub voices: [Voice; 8],
    pub last_key_mask: u32,

    pub shared_key_mask: Arc<AtomicU32>,
    pub shared_duty_bits: Arc<AtomicU32>,
    pub shared_mode: Arc<AtomicU32>,
    pub shared_scale: Arc<AtomicU32>,
}

impl Iterator for TaskManager {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        let duty = f32::from_bits(self.shared_duty_bits.load(Ordering::Relaxed));
        let mode = self.shared_mode.load(Ordering::Relaxed);
        let current_mask = self.shared_key_mask.load(Ordering::Relaxed);

        // detect pressed keys
        let offset = self.shared_scale.load(Ordering::Relaxed) as usize;

        if current_mask != self.last_key_mask {
            for i in 0..13 {
                let is_pressed = (current_mask & (1 << i)) != 0;
                let was_pressed = (self.last_key_mask & (1 << i)) != 0;
                let freq = FREQS[i + offset];

                if is_pressed && !was_pressed {
                    // NOTE ON: Căutăm o voce liberă
                    for voice in self.voices.iter_mut() {
                        if voice.adsr.stage == AdsrStage::Off {
                            voice.set_freq(freq);
                            voice.adsr.note_on();
                            break;
                        }
                    }
                } else if !is_pressed && was_pressed {
                    // NOTE OFF: Oprim vocea care cântă această frecvență
                    for voice in self.voices.iter_mut() {
                        if voice.active_freq == freq && voice.adsr.stage != AdsrStage::Off {
                            voice.adsr.note_off();
                        }
                    }
                }
            }
            self.last_key_mask = current_mask;
        }

        // mix sound
        let mut mixed_sample = 0.0;

        for voice in self.voices.iter_mut() {
            if voice.adsr.stage != AdsrStage::Off {
                let env_vol = voice.adsr.tick();

                let raw_sample = match mode {
                    0 => voice.pulse.get_sample(duty),
                    1 => voice.sine.get_sample(),
                    2 => voice.square.get_sample(),
                    3 => voice.triangle.get_sample(),
                    4 => voice.saw.get_sample(),
                    _ => 0.0,
                };
                mixed_sample += raw_sample * env_vol;
            }
        }

        Some(mixed_sample)
    }
}

impl Source for TaskManager {
    fn channels(&self) -> u16 { 1 }
    fn sample_rate(&self) -> u32 { self.voices[0].pulse.sample_rate }
    fn current_frame_len(&self) -> Option<usize> { None }
    fn total_duration(&self) -> Option<Duration> { None }
}

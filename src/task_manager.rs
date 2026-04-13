use rodio::Source;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::classic_osc::WtableUnison;
use crate::pulse_osc::PulseUnison;
use crate::classic_osc::WtableOscillator;
use crate::adsr::AdsrEnvelope;
use crate::adsr::AdsrStage;
use crate::svf_filter::SvfFilter;

const FREQS: [f32; 73] = [
    // C1 - B1
    32.70, 34.65, 36.71, 38.89, 41.20, 43.65, 46.25, 49.00, 51.91, 55.00, 58.27, 61.74,
    // C2 - B2
    65.41, 69.30, 73.42, 77.78, 82.41, 87.31, 92.50, 98.00, 103.83, 110.00, 116.54, 123.47,
    // C3 - B3
    130.81, 138.59, 146.83, 155.56, 164.81, 174.61, 185.00, 196.00, 207.65, 220.00, 233.08, 246.94,
    // C4 - B4
    261.63, 277.18, 293.66, 311.13, 329.63, 349.23, 369.99, 392.00, 415.30, 440.00, 466.16, 493.88,
    // C5 - B5
    523.25, 554.37, 587.33, 622.25, 659.25, 698.46, 739.99, 783.99, 830.61, 880.00, 932.33, 987.77,
    // C6 - B6
    1046.5, 1108.73, 1174.66, 1244.51, 1318.51, 1396.91, 1479.98, 1567.98, 1661.22, 1760.00, 1864.66, 1975.53,
    // C7
    2093.00
];

pub struct Voice {
    pub table_unison: WtableUnison,
    pub pulse_unison: PulseUnison,

    pub filter: SvfFilter,
    pub modulator: WtableOscillator,

    pub adsr: AdsrEnvelope,
    pub active_freq: f32,
    pub active_key: usize,
}

impl Voice {
    pub fn set_freq(&mut self, freq: f32, detune: f32) {
        self.active_freq = freq;
        self.pulse_unison.set_freq_and_detune(freq, detune);
        self.table_unison.set_freq_and_detune(freq, detune);
    }
}

pub struct TaskManager {
    pub voices: [Voice; 8],
    pub shared_unison_voice_cnt: Arc<AtomicU32>,
    pub shared_detune: Arc<AtomicU32>,

    pub cutoff: Arc<AtomicU32>,
    pub shared_filter_res: Arc<AtomicU32>,
    pub shared_filter_type: Arc<AtomicU32>,
    pub shared_filter_env_amt: Arc<AtomicU32>,

    pub sin_table: &'static [f32; 128],
    pub tri_table: &'static [f32; 128],
    pub saw_table: &'static [f32; 128],

    pub fm_ratio: Arc<AtomicU32>,
    pub fm_amount: Arc<AtomicU32>,

    // 0=Sin, 1=Tri, 2=Saw
    pub mod_shape: Arc<AtomicU32>,

    pub last_key_mask: u32,
    pub shared_key_mask: Arc<AtomicU32>,
    pub shared_duty_cycle: Arc<AtomicU32>,
    pub shared_mode: Arc<AtomicU32>,
    pub shared_scale: Arc<AtomicU32>,
}

impl Iterator for TaskManager {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        let duty = f32::from_bits(self.shared_duty_cycle.load(Ordering::Relaxed));
        let mode = self.shared_mode.load(Ordering::Relaxed);
        let current_mask = self.shared_key_mask.load(Ordering::Relaxed);

        let detune_amt = self.shared_detune.load(Ordering::Relaxed) as f32 / 1000.0;

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
                            voice.set_freq(freq, detune_amt);
                            voice.adsr.note_on();
                            voice.active_key = i;
                            break;
                        }
                    }
                } else if !is_pressed && was_pressed {
                    // NOTE OFF: Oprim vocea care cântă această frecvență
                    for voice in self.voices.iter_mut() {
                        if voice.active_key == i && voice.adsr.stage != AdsrStage::Off {
                            voice.adsr.note_off();
                            voice.active_key = 99;
                        }
                    }
                }
            }
            self.last_key_mask = current_mask;
        }

        // read fm controls
        let fm_ratio = f32::from_bits(self.fm_ratio.load(Ordering::Relaxed));
        let fm_amt = f32::from_bits(self.fm_amount.load(Ordering::Relaxed));
        let shape = self.mod_shape.load(Ordering::Relaxed);

        let voices_count = self.shared_unison_voice_cnt.load(Ordering::Relaxed) as usize;

        // filter controls
        let cutoff = f32::from_bits(self.cutoff.load(Ordering::Relaxed));
        let resonance = f32::from_bits(self.shared_filter_res.load(Ordering::Relaxed));
        let filter_type = self.shared_filter_type.load(Ordering::Relaxed);
        let filter_env_amt = f32::from_bits(self.shared_filter_env_amt.load(Ordering::Relaxed));

        // mix sound
        let mut mixed_sample = 0.0;

        for voice in self.voices.iter_mut() {
            if voice.adsr.stage != AdsrStage::Off {
                let env_vol = voice.adsr.tick();

                voice.pulse_unison.active_voices = voices_count;
                voice.table_unison.active_voices = voices_count;

                voice.modulator.set_table(match shape { 1 => self.tri_table, 2 => self.saw_table, _ => self.sin_table });
                voice.modulator.set_freq(voice.active_freq * fm_ratio);

                let mut m_sig = voice.modulator.get_sample();

                // for pitch shift effect
                if fm_ratio == 0.0 {
                    m_sig = 1.0;
                }

                let modulated_freq = (voice.active_freq + (m_sig * fm_amt * voice.active_freq)).max(1.0);

                let raw_sample = match mode {
                    // Pulse
                    0 => {
                        voice.pulse_unison.set_freq_and_detune(modulated_freq, detune_amt);
                        voice.pulse_unison.get_sample(duty)
                    },

                    // Sine, Tri, Saw
                    1..=3 => {
                        let table = match mode { 1 => self.sin_table, 2 => self.tri_table, _ => self.saw_table };
                        voice.table_unison.set_table(table);
                        voice.table_unison.set_freq_and_detune(modulated_freq, detune_amt);
                        voice.table_unison.get_sample()
                    },
                    _ => 0.0,
                };

                let dynamic_cutoff = (cutoff + (env_vol * filter_env_amt)).clamp(0.0, 1.0);
                let (low_pass, high_pass, band_pass) = voice.filter.process(raw_sample, dynamic_cutoff, resonance);

                let filtered_sample = match filter_type {
                    0 => low_pass,
                    1 => high_pass,
                    2 => band_pass,
                    _ => raw_sample,
                };

                mixed_sample += filtered_sample * env_vol;
            }
        }

        Some(mixed_sample * 0.125)
    }
}

impl Source for TaskManager {
    fn channels(&self) -> u16 { 1 }
    fn sample_rate(&self) -> u32 { 44100 }
    fn current_frame_len(&self) -> Option<usize> { None }
    fn total_duration(&self) -> Option<Duration> { None }
}

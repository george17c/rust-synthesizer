use crate::VOLUME;
use {defmt_rtt as _, panic_probe as _};
use embassy_stm32::{peripherals, sai::Sai};
use core::sync::atomic::Ordering;

use crate::synth::{WtableUnison, WtableOscillator, PulseUnison, SvfFilter, AdsrEnvelope};

const FREQS: [f32; 42] = [
    // C2 - B2
    65.41, 69.30, 73.42, 77.78, 82.41, 87.31, 92.50, 98.00, 103.83, 110.00, 116.54, 123.47,
    // C3 - B3
    130.81, 138.59, 146.83, 155.56, 164.81, 174.61, 185.00, 196.00, 207.65, 220.00, 233.08, 246.94,
    // C4 - B4
    261.63, 277.18, 293.66, 311.13, 329.63, 349.23, 369.99, 392.00, 415.30, 440.00, 466.16, 493.88,
    // C5 - F5
    523.25, 554.37, 587.33, 622.25, 659.25, 698.46,
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

#[embassy_executor::task]
pub async fn audio_task(
    mut sai: Sai<'static, peripherals::SAI1, u16>,
    sin_table: &'static [f32; 128],
    tri_table: &'static [f32; 128],
    saw_table: &'static [f32; 128],
) {
    let mut data: [u16; _] = [0u16; 512];

    let mut test_voice = Voice {
        table_unison: WtableUnison::new(48000, sin_table),
        pulse_unison: PulseUnison::new(48000),
        filter: SvfFilter::new(),
        modulator: WtableOscillator::new(48000, sin_table),
        adsr: AdsrEnvelope::new(48000),
        active_freq: 440.0,
        active_key: 99,
    };

    loop {
        let volume = VOLUME.load(Ordering::Relaxed) as f32;

        for i in (0..data.len()).step_by(2) {

            let raw_sample: f32 = test_voice.table_unison.get_sample();
            let scaled_sample: f32 = raw_sample * volume;

            let final_sample = scaled_sample as i16 as u16;

            data[i]     = final_sample; // left channel
            data[i + 1] = final_sample; // right channel
        }

        let _ = sai.write(&data).await;
    }
}
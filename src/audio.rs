use defmt::info;
use crate::VOLUME;
use crate::input::{KEY_BITMASK, SEQ_BITMASK};
use crate::synth::adsr::AdsrStage;
use crate::synth::effects::SynthState;
use {defmt_rtt as _, panic_probe as _};
use embassy_sync::blocking_mutex::{Mutex, raw::ThreadModeRawMutex};
use core::cell::RefCell;
use embassy_stm32::{peripherals, sai::Sai};
use core::sync::atomic::Ordering;
use embassy_executor::task;

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

#[derive(Clone, Copy)]
struct FrameParams {
    duty: f32, mode: u32,
    octave: usize, semi: usize,
    atk: f32, dcy: f32, sus: f32, rel: f32,
    unison_voices: usize, detune: f32,
    fm_ratio: f32, fm_shape: u32, fm_amt: f32,
    cutoff: f32, res: f32, filter_type: u32, filter_env: f32,
}

#[task]
pub async fn audio_task(
    state: &'static Mutex<ThreadModeRawMutex, RefCell<SynthState>>,
    mut sai: Sai<'static, peripherals::SAI1, u16>,
    sin_table: &'static [f32; 128],
    tri_table: &'static [f32; 128],
    saw_table: &'static [f32; 128],
) {
    let mut data = [0u16; 1024];

    let mut voices: [Voice; 5] = core::array::from_fn(|_| {
        Voice {
            table_unison: WtableUnison::new(48000, sin_table),
            pulse_unison: PulseUnison::new(48000),
            filter: SvfFilter::new(),
            modulator: WtableOscillator::new(48000, sin_table),
            adsr: AdsrEnvelope::new(48000),
            active_freq: 0.0,
            active_key: 99,
        }
    });

    let mut last_mask = KEY_BITMASK.load(Ordering::Relaxed);

    loop {
        let params = state.lock(|s| {
            let synth = s.borrow();
            let p = synth.active_preset();

            FrameParams {
                // Waveform (eff2 = duty, eff3 = shape)
                duty: p.effects[0].eff2,
                mode: p.effects[0].eff3 as u32,
                // Range (eff2 = octave, eff4 = semi)
                octave: p.effects[1].eff2 as usize,
                semi: p.effects[1].eff4 as usize,
                // Envelope
                atk: p.effects[2].eff1,
                dcy: p.effects[2].eff2,
                sus: p.effects[2].eff3,
                rel: p.effects[2].eff4,
                // Unison
                unison_voices: p.effects[3].eff3 as usize,
                detune: p.effects[3].eff4,
                // FM Mod
                fm_ratio: p.effects[4].eff2,
                fm_shape: p.effects[4].eff3 as u32,
                fm_amt: p.effects[4].eff4,
                // Filter
                cutoff: p.effects[5].eff1,
                res: p.effects[5].eff2,
                filter_type: p.effects[5].eff3 as u32,
                filter_env: p.effects[5].eff4,
            }
        });

        for voice in voices.iter_mut() {
            voice.adsr.set_params(params.atk, params.dcy, params.sus, params.rel);
            voice.pulse_unison.active_voices = params.unison_voices;
            voice.table_unison.active_voices = params.unison_voices;
        }

        let current_mask = KEY_BITMASK.load(Ordering::Relaxed) | SEQ_BITMASK.load(Ordering::Relaxed);
        if current_mask != last_mask {
            for i in 0..13 {
                let is_pressed = (current_mask & (1 << i)) != 0;
                let was_pressed = (last_mask & (1 << i)) != 0;

                if is_pressed && !was_pressed {
                    // NOTE ON: Căutăm o voce liberă
                    for voice in voices.iter_mut() {
                        if voice.adsr.stage == AdsrStage::Off {
                            let freq = FREQS[params.octave * 12 + params.semi + i];
                            info!("freq: {}", freq);
                            voice.set_freq(freq, 0.0);
                            voice.adsr.note_on();
                            voice.active_key = i;
                            break;
                        }
                    }
                } else if !is_pressed && was_pressed {
                    // NOTE OFF: Oprim vocea care cântă această frecvență
                    for voice in voices.iter_mut() {
                        if voice.active_key == i && voice.adsr.stage != AdsrStage::Off {
                            voice.adsr.note_off();
                            voice.active_key = 99;
                        }
                    }
                }
            }
            last_mask = current_mask;
        }

        for i in (0..data.len()).step_by(2) {
            let mut mixed_sample = 0.0;

            for voice in voices.iter_mut() {
                if voice.adsr.stage != AdsrStage::Off {
                    let env_vol = voice.adsr.tick();

                    // modulator
                    voice.modulator.set_table(match params.fm_shape { 
                        1 => tri_table, 2 => saw_table, _ => sin_table 
                    });
                    voice.modulator.set_freq(voice.active_freq * params.fm_ratio);

                    let m_sig = voice.modulator.get_sample();
                    let phase_mod = m_sig * params.fm_amt * 0.25;

                    // main oscillator
                    let raw_sample = match params.mode {
                        0 => { // Pulse
                            voice.pulse_unison.set_freq_and_detune(voice.active_freq, params.detune);
                            if params.fm_ratio == 0.0 || params.fm_amt == 0.0 {
                                voice.pulse_unison.get_sample(params.duty)
                            } else {
                                voice.pulse_unison.get_sample_fm(params.duty, phase_mod)
                            }
                        },
                        1..=3 => { // Sine, Tri, Saw
                            let table = match params.mode { 1 => sin_table, 2 => tri_table, _ => saw_table };
                            voice.table_unison.set_table(table);
                            voice.table_unison.set_freq_and_detune(voice.active_freq, params.detune);
                            if params.fm_ratio == 0.0 || params.fm_amt == 0.0 {
                                voice.table_unison.get_sample()
                            } else {
                                voice.table_unison.get_sample_fm(phase_mod)
                            }
                        },
                        _ => 0.0,
                    };

                    let filtered_sample = if params.filter_type == 3 {
                        raw_sample
                    } else {
                        let dynamic_cutoff = (params.cutoff + (env_vol * params.filter_env)).clamp(0.0, 1.0);
                        let (low_pass, high_pass, band_pass) =
                            voice.filter.process(raw_sample, dynamic_cutoff, params.res);
                        match params.filter_type {
                            0 => low_pass,
                            1 => high_pass,
                            2 => band_pass,
                            _ => raw_sample,
                        }
                    };

                    mixed_sample += filtered_sample * env_vol;
                }
            }

            let vol_norm = VOLUME.load(Ordering::Relaxed) as f32 / 100.0;
            let vol = vol_norm * vol_norm;

            let master_out = mixed_sample * vol * 0.2;

            let final_sample = (master_out * 20000.0).clamp(-20000.0, 20000.0) as i16 as u16;

            data[i]     = final_sample; // left channel
            data[i + 1] = final_sample; // right channel

            if i % 128 == 0 {
                embassy_futures::yield_now().await;
            }
        }

        let _ = sai.write(&data).await;
    }
}
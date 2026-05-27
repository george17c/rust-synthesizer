// use defmt::info;
use crate::VOLUME;
use crate::DisplayPage;
use crate::synth::effects::EffectType;
use crate::synth::effects::SynthState;
use crate::synth::Preset;
use defmt::info;
use core::sync::atomic::{AtomicU16, Ordering};
use {defmt_rtt as _, panic_probe as _};
use embassy_stm32::{peripherals, Peri};
use embassy_stm32::adc::{Adc, AnyAdcChannel, SampleTime};
use embassy_time::Timer;
use embassy_stm32::timer::qei::Qei;
use embassy_stm32::mode::Async;
use embassy_stm32::exti::ExtiInput;
use embassy_executor::{task};
use embassy_sync::blocking_mutex::{Mutex, raw::ThreadModeRawMutex};
use core::cell::RefCell;
use embassy_stm32::gpio::Input;
use crate::Irqs;

const POT_MIN: u32 = 2000;
const POT_MAX: u32 = 13000;

fn pot_to_volume(val: u32) -> i16 {
    let clamped = val.clamp(POT_MIN, POT_MAX);
    let adjusted_val = clamped - POT_MIN;
    let range = POT_MAX - POT_MIN;
    let discrete_steps = ((adjusted_val * 100) + (range / 2)) / range;

    discrete_steps as i16
}

fn pot_to_0_05(val: u32) -> f32 {
    let clamped = val.clamp(POT_MIN, POT_MAX);
    let adjusted_val = clamped - POT_MIN;
    let range = POT_MAX - POT_MIN;
    let discrete_steps = ((adjusted_val * 100) + (range / 2)) / range;

    discrete_steps as f32 * 0.005
}

pub fn pot_to_0_1(val: u32) -> f32 {
    let clamped = val.clamp(POT_MIN, POT_MAX);
    let adjusted_val = clamped - POT_MIN;
    let range = POT_MAX - POT_MIN;
    let discrete_steps = ((adjusted_val * 10) + (range / 2)) / range;

    discrete_steps as f32 * 0.1
}

pub fn pot_to_0_10(val: u32) -> f32 {
    let clamped = val.clamp(POT_MIN, POT_MAX);
    let adjusted_val = clamped - POT_MIN;
    let range = POT_MAX - POT_MIN;
    let discrete_steps = ((adjusted_val * 20) + (range / 2)) / range;

    discrete_steps as f32 * 0.5
}

fn pot_to_octave(val: u32) -> f32 {
    if val < 5000 {
        0.0
    } else if val > 10000 {
        2.0
    } else {
        1.0
    }
}

pub fn pot_to_semi(val: u32) -> f32 {
    let clamped = val.clamp(POT_MIN, POT_MAX);
    let adjusted_val = clamped - POT_MIN;
    let range = POT_MAX - POT_MIN;
    let discrete_steps = ((adjusted_val * 5) + (range / 2)) / range;

    discrete_steps as f32
}

pub fn pot_to_0_1_fine(val: u32) -> f32 {
    let clamped = val.clamp(POT_MIN, POT_MAX);
    let adjusted_val = clamped - POT_MIN;
    let range = POT_MAX - POT_MIN;
    let discrete_steps = ((adjusted_val * 100) + (range / 2)) / range;

    discrete_steps as f32 * 0.01
}

pub static KEY_BITMASK: AtomicU16 = AtomicU16::new(0);

#[task(pool_size = 13)]
pub async fn key_task(
    mut key: ExtiInput<'static, Async>,
    idx: usize,
) {
    loop {
        key.wait_for_low().await;
        KEY_BITMASK.fetch_or(1 << idx, Ordering::Relaxed);

        Timer::after_millis(50).await;

        while key.is_low() {
            Timer::after_millis(10).await;
        }

        key.wait_for_high().await;
        KEY_BITMASK.fetch_and(!(1 << idx), Ordering::Relaxed);

        Timer::after_millis(30).await;
    }
}

#[task(pool_size = 4)]
pub async fn page_task(state: &'static Mutex<ThreadModeRawMutex, RefCell<SynthState>>, button: Input<'static>, idx: u16) {
    loop {
        if button.is_low() {
            match idx {
                1 => state.lock(|s| s.borrow_mut().next_page()),
                _ => state.lock(|s| s.borrow_mut().prev_page()),
            }

            info!("page");

            while button.is_low() {
                Timer::after_millis(10).await;
            }

            Timer::after_millis(50).await;
        }

        Timer::after_millis(50).await;
    }
}

#[task]
pub async fn input_task(
    state: &'static Mutex<ThreadModeRawMutex, RefCell<SynthState>>,
    mut adc:        Adc<'static, peripherals::ADC1>,
    mut adc_dma:    Peri<'static, peripherals::GPDMA1_CH1>,
    mut vol_pot:    AnyAdcChannel<'static, peripherals::ADC1>,
    mut eff1_pot:   AnyAdcChannel<'static, peripherals::ADC1>,
    mut eff2_pot:   AnyAdcChannel<'static, peripherals::ADC1>,
    mut eff4_pot:   AnyAdcChannel<'static, peripherals::ADC1>,
    select_enc:     Qei<'static, peripherals::TIM2>,
    eff3_enc:       Qei<'static, peripherals::TIM3>,
) {
    let mut pot_readings = [0u16; 4];
    let mut last_volume: i16 = 0;

    let mut last_select = select_enc.count();
    let mut last_eff3 = eff3_enc.count();

    let mut seq = [
        (&mut vol_pot    as &mut AnyAdcChannel<peripherals::ADC1>, SampleTime::CYCLES160_5),
        (&mut eff1_pot   as &mut AnyAdcChannel<peripherals::ADC1>, SampleTime::CYCLES160_5),
        (&mut eff2_pot   as &mut AnyAdcChannel<peripherals::ADC1>, SampleTime::CYCLES160_5),
        (&mut eff4_pot   as &mut AnyAdcChannel<peripherals::ADC1>, SampleTime::CYCLES160_5),
    ];

    loop {
        let nsamples = 16;
        let mut vol: u32 = 0;
        let mut pot1: u32 = 0;
        let mut pot2: u32 = 0;
        let mut pot4: u32 = 0;

        for _ in 0..nsamples {
            adc.read(
                adc_dma.reborrow(),
                Irqs,
                seq.iter_mut().map(|(ch, st)| (&mut **ch, *st)),
                &mut pot_readings,
            ).await;
            vol  += pot_readings[0] as u32;
            pot1 += pot_readings[1] as u32;
            pot2 += pot_readings[2] as u32;
            pot4 += pot_readings[3] as u32;
        }

        vol  /= nsamples;
        pot1 /= nsamples;
        pot2 /= nsamples;
        pot4 /= nsamples;

        let new_volume = pot_to_volume(vol);
        if new_volume != last_volume {
            last_volume = new_volume;
            VOLUME.store(new_volume, Ordering::Relaxed);
        }

        // info!("vol: {}, ef1: {}, ef2: {}, ef4: {}", vol, pot1, pot2, pot4);

        let current_select = select_enc.count();
        let current_eff3 = eff3_enc.count();

        let delta_select = current_select.wrapping_sub(last_select) as i16;
        let delta_eff3 = current_eff3.wrapping_sub(last_eff3) as i16;
        let mut rot_select = 0;
        let mut rot_eff3 = 0;

        if delta_select >= 4 || delta_select <= -4 {
            rot_select = delta_select / 4;
            last_select = last_select.wrapping_add((rot_select * 4) as u16);
        }

        if delta_eff3 >= 4 || delta_eff3 <= -4 {
            rot_eff3 = delta_eff3 / 4;
            last_eff3 = last_eff3.wrapping_add((rot_eff3 * 4) as u16);
        }

        state.lock(|s| {
            let mut synth = s.borrow_mut();

            if synth.page == DisplayPage::PresetList {
                if rot_select > 0 {
                    synth.next_preset();
                } else if rot_select < 0 {
                    synth.prev_preset();
                }

            } else if synth.page == DisplayPage::EffectList {
                let preset: &mut Preset = synth.active_preset_mut();
                let eff = &mut preset.effects[preset.idx];

                if rot_select > 0 {
                    preset.next_effect();
                } else if rot_select < 0 {
                    preset.prev_effect();
                } else {
                    match eff.typ {
                        EffectType::Waveform => {
                            let mut waveform_diff = 0.0;
                            if delta_eff3 != 0 {
                                waveform_diff = rot_eff3 as f32;
                            }
                            eff.change_waveform(waveform_diff, pot_to_0_1(pot2));
                        },
                        EffectType::MusicalRange => {
                            eff.change_range(pot_to_octave(pot2), pot_to_semi(pot4));
                        },
                        EffectType::Envelope => {
                            let mut sustain_diff = 0.0;
                            if rot_eff3 != 0 {
                                sustain_diff = rot_eff3 as f32 / 10.0;
                            }

                            eff.change_envelope(pot_to_0_1_fine(pot1), pot_to_0_1_fine(pot2), sustain_diff, pot_to_0_1_fine(pot4));
                        }
                        EffectType::Unison => {
                            let mut voices_diff = 0.0;
                            if delta_eff3 != 0 {
                                voices_diff = rot_eff3 as f32;
                            }
                            eff.change_unison(voices_diff, pot_to_0_05(pot4));
                        },
                        EffectType::FMMod => {
                            let mut wave_diff = 0.0;
                            if delta_eff3 != 0 {
                                wave_diff = rot_eff3 as f32;
                            }
                            eff.change_fmmod(pot_to_0_10(pot2), wave_diff, pot_to_0_10(pot4));
                        }
                        EffectType::Filter => {
                            let mut type_diff = 0.0;
                            if delta_eff3 != 0 {
                                type_diff = rot_eff3 as f32;
                            }
                            eff.change_filter(pot_to_0_1_fine(pot1), pot_to_0_1_fine(pot2), type_diff, pot_to_0_1_fine(pot4));
                        },
                    }
                }
            }
        });

        Timer::after_millis(20).await;
    }
}

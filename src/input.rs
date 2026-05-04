use crate::VOLUME;
use defmt::{info};
use core::sync::atomic::{Ordering};
use {defmt_rtt as _, panic_probe as _};
use embassy_stm32::{peripherals, Peri};
use embassy_stm32::adc::{Adc, AnyAdcChannel, SampleTime};
use crate::Irqs;

// revise, maybe set the amplitude for the oscillators
fn pot_to_volume(raw: u16) -> i16 {
    let pot_min = 0.0f32;
    let pot_max = 16000.0f32;

    let clamped = (raw as f32).clamp(pot_min, pot_max);

    let normalized = (clamped - pot_min) / (pot_max - pot_min);

    let curved = normalized * normalized;

    (curved * 32767.0) as i16
}

#[embassy_executor::task]
pub async fn input_task(
    mut adc:      Adc<'static, peripherals::ADC1>,
    mut adc_dma:  Peri<'static, peripherals::GPDMA1_CH1>,
    mut vol_pot:  AnyAdcChannel<'static, peripherals::ADC1>,
    mut dummy1:   AnyAdcChannel<'static, peripherals::ADC1>,
    mut dummy2:   AnyAdcChannel<'static, peripherals::ADC1>,
) {
    let mut pot_readings = [0u16; 3];
    let mut last_volume: i16 = 0;
    let hysteresis: i32 = 1;

    let mut seq = [
        (&mut vol_pot  as &mut AnyAdcChannel<peripherals::ADC1>, SampleTime::CYCLES160_5),
        (&mut dummy1   as &mut AnyAdcChannel<peripherals::ADC1>, SampleTime::CYCLES12_5),
        (&mut dummy2   as &mut AnyAdcChannel<peripherals::ADC1>, SampleTime::CYCLES12_5),
    ];

    loop {
        // Oversampling x16
        let nsamples = 16;
        let mut sum = 0u32;
        for _ in 0..nsamples {
            adc.read(
                adc_dma.reborrow(),
                Irqs,
                seq.iter_mut().map(|(ch, st)| (&mut **ch, *st)),
                &mut pot_readings,
            ).await;
            sum += pot_readings[0] as u32;
        }

        let raw = (sum / nsamples) as u16;
        let new_volume = pot_to_volume(raw);

        // Hysteresis: ignora schimbari mici
        if (new_volume as i32 - last_volume as i32).abs() > hysteresis {
            last_volume = new_volume;
            VOLUME.store(new_volume, Ordering::Relaxed);
            info!("raw={} volume={}", raw, new_volume);
        }

        // Nu citi mai des decat are sens
        embassy_time::Timer::after_millis(10).await;
    }
}
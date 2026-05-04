use crate::VOLUME;
use {defmt_rtt as _, panic_probe as _};
use embassy_stm32::{peripherals, sai::Sai};
use core::sync::atomic::Ordering;

#[embassy_executor::task]
pub async fn audio_task(
    mut sai: Sai<'static, peripherals::SAI1, u16>,
) {
    let mut data: [u16; _] = [0u16; 512];
    let increment: f32 = 261.63 / 48000.0;
    let mut idx: f32 = 0.0;

    loop {
        let volume = VOLUME.load(Ordering::Relaxed);

        for i in (0..data.len()).step_by(2) {
            idx += increment;
            if idx > 1.0 { idx -= 1.0; }
            let sample = if idx > 0.5 { -volume } else { volume };
            data[i]     = sample as u16;
            data[i + 1] = sample as u16;
        }

        let _ = sai.write(&data).await;
    }
}
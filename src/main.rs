mod classic_osc;
mod pulse_osc;
mod adsr;
mod task_manager;
use rodio::{OutputStream, Sink};
use device_query::{DeviceQuery, DeviceState, Keycode};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;
use classic_osc::{make_wtable, wave_sine, wave_triangle, wave_saw};
use classic_osc::WtableOscillator;
use pulse_osc::PulseOscillator;
use task_manager::{TaskManager, Voice};
use adsr::AdsrEnvelope;

fn main() {
    // calculeaza la inceput static mut _TABLE si da referintele, ca sa nu ocupe mult ram
    // let sin_table: &'static = make_wtable(wave_sine);
    // let tri_table: &'static = make_wtable(wave_triangle);
    // let saw_table: &'static = make_wtable(wave_saw);
    let sin_table = make_wtable(wave_sine);
    let tri_table = make_wtable(wave_triangle);
    let saw_table = make_wtable(wave_saw);

    let device_state = DeviceState::new();
    let (_stream, stream_handle) = OutputStream::try_default().unwrap();
    let sink = Sink::try_new(&stream_handle).unwrap();

    let shared_mode = Arc::new(AtomicU32::new(0));
    let shared_duty = Arc::new(AtomicU32::new(0.5f32.to_bits()));
    let shared_key_mask = Arc::new(AtomicU32::new(0));
    let shared_scale_offset = Arc::new(AtomicU32::new(8));
    let shared_fm_ratio = Arc::new(AtomicU32::new(1.0f32.to_bits()));
    let shared_fm_amount = Arc::new(AtomicU32::new(0.0f32.to_bits()));

    let mut current_fm_ratio: f32 = 1.0;
    let mut current_fm_amt: f32 = 0.0;

    shared_scale_offset.store(24, Ordering::Relaxed);
    shared_mode.store(1, Ordering::Relaxed);

    let voices: [Voice; 8] = core::array::from_fn(|_| {
        Voice {
            pulse: PulseOscillator::new(44100),
            sine: WtableOscillator::new(44100, sin_table),
            triangle: WtableOscillator::new(44100, tri_table),
            saw: WtableOscillator::new(44100, saw_table),
            modulator: WtableOscillator::new(44100, sin_table),
            adsr: AdsrEnvelope::new(44100),
            active_freq: 0.0, active_key: 99,
        }
    });

    let audio_task = TaskManager {
        voices,
        shared_key_mask: Arc::clone(&shared_key_mask),
        shared_fm_ratio: Arc::clone(&shared_fm_ratio),
        shared_fm_amount: Arc::clone(&shared_fm_amount),
        shared_duty_bits: Arc::clone(&shared_duty),
        shared_mode: Arc::clone(&shared_mode),
        shared_scale: Arc::clone(&shared_scale_offset),
        last_key_mask: 0,
    };

    sink.append(audio_task);

    let mut current_duty: f32 = 0.5;

    println!("Pian: A, S, D, F, G, ...");
    println!("Sunet: 1=Pulse, 2=Sine, 3=Triangle, 4=Saw");
    println!("Pulse Width: ^ / v");

    loop {
        let keys = device_state.get_keys();
        let mut changed_duty = false;
        let mut changed_mod = false;

        if keys.contains(&Keycode::Z) { shared_scale_offset.store(0, Ordering::Relaxed); }
        if keys.contains(&Keycode::X) { shared_scale_offset.store(12, Ordering::Relaxed); }
        if keys.contains(&Keycode::C) { shared_scale_offset.store(24, Ordering::Relaxed); }
        if keys.contains(&Keycode::V) { shared_scale_offset.store(36, Ordering::Relaxed); }
        if keys.contains(&Keycode::B) { shared_scale_offset.store(48, Ordering::Relaxed); }
        if keys.contains(&Keycode::N) { shared_scale_offset.store(60, Ordering::Relaxed); }

        // timbre change
        if keys.contains(&Keycode::Key1) {
            shared_mode.store(0, Ordering::Relaxed); // Switch to Pulse
            println!("select Pulse");
        } else if keys.contains(&Keycode::Key2) {
            shared_mode.store(1, Ordering::Relaxed); // Switch to Sine
            println!("select Sine");
        } else if keys.contains(&Keycode::Key3) {
            shared_mode.store(2, Ordering::Relaxed); // Switch to Triangle
            println!("select Triangle");
        } else if keys.contains(&Keycode::Key4) {
            shared_mode.store(3, Ordering::Relaxed); // Switch to Saw
            println!("select Saw");
        }

        // duty cycle
        if shared_mode.load(Ordering::Relaxed) == 0 {
            if keys.contains(&Keycode::Up) {
                current_duty = (current_duty + 0.01).min(0.95);
                changed_duty = true;
            }
            if keys.contains(&Keycode::Down) {
                current_duty = (current_duty - 0.01).max(0.50);
                changed_duty = true;
            }
            if changed_duty {
                shared_duty.store(current_duty.to_bits(), Ordering::Relaxed);
            }
        }

        if keys.contains(&Keycode::Left) { current_fm_amt = (current_fm_amt - 0.1).max(0.0); changed_mod = true; std::thread::sleep(Duration::from_millis(150)); }
        if keys.contains(&Keycode::Right) { current_fm_amt = (current_fm_amt + 0.1).min(10.0); changed_mod = true; std::thread::sleep(Duration::from_millis(150)); }

        if keys.contains(&Keycode::PageUp) { current_fm_ratio += 0.5; changed_mod = true; std::thread::sleep(Duration::from_millis(200)); }
        if keys.contains(&Keycode::PageDown) { current_fm_ratio = (current_fm_ratio - 0.5).max(0.0); changed_mod = true; std::thread::sleep(Duration::from_millis(200)); }
        if keys.contains(&Keycode::Slash) { println!("fm mod reset"); current_fm_ratio = 1.0; current_fm_amt = 0.0; changed_mod = true; std::thread::sleep(Duration::from_millis(200)); }

        if changed_mod {
            shared_fm_amount.store(current_fm_amt.to_bits(), Ordering::Relaxed);
            shared_fm_ratio.store(current_fm_ratio.to_bits(), Ordering::Relaxed);
            println!("FM Ratio: {:.1} | FM Amount: {:.1}", current_fm_ratio, current_fm_amt);
        }

        let mut bitmask: u32 = 0;

        if keys.contains(&Keycode::A) { bitmask |= 1 << 0; }  // C
        if keys.contains(&Keycode::W) { bitmask |= 1 << 1; }  // C#
        if keys.contains(&Keycode::S) { bitmask |= 1 << 2; }  // D
        if keys.contains(&Keycode::E) { bitmask |= 1 << 3; }  // D#
        if keys.contains(&Keycode::D) { bitmask |= 1 << 4; }  // E
        if keys.contains(&Keycode::F) { bitmask |= 1 << 5; }  // F
        if keys.contains(&Keycode::T) { bitmask |= 1 << 6; }  // F#
        if keys.contains(&Keycode::G) { bitmask |= 1 << 7; }  // G
        if keys.contains(&Keycode::Y) { bitmask |= 1 << 8; }  // G#
        if keys.contains(&Keycode::H) { bitmask |= 1 << 9; }  // A
        if keys.contains(&Keycode::U) { bitmask |= 1 << 10; } // A#
        if keys.contains(&Keycode::J) { bitmask |= 1 << 11; } // B
        if keys.contains(&Keycode::K) { bitmask |= 1 << 12; } // C

        shared_key_mask.store(bitmask, Ordering::Relaxed);

        if keys.contains(&Keycode::Escape) { break; }

        std::thread::sleep(Duration::from_millis(5));
    }
}

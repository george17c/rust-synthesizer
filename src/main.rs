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
    // scapa de box leak mai incolo
    // let sin_table: &'static = make_wtable(wave_sine);
    // let tri_table: &'static = make_wtable(wave_triangle);
    // let saw_table: &'static = make_wtable(wave_saw);
    let sin_table: &'static [f32; 128] = Box::leak(Box::new(make_wtable(wave_sine)));
    let tri_table: &'static [f32; 128] = Box::leak(Box::new(make_wtable(wave_triangle)));
    let saw_table: &'static [f32; 128] = Box::leak(Box::new(make_wtable(wave_saw)));

    let device_state = DeviceState::new();
    let (_stream, stream_handle) = OutputStream::try_default().unwrap();
    let sink = Sink::try_new(&stream_handle).unwrap();

    let shared_mode = Arc::new(AtomicU32::new(1));
    let shared_duty = Arc::new(AtomicU32::new(0.5f32.to_bits()));
    let shared_key_mask = Arc::new(AtomicU32::new(0));
    let shared_scale_offset = Arc::new(AtomicU32::new(24));
    
    // Controale Modulator
    let shared_fm_ratio = Arc::new(AtomicU32::new(1.0f32.to_bits()));
    let shared_fm_amount = Arc::new(AtomicU32::new(0.0f32.to_bits()));
    let shared_mod_shape = Arc::new(AtomicU32::new(0)); // 0=Sin, 1=Tri, 2=Saw

    // Variabile locale pentru UI
    let mut current_fm_ratio: f32 = 1.0;
    let mut current_fm_amt: f32 = 0.0;
    let mut current_mod_shape: u32 = 0;

    let mut current_duty: f32 = 0.5;

    let voices: [Voice; 8] = core::array::from_fn(|_| {
        Voice {
            pulse: PulseOscillator::new(44100),
            sine: WtableOscillator::new(44100, sin_table),
            triangle: WtableOscillator::new(44100, tri_table),
            saw: WtableOscillator::new(44100, saw_table),
            modulator: WtableOscillator::new(44100, sin_table),
            adsr: AdsrEnvelope::new(44100),
            active_freq: 0.0,
            active_key: 99,
        }
    });

    let audio_task = TaskManager {
        voices,
        sin_table,
        tri_table,
        saw_table,
        shared_key_mask: Arc::clone(&shared_key_mask),

        fm_ratio: Arc::clone(&shared_fm_ratio),
        fm_amount: Arc::clone(&shared_fm_amount),
        mod_shape: Arc::clone(&shared_mod_shape),

        shared_duty_cycle: Arc::clone(&shared_duty),
        shared_mode: Arc::clone(&shared_mode),
        shared_scale: Arc::clone(&shared_scale_offset),
        last_key_mask: 0,
    };

    sink.append(audio_task);

    println!("Pian: A, S, D, F, G...");
    println!(" Sunet Carrier: 1=Pulse, 2=Sine, 3=Tri, 4=Saw");
    println!(" fm (Amount: <- / ->) | (Ratio: PgUp / PgDn) | (Forma: O)");
    println!(" Reset FM: /");

    loop {
        let keys = device_state.get_keys();
        let mut changed_duty = false;
        let mut changed_fm = false;

        if keys.contains(&Keycode::Z) { shared_scale_offset.store(0, Ordering::Relaxed); }
        if keys.contains(&Keycode::X) { shared_scale_offset.store(12, Ordering::Relaxed); }
        if keys.contains(&Keycode::C) { shared_scale_offset.store(24, Ordering::Relaxed); }
        if keys.contains(&Keycode::V) { shared_scale_offset.store(36, Ordering::Relaxed); }
        if keys.contains(&Keycode::B) { shared_scale_offset.store(48, Ordering::Relaxed); }
        if keys.contains(&Keycode::N) { shared_scale_offset.store(60, Ordering::Relaxed); }

        // timbre change
        if keys.contains(&Keycode::Key1) {
            shared_mode.store(0, Ordering::Relaxed); // Switch to Pulse
        } else if keys.contains(&Keycode::Key2) {
            shared_mode.store(1, Ordering::Relaxed); // Switch to Sine
        } else if keys.contains(&Keycode::Key3) {
            shared_mode.store(2, Ordering::Relaxed); // Switch to Triangle
        } else if keys.contains(&Keycode::Key4) {
            shared_mode.store(3, Ordering::Relaxed); // Switch to Saw
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

        // control fm
        if keys.contains(&Keycode::Left) { current_fm_amt = (current_fm_amt - 0.1).max(0.0); changed_fm = true; std::thread::sleep(Duration::from_millis(50)); }
        if keys.contains(&Keycode::Right) { current_fm_amt = (current_fm_amt + 0.1).min(10.0); changed_fm = true; std::thread::sleep(Duration::from_millis(50)); }
        
        if keys.contains(&Keycode::PageUp) { current_fm_ratio += 0.5; changed_fm = true; std::thread::sleep(Duration::from_millis(150)); }
        if keys.contains(&Keycode::PageDown) { current_fm_ratio = (current_fm_ratio - 0.5).max(0.0); changed_fm = true; std::thread::sleep(Duration::from_millis(150)); }

        if keys.contains(&Keycode::O) { 
            current_mod_shape = (current_mod_shape + 1) % 3; 
            shared_mod_shape.store(current_mod_shape, Ordering::Relaxed);
            changed_fm = true; 
            std::thread::sleep(Duration::from_millis(200)); 
        }

        if changed_fm {
            shared_fm_amount.store(current_fm_amt.to_bits(), Ordering::Relaxed);
            shared_fm_ratio.store(current_fm_ratio.to_bits(), Ordering::Relaxed);
            let shape_str = match current_mod_shape { 0 => "Sin", 1 => "Tri", _ => "Saw" };
            println!("fm -> Shape: {} | Ratio: {:.1} | Amount: {:.1}", shape_str, current_fm_ratio, current_fm_amt);
        }

        // reset fm
        if keys.contains(&Keycode::Slash) { 
            current_fm_ratio = 1.0; current_fm_amt = 0.0;
            shared_fm_amount.store(0.0f32.to_bits(), Ordering::Relaxed);
            shared_fm_ratio.store(1.0f32.to_bits(), Ordering::Relaxed);
            println!("FM Modulator Reset");
            std::thread::sleep(Duration::from_millis(200));
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

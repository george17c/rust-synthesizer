mod drums;
mod svf_filter;
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
use classic_osc::{WtableOscillator, WtableUnison};
use pulse_osc::{PulseUnison};
use task_manager::{TaskManager, Voice};
use adsr::AdsrEnvelope;
use crate::drums::KickDrum;
use crate::svf_filter::SvfFilter;

fn print_menu() {
    println!("Print this menu: F3");
    println!("Pian: A, S, D, F, G...K");
    println!("Gama: Z/X/C, Mod: N/M");
    println!(" Sunet Carrier: 7=Pulse, 8=Sine, 9=Tri, 0=Saw");
    println!(" Voices: 1, 3, 5, Detune: -/=");
    println!(" fm (Amount: <- / ->) | (Ratio: PgUp / PgDn) | (Forma: O) | Reset: /");
    println!(" Filter: type: F2, cutoff: 2/4 | resonance: F5/F6 | envelope: F7/F8");
}

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

    let shared_base_wf = Arc::new(AtomicU32::new(1));
    let shared_duty = Arc::new(AtomicU32::new(0.5f32.to_bits()));
    let shared_key_mask = Arc::new(AtomicU32::new(0));
    let shared_scale_offset = Arc::new(AtomicU32::new(12));
    let shared_mode_idx = Arc::new(AtomicU32::new(0));
    let shared_unison_voice_cnt = Arc::new(AtomicU32::new(1));
    let shared_detune = Arc::new(AtomicU32::new(0));
    let shared_filter_type= Arc::new(AtomicU32::new(3)); // 0-cutoff  1-high  2-band  3-no filter
    let shared_filter_res = Arc::new(AtomicU32::new(1.0f32.to_bits()));
    let shared_filter_env_amt = Arc::new(AtomicU32::new(1.0f32.to_bits()));
    let shared_drum_mask = Arc::new(AtomicU32::new(0));

    // Controale Modulator
    let shared_fm_ratio = Arc::new(AtomicU32::new(1.0f32.to_bits()));
    let shared_fm_amount = Arc::new(AtomicU32::new(0.0f32.to_bits()));
    let shared_mod_shape = Arc::new(AtomicU32::new(0)); // 0=Sin, 1=Tri, 2=Saw
    let cutoff = Arc::new(AtomicU32::new(1.0f32.to_bits()));

    // Variabile locale pentru UI
    let mut current_fm_ratio: f32 = 1.0;
    let mut current_fm_amt: f32 = 0.0;
    let mut current_mod_shape: u32 = 0;
    let mut current_detune: u32 = 0;
    let mut current_duty: f32 = 0.5;
    let mut current_cutoff: f32 = 1.0;
    let mut current_filter_type: u32 = 3;
    let mut current_res: f32 = 1.0;
    let mut current_filter_env_amt: f32 = 1.0;
    let mut current_mode_idx: u32 = 0;

    let voices: [Voice; 6] = core::array::from_fn(|_| {
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

    let audio_task = TaskManager {
        voices,
        shared_unison_voice_cnt: Arc::clone(&shared_unison_voice_cnt),
        shared_detune: Arc::clone(&shared_detune),

        cutoff: Arc::clone(&cutoff),
        shared_filter_type: Arc::clone(&shared_filter_type),
        shared_filter_res: Arc::clone(&shared_filter_res),
        shared_filter_env_amt: Arc::clone(&shared_filter_env_amt),

        sin_table,
        tri_table,
        saw_table,

        fm_ratio: Arc::clone(&shared_fm_ratio),
        fm_amount: Arc::clone(&shared_fm_amount),
        mod_shape: Arc::clone(&shared_mod_shape),

        shared_drum_mask: Arc::clone(&shared_drum_mask),
        last_drum_mask: 0,
        kick: KickDrum::new(48000, sin_table),

        last_key_mask: 0,
        shared_key_mask: Arc::clone(&shared_key_mask),
        shared_duty_cycle: Arc::clone(&shared_duty),
        shared_base_wf: Arc::clone(&shared_base_wf),
        shared_mode_idx: Arc::clone(&shared_mode_idx),
        shared_scale: Arc::clone(&shared_scale_offset),
    };

    sink.append(audio_task);

    let mut times = 0;
    let mut sample = false;

    print_menu();

    loop {
        let keys = device_state.get_keys();
        let mut changed_duty = false;
        let mut changed_fm = false;

        if keys.contains(&Keycode::Z) { shared_scale_offset.store(0, Ordering::Relaxed); }
        if keys.contains(&Keycode::X) { shared_scale_offset.store(12, Ordering::Relaxed); }
        if keys.contains(&Keycode::C) { shared_scale_offset.store(24, Ordering::Relaxed); }
        if keys.contains(&Keycode::N) {
            if current_mode_idx > 0 { current_mode_idx -= 1; }
            shared_mode_idx.store(current_mode_idx, Ordering::Relaxed);
            std::thread::sleep(Duration::from_millis(150));
        }
        if keys.contains(&Keycode::M) {
            current_mode_idx = (current_mode_idx + 1).min(5);
            shared_mode_idx.store(current_mode_idx, Ordering::Relaxed);
            std::thread::sleep(Duration::from_millis(150));
        }
        if keys.contains(&Keycode::N) || keys.contains(&Keycode::M) {
            print!("Mode: ");
            match current_mode_idx {
                0 => println!("Ionian"),
                1 => println!("Dorian"),
                2 => println!("Phrygian"),
                3 => println!("Lydian"),
                4 => println!("Mixolydian"),
                _ => println!("Aeolian"),
            }
        }

        if keys.contains(&Keycode::F3) {
            print_menu();
            std::thread::sleep(Duration::from_millis(500));
        }

        // timbre change
        if keys.contains(&Keycode::Key7) {
            shared_base_wf.store(0, Ordering::Relaxed); // Switch to Pulse
            println!("Mode: Pulse");
            std::thread::sleep(Duration::from_millis(150));
        } else if keys.contains(&Keycode::Key8) {
            shared_base_wf.store(1, Ordering::Relaxed); // Switch to Sine
            println!("Mode: Sine");
            std::thread::sleep(Duration::from_millis(150));
        } else if keys.contains(&Keycode::Key9) {
            shared_base_wf.store(2, Ordering::Relaxed); // Switch to Triangle
            println!("Mode: Triangle");
            std::thread::sleep(Duration::from_millis(150));
        } else if keys.contains(&Keycode::Key0) {
            shared_base_wf.store(3, Ordering::Relaxed); // Switch to Saw
            println!("Mode: Saw");
            std::thread::sleep(Duration::from_millis(150));
        }

        if keys.contains(&Keycode::Key1) {
            shared_unison_voice_cnt.store(1, Ordering::Relaxed); 
            println!("Unison: 1 voce");
            std::thread::sleep(Duration::from_millis(50));
        }
        if keys.contains(&Keycode::Key3) {
            shared_unison_voice_cnt.store(3, Ordering::Relaxed);
            println!("Unison: 3 voci");
            std::thread::sleep(Duration::from_millis(50));
        }
        if keys.contains(&Keycode::Key5) {
            shared_unison_voice_cnt.store(5, Ordering::Relaxed);
            println!("Unison: 5 voci");
            std::thread::sleep(Duration::from_millis(50));
        }

        // detune control
        if keys.contains(&Keycode::Minus) {
            if current_detune <= 5 {
                current_detune = 0;
            } else {
                current_detune -= 5;
            }
            shared_detune.store(current_detune, Ordering::Relaxed);
            println!("Detune amount: {}", current_detune as f32 / 1000.0);
            std::thread::sleep(Duration::from_millis(50));
        }
        if keys.contains(&Keycode::Equal) {
            current_detune = (current_detune + 5).min(500);
            shared_detune.store(current_detune, Ordering::Relaxed);
            println!("Detune amount: {}", current_detune as f32 / 1000.0);
            std::thread::sleep(Duration::from_millis(50));
        }

        // duty cycle
        if shared_base_wf.load(Ordering::Relaxed) == 0 {
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
        if keys.contains(&Keycode::Left) { current_fm_amt = (current_fm_amt - 0.1).max(0.0); changed_fm = true; std::thread::sleep(Duration::from_millis(10)); }
        if keys.contains(&Keycode::Right) { current_fm_amt = (current_fm_amt + 0.1).min(31.0); changed_fm = true; std::thread::sleep(Duration::from_millis(10)); }

        if keys.contains(&Keycode::PageUp) { current_fm_ratio += 0.1; changed_fm = true; std::thread::sleep(Duration::from_millis(15)); }
        if keys.contains(&Keycode::PageDown) { current_fm_ratio = (current_fm_ratio - 0.1).max(0.0); changed_fm = true; std::thread::sleep(Duration::from_millis(15)); }

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

        // update filter cutoff
        if keys.contains(&Keycode::Key4) {
            current_cutoff = (current_cutoff + 0.02).min(1.0);
            cutoff.store(current_cutoff.to_bits(), Ordering::Relaxed);
            println!("cutoff: {}", current_cutoff);
            std::thread::sleep(Duration::from_millis(10));
        }
        if keys.contains(&Keycode::Key2) {
            current_cutoff = (current_cutoff - 0.02).max(0.0);
            cutoff.store(current_cutoff.to_bits(), Ordering::Relaxed);
            println!("cutoff: {}", current_cutoff);
            std::thread::sleep(Duration::from_millis(10));
        }
        // update filter resonance
        if keys.contains(&Keycode::F6) {
            current_res = (current_res + 0.02).min(1.0);
            shared_filter_res.store(current_res.to_bits(), Ordering::Relaxed);
            println!("Resonance: {:.2}", current_res);
            std::thread::sleep(Duration::from_millis(10));
        }
        if keys.contains(&Keycode::F5) {
            current_res = (current_res - 0.02).max(0.0);
            shared_filter_res.store(current_res.to_bits(), Ordering::Relaxed);
            println!("Resonance: {:.2}", current_res);
            std::thread::sleep(Duration::from_millis(10));
        }
        // update filter type
        if keys.contains(&Keycode::F2) {
            current_filter_type = (current_filter_type + 1) % 4;
            shared_filter_type.store(current_filter_type, Ordering::Relaxed);
            match current_filter_type {
                0 => { println!("Selected low pass"); cutoff.store(1.0f32.to_bits(), Ordering::Relaxed); current_cutoff = 1.0; },
                1 => { println!("Selected high pass"); cutoff.store(0.0f32.to_bits(), Ordering::Relaxed); current_cutoff = 0.0; },
                2 => { println!("Selected band pass"); cutoff.store(0.1f32.to_bits(), Ordering::Relaxed); current_cutoff = 0.1; },
                _ => println!("Selected no filter"),
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        if keys.contains(&Keycode::F8) {
            current_filter_env_amt = (current_filter_env_amt + 0.02).min(1.0);
            shared_filter_env_amt.store(current_filter_env_amt.to_bits(), Ordering::Relaxed);
            println!("Filter Env Amount: {:.2}", current_filter_env_amt);
            std::thread::sleep(Duration::from_millis(10));
        }
        if keys.contains(&Keycode::F7) {
            current_filter_env_amt = (current_filter_env_amt - 0.02).max(0.0);
            shared_filter_env_amt.store(current_filter_env_amt.to_bits(), Ordering::Relaxed);
            println!("Filter Env Amount: {:.2}", current_filter_env_amt);
            std::thread::sleep(Duration::from_millis(10));
        }

        let mut drum_mask: u32 = 0;
        if keys.contains(&Keycode::Home) { drum_mask |= 1 << 0; } // kick

        shared_drum_mask.store(drum_mask, Ordering::Relaxed);

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
        if keys.contains(&Keycode::Q) {
            if sample == true {
                sample = false;
            } else {
                sample = true;
            }
            std::thread::sleep(Duration::from_millis(30));
        }

        if sample == true {
            // 5 notes at close intervals
            if times % 78 == 0 {
                times = 0;
            }
            if times >= 40 && (times - 40) % 9 == 0 {
                bitmask |= 1 << 0;
            }
        }

        shared_key_mask.store(bitmask, Ordering::Relaxed);

        if keys.contains(&Keycode::Escape) { break; }

        if sample == true {
            if times >= 40 && (times - 40) % 9 == 0 {
                std::thread::sleep(Duration::from_millis(30));
            }
            times += 1;
        }

        std::thread::sleep(Duration::from_millis(10));

    }
    std::thread::sleep(Duration::from_millis(500));
}

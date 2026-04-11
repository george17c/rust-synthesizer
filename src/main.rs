mod classic_osc;
mod pulse_osc;
mod task_manager;
use rodio::{OutputStream, Sink};
use device_query::{DeviceQuery, DeviceState, Keycode};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;
use classic_osc::make_wtable;
use classic_osc::wave_sine;
use classic_osc::wave_square;
use classic_osc::wave_triangle;
use classic_osc::wave_saw;
use classic_osc::WtableOscillator;
use pulse_osc::PulseOscillator;
use task_manager::TaskManager;

fn main() {
    let sin_osc  = WtableOscillator::new(44100, make_wtable(wave_sine));
    let sqr_osc  = WtableOscillator::new(44100, make_wtable(wave_square));
    let tri_osc  = WtableOscillator::new(44100, make_wtable(wave_triangle));
    let saw_osc  = WtableOscillator::new(44100, make_wtable(wave_saw));

    let device_state = DeviceState::new();
    let (_stream, stream_handle) = OutputStream::try_default().unwrap();
    let sink = Sink::try_new(&stream_handle).unwrap();

    let shared_duty = Arc::new(AtomicU32::new(0.5f32.to_bits()));
    let shared_freq = Arc::new(AtomicU32::new(440.0f32.to_bits())); 
    let shared_mode = Arc::new(AtomicU32::new(0)); // 0 = Pulse

    let audio_task = TaskManager {
        pulse: PulseOscillator::new(44100, 440.0),
        sine: sin_osc, square: sqr_osc, triangle: tri_osc, saw: saw_osc,
        shared_duty_bits: Arc::clone(&shared_duty),
        shared_freq_bits: Arc::clone(&shared_freq),
        shared_mode: Arc::clone(&shared_mode),
    };

    sink.append(audio_task);

    let mut current_duty: f32 = 0.5;

    println!("Pian: A, S, D, F, G");
    println!("Sunet: 1=Pulse, 2=Sine, 3=Square, 4=Triangle, 5=Saw");
    println!("Pulse Width: Sus / Jos");

    loop {
        let keys = device_state.get_keys();
        let mut changed_duty = false;

        // --- SCHIMBARE TIMBRU (Switch Oscilatoare) ---
        if keys.contains(&Keycode::Key1) {
            shared_mode.store(0, Ordering::Relaxed); // Switch to Pulse
        } else if keys.contains(&Keycode::Key2) {
            shared_mode.store(1, Ordering::Relaxed); // Switch to Sine
        } else if keys.contains(&Keycode::Key3) {
            shared_mode.store(2, Ordering::Relaxed); // Switch to Saw
        } else if keys.contains(&Keycode::Key4) {
            shared_mode.store(3, Ordering::Relaxed); // Switch to Saw
        } else if keys.contains(&Keycode::Key5) {
            shared_mode.store(4, Ordering::Relaxed); // Switch to Saw
        }

        // --- CONTROL DUTY CYCLE ---
        if keys.contains(&Keycode::Up) {
            current_duty = (current_duty + 0.01).min(0.95);
            changed_duty = true;
        }
        if keys.contains(&Keycode::Down) {
            current_duty = (current_duty - 0.01).max(0.05);
            changed_duty = true;
        }
        if changed_duty {
            shared_duty.store(current_duty.to_bits(), Ordering::Relaxed);
        }

        // --- CONTROL PITCH ---
        if keys.contains(&Keycode::A) { shared_freq.store(440.00f32.to_bits(), Ordering::Relaxed); }
        if keys.contains(&Keycode::S) { shared_freq.store(493.88f32.to_bits(), Ordering::Relaxed); }
        if keys.contains(&Keycode::D) { shared_freq.store(523.25f32.to_bits(), Ordering::Relaxed); }
        if keys.contains(&Keycode::F) { shared_freq.store(587.33f32.to_bits(), Ordering::Relaxed); }

        if keys.contains(&Keycode::Escape) { break; }

        std::thread::sleep(Duration::from_millis(15));
    }
}

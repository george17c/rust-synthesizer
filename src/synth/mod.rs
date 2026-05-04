pub mod adsr;
pub mod classic_osc;
pub mod pulse_osc;
pub mod svf_filter;

pub use classic_osc::{WtableUnison, WtableOscillator, make_wtable};
pub use classic_osc::{wave_sine, wave_saw, wave_triangle};
pub use pulse_osc::{PulseUnison};
pub use svf_filter::SvfFilter;
pub use adsr::AdsrEnvelope;
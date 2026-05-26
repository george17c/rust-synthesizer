use crate::synth::effects::{Preset, EffectState, EffectType, PRESETS_NUM};

use crate::DisplayPage;
use embedded_graphics::{
    mono_font::{ascii::{FONT_6X10, FONT_10X20}},
    pixelcolor::Rgb565,
    prelude::*,
    primitives::{Line, PrimitiveStyle, Rectangle},
    text::Text,
};
use core::fmt::Write;
use heapless::String;
use mipidsi::models::ILI9341Rgb565;
use display_interface_spi::SPIInterface;
use embedded_hal_bus::spi::ExclusiveDevice;
use embassy_stm32::spi::Spi;
use embedded_graphics::mono_font::MonoTextStyleBuilder;
use embassy_stm32::gpio::Output;

type DisplayType = mipidsi::Display<SPIInterface<ExclusiveDevice<Spi<'static, embassy_stm32::mode::Blocking, embassy_stm32::spi::mode::Master>,
    Output<'static>, embedded_hal_bus::spi::NoDelay>, Output<'static>>,
    ILI9341Rgb565, Output<'static>>;

pub struct DisplayManager {
    pub display: DisplayType,

    last_page: Option<DisplayPage>,
    last_preset_idx: usize,
    last_effect_idx: usize,
    last_eff_vals: [f32; 4],
}

impl DisplayManager {
    pub fn new(display: DisplayType) -> Self {
        Self {
            display,
            last_page: None,
            last_preset_idx: 999,
            last_effect_idx: 999,
            last_eff_vals: [-999.0; 4],
        }
    }

    fn clear_band(&mut self, y: i32, height: u32) {
        Rectangle::new(Point::new(0, y), Size::new(320, height))
            .into_styled(PrimitiveStyle::with_fill(Rgb565::BLUE))
            .draw(&mut self.display).unwrap();
    }

    pub fn render(&mut self, presets: &[Preset; PRESETS_NUM], selected_preset: usize, page: DisplayPage) {
        let page_changed = self.last_page != Some(page);

        if page_changed {
            self.last_page = Some(page);
            self.last_preset_idx = 999;
            self.last_effect_idx = 999;
            self.last_eff_vals = [-999.0; 4];
        }

        match page {
            DisplayPage::PresetList => {
                if page_changed || self.last_preset_idx != selected_preset {
                    self.draw_preset_list(presets, selected_preset, page_changed);
                    self.last_preset_idx = selected_preset;
                }
            }
            DisplayPage::EffectList => {
                let selected_effect = presets[selected_preset].idx;
                let current_eff = &presets[selected_preset].effects[selected_effect];

                let current_vals = [current_eff.eff1, current_eff.eff2, current_eff.eff3, current_eff.eff4];

                let effect_changed = self.last_effect_idx != selected_effect;
                let vals_changed = self.last_eff_vals != current_vals;

                if page_changed || effect_changed || vals_changed {
                    self.last_preset_idx = selected_preset;
                    self.draw_effect_list(&presets[selected_preset], page_changed, self.last_effect_idx, vals_changed);

                    self.last_effect_idx = selected_effect;
                    self.last_eff_vals = current_vals;
                }
            }
        }
    }

    fn draw_preset_list(&mut self, presets: &[Preset; PRESETS_NUM], selected: usize, force_full: bool) {
        let style_normal = MonoTextStyleBuilder::new()
            .font(&FONT_10X20).text_color(Rgb565::WHITE).background_color(Rgb565::BLUE).build();

        let style_highlight = MonoTextStyleBuilder::new()
            .font(&FONT_10X20).text_color(Rgb565::BLUE).background_color(Rgb565::WHITE).build();

        if force_full {
            self.clear_band(0, 40);
            let presets_end_y = 20 + (PRESETS_NUM as i32 * 20);
            self.clear_band(presets_end_y, (220 - presets_end_y) as u32); 
        }

        for (i, p) in presets.iter().enumerate() {
            if force_full || i == selected || i == self.last_preset_idx {
                let current_style = if i == selected { style_highlight } else { style_normal };

                let mut buf: String<32> = String::new();
                write!(buf, " {:<31}", p.name).unwrap();

                Text::new(&buf, Point::new(0, 40 + (i as i32 * 20)), current_style)
                    .draw(&mut self.display).unwrap();
            }
        }

        if force_full {
            let mut buf: String<32> = String::new();
            write!(buf, " {:<31}", "Select").unwrap();
            Text::new(&buf, Point::new(0, 230), style_normal).draw(&mut self.display).unwrap();
        }
    }

    pub fn draw_effect_list(&mut self, preset: &Preset, force_full: bool, old_selected_effect: usize, vals_changed: bool) {
        let selected_index = preset.idx;

        let style_normal = MonoTextStyleBuilder::new().font(&FONT_10X20).text_color(Rgb565::WHITE).background_color(Rgb565::BLUE).build();
        let style_highlight = MonoTextStyleBuilder::new().font(&FONT_10X20).text_color(Rgb565::BLUE).background_color(Rgb565::WHITE).build();
        let style_disabled = MonoTextStyleBuilder::new().font(&FONT_10X20).text_color(Rgb565::CSS_GRAY).background_color(Rgb565::BLUE).build();

        if force_full {
            // row 1
            self.clear_band(20, 40); 

            // previously selected preset
            self.clear_band(20 + self.last_preset_idx as i32 * 20, 40);

            // effect values band
            self.clear_band(206, 34);

            let mut title: String<32> = String::new();
            write!(title, " Preset: {:<23}", preset.name).unwrap();
            Text::new(&title, Point::new(0, 20), style_normal).draw(&mut self.display).unwrap();

            Line::new(Point::new(0, 205), Point::new(320, 205))
                .into_styled(PrimitiveStyle::with_stroke(Rgb565::WHITE, 1))
                .draw(&mut self.display).unwrap();
        }

        let start_y = 60;
        let step_y = 20;

        for (i, effect) in preset.effects.iter().enumerate() {
            if force_full || i == selected_index || i == old_selected_effect {
                let col = i / 5; 
                let row = i % 5; 

                let x_pos = col as i32 * 160; 
                let y_pos = start_y + (row as i32 * step_y);

                let eff_name = match effect.typ {
                    EffectType::Waveform => "1.Waveform",
                    EffectType::MusicalRange => "2.Range   ",
                    EffectType::Envelope => "3.Envelope",
                    EffectType::Unison => "4.Unison  ",
                    EffectType::FMMod => "5.FM Mod  ",
                    EffectType::Filter => "6.Filter  ",
                };

                let current_style = if i == selected_index {
                    style_highlight
                } else {
                    match effect.enabled {
                        EffectState::On => style_normal,
                        EffectState::Off => style_disabled,
                    }
                };

                let mut buf: String<16> = String::new();
                write!(buf, " {:<15}", eff_name).unwrap();

                Text::new(&buf, Point::new(x_pos, y_pos), current_style).draw(&mut self.display).unwrap();
            }
        }

        if force_full || selected_index != old_selected_effect || vals_changed {
            let sel_eff =  preset.effects[selected_index];

            let labels = match sel_eff.typ {
                EffectType::Waveform => ("--", "Duty", "Shape", "--"),
                EffectType::MusicalRange => ("--", "Octave", "--", "Semi"),
                EffectType::Envelope => ("Attack", "Decay", "Sustain", "Release"),
                EffectType::Unison => ("--", "--", "Voices", "Detune"),
                EffectType::FMMod => ("--", "Ratio", "Shape", "Amount"),
                EffectType::Filter => ("Cutoff", "Res", "Type", "Env"),
            };

            let style_bottom = MonoTextStyleBuilder::new()
                .font(&FONT_6X10).text_color(Rgb565::YELLOW).background_color(Rgb565::BLUE).build();

            let mut l0: String<16> = String::new(); 
            let mut l1: String<16> = String::new(); 
            let mut l2: String<16> = String::new(); 
            let mut l3: String<16> = String::new(); 

            write!(l0, " {:<12}", labels.0).unwrap();
            write!(l1, " {:<12}", labels.1).unwrap();
            write!(l2, " {:<12}", labels.2).unwrap();
            write!(l3, " {:<12}", labels.3).unwrap();

            Text::new(&l0, Point::new(0, 220), style_bottom).draw(&mut self.display).unwrap();
            Text::new(&l1, Point::new(80, 220), style_bottom).draw(&mut self.display).unwrap();
            Text::new(&l2, Point::new(160, 220), style_bottom).draw(&mut self.display).unwrap();
            Text::new(&l3, Point::new(240, 220), style_bottom).draw(&mut self.display).unwrap();

            l0.clear(); l1.clear(); l2.clear(); l3.clear();

            if labels.0 == "--" { write!(l0, " {:<12}", "").unwrap(); } else { write!(l0, " {:<12.2}", sel_eff.eff1).unwrap(); }
            if labels.1 == "--" { write!(l1, " {:<12}", "").unwrap(); } else { write!(l1, " {:<12.2}", sel_eff.eff2).unwrap(); }
            if labels.2 == "--" { write!(l2, " {:<12}", "").unwrap(); } else { write!(l2, " {:<12.2}", sel_eff.eff3).unwrap(); }
            if labels.3 == "--" { write!(l3, " {:<12}", "").unwrap(); } else { write!(l3, " {:<12.2}", sel_eff.eff4).unwrap(); }

            Text::new(&l0, Point::new(0, 230), style_bottom).draw(&mut self.display).unwrap();
            Text::new(&l1, Point::new(80, 230), style_bottom).draw(&mut self.display).unwrap();
            Text::new(&l2, Point::new(160, 230), style_bottom).draw(&mut self.display).unwrap();
            Text::new(&l3, Point::new(240, 230), style_bottom).draw(&mut self.display).unwrap();
        }
    }
}

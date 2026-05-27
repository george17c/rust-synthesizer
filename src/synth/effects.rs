pub const PRESETS_NUM: usize = 5;
pub const EFFECTS_NUM: usize = 6;

#[derive(Clone, Copy, PartialEq)]
pub enum EffectType {
    Waveform,
    MusicalRange,
    Envelope,
    Unison,
    FMMod,
    Filter,
}

#[derive(Clone, Copy, PartialEq)]
pub enum EffectState {
    On, Off,
}

#[derive(Clone, Copy)]
pub struct Effect {
    pub enabled: EffectState,
    pub typ: EffectType,
    pub fresh_change1: bool,
    pub fresh_change2: bool,
    pub fresh_change4: bool,
    pub eff1: f32,
    pub eff2: f32,
    pub eff3: f32,
    pub eff4: f32,
}

impl Effect {
    pub fn new(enabled: EffectState, typ: EffectType, eff1: f32, eff2: f32, eff3: f32, eff4: f32) -> Effect {
        Self {
            enabled,
            typ,
            fresh_change1: false,
            fresh_change2: false,
            fresh_change4: false,
            eff1,
            eff2,
            eff3,
            eff4,
        }
    }

    pub fn fresh(&mut self) {
        self.fresh1();
        self.fresh2();
        self.fresh4();
    }

    pub fn fresh1(&mut self) {
        self.fresh_change1 = true;
    }

    pub fn stale1(&mut self) {
        self.fresh_change1 = false;
    }

    pub fn fresh2(&mut self) {
        self.fresh_change2 = true;
    }

    pub fn stale2(&mut self) {
        self.fresh_change2 = false;
    }

    pub fn fresh4(&mut self) {
        self.fresh_change4 = true;
    }

    pub fn stale4(&mut self) {
        self.fresh_change4 = false;
    }

    pub fn enable(&mut self) {
        self.enabled = EffectState::On;
    }

    pub fn disable(&mut self) {
        self.enabled = EffectState::Off;
    }

    pub fn change_eff(&mut self, eff1: f32, eff2: f32, eff4: f32, delta: f32) {
        if eff1 >= 0.0 {
            if self.fresh_change1 {
                if (eff1 - self.eff1).abs() <= delta {
                    self.stale1();
                }
            } else {
                self.eff1 = eff1;
            }
        }

        if eff2 >= 0.0 {
            if self.fresh_change2 {
                if (eff2 - self.eff2).abs() <= delta {
                    self.stale2();
                }
            } else {
                self.eff2 = eff2;
            }
        }

        if eff4 >= 0.0 {
            if self.fresh_change4 {
                if (eff4 - self.eff4).abs() <= delta {
                    self.stale4();
                }
            } else {
                self.eff4 = eff4;
            }
        }
    }

    pub fn change_waveform(&mut self, typ: f32, duty: f32) {        
        self.eff3 += typ;
        self.eff3 = self.eff3.clamp(0.0, 3.0);

        self.change_eff(-1.0, duty, -1.0, 0.0);
    }

    pub fn change_range(&mut self, octave: f32, semi: f32) {
        self.change_eff(-1.0, octave, semi, 0.0);
    }

    pub fn change_envelope(&mut self, atk: f32, dcy: f32, sus: f32, rel: f32) {        
        self.eff3 += sus;
        self.eff3 = self.eff3.clamp(0.0, 1.0);

        self.change_eff(atk, dcy, rel, 0.01);
    }

    pub fn change_unison(&mut self, voices: f32, detune: f32) {
        self.eff3 += 2.0 * voices;
        if self.eff3 == 0.0 {
            self.disable();
        } else {
            self.enable();
        }
        self.eff3 = self.eff3.clamp(0.0, 5.0);

        self.change_eff(-1.0, -1.0, detune, 0.0);
    }

    pub fn change_fmmod(&mut self, ratio: f32, wave: f32, amount: f32) {
        self.eff3 += wave;
        self.eff3 = self.eff3.clamp(0.0, 2.0);

        if amount == 0.0 || ratio == 0.0  {
            self.disable();
        } else {
            self.enable();
        }

        self.change_eff(-1.0, ratio, amount, 0.0);
    }

    pub fn change_filter(&mut self, cutoff: f32, resonance: f32, typ: f32, envelope: f32) {
        self.eff3 += typ;
        self.eff3 = self.eff3.clamp(0.0, 3.0);

        if self.eff3 == 3.0 {
            self.disable();
        } else {
            self.enable();
        }

        self.change_eff(cutoff, resonance, envelope, 0.1);
    }
}

#[derive(Clone, Copy)]
pub struct Preset {
    pub name: &'static str,
    pub effects: [Effect; EFFECTS_NUM],
    pub idx: usize,
}

impl Preset {
    pub fn new(name: &'static str) -> Preset {
        let effects: [Effect; EFFECTS_NUM] = [
            // duty_cycle (eff2), sine (eff3)
            Effect::new(EffectState::On, EffectType::Waveform, 0.0, 0.0, 1.0, 0.0),
            // range (ex C2 - B2, C3 - B3 etc, eff2) & shift (eff4)
            Effect::new(EffectState::On, EffectType::MusicalRange, 0.0, 2.0, 0.0, 0.0),
            // attack decay sustain release
            Effect::new(EffectState::On, EffectType::Envelope, 0.05, 0.4, 0.8, 0.4),
            // unison voices (eff3) & detune (eff4)
            Effect::new(EffectState::Off, EffectType::Unison, 0.0, 0.0, 1.0, 0.0),
            //  ratio (eff2), sine waveform (eff3), amount (eff4)
            Effect::new(EffectState::Off, EffectType::FMMod, 1.0, 1.0, 0.0, 0.0),
            // cutoff (eff1), resonance (eff2), type (eff3), envelope_amount (eff4)
            Effect::new(EffectState::Off, EffectType::Filter, 0.0, 0.0, 3.0, 0.0),
        ];

        Self {
            name: name,
            effects: effects,
            idx: 0,
        }
    }

    pub fn with_effects(name: &'static str, effects: [Effect; EFFECTS_NUM]) -> Self {
        Self {
            name,
            effects,
            idx: 0,
        }
    }

    pub fn next_effect(&mut self) {
        if self.idx < EFFECTS_NUM - 1 {
            self.idx += 1;
            self.effects[self.idx].fresh();
        }
    }

    pub fn prev_effect(&mut self) {
        if self.idx > 0 {
            self.idx -= 1;
            self.effects[self.idx].fresh();
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum DisplayPage {
    PresetList,
    EffectList,
}

pub struct SynthState {
    pub presets: [Preset; PRESETS_NUM],
    pub idx: usize,
    pub page: DisplayPage,
}

impl SynthState {
    pub fn new() -> SynthState {
        Self {
            presets: [
                Preset::with_effects("Bells", [
                    // duty_cycle (eff2), sine (eff3)
                    Effect::new(EffectState::On, EffectType::Waveform, 0.0, 0.0, 1.0, 0.0),
                    // range (ex C2 - B2, C3 - B3 etc, eff2) & shift (eff4)
                    Effect::new(EffectState::On, EffectType::MusicalRange, 0.0, 2.0, 0.0, 0.0),
                    // attack decay sustain release
                    Effect::new(EffectState::On, EffectType::Envelope, 0.0, 0.0, 1.0, 1.0),
                    // unison voices (eff3) & detune (eff4)
                    Effect::new(EffectState::Off, EffectType::Unison, 0.0, 0.0, 1.0, 0.0),
                    //  ratio (eff2), sine waveform (eff3), amount (eff4)
                    Effect::new(EffectState::Off, EffectType::FMMod, 0.0, 3.5, 0.0, 4.0),
                    // cutoff (eff1), resonance (eff2), type (eff3), envelope_amount (eff4)
                    Effect::new(EffectState::Off, EffectType::Filter, 0.0, 0.0, 3.0, 0.0),
                ]),
                Preset::new("Simple"),
                Preset::with_effects("Bass", [
                    // duty_cycle (eff2), sine (eff3)
                    Effect::new(EffectState::On, EffectType::Waveform, 0.0, 0.0, 3.0, 0.0),
                    // range (ex C2 - B2, C3 - B3 etc, eff2) & shift (eff4)
                    Effect::new(EffectState::On, EffectType::MusicalRange, 0.0, 0.0, 0.0, 0.0),
                    // attack decay sustain release
                    Effect::new(EffectState::On, EffectType::Envelope, 0.05, 0.4, 0.8, 0.4),
                    // unison voices (eff3) & detune (eff4)
                    Effect::new(EffectState::Off, EffectType::Unison, 0.0, 0.0, 1.0, 0.0),
                    //  ratio (eff2), sine waveform (eff3), amount (eff4)
                    Effect::new(EffectState::Off, EffectType::FMMod, 0.0, 0.5, 0.0, 1.0),
                    // cutoff (eff1), resonance (eff2), type (eff3), envelope_amount (eff4)
                    Effect::new(EffectState::Off, EffectType::Filter, 0.0, 0.0, 3.0, 0.0),
                ]),
                Preset::with_effects("Cat", [
                    // duty_cycle (eff2), sine (eff3)
                    Effect::new(EffectState::On, EffectType::Waveform, 0.0, 0.0, 3.0, 0.0),
                    // range (ex C2 - B2, C3 - B3 etc, eff2) & shift (eff4)
                    Effect::new(EffectState::On, EffectType::MusicalRange, 0.0, 2.0, 0.0, 0.0),
                    // attack decay sustain release
                    Effect::new(EffectState::On, EffectType::Envelope, 0.05, 0.4, 0.8, 0.4),
                    // unison voices (eff3) & detune (eff4)
                    Effect::new(EffectState::Off, EffectType::Unison, 0.0, 0.0, 1.0, 0.0),
                    //  ratio (eff2), sine waveform (eff3), amount (eff4)
                    Effect::new(EffectState::Off, EffectType::FMMod, 0.0, 1.0, 0.0, 0.0),
                    // cutoff (eff1), resonance (eff2), type (eff3), envelope_amount (eff4)
                    Effect::new(EffectState::Off, EffectType::Filter, 0.2, 0.85, 0.0, 0.5),
                ]),
                Preset::with_effects("Chorus", [
                    // duty_cycle (eff2), sine (eff3)
                    Effect::new(EffectState::On, EffectType::Waveform, 0.0, 0.0, 3.0, 0.0),
                    // range (ex C2 - B2, C3 - B3 etc, eff2) & shift (eff4)
                    Effect::new(EffectState::On, EffectType::MusicalRange, 0.0, 1.0, 0.0, 0.0),
                    // attack decay sustain release
                    Effect::new(EffectState::On, EffectType::Envelope, 0.8, 0.0, 1.0, 1.0),
                    // unison voices (eff3) & detune (eff4)
                    Effect::new(EffectState::Off, EffectType::Unison, 0.0, 0.0, 3.0, 0.01),
                    //  ratio (eff2), sine waveform (eff3), amount (eff4)
                    Effect::new(EffectState::Off, EffectType::FMMod, 0.0, 1.0, 0.0, 0.0),
                    // cutoff (eff1), resonance (eff2), type (eff3), envelope_amount (eff4)
                    Effect::new(EffectState::Off, EffectType::Filter, 0.0, 0.0, 3.0, 0.0),
                ]),
            ],
            idx: 0,
            page: DisplayPage::PresetList,
        }
    }

    pub fn active_preset(&self) -> &Preset {
        &self.presets[self.idx]
    }

    pub fn active_preset_mut(&mut self) -> &mut Preset {
        &mut self.presets[self.idx]
    }

    pub fn next_preset(&mut self) {
        if self.idx < PRESETS_NUM - 1 {
            self.idx += 1;
        }
    }

    pub fn prev_preset(&mut self) {
        if self.idx > 0 {
            self.idx -= 1;
        }
    }

    pub fn next_page(&mut self) {
        let page = match self.page {
            DisplayPage::PresetList => DisplayPage::EffectList,
            DisplayPage::EffectList => DisplayPage::EffectList,
        };
        self.page = page;
    }

    pub fn prev_page(&mut self) {
        let page = match self.page {
            DisplayPage::PresetList => DisplayPage::PresetList,
            DisplayPage::EffectList => DisplayPage::PresetList,
        };
        self.page = page;
    }
}
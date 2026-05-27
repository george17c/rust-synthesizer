#[derive(Clone, Copy)]
pub struct Sequencer {
    pub steps: [[Option<usize>; 4]; 8],
    pub length: usize,
    pub is_recording: bool,
    pub is_playing: bool,
    pub rec_step: usize,
}

impl Sequencer {
    pub fn new() -> Self {
        Self {
            steps: [[None; 4]; 8],
            length: 8,
            is_recording: false,
            is_playing: false,
            rec_step: 0,
        }
    }

    pub fn clear(&mut self) {
        self.steps = [[None; 4]; 8];
        self.rec_step = 0;
        self.is_recording = false;
        self.is_playing = false;
    }
}

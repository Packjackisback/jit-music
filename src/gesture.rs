// 21 landmarks (x, y)
pub type Landmarks = [[f32; 2]; 21];

#[derive(Clone, Default)]
pub struct GestureState {
    // in hertz
    pub frequency_hz: f32,
    // normalized 0-1
    pub volume: f32,
    // self explanatory
    pub is_playing: bool,
    // human readable note
    pub note_name: String,
    pub fingers_up: usize,
}

// offset from C3 over 2 octaves here
const PENTATONIC_SEMITONES: &[i32] = &[0, 2, 4, 7, 9, 12, 14, 16, 19, 21, 24];

const NOTE_NAMES: &[&str] = &["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];

// MIDI 69 = A4 = 440 Hz. Each semitone = multiply by 2^(1/12).
fn midi_to_hz(midi: i32) -> f32 {
    440.0 * 2f32.powf((midi - 69) as f32 / 12.0)
}

fn note_name(midi: i32) -> String {
    let name = NOTE_NAMES[((midi % 12) + 12) as usize % 12];
    let octave = midi / 12 - 1;
    format!("{name}{octave}")
}


// I explain this in the planning doc
fn count_extended_fingers(lm: &Landmarks) -> usize {
    // (tip_index, pip_index) pairs for the four fingers
    let fingers = [(8, 6), (12, 10), (16, 14), (20, 18)];
    let mut count = 0;
 
    for (tip, pip) in fingers {
        if lm[tip][1] < lm[pip][1] {
            count += 1;
        }
    }
 
    // if its being weird switch this, i can't rmbr left vs right
    if lm[4][0] < lm[3][0] {
        count += 1;
    }
 
    count
}

pub fn analyse(lm: &Landmarks) -> GestureState {
    let fingers_up = count_extended_fingers(lm);
 
    // use the base of the middle finger as the anchor for stability
    let palm_y = lm[9][1];
    let palm_x = lm[9][0];
 
    let scale_len = PENTATONIC_SEMITONES.len();
    let scale_idx = ((1.0 - palm_y) * scale_len as f32) as usize;
    let scale_idx = scale_idx.min(scale_len - 1);
 
    let semitones = PENTATONIC_SEMITONES[scale_idx];
    let midi = 48 + semitones;
    let frequency_hz = midi_to_hz(midi);
 
    let volume = (palm_x * 0.7).clamp(0.1, 0.7);
 
    let is_playing = fingers_up > 0;
 
    GestureState {
        frequency_hz,
        volume: if is_playing { volume } else { 0.0 },
        is_playing,
        note_name: note_name(midi),
        fingers_up,
    }
}

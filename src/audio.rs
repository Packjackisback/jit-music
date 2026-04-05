use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::{
    atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering},
    Arc,
};

pub const LOOP_NAMES: [&str; 4] = [
    "Deep House",
    "Trance Drive",
    "Build",
    "Drop",
];

struct SharedAudioState {
    master_volume_bits: AtomicU32,
    filter_bits: AtomicU32,
    intensity_bits: AtomicU32,
    delay_wet_bits: AtomicU32,
    dial_bits: AtomicU32,
    loop_index: AtomicUsize,
    hold: AtomicBool,
    sample_trigger: AtomicU32,
    drop_trigger: AtomicU32,
}

impl SharedAudioState {
    fn new(master_volume: f32, filter_cutoff: f32, intensity: f32, delay_wet: f32, dial: f32, loop_index: usize) -> Arc<Self> {
        Arc::new(Self {
            master_volume_bits: AtomicU32::new(master_volume.to_bits()),
            filter_bits: AtomicU32::new(filter_cutoff.to_bits()),
            intensity_bits: AtomicU32::new(intensity.to_bits()),
            delay_wet_bits: AtomicU32::new(delay_wet.to_bits()),
            dial_bits: AtomicU32::new(dial.to_bits()),
            loop_index: AtomicUsize::new(loop_index),
            hold: AtomicBool::new(false),
            sample_trigger: AtomicU32::new(0),
            drop_trigger: AtomicU32::new(0),
        })
    }

    fn master_volume(&self) -> f32 {
        f32::from_bits(self.master_volume_bits.load(Ordering::Relaxed))
    }

    fn filter_cutoff(&self) -> f32 {
        f32::from_bits(self.filter_bits.load(Ordering::Relaxed))
    }

    fn intensity(&self) -> f32 {
        f32::from_bits(self.intensity_bits.load(Ordering::Relaxed))
    }

    fn delay_wet(&self) -> f32 {
        f32::from_bits(self.delay_wet_bits.load(Ordering::Relaxed))
    }

    fn dial(&self) -> f32 {
        f32::from_bits(self.dial_bits.load(Ordering::Relaxed))
    }

    fn loop_index(&self) -> usize {
        self.loop_index.load(Ordering::Relaxed)
    }

    fn hold(&self) -> bool {
        self.hold.load(Ordering::Relaxed)
    }

    fn sample_trigger(&self) -> u32 {
        self.sample_trigger.load(Ordering::Relaxed)
    }

    fn drop_trigger(&self) -> u32 {
        self.drop_trigger.load(Ordering::Relaxed)
    }

    fn set_master_volume(&self, value: f32) {
        self.master_volume_bits.store(value.to_bits(), Ordering::Relaxed);
    }

    fn set_filter_cutoff(&self, value: f32) {
        self.filter_bits.store(value.to_bits(), Ordering::Relaxed);
    }

    fn set_intensity(&self, value: f32) {
        self.intensity_bits.store(value.to_bits(), Ordering::Relaxed);
    }

    fn set_delay_wet(&self, value: f32) {
        self.delay_wet_bits.store(value.to_bits(), Ordering::Relaxed);
    }

    fn set_dial(&self, value: f32) {
        self.dial_bits.store(value.to_bits(), Ordering::Relaxed);
    }

    fn set_loop_index(&self, value: usize) {
        self.loop_index.store(value, Ordering::Relaxed);
    }

    fn set_hold(&self, value: bool) {
        self.hold.store(value, Ordering::Relaxed);
    }

    fn trigger_sample(&self) {
        self.sample_trigger.fetch_add(1, Ordering::Relaxed);
    }

    fn trigger_drop(&self) {
        self.drop_trigger.fetch_add(1, Ordering::Relaxed);
    }
}

pub struct AudioEngine {
    shared: Arc<SharedAudioState>,
    _stream: cpal::Stream,
}

fn midi_to_hz(midi: i32) -> f32 {
    440.0 * 2f32.powf((midi - 69) as f32 / 12.0)
}

fn next_noise(seed: &mut u32) -> f32 {
    *seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
    (((*seed >> 16) as f32) / 32_767.5) - 1.0
}

impl AudioEngine {
    pub fn new() -> Self {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .expect("No audio output device found");

        let config = device
            .default_output_config()
            .expect("Could not get default audio config");

        let sample_rate = u32::from(config.sample_rate()) as f32;
        let channels = config.channels() as usize;

        let shared = SharedAudioState::new(0.3, 0.55, 0.0, 0.25, 0.0, 0);
        let shared_clone = shared.clone();

        let mut sample_counter: u64 = 0;
        let mut smooth_master: f32 = 0.0;
        let mut smooth_filter: f32 = 0.55;
        let mut smooth_intensity: f32 = 0.0;
        let mut filter_state: f32 = 0.0;
        let mut bass_phase: f32 = 0.0;
        let mut lead_phase: f32 = 0.0;
        let mut kick_phase: f32 = 0.0;
        let mut kick_env: f32 = 0.0;
        let mut hat_env: f32 = 0.0;
        let mut clap_env: f32 = 0.0;
        let mut sample_env: f32 = 0.0;
        let mut drop_env: f32 = 0.0;
        let mut sample_pitch: f32 = 180.0;
        let mut drop_pitch: f32 = 900.0;
        let mut last_step: usize = 15;
        let mut last_sample_trigger: u32 = 0;
        let mut last_drop_trigger: u32 = 0;
        let mut noise_seed: u32 = 0x1234_5678;
        let mut delay_buffer = vec![0.0f32; (sample_rate * 2.0) as usize + 1];
        let mut delay_index: usize = 0;

        const BPM: f32 = 128.0;
        const STEPS: usize = 16;
        const BASS_PATTERNS: [[i32; STEPS]; 4] = [
            [0, 0, 3, 0, 5, 0, 3, 0, 0, 0, 3, 0, 7, 0, 5, 3],
            [0, 0, 7, 0, 10, 7, 5, 7, 0, 3, 7, 0, 10, 12, 10, 7],
            [0, 2, 4, 7, 9, 7, 4, 2, 0, 2, 4, 7, 12, 14, 12, 9],
            [0, 0, 0, 0, 0, 0, 0, 0, 12, 12, 14, 12, 10, 10, 7, 5],
        ];
        const LEAD_PATTERNS: [[i32; STEPS]; 4] = [
            [12, 14, 15, 14, 12, 10, 12, 14, 15, 17, 15, 14, 12, 10, 7, 10],
            [7, 10, 12, 14, 15, 14, 12, 10, 7, 10, 12, 14, 15, 17, 19, 17],
            [4, 7, 9, 12, 14, 12, 9, 7, 4, 7, 9, 12, 16, 19, 16, 12],
            [12, 12, 15, 12, 17, 15, 12, 10, 12, 14, 15, 17, 19, 17, 15, 14],
        ];

        let stream = device
            .build_output_stream(
                &config.into(),
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    let step_samples = sample_rate * 60.0 / BPM / 4.0;

                    for frame in data.chunks_mut(channels) {
                        let loop_index = shared_clone.loop_index().min(LOOP_NAMES.len() - 1);
                        let master_volume = shared_clone.master_volume();
                        let filter_target = shared_clone.filter_cutoff().clamp(0.0, 1.0);
                        let intensity_target = shared_clone.intensity().clamp(0.0, 1.0);
                        let delay_wet_target = shared_clone.delay_wet().clamp(0.0, 1.0);
                        let dial_target = shared_clone.dial().clamp(0.0, 1.0);
                        let hold = shared_clone.hold();

                        smooth_master += (master_volume - smooth_master) * 0.01;
                        smooth_filter += (filter_target - smooth_filter) * 0.01;
                        let smooth_delay_wet = delay_wet_target;
                        let smooth_dial = dial_target;
                        let intensity_goal = if hold { intensity_target.max(0.7) } else { intensity_target };
                        smooth_intensity += (intensity_goal - smooth_intensity) * 0.008;

                        let current_step = ((sample_counter as f32 / step_samples).floor() as usize) % STEPS;
                        if current_step != last_step {
                            last_step = current_step;

                            if matches!(current_step, 0 | 4 | 8 | 12) {
                                kick_env = 1.0;
                            }
                            if matches!(current_step, 4 | 12) {
                                clap_env = 1.0;
                            }
                            if current_step % 2 == 1 {
                                hat_env = 0.85;
                            }
                        }

                        let sample_trigger = shared_clone.sample_trigger();
                        if sample_trigger != last_sample_trigger {
                            last_sample_trigger = sample_trigger;
                            sample_env = 1.0;
                            sample_pitch = 180.0 + smooth_intensity * 120.0;
                        }

                        let drop_trigger = shared_clone.drop_trigger();
                        if drop_trigger != last_drop_trigger {
                            last_drop_trigger = drop_trigger;
                            drop_env = 1.0;
                            drop_pitch = 900.0;
                        }

                        let bass_offset = BASS_PATTERNS[loop_index][current_step];
                        let lead_offset = LEAD_PATTERNS[loop_index][current_step];
                        let bass_freq = midi_to_hz(36 + bass_offset);
                        let lead_freq = midi_to_hz(60 + lead_offset);

                        bass_phase = (bass_phase + bass_freq / sample_rate).fract();
                        lead_phase = (lead_phase + lead_freq / sample_rate).fract();

                        kick_phase = (kick_phase + (110.0 + 180.0 * kick_env) / sample_rate).fract();
                        kick_env *= 0.993;
                        hat_env *= 0.955;
                        clap_env *= 0.966;
                        sample_env *= 0.986;
                        drop_env *= 0.992;

                        let bass_wave = (bass_phase * std::f32::consts::TAU).sin() * 0.7
                            + ((bass_phase * 2.0).fract() * 2.0 - 1.0) * 0.3;
                        let lead_wave = ((lead_phase * std::f32::consts::TAU).sin()
                            + ((lead_phase * 1.01) * std::f32::consts::TAU).sin() * 0.7)
                            * 0.5;
                        let kick_wave = (kick_phase * std::f32::consts::TAU).sin() * kick_env;
                        let noise = next_noise(&mut noise_seed);
                        let hat = noise * hat_env * 0.18;
                        let clap = noise * clap_env * 0.16;
                        let sample_hit = ((sample_pitch / sample_rate) * std::f32::consts::TAU).sin() * sample_env * 0.18
                            + noise * sample_env * 0.08;
                        sample_pitch = (sample_pitch + 4.0).min(1_800.0);
                        let drop_hit = ((drop_pitch / sample_rate) * std::f32::consts::TAU).sin() * drop_env * 0.28
                            + noise * drop_env * 0.10;
                        drop_pitch = (drop_pitch - 5.0).max(90.0);

                        let mut mix = bass_wave * (0.48 + smooth_intensity * 0.22)
                            + lead_wave * (0.20 + smooth_intensity * 0.18)
                            + kick_wave * 0.9
                            + hat
                            + clap
                            + sample_hit
                            + drop_hit;

                        let delay_time_seconds = 0.08 + smooth_dial * 0.42;
                        let delay_samples = (delay_time_seconds * sample_rate) as usize;
                        let delay_offset = delay_samples.min(delay_buffer.len() - 1);
                        let delayed_index = (delay_index + delay_buffer.len() - delay_offset) % delay_buffer.len();
                        let delayed_sample = delay_buffer[delayed_index];
                        let delay_feedback = 0.2 + smooth_delay_wet * 0.55;
                        delay_buffer[delay_index] = mix + delayed_sample * delay_feedback;
                        delay_index = (delay_index + 1) % delay_buffer.len();
                        mix += delayed_sample * smooth_delay_wet * 0.45;

                        let cutoff_hz = 180.0 + smooth_filter.powf(1.3) * 6_500.0 + smooth_intensity * 1_800.0;
                        let filter_alpha = (cutoff_hz / sample_rate).clamp(0.01, 0.25);
                        filter_state += (mix - filter_state) * filter_alpha;
                        mix = filter_state * smooth_master * 0.9;

                        let sample = mix.clamp(-1.0, 1.0) * 0.85;
                        for value in frame.iter_mut() {
                            *value = sample;
                        }

                        sample_counter = sample_counter.wrapping_add(1);
                    }
                },
                |err| eprintln!("[audio] {err}"),
                None,
            )
            .expect("Failed to build audio stream");

        stream.play().expect("Failed to start audio stream");
        AudioEngine { shared, _stream: stream }
    }

    pub fn set_scene(
        &self,
        master_volume: f32,
        filter_cutoff: f32,
        intensity: f32,
        delay_wet: f32,
        dial: f32,
        loop_index: usize,
        hold: bool,
    ) {
        self.shared.set_master_volume(master_volume.clamp(0.0, 1.0));
        self.shared.set_filter_cutoff(filter_cutoff.clamp(0.0, 1.0));
        self.shared.set_intensity(intensity.clamp(0.0, 1.0));
        self.shared.set_delay_wet(delay_wet.clamp(0.0, 1.0));
        self.shared.set_dial(dial.clamp(0.0, 1.0));
        self.shared.set_loop_index(loop_index.min(LOOP_NAMES.len() - 1));
        self.shared.set_hold(hold);
    }

    pub fn trigger_sample(&self) {
        self.shared.trigger_sample();
    }

    pub fn trigger_drop(&self) {
        self.shared.trigger_drop();
    }
}

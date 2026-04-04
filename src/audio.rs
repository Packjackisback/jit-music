use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::{
    atomic::{AtomicU32, Ordering},
    Arc,
};

struct SharedAudioState {
    frequency_bits: AtomicU32,
    volume_bits:    AtomicU32,
}

impl SharedAudioState {
    fn new(freq: f32, vol: f32) -> Arc<Self> {
        Arc::new(Self {
            frequency_bits: AtomicU32::new(freq.to_bits()),
            volume_bits:    AtomicU32::new(vol.to_bits()),
        })
    }
    fn get_freq(&self) -> f32 { f32::from_bits(self.frequency_bits.load(Ordering::Relaxed)) }
    fn get_vol(&self)  -> f32 { f32::from_bits(self.volume_bits.load(Ordering::Relaxed)) }
    fn set_freq(&self, v: f32) { self.frequency_bits.store(v.to_bits(), Ordering::Relaxed); }
    fn set_vol(&self,  v: f32) { self.volume_bits.store(v.to_bits(), Ordering::Relaxed); }
}

pub struct AudioEngine {
    shared:  Arc<SharedAudioState>,
    _stream: cpal::Stream,
}

impl AudioEngine {
    pub fn new() -> Self {
        let host   = cpal::default_host();
        let device = host
            .default_output_device()
            .expect("No audio output device found");

        let config = device
            .default_output_config()
            .expect("Could not get default audio config");

        let sample_rate = u32::from(config.sample_rate()) as f32;
        let channels    = config.channels() as usize;

        let shared       = SharedAudioState::new(440.0, 0.0);
        let shared_clone = shared.clone();

        let mut phase:      f32 = 0.0;
        let mut smooth_vol: f32 = 0.0;

        let stream = device
            .build_output_stream(
                &config.into(),
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    let target_freq = shared_clone.get_freq();
                    let target_vol  = shared_clone.get_vol();
                    for frame in data.chunks_mut(channels) {
                        smooth_vol += (target_vol - smooth_vol) * 0.005;
                        let sample = (phase * std::f32::consts::TAU).sin()
                            * smooth_vol * 0.4;
                        phase = (phase + target_freq / sample_rate).fract();
                        for s in frame.iter_mut() { *s = sample; }
                    }
                },
                |err| eprintln!("[audio] {err}"),
                None,
            )
            .expect("Failed to build audio stream");

        stream.play().expect("Failed to start audio stream");
        AudioEngine { shared, _stream: stream }
    }

    pub fn set_note(&self, frequency_hz: f32, volume: f32) {
        self.shared.set_freq(frequency_hz);
        self.shared.set_vol(volume.clamp(0.0, 1.0));
    }

    pub fn silence(&self) {
        self.shared.set_vol(0.0);
    }
}

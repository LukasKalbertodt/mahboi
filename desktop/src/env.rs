use std::sync::{Arc, Mutex};

use cpal::{Sample, SampleFormat, SampleRate, traits::{DeviceTrait, HostTrait, StreamTrait}};
use failure::{bail, format_err, Error, ResultExt};
use pixels::{Pixels, SurfaceTexture};
use winit::{event::VirtualKeyCode, window::Window};
use winit_input_helper::WinitInputHelper;

use mahboi::{
    SCREEN_WIDTH, SCREEN_HEIGHT, FRAME_RATE, MACHINE_CYCLES_PER_SECOND,
    env::Peripherals,
    primitives::PixelColor,
    machine::input::{Keys, JoypadKey},
    log::*,
};
use crate::args::Args;


type AudioBuffer = Arc<Mutex<Vec<f32>>>;

const OPTIMAL_AUDIO_BUFFER_SIZE: u32 = 735;

/// The number of samples in the source buffer above which we consider it "full
/// enough" to start copying it into the output buffer.
const SOURCE_BUFFER_READY_ABOVE: u32 = 5;

/// The number of samples in the source buffer below which we consider the
/// buffer too short. If that's reached, we will stop copying into the host
/// buffer. This avoids audio glitches where the source buffer is not quite full
/// enough for the host buffe every second callback or so.
const SOURCE_BUFFER_TOO_SHORT_BELOW: u32 = 2;

/// The environment of the Gameboy. Implements `Peripherals`.
pub(crate) struct Env {
    pub(crate) pixels: Pixels<Window>,
    keys: Keys,

    // Sound system
    audio_buffer: AudioBuffer,
    cycles_till_next_sample: f64,
    _stream: cpal::Stream,
    sample_rate: f32,

    /// A fixed (set in `new`) value determining how many emulation cycles pass
    /// per host audio sample (without turbo mode).
    cycles_per_host_sample: f64,
}

impl Env {
    pub(crate) fn new(args: &Args, window: &Window) -> Result<Self, Error> {
        // Pixelbuffer for the Gameboy to render into
        let pixels = {
            let window_size = window.inner_size();
            let surface_texture
                = SurfaceTexture::new(window_size.width, window_size.height, window);
            Pixels::new(SCREEN_WIDTH as u32, SCREEN_HEIGHT as u32, surface_texture)?
        };

        // Audio stream for emulated audio
        let audio_buffer = Arc::new(Mutex::new(Vec::new()));
        let cycles_till_next_sample = 0.0;
        let (stream, stream_config) = create_audio_stream(audio_buffer.clone())?;
        stream.play().context("failed to play audio stream")?;

        // Calculate the ratio between Gameboy cycle frequency and host sample
        // rate.
        let cycles_per_host_second = (args.fps / FRAME_RATE) * MACHINE_CYCLES_PER_SECOND as f64;
        let cycles_per_host_sample = cycles_per_host_second / stream_config.sample_rate.0 as f64;

        Ok(Self {
            keys: Keys::none(),
            pixels,
            audio_buffer,
            _stream: stream,
            sample_rate: stream_config.sample_rate.0 as f32,
            cycles_till_next_sample,
            cycles_per_host_sample,
        })
    }

    pub(crate) fn update_keys(&mut self, input: &WinitInputHelper) {
        self.keys = Keys::none()
            .set_key(JoypadKey::Up, input.key_held(VirtualKeyCode::W))
            .set_key(JoypadKey::Left, input.key_held(VirtualKeyCode::A))
            .set_key(JoypadKey::Down, input.key_held(VirtualKeyCode::S))
            .set_key(JoypadKey::Right, input.key_held(VirtualKeyCode::D))
            .set_key(JoypadKey::A, input.key_held(VirtualKeyCode::J))
            .set_key(JoypadKey::B, input.key_held(VirtualKeyCode::K))
            .set_key(JoypadKey::Select, input.key_held(VirtualKeyCode::N))
            .set_key(JoypadKey::Start, input.key_held(VirtualKeyCode::M));
    }
}

impl Peripherals for Env {
    fn get_pressed_keys(&self) -> Keys {
        self.keys
    }

    fn write_lcd_line(&mut self, line_idx: u8, pixels: &[PixelColor; SCREEN_WIDTH]) {
        let buffer = self.pixels.get_frame();
        let offset = line_idx as usize * SCREEN_WIDTH * 4;

        // TODO: use zip
        for col in 0..SCREEN_WIDTH {
            let [r, g, b] = pixels[col].to_srgb();

            buffer[offset + 4 * col + 0] = r;
            buffer[offset + 4 * col + 1] = g;
            buffer[offset + 4 * col + 2] = b;
        }
    }

    fn offer_sound_sample(&mut self, f: impl FnOnce(f32) -> f32) {
        if self.cycles_till_next_sample <= 0.0 {
            self.audio_buffer.lock().unwrap().push(f(self.sample_rate));
            self.cycles_till_next_sample += self.cycles_per_host_sample;
        }
        self.cycles_till_next_sample -= 1.0;
    }
}

fn find_best_stream_config(device: &cpal::Device) -> Result<cpal::SupportedStreamConfig, Error> {
    let default_config = device.default_output_config()
        .context("failed to retrieve default audio stream  config")?;

    // If the default config has all properties we certainly want, we
    // immediately take it.
    if default_config.channels() == 2 && default_config.sample_format() == SampleFormat::F32 {
        return Ok(default_config);
    }

    // Otherwise, we have to search through all other configs to find one.
    let mut supported_configs = device.supported_output_configs()
        .context("could not retrieve supported configs from audio device")?
        .filter(|config| config.channels() == 2)
        .collect::<Vec<_>>();

    if supported_configs.is_empty() {
        bail!("your default audio device does not support stereo");
    }

    debug!("Supported stereo audio config ranges: {:#?}", supported_configs);

    // Sort by sample format as we prefer `f32` samples.
    supported_configs.sort_by_key(|config| config.sample_format().sample_size());
    let candidate = supported_configs.pop().unwrap();

    let default_sample_rate = default_config.sample_rate();
    let supported_sample_rates = candidate.min_sample_rate()..candidate.max_sample_rate();

    for sample_rate in &[default_sample_rate, SampleRate(44100), SampleRate(48000)] {
        if supported_sample_rates.contains(sample_rate) {
            return Ok(candidate.with_sample_rate(default_sample_rate));
        }
    }

    Err(format_err!("could not find a stereo audio stream config with an expected sample rate"))
}

fn create_audio_stream(
    audio_buffer: AudioBuffer,
) -> Result<(cpal::Stream, cpal::StreamConfig), Error> {
    let device = cpal::default_host()
        .default_output_device()
        .ok_or(failure::format_err!("failed to find a default output device"))?;

    if let Ok(name) = device.name() {
        info!("Using audio device '{}'", name);
    }

    // Create a good configuration for the audio stream.
    let supported_config = find_best_stream_config(&device)?;
    let buffer_size = match *supported_config.buffer_size() {
        cpal::SupportedBufferSize::Unknown => OPTIMAL_AUDIO_BUFFER_SIZE,
        cpal::SupportedBufferSize::Range { min, max } => {
            if min > OPTIMAL_AUDIO_BUFFER_SIZE {
                warn!(
                    "Minimum buffer size {} of audio device is quite large. The audio might \
                        be delayed.",
                    min,
                );

                min
            } else {
                std::cmp::min(OPTIMAL_AUDIO_BUFFER_SIZE, max)
            }
        }
    };

    let config = cpal::StreamConfig {
        channels: 2, // We made sure we have a stereo config in `find_best_stream_config`
        sample_rate: supported_config.sample_rate(),
        buffer_size: cpal::BufferSize::Fixed(buffer_size),
    };
    debug!("Using audio stream configuration {:?}", config);

    let stream = match supported_config.sample_format() {
        SampleFormat::I16 => create_stream::<i16>(&device, &config, audio_buffer, buffer_size),
        SampleFormat::U16 => create_stream::<u16>(&device, &config, audio_buffer, buffer_size),
        SampleFormat::F32 => create_stream::<f32>(&device, &config, audio_buffer, buffer_size),
    };

    Ok((stream?, config))
}

fn create_stream<T: Sample>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    audio_buffer: AudioBuffer,
    buffer_size: u32,
) -> Result<cpal::Stream, Error> {
    // Calculate buffer size thresholds to avoid stuttering and other
    // unwanted audio glitches.
    let sufficient_data_above = buffer_size * SOURCE_BUFFER_READY_ABOVE;
    let missing_data_below = buffer_size * SOURCE_BUFFER_TOO_SHORT_BELOW;

    let mut sufficient_source_data = false;
    device.build_output_stream(
        &config,
        move |out: &mut [T], _: &cpal::OutputCallbackInfo| {
            let mut buffer = audio_buffer.lock().unwrap();
            // println!("src {} <-> dst {}", buffer.len(), out.len() / 2);
            if buffer.len() > sufficient_data_above as usize {
                sufficient_source_data = true;
            } else if buffer.len() < missing_data_below as usize {
                sufficient_source_data = false;
            }

            if !sufficient_source_data {
                trace!("No emulation audio data available for host audio buffer");
                for out in out {
                    *out = T::from(&0.0f32);
                }
            } else {
                // Reminder: we make sure to have a stereo config, so we always
                // have two channels.
                let num_samples = out.len() / 2;
                for (dst, src) in out.chunks_mut(2).zip(buffer.drain(..num_samples)) {
                    for channel in dst {
                        // TODO: random 0.2 here to make the volume slightly
                        // more ok. With the original value, this destroys my
                        // ears.
                        *channel = T::from(&(src * 0.2));
                    }
                }
            }
        },
        |e| error!("audio error: {}", e),
    ).map_err(Into::into)
}

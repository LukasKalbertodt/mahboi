use std::{
    thread,
    sync::mpsc::{channel, Receiver, Sender},
};

use cpal::{
    default_output_device,
    EventLoop,
    StreamData,
    UnknownTypeOutputBuffer,
};
use failure::{Error, ResultExt};
use minifb::{Key, WindowOptions, Window};
use sample::slice::{to_frame_slice_mut, equilibrium};

use mahboi::{
    SCREEN_WIDTH, SCREEN_HEIGHT,
    log::*,
    env::{self, Peripherals, Display, Input},
    primitives::{PixelColor, PixelPos},
    machine::{
        input::{Keys, JoypadKey},
        sound::{Tone, SoundChannel},
    },
};
use crate::{
    args::Args,
};



const WINDOW_TITLE: &str = "Mahboi";

/// Native application window which also handles input and sound.
pub(crate) struct NativeWindow {
    win: Window,
    buf: WinBuffer,
    sound: Sound,
}

impl NativeWindow {
    /// Opens a window configured by `args`.
    pub(crate) fn open(args: &Args) -> Result<Self, Error> {
        let options = WindowOptions {
            borderless: false,
            title: true,
            resize: false,
            scale: args.scale,
        };

        let win = Window::new(WINDOW_TITLE, SCREEN_WIDTH, SCREEN_HEIGHT, options)?;
        let buf = WinBuffer {
            data: vec![0xa0a0; SCREEN_WIDTH * SCREEN_HEIGHT],
            buffer_up_to_date: false,
        };
        let sound = Sound::new();
        info!("[desktop] Opened window");

        Ok(Self {
            win,
            buf,
            sound,
        })
    }

    /// Returns `true` if the window received signals to stop.
    pub(crate) fn should_stop(&self) -> bool {
        !self.win.is_open() || self.win.is_key_down(Key::Escape)
    }

    /// Updates the window with the internal buffer and handles new events.
    pub(crate) fn update(&mut self) -> Result<(), Error> {
        if !self.buf.buffer_up_to_date {
            self.win.update_with_buffer(&self.buf.data)
                .context("failed to update window buffer")?;
            self.buf.buffer_up_to_date = true;
        } else {
            self.win.update();
        }

        Ok(())
    }

    pub(crate) fn set_title_postfix(&mut self, postfix: &str) {
        let new_title = format!("{} - {}", WINDOW_TITLE, postfix);
        self.win.set_title(&new_title);
    }

    pub(crate) fn in_turbo_mode(&self) -> bool {
        self.win.is_key_down(Key::Q)
    }
}

impl Input for NativeWindow {
    fn get_pressed_keys(&self) -> Keys {
        Keys::none()
            .set_key(JoypadKey::Up, self.win.is_key_down(Key::W))
            .set_key(JoypadKey::Left, self.win.is_key_down(Key::A))
            .set_key(JoypadKey::Down, self.win.is_key_down(Key::S))
            .set_key(JoypadKey::Right, self.win.is_key_down(Key::D))
            .set_key(JoypadKey::A, self.win.is_key_down(Key::J))
            .set_key(JoypadKey::B, self.win.is_key_down(Key::K))
            .set_key(JoypadKey::Select, self.win.is_key_down(Key::N))
            .set_key(JoypadKey::Start, self.win.is_key_down(Key::M))
    }
}

pub(crate) struct WinBuffer {
    data: Vec<u32>,
    buffer_up_to_date: bool,
}

impl Peripherals for NativeWindow {
    type Display = WinBuffer;
    type Sound = Sound;
    type Input = Self;

    fn display(&mut self) -> &mut Self::Display {
        &mut self.buf
    }

    fn sound(&mut self) -> &mut Self::Sound {
        &mut self.sound
    }

    fn input(&mut self) -> &mut Self::Input {
        self
    }
}

impl Display for WinBuffer {
    fn set_pixel(&mut self, pos: PixelPos, color: PixelColor) {
        let idx = pos.x() as usize + pos.y() as usize * 160;
        let [r, g, b] = color.to_srgb();
        let combined = ((r as u32) << 16) | ((g as u32) << 8) | (b as u32);
        self.data[idx] = combined;
        self.buffer_up_to_date = false;
    }
}


// Dummy implementations

pub(crate) struct Sound {
    sender: Sender<Tone>,
}

impl Sound {
    fn new() -> Self {
        let (sender, receiver) = channel();
        thread::spawn(|| Self::run(receiver));

        Self {
            sender,
        }
    }

    fn run(mut receiver: Receiver<Tone>) {
        let mut synth = {
            use synth::{Point, Oscillator, oscillator, Envelope, Synth};

            // The following envelopes should create a downward pitching sine wave that gradually quietens.
            // Try messing around with the points and adding some of your own!
            let amp_env = Envelope::from(vec!(
                //         Time ,  Amp ,  Curve
                Point::new(0.0  ,  0.0 ,  0.0),
                Point::new(0.01 ,  1.0 ,  0.0),
                Point::new(0.45 ,  1.0 ,  0.0),
                Point::new(0.81 ,  0.8 ,  0.0),
                Point::new(1.0  ,  0.0 ,  0.0),
            ));
            let freq_env = Envelope::from(vec!(
                //         Time    , Freq   , Curve
                Point::new(0.0     , 0.0    , 0.0),
                Point::new(0.00136 , 1.0    , 0.0),
                Point::new(0.015   , 0.02   , 0.0),
                Point::new(0.045   , 0.005  , 0.0),
                Point::new(0.1     , 0.0022 , 0.0),
                Point::new(0.35    , 0.0011 , 0.0),
                Point::new(1.0     , 0.0    , 0.0),
            ));

            // Now we can create our oscillator from our envelopes.
            // There are also Sine, Noise, NoiseWalk, SawExp and Square waveforms.
            let oscillator = Oscillator::new(oscillator::waveform::Square, amp_env, freq_env, ());

            // Here we construct our Synth from our oscillator.
            Synth::retrigger(())
                .oscillator(oscillator) // Add as many different oscillators as desired.
                .duration(6000.0) // Milliseconds.
                .base_pitch(2100.0) // Hz.
                .loop_points(0.49, 0.51) // Loop start and end points.
                .fade(500.0, 500.0) // Attack and Release in milliseconds.
                .num_voices(16) // By default Synth is monophonic but this gives it `n` voice polyphony.
                .volume(0.2)
                .detune(0.5)
                .spread(1.0)

            // Other methods include:
            // .loop_start(0.0)
            // .loop_end(1.0)
            // .attack(ms)
            // .release(ms)
            // .note_freq_generator(nfg)
            // .oscillators([oscA, oscB, oscC])
            // .volume(1.0)
        };

        let device = default_output_device().expect("Failed to get default output device");
        let format = device.default_output_format().expect("Failed to get default output format");
        let event_loop = EventLoop::new();
        let stream_id = event_loop.build_output_stream(&device, &format).unwrap();
        event_loop.play_stream(stream_id);

        let mut last_tone: Option<Tone> = None;

        let receiver = &mut receiver;
        let tone = &mut last_tone;
        event_loop.run(move |_, data| {

            if let Ok(new_tone) = receiver.try_recv() {
                *tone = Some(new_tone);
            }

            if let Some(tone) = *tone {
                synth.base_pitch = tone.frequency as f32;

                match data {
                    StreamData::Output { buffer: UnknownTypeOutputBuffer::F32(mut buffer) } => {

                        let buffer = to_frame_slice_mut(buffer).unwrap();
                        equilibrium(buffer);

                        synth.fill_slice(buffer, 44100.0);

//                        for dest_sample in buffer.chunks_mut(format.channels as usize) {
//                            let value = 0f32;
//                            for out in dest_sample.iter_mut() {
//                                *out = value;
//                            }
//                        }
                    },
                    _ => (),
                }
            }
        })
    }
}

impl env::Sound for Sound {
    fn play_on(&mut self, (left, right): (Tone, Tone), channel: SoundChannel) {
        match channel {
            SoundChannel::Tone1 => {
                self.sender.send(left).expect("sound thread died");
            }
            _ => {},
        }
    }
}

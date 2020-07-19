use failure::{Error, ResultExt};
use minifb::{Key, ScaleMode, WindowOptions, Window};

use mahboi::{
    SCREEN_WIDTH, SCREEN_HEIGHT,
    log::*,
    env::{self, Peripherals, Display, Input},
    primitives::PixelColor,
    machine::input::{Keys, JoypadKey},
};
use crate::{
    args::Args,
};



const WINDOW_TITLE: &str = "Mahboi";

/// Native application window which also handles input and sound.
pub(crate) struct NativeWindow {
    win: Window,
    buf: WinBuffer,
}

impl NativeWindow {
    /// Opens a window configured by `args`.
    pub(crate) fn open(args: &Args) -> Result<Self, Error> {
        let options = WindowOptions {
            resize: true,
            scale: args.scale,
            scale_mode: ScaleMode::AspectRatioStretch,
            ..WindowOptions::default()
        };

        let win = Window::new(WINDOW_TITLE, SCREEN_WIDTH, SCREEN_HEIGHT, options)?;
        let buf = WinBuffer {
            data: vec![0xa0a0; SCREEN_WIDTH * SCREEN_HEIGHT],
            buffer_up_to_date: false,
        };
        info!("[desktop] Opened window");

        Ok(Self {
            win,
            buf,
        })
    }

    /// Returns `true` if the window received signals to stop.
    pub(crate) fn should_stop(&self) -> bool {
        !self.win.is_open() || self.win.is_key_down(Key::Escape)
    }

    /// Updates the window with the internal buffer and handles new events.
    pub(crate) fn update(&mut self) -> Result<(), Error> {
        if !self.buf.buffer_up_to_date {
            self.win.update_with_buffer(&self.buf.data, SCREEN_WIDTH, SCREEN_HEIGHT)
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

    pub(crate) fn reset_to_pink(&mut self) {
        self.buf.data = vec![0xFF69B4; SCREEN_WIDTH * SCREEN_HEIGHT];
        self.buf.buffer_up_to_date = false;
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
        unimplemented!()
    }

    fn input(&mut self) -> &mut Self::Input {
        self
    }
}

impl Display for WinBuffer {
    fn set_line(&mut self, line_idx: u8, pixels: &[PixelColor; SCREEN_WIDTH]) {
        let offset = line_idx as usize * SCREEN_WIDTH;
        for col in 0..SCREEN_WIDTH {
            let [r, g, b] = pixels[col].to_srgb();
            let combined = ((r as u32) << 16) | ((g as u32) << 8) | (b as u32);
            self.data[offset + col] = combined;
        }
        self.buffer_up_to_date = false;
    }
}


// Dummy implementations

pub(crate) struct Sound {

}

impl env::Sound for Sound {

}

use failure::{Error, ResultExt};
use minifb::{Key, WindowOptions, Window};

use mahboi::{
    SCREEN_WIDTH, SCREEN_HEIGHT,
    log::*,
    env::{self, Peripherals, Display},
    primitives::{PixelColor, PixelPos},
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
            borderless: false,
            title: true,
            resize: false,
            scale: args.scale,
        };

        let win = Window::new(WINDOW_TITLE, SCREEN_WIDTH, SCREEN_HEIGHT, options)?;
        let buf = WinBuffer(vec![0xa0a0; SCREEN_WIDTH * SCREEN_HEIGHT]);
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
        self.win.update_with_buffer(&self.buf.0)
            .context("failed to update window buffer")?;

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

pub(crate) struct WinBuffer(Vec<u32>);

impl Peripherals for NativeWindow {
    type Display = WinBuffer;
    type Sound = Sound;
    type Input = Input;

    fn display(&mut self) -> &mut Self::Display {
        &mut self.buf
    }

    fn sound(&mut self) -> &mut Self::Sound {
        unimplemented!()
    }

    fn input(&mut self) -> &mut Self::Input {
        unimplemented!()
    }
}

impl Display for WinBuffer {
    fn set_pixel(&mut self, pos: PixelPos, color: PixelColor) {
        let idx = pos.x() as usize + pos.y() as usize * 160;
        let [r, g, b] = color.to_srgb();
        let combined = ((r as u32) << 16) | ((g as u32) << 8) | (b as u32);
        self.0[idx] = combined;
    }
}


// Dummy implementations

pub(crate) struct Input {

}

impl env::Input for Input {

}

pub(crate) struct Sound {

}

impl env::Sound for Sound {

}

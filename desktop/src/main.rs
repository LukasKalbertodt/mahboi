#![feature(rust_2018_preview)]

use std::fs;

use failure::{Error, ResultExt};
use minifb::{Key, WindowOptions, Window};
use structopt::StructOpt;

use mahboi::{
    SCREEN_WIDTH,
    SCREEN_HEIGHT,
    Emulator,
    cartridge::Cartridge,
    env::{EventLevel, Debugger},
};
use crate::{
    debug::{Action, TuiDebugger},
    env::Peripherals,
    args::Args,
};


mod args;
mod debug;
mod env;


fn main() {
    if let Err(e) = run() {
        println!("ERROR: {}", e);

        for cause in e.iter_causes() {
            println!("  ... caused by: {}", cause);
        }
    }
}

fn run() -> Result<(), Error> {
    // Parse CLI arguments
    let args = Args::from_args();

    // Create debugger
    let debugger = TuiDebugger::new()?;

    // Load ROM
    let rom = fs::read(&args.path_to_rom)?;

    // Create emulator
    debugger.post_event(EventLevel::Info, "hallooo".into());
    let cartridge = Cartridge::from_bytes(&rom);
    debugger.post_event(EventLevel::Debug, format!("Loaded: {:#?}", cartridge));
    let mut peripherals = Peripherals {};

    let mut emulator = Emulator::new(cartridge, &mut peripherals, &debugger);
    debugger.post_event(EventLevel::Debug, "tarumtumtum".into());

    let mut window = open_window(&args).context("failed to open window")?;

    let mut buffer: Vec<u32> = vec![0; SCREEN_WIDTH * SCREEN_HEIGHT];
    let mut color = 0;
    while window.is_open() && !window.is_key_down(Key::Escape) {
        for i in buffer.iter_mut() {
            *i = color;
        }
        color += 1;

        window.update_with_buffer(&buffer).unwrap();

        // Run the emulator.
        emulator.execute_frame();
        let action = debugger.update()?;
        match action {
            Action::Quit => break,
            Action::Pause => {} // TODO
            Action::Nothing => {}
        }
    }

    Ok(())
}

/// Opens a `minifb` window configured by `args`.
fn open_window(args: &Args) -> Result<Window, Error> {
    const TITLE: &str = "Mahboi";

    let options = WindowOptions {
        borderless: false,
        title: true,
        resize: false,
        scale: args.scale,
    };

    Window::new(TITLE, SCREEN_WIDTH, SCREEN_HEIGHT, options)
        .map_err(|e| e.into())
}

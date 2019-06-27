use std::{
    fs,
};

use failure::{Error, ResultExt};
use structopt::StructOpt;

use mahboi::{
    Emulator, Disruption,
    cartridge::Cartridge,
    log::*,
};
use crate::{
    env::NativeWindow,
    args::Args,
};


mod args;
// mod debug;
mod env;


const TARGET_FPS: f64 = 59.73;

fn main() {
    // We just catch potential errors here and pretty print them.
    if let Err(e) = run() {
        println!("ERROR: {}", e);

        for cause in e.iter_causes() {
            println!("  ... caused by: {}", cause);
        }

        std::process::exit(1);
    }
}

/// The actual main function.
fn run() -> Result<(), Error> {
    // Parse CLI arguments
    let args = Args::from_args();

    // Prepare everything
    // let mut tui_debugger = init_debugger(&args)?;
    let mut emulator = init_emulator(&args)?;
    let mut window = NativeWindow::open(&args).context("failed to open window")?;


    // ===== MAIN LOOP ========================================================
    while !window.should_stop() {
        // Update to react to events.
        window.update()?;

        // Run the emulator.
        let res = emulator.execute_frame(&mut window, |_| false);

        // React to abnormal disruptions
        match res {
            Err(Disruption::Terminated) => break,
            _ => {}
        }

        // Draw to the actual window.
        window.draw()?;
    }

    Ok(())
}

/// Loads the ROM and initializes the emulator.
fn init_emulator(args: &Args) -> Result<Emulator, Error> {
    // Load ROM
    let rom = fs::read(&args.path_to_rom).context("failed to load ROM file")?;
    let cartridge = Cartridge::from_bytes(&rom);
    info!("[desktop] Loaded: {:#?}", cartridge);

    // Create emulator
    Ok(Emulator::new(cartridge, args.bios))
}

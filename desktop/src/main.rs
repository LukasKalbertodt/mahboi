#![feature(rust_2018_preview)]


mod args;


use failure::{Error, ResultExt};
use minifb::{Key, WindowOptions, Window};
use mahboi::{SCREEN_WIDTH, SCREEN_HEIGHT};
use structopt::StructOpt;

use crate::args::Args;


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

    let mut window = open_window(&args).context("failed to open window")?;

    let mut buffer: Vec<u32> = vec![0; SCREEN_WIDTH * SCREEN_HEIGHT];
    let mut color = 0;
    while window.is_open() && !window.is_key_down(Key::Escape) {
        for i in buffer.iter_mut() {
            *i = color;
        }
        color += 1;

        window.update_with_buffer(&buffer).unwrap();
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

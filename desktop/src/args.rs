use std::path::PathBuf;

use log::LevelFilter;
use structopt::StructOpt;

use mahboi::{
    BiosKind,
    primitives::Word,
};


/// Gameboy Emulator.
///
/// The keys WASD are mapped to the up, left, down and right button
/// respectively. 'J' is mapped to the gameboy's A button, 'K' to the B button,
/// 'N' to the Select button and 'M' to the Start button. The button 'Q' can be
/// used to speed up the emulation.
#[derive(Debug, StructOpt)]
#[structopt(author)]
pub(crate) struct Args {
    /// Set the scale factor for the window. The native Gameboy resolution
    /// 144x160 multiplied with the scale factor is the size of the window in
    /// physical pixels. Between 1 and 16.
    #[structopt(
        long,
        default_value = "4",
        validator(check_scale),
    )]
    pub(crate) scale: u8,

    /// Start in debugging mode (a TUI debugger). Not usable on Windows!
    #[structopt(long)]
    pub(crate) debug: bool,

    /// Path to the ROM that should be loaded into the emulator.
    #[structopt(parse(from_os_str))]
    pub(crate) path_to_rom: PathBuf,

    /// Breakpoint that is added to the debugger at the very beginning.
    /// Breakpoints are specified in hexadecimal. To add multiple breakpoints,
    /// you can either list them after one `--breakpoints` flag or specify
    /// `--breakpoints` multiple times. Example: `--breakpoints 23 FF
    /// --breakpoints 10B`.
    #[structopt(
        long,
        parse(try_from_str = parse_breakpoint),
        requires = "debug",
    )]
    #[cfg_attr(windows, allow(dead_code))]
    pub(crate) breakpoints: Vec<Word>,

    /// When starting in debugging mode, don't pause at the beginning, but
    /// start running right ahead (particularly useful in combination with
    /// `--breakpoints`)
    #[structopt(long, requires = "debug")]
    pub(crate) instant_start: bool,

    /// Defines how much faster turbo mode (key Q) is than 100%. So, a value of
    /// `2` means double the speed, while `4` would mean 400% speed (= roughly
    /// 240FPS).
    #[structopt(long, default_value = "4")]
    pub(crate) turbo_mode_factor: f64,

    /// Defines the target framerate for the emulation. The original Gameboy
    /// runs at approximately 59.7275 FPS.
    // TODO: maybe change default to "smart": if the display FPS is 60, then 60,
    // otherwise the original 59.7275?
    #[structopt(long, default_value = "60")]
    pub(crate) fps: f64,

    /// Specifies which log messages to display and which to supress. The
    /// specified value will show all log messages with the same level or any
    /// higher level. So `-l warn` will print errors and warnings and `-l
    /// trace` will show all levels. You can also disable all log messages with
    /// `-l off`. Valid values: 'off', 'error', 'warn', 'info', 'debug' and
    /// 'trace'. Note that `trace` messages are statically disabled in release
    /// builds and cannot be reenabled by this flag. [default: 'trace' in
    /// `--debug` mode, 'error' otherwise]
    #[structopt(short, long, parse(try_from_str = parse_log_level))]
    pub(crate) log_level: Option<LevelFilter>,

    /// Specifies which BIOS (boot ROM) to load. The original BIOS scrolls in
    /// the Nintendo logo and plays a sound. The minimal one skips all that and
    /// you immediately see your game.
    #[structopt(
        long,
        short,
        default_value = "minimal",
        parse(try_from_str = parse_bios_kind),
    )]
    pub(crate) bios: BiosKind,
}

fn parse_breakpoint(src: &str) -> Result<Word, String> {
    u16::from_str_radix(src, 16)
        .map(Word::new)
        .map_err(|e| format!(
            "failed to parse breakpoint: {} (values like '1f' are valid -- no \
                leading `0x`!)",
            e,
        ))
}

fn parse_log_level(src: &str) -> Result<LevelFilter, &'static str> {
    match src {
        "off" => Ok(LevelFilter::Off),
        "error" => Ok(LevelFilter::Error),
        "warn" => Ok(LevelFilter::Warn),
        "info" => Ok(LevelFilter::Info),
        "debug" => Ok(LevelFilter::Debug),
        "trace" => Ok(LevelFilter::Trace),
        _ => Err(
            "invalid log level (valid values: 'off', 'error', 'warn', 'info', 'debug' \
                and 'trace'"
        ),
    }
}

fn parse_bios_kind(src: &str) -> Result<BiosKind, &'static str> {
    match src {
        "original" => Ok(BiosKind::Original),
        "minimal" => Ok(BiosKind::Minimal),
        _ => Err("invalid bios kind (valid values: 'original' and 'minimal')"),
    }
}

fn check_scale(src: String) -> Result<(), String> {
    match src.parse::<u8>() {
        Err(e) => Err(format!("failed to parse '{}' as `u8`: {}", src, e)),
        Ok(v) if v >= 1 && v <= 16 => Ok(()),
        Ok(v) => Err(format!("has to be >= 0 and <= 16, but {} is not", v)),
    }
}

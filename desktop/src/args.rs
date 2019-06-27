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
pub(crate) struct Args {
    /// Set the scale factor for the window: 1, 2, 4, 8, 16, 32 or 'fit'
    /// (automatically chooses the largest scale factor that still fits on the
    /// screen).
    #[structopt(
        long = "--scale",
        default_value = "4",
    )]
    // TODO: add validator to have this positive!
    pub(crate) scale: f32,

    /// Start in debugging mode (a TUI debugger). Not usable on Windows!
    #[structopt(
        long = "--debug",
    )]
    pub(crate) debug: bool,

    /// Path to the ROM that should be loaded into the emulator.
    #[structopt(
        parse(from_os_str),
    )]
    pub(crate) path_to_rom: PathBuf,

    /// Breakpoint that is added to the debugger at the very beginning.
    /// Breakpoints are specified in hexadecimal. To add multiple breakpoints,
    /// you can either list them after one `--breakpoints` flag or specify
    /// `--breakpoints` multiple times. Example: `--breakpoints 23 FF
    /// --breakpoints 10B`.
    #[structopt(
        long = "--breakpoints",
        parse(try_from_str = "parse_breakpoint"),
        requires = "debug",
    )]
    pub(crate) breakpoints: Vec<Word>,

    /// When starting in debugging mode, don't pause at the beginning, but
    /// start running right ahead (particularly useful in combination with
    /// `--breakpoints`)
    #[structopt(
        long = "--instant-start",
        requires = "debug",
    )]
    pub(crate) instant_start: bool,

    /// Defines how much faster turbo mode (key Q) is than 100%. So, a value of
    /// `2` means double the speed, while `4` would mean 400% speed (= roughly
    /// 240FPS).
    #[structopt(
        long = "--turbo-mode-factor",
        default_value = "4",
    )]
    pub(crate) turbo_mode_factor: f64,

    /// Specifies which log messages to display and which to supress. The
    /// specified value will show all log messages with the same level or any
    /// higher level. So `-l warn` will print errors and warnings and `-l
    /// trace` will show all levels. You can also disable all log messages with
    /// `-l off`. Valid values: 'off', 'error', 'warn', 'info', 'debug' and
    /// 'trace'. Note that `trace` messages are statically disabled in release
    /// builds and cannot be reenabled by this flag. [default: 'trace' in
    /// `--debug` mode, 'error' otherwise]
    #[structopt(
        long = "--log-level",
        short = "-l",
        parse(try_from_str = "parse_log_level"),
    )]
    pub(crate) log_level: Option<LevelFilter>,

    /// Specifies which BIOS (boot ROM) to load. The original BIOS scrolls in
    /// the Nintendo logo and plays a sound. The minimal one skips all that and
    /// you immediately see your game.
    #[structopt(
        long = "--bios",
        short = "-b",
        default_value = "minimal",
        parse(try_from_str = "parse_bios_kind"),
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

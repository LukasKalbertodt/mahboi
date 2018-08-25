use std::path::PathBuf;

use minifb::Scale;
use structopt::StructOpt;

use mahboi::primitives::Word;


#[derive(Debug, StructOpt)]
pub(crate) struct Args {
    #[structopt(
        long = "--scale",
        default_value = "4",
        parse(try_from_str = "parse_scale"),
        help = "Set the scale factor for the window: 1, 2, 4, 8, 16, 32 or 'fit' (automatically \
                chooses the largest scale factor that still fits on the screen)."
    )]
    pub(crate) scale: Scale,

    #[structopt(
        long = "--debug",
        help = "Start in debugging mode (a TUI debugger)",
    )]
    pub(crate) debug: bool,

    #[structopt(
        parse(from_os_str),
        help = "Path to the ROM that should be loaded into the emulator.",
    )]
    pub(crate) path_to_rom: PathBuf,

    #[structopt(
        long = "--breakpoints",
        parse(try_from_str = "parse_breakpoint"),
        requires = "debug",
        help = "Breakpoint that is added to the debugger at the very beginning. Breakpoints are \
            specified in hexadecimal. To add multiple breakpoints, you can either list them after \
            one `--breakpoints` flag or specify `--breakpoints` multiple times. Example: \
            `--breakpoints 23 FF --breakpoints 10B`.",
    )]
    pub(crate) breakpoints: Vec<Word>,

    #[structopt(
        long = "--instant-start",
        requires = "debug",
        help = "When starting in debugging mode, don't pause at the beginning, but start running \
            right ahead (particularly useful in combination with `--breakpoints`)",
    )]
    pub(crate) instant_start: bool,
}

fn parse_scale(src: &str) -> Result<Scale, &'static str> {
    match src {
        "1" => Ok(Scale::X1),
        "2" => Ok(Scale::X2),
        "4" => Ok(Scale::X4),
        "8" => Ok(Scale::X8),
        "16" => Ok(Scale::X16),
        "32" => Ok(Scale::X32),
        "fit" => Ok(Scale::FitScreen),
        _ => Err("only '1', '2', '4', '8', '16', '32' or 'fit' are allowed"),
    }
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

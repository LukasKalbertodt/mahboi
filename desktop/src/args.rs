use minifb::Scale;
use structopt::StructOpt;
use std::path::PathBuf;


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

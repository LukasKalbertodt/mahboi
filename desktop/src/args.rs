use minifb::Scale;
use structopt::StructOpt;



#[derive(Debug, StructOpt)]
crate struct Args {
    #[structopt(
        long = "--scale",
        default_value = "4",
        parse(try_from_str = "parse_scale"),
        help = "Set the scale factor for the window: 1, 2, 4, 8, 16, 32 or 'fit' (automatically \
                chooses the largest scale factor that still fits on the screen)."
    )]
    crate scale: Scale,
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

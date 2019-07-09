use std::{
    path::PathBuf,
    str::FromStr,
    time::Duration,
};

use log::LevelFilter;
use structopt::StructOpt;
use vulkano::swapchain::PresentMode;

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
    /// Set the scale factor for the window. You can still resize the window
    /// when the application is running. Value has to be greater than 0.
    #[structopt(
        long = "--scale",
        default_value = "4",
        raw(validator = "check_scale"),
    )]
    pub(crate) scale: f64,

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
    /// builds and cannot be reenabled by this flag.
    #[structopt(
        long = "--log-level",
        short = "-l",
        default_value = "error",
        parse(try_from_str = "parse_log_level"),
    )]
    pub(crate) log_level: LevelFilter,

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

    /// In order to reduce input lag, the OpenGL drawing is done as close to
    /// the next host V-Blank as possible. But since the time for drawing can
    /// change a bit from frame to frame, there should be some time buffer
    /// after drawin to avoid missing a V-Blank. This argument specifies that
    /// buffer in milliseconds. In other words: this is the time we want OpenGL
    /// to block when swapping buffers. If you do not reliably get 60 FPS,
    /// increase this value. If this value is the frame time (e.g. 16.6ms for
    /// 60FPS), we won't wait before drawing.
    #[structopt(
        long = "--host-block-margin",
        default_value = "1.5",
        parse(try_from_str = "parse_block_margin"),
    )]
    pub(crate) host_block_margin: Duration,

    /// How quickly the sleeping delay for the rendering thread adjusts to the
    /// measured optimum. A value close to 0 means slower adjustments and a
    /// sleep time more stable against outliers. A value close to 1 means
    /// faster reaction to changes in measure performance, but is more
    /// vulnerable to outliers. You most certainly can keep it at the default
    /// value.
    #[structopt(
        long = "--host-delay-learn-rate",
        default_value = "0.1",
        raw(validator = "check_learn_rate"),
    )]
    pub(crate) host_delay_learn_rate: f32,

    /// In order to reduce input lag, an emulation step might get executed a
    /// bit earlier or a bit later than the regular rate would dictate. This
    /// parameter controls how much the time of emulation may differ from the
    /// strict tick rate.
    #[structopt(
        long = "--max-emu-deviation",
        default_value = "4",
        parse(try_from_str = "parse_max_emu_deviation"),
    )]
    pub(crate) max_emu_deviation: Duration,

    /// How quickly the emulation delay adjusts to the measured optimum. A
    /// value close to 0 means slower adjustments and a sleep time more stable
    /// against outliers. A value close to 1 means faster reaction to changes
    /// in measure performance, but is more vulnerable to outliers. You most
    /// certainly can keep it at the default value.
    #[structopt(
        long = "--emu-sleep-learn-rate",
        default_value = "0.1",
        raw(validator = "check_learn_rate"),
    )]
    pub(crate) emu_delay_learn_rate: f32,

    /// If specified, all available, suitable physical Vulkan devices are
    /// listed on startup. This is useful to select a device via `--device`
    /// afterwards.
    #[structopt(long = "--list-devices")]
    pub(crate) list_devices: bool,

    /// Specifies which physical Vulkan device to use. Can be either an index
    /// or a hex-encoded UUID. Note that in theory, the index of devices can
    /// change while the UUID stays fixed. In practice, the index is fine.
    #[structopt(long = "--device")]
    pub(crate) device: Option<VulkanDevice>,

    /// Vulkan present mode: 'immediate', 'mailbox', 'fifo' or 'relaxed'. Check
    /// the Vulkan documentation for the meaning of those modes. Only FIFO is
    /// always supported. If this is not specified, Mahboi uses mailbox, if
    /// available, and falls back to FIFO.
    #[structopt(
        long = "--present-mode",
        parse(try_from_str = "parse_mode"),
    )]
    pub(crate) present_mode: Option<PresentMode>,
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
    match src.parse::<f64>() {
        Err(e) => Err(format!("failed to parse '{}' as `f64`: {}", src, e)),
        Ok(v) if v > 0.0 => Ok(()),
        Ok(v) => Err(format!("has to be greater than 0, but {} is not", v)),
    }
}

fn parse_block_margin(src: &str) -> Result<Duration, String> {
    match src.parse::<f64>() {
        Err(e) => Err(format!("invalid float: {}", e)),
        Ok(v) if v > 100.0 => {
            Err("a block margin larger than the frame time does not make sense".into())
        },
        Ok(v) if v < 0.0 => Err("block margin cannot be negative".into()),
        Ok(v) if v.is_nan() => Err("block margin cannot be NaN".into()),
        Ok(v) => Ok(Duration::from_nanos((v * 1_000_000.0) as u64)),
    }
}

fn check_learn_rate(src: String) -> Result<(), String> {
    match src.parse::<f32>() {
        Ok(v) if v >= 0.0 && v <= 1.0 => Ok(()),
        Ok(v) => Err(format!("has to be between 0 and 1, but {} is not", v)),
        Err(e) => Err(format!("failed to parse '{}' as `f32`: {}", src, e)),
    }
}

fn parse_max_emu_deviation(src: &str) -> Result<Duration, String> {
    match src.parse::<f64>() {
        Err(e) => Err(format!("invalid float: {}", e)),
        Ok(v) if v < 0.0 => Err("must not be negative".into()),
        Ok(v) if v.is_nan() => Err("must not be NaN".into()),
        Ok(v) => Ok(Duration::from_nanos((v * 1_000_000.0) as u64)),
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum VulkanDevice {
    Index(u32),
    Uuid([u8; 16]),
}

impl FromStr for VulkanDevice {
    type Err = String;
    fn from_str(src: &str) -> Result<Self, Self::Err> {
        let src = src.trim();
        if src.len() == 32 {
            let uuid = <[u8; 16] as hex::FromHex>::from_hex(src)
                .map_err(|e| format!("could not parse hex string: {}", e))?;
            Ok(VulkanDevice::Uuid(uuid))
        } else {
            let index = src.parse::<u32>()
                .map_err(|e| format!("invalid 'u32' index: {}", e))?;
            Ok(VulkanDevice::Index(index))
        }
    }
}

fn parse_mode(s: &str) -> Result<PresentMode, &'static str> {
    use self::PresentMode::*;

    Ok(match s {
        "immediate" => Immediate,
        "mailbox" => Mailbox,
        "fifo" => Fifo,
        "relaxed" => Relaxed,
        _ => Err("invalid present mode")?,
    })
}

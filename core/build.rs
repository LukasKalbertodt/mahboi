use std::{
    path::Path,
    process::Command,
};

fn main() {
    println!("cargo:rerun-if-changed=data");

    let bios_path = Path::new("data/DMG_BIOS_ROM.bin");


    if !bios_path.exists() {
        Command::new("curl")
            .arg("http://www.neviksti.com/DMG/DMG_ROM.bin")
            .arg("--output")
            .arg(bios_path)
            .arg("--silent")
            .status()
            .expect("failed to execute curl");
    }
}

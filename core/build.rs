use std::{
    fs::File,
    path::Path,
};

use failure::Error;

fn main() -> Result<(), Error> {
    println!("cargo:rerun-if-changed=data");

    let bios_path = Path::new("data/DMG_BIOS_ROM.bin");

    if !bios_path.exists() {
        reqwest::get("http://www.neviksti.com/DMG/DMG_ROM.bin")?
            .copy_to(&mut File::create(bios_path)?)?;
    }

    Ok(())
}

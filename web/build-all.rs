#!/usr/bin/env run-cargo-script
// cargo-deps: failure, term-painter
//
// # THE BUILD SYSTEM
//
// This file contains the build system. It consists of four steps, each defined
// in its own function. You can read the function documentation to learn more
// about each step.
//
// To run this file, you need `cargo-script`. You can install it via:
// `cargo install cargo-script`. Afterwards you can just run `./build-all.rs`.
//
// TODO: This file should be called `build.rs`, but this is currently not
// possible. See: https://github.com/DanielKeep/cargo-script/issues/58

#[macro_use]
extern crate failure;
extern crate term_painter;

use failure::{Error, ResultExt};
use term_painter::{Attr, Color, ToStyle};

use std::{
    fs::{self, File},
    io::{BufRead, BufReader},
    path::Path,
    process::{self, Command},
};



fn main() {
    if let Err(e) = run() {
        println!(
            "  [!!!] 💥 {}",
            Color::Red.bold().paint(&e),
        );

        for cause in e.iter_causes() {
            println!("       ... caused by: {}", cause);
        }

        process::exit(1);
    }
}

/// Wrapper for the actual `main` to catch errors.
fn run() -> Result<(), Error> {
    let release_mode = match &std::env::args().nth(1) {
        Some(s) if s == "--release" => true,
        _ => false,
    };

    // Create out dir if it doesn't exist yet.
    let out_dir = Path::new("dist");
    if !out_dir.exists() {
        fs::create_dir(out_dir)?;
    }

    cargo_build(release_mode)?;
    wasm_bindgen(release_mode, out_dir)?;
    compile_typescript(out_dir)?;
    copy_into_dist(out_dir)?;
    println!();

    Ok(())
}

/// Runs `cargo build --target wasm32-unknown-unknown`.
///
/// This generates the actualy wasm file in the shared `target/` folder located
/// `../target/`.
fn cargo_build(release_mode: bool) -> Result<(), Error> {
    println!(
        "  [1/4] 🌀 {} (`cargo build{}`) ...",
        Attr::Bold.paint("Compiling Rust to WASM"),
        if release_mode { " --release" } else { "" },
    );

    let mut args = vec!["build", "--target", "wasm32-unknown-unknown"];
    if release_mode {
        args.push("--release");
    }

    let status = Command::new("cargo")
        .args(args)
        .spawn()
        .context("failed to spawn `cargo`")?
        .wait()?;

    if !status.success() {
        bail!("Failed to run `cargo` (exit code {:?})", status.code());
    }

    Ok(())
}

/// Runs `wasm-bindgen --no-modules ...`.
///
/// So the WASM file output by `cargo build` is not that nice yet. To have a
/// nice interface, `wasm-bindgen` postprocesses the file and creates a JS
/// wrapper with a nice interface. It also outputs a TypeScript declaration
/// file.
///
/// Running this build step results in:
/// - `dist/mahboi_web.js`
/// - `dist/mahboi_web_bg.wasm`
/// - `src/mahboi.d.ts` (the TS declaration file)
///
/// TODO: We might want to put the TS declaration file somewhere else...
fn wasm_bindgen(release_mode: bool, out_dir: &Path) -> Result<(), Error> {
    println!(
        "  [2/4] 🔗 {} ... ",
        Attr::Bold.paint("Running `wasm-bindgen`"),
    );

    // Determine path of the WASM file generated by cargo
    let folder = match release_mode {
        true => "release",
        false => "debug",
    };
    let input = format!("../target/wasm32-unknown-unknown/{}/mahboi_web.wasm", folder);

    // Only build if the WASM file has changed. We don't do it here for the
    // speed, but to avoid overwriting already existing files and thus changing
    // the modified date. Otherwise, the next build step couldn't tell if
    // modifications are necessary.
    let out_wasm = out_dir.join("mahboi_web_bg.wasm");
    let needs_rebuild = !out_wasm.exists()
        || Path::new(&input).metadata()?.modified()? >= out_wasm.metadata()?.modified()?;

    if !needs_rebuild {
        println!("           ... files up to date.");
        return Ok(());
    }

    // Execute `wasm-bindgen` and let it put all the files in `dist`. Three
    // files are written:
    // - `mahboi_web.js` (JS shim)
    // - `mahboi_web_bg.wasm`
    // - `mahboi_web.d.ts` (TS declarations)
    let status = Command::new("wasm-bindgen")
        .arg("--no-modules")
        .arg("--out-dir")
        .arg(out_dir)
        .arg(input)
        .spawn()
        .context("failed to spawn `wasm-bindgen`")?
        .wait()?;

    if !status.success() {
        bail!("Failed to run `wasm-bindgen` (exit code {:?})", status.code());
    }

    // We need to postprocess the typescript definition file emitted by
    // wasm-bg. We wrap the whole file into `namespace wasm_bindgen { ... }` and
    // add a `wasm_bindgen` function at the end.
    let type_decl_path = out_dir.join("mahboi_web.d.ts");
    let orig = BufReader::new(File::open(&type_decl_path)?);

    let mut types = String::new();
    types.push_str("declare namespace wasm_bindgen {\n");
    for line in orig.lines() {
        types.push_str("    ");
        types.push_str(&line?);
        types.push_str("\n");
    }
    types.push_str("}\n");
    types.push_str("\n");
    types.push_str("declare function wasm_bindgen(path: string): Promise<void>;\n");

    // Write the modified version to `src/` and remove the original file.
    fs::write(Path::new("src").join("mahboi.d.ts"), &types)?;
    fs::remove_file(&type_decl_path)?;

    Ok(())
}

/// Just execute `tsc`.
///
/// This build steps takes the file `src/main.ts` and generates the file
/// `dist/main.js`. The typescript compiler is only run if either `src/main.ts`
/// or `src/mahboi.d.ts` is newer than the `dist/main.js`.
fn compile_typescript(out_dir: &Path) -> Result<(), Error> {
    println!(
        "  [3/4] 🔬 {} ...",
        Attr::Bold.paint("Compiling TypeScript"),
    );

    // The TS compiler can be super slow, so we check if compilation is
    // necessary.
    let src_modified = Path::new("src").join("main.ts").metadata()?.modified()?;
    let decl_modified = Path::new("src").join("mahboi.d.ts").metadata()?.modified()?;
    let out_file = out_dir.join("main.js");

    let needs_rebuild = !out_file.exists()
        || src_modified >= out_file.metadata()?.modified()?
        || decl_modified >= out_file.metadata()?.modified()?;

    if needs_rebuild {
        let status = Command::new("tsc")
            .spawn()
            .context("failed to spawn `tsc`")?
            .wait()?;

        if !status.success() {
            bail!("Failed to run `tsc` (exit code {:?})", status.code());
        }
    } else {
        println!("           ... files up to date.");
    }

    Ok(())
}

/// Copies all files from `static/` to `dist/`.
fn copy_into_dist(out_dir: &Path) -> Result<(), Error> {
    println!(
        "  [4/4] 🎁 {} ...",
        Attr::Bold.paint("Copy static files into `dist/`"),
    );

    for entry in fs::read_dir("static")? {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            let dst = out_dir.join(entry.file_name());
            fs::copy(entry.path(), dst)?;
        }
    }

    Ok(())
}

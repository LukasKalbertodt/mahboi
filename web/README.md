Mahboi Web
==========

This is the web interface for mahboi, allowing you to play in the browser.


## Development

To work on this, you should have a few things:

- `cargo-script` (necessary to execute build script, install with `cargo install cargo-script`)
- `wasm32-unknown-unknown` target for Rust (install it with `rustup target add wasm32-unknown-unknown`)
- `wasm-bindgen-cli` (install via `cargo install wasm-bindgen-cli`)
- `watchexec` (necessary for watch mode, install via `cargo install watchexec`)
- A simple HTTP server (`cd dist && python -m SimpleHTTPServer` is sufficient)

#### Build
```
./build-all.rs
```

This builds everything and the resulting files in the `dist/` folder. Start a webserver in that folder to try the program.

#### Watch
```
./watch.sh
```

This uses `watchexec` and executes `build-all.rs` whenever something important changes. You only need to start the webserver once.

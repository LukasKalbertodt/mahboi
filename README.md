Mahboi – yet another Game Boy emulator
======================================

Hobby project. WIP.


## Requirements

To compile this project, you need a nightly compiler. This is because we are already using the *Rust 2018* edition which is not quite stable yet. However, most features contained in *Rust 2018* are already fairly stable and are just waiting for stabilization. So it's not like we're using features that will break tomorrow. To use (and install) a nightly compiler for this project, run:

```
$ rustup override set nightly
```

You can also use `nightly-2018-08-06` instead, if you want to have the exact version I'm using right now. But as I said: future versions shouldn't break anything.

To compile the WASM part of this project, additional software is required. See the README in the `web/` folder for more information.

---

## License

Licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.

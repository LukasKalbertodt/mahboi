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

## Documentation and Information

#### Random facts:
- The classic *Game Boy* is also called *DMG* or *DMG-01* (dot matrix game, development name).
- The *Game Boy Color* is also called *CGB*.

#### Links:

- [Complete reference explanation of everything GB (inclusive GBC)](http://bgb.bircd.org/pandocs.htm) (also as [wiki version](http://gbdev.gg8.se/wiki/articles/Pan_Docs))
- [OP code cheat sheet](http://pastraiser.com/cpu/gameboy/gameboy_opcodes.html)
- [Information about the BIOS](http://gbdev.gg8.se/wiki/articles/Gameboy_Bootstrap_ROM)
    - [Link the the GB bios](http://www.neviksti.com/DMG/)
    - [Post describing the CGB boot extraction (incl. download)](https://web.archive.org/web/20091001114207/http://www.fpgb.org:80/?page_id=17)
- [A super extensive list of everything useful for GB development](https://github.com/avivace/awesome-gbdev)


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

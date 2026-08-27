# b2t80s_rust

A ZX Spectrum 48K emulator written in Rust.

## Run

The emulator needs a 16 KiB ZX Spectrum 48K ROM image. Because ROM images are
not included in the repository, either place one at `bin/48.rom` or point the
emulator to it explicitly:

```sh
B2T80S_ROM=/path/to/48.rom cargo run
```

Audio is optional. If no compatible output device is available, the emulator
continues with audio disabled.

## Tests

```sh
cargo test --all-targets
```

The opcode fixtures originate from
[FUSE](https://github.com/FrodeSolheim/fs-fuse/tree/master/z80/tests). The full
ZEXDOC diagnostic is available as an ignored, long-running test.

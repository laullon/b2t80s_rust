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

### macOS app

Build a native application bundle so macOS uses the project icon in Finder,
the app switcher, and the Dock:

```sh
./scripts/build-macos-app.sh
open target/release/bundle/b2t80s.app
```

Launching with `cargo run` still runs a bare executable rather than the bundled
application.

## Debugger

The desktop UI includes a live Z80 debugger panel with register and flag state,
beam position, chronological instruction history, forward disassembly with the
next instruction highlighted, and controls to pause, resume, reset, or execute
one instruction at a time.

Entering native fullscreen switches to presentation mode, showing only the
Spectrum framebuffer. The debugger, toolbar, status bar, and panel chrome
return automatically when leaving fullscreen.

The **Load Game…** control resets the Spectrum and types the original 48K BASIC
`LOAD ""` key sequence. The ROM's existing tape-loading path then opens the
native TAP file dialog. **Fast Tape** uses the ROM trap for near-instant loads
and holds a completed loading screen for about two seconds before continuing.
Turn it off before loading to play the TAP as real EAR pulses, letting the
original ROM produce authentic loading time, border stripes, screen reveal,
and sound.

Planned debugger additions include address and conditional breakpoints, a
memory/stack inspector, watch expressions, and a raster timeline for diagnosing
ULA contention and border effects.

## Tests

```sh
cargo test --all-targets
```

The opcode fixtures originate from
[FUSE](https://github.com/FrodeSolheim/fs-fuse/tree/master/z80/tests). The full
ZEXDOC diagnostic is available as an ignored, long-running test.

# Contributing

Thanks for helping with Just Metal Shapes. The project is a Godot 4 game with gameplay mostly in GDScript and an expanding Rust GDExtension in `rust/`.

## Required Tools

- Godot 4.6.2 for normal editor work.
- A non-Mono/non-.NET Godot 4.6.2 editor for Web exports.
- Rust stable for native extension checks.
- Rust nightly for Web builds.
- Emscripten 3.1.74 through `emsdk` for Godot Rust Web builds.
- Python 3 for serving local Web exports.

Godot 4 cannot export Web builds from the Mono/.NET editor. If your `godot --version` includes `mono`, install the standard Godot editor too and use it for Web export commands.

The Makefile will automatically use `/tmp/godot-4.6.2-standard/Godot_v4.6.2-stable_linux.x86_64` when that local standard editor exists. For a permanent setup, either put a standard Godot binary earlier on your `PATH` or pass `GODOT_EXPORT_BIN` explicitly.

## First-Time Web Setup

Install the Rust pieces:

```sh
rustup toolchain install nightly
rustup component add rust-src --toolchain nightly
rustup target add wasm32-unknown-emscripten --toolchain nightly
```

Install Emscripten:

```sh
git clone https://github.com/emscripten-core/emsdk.git ~/opt/emsdk
cd ~/opt/emsdk
./emsdk install 3.1.74
./emsdk activate 3.1.74
```

Install the Godot 4.6.2 export templates from the Godot editor, or place the official templates under:

```text
~/.local/share/godot/export_templates/4.6.2.stable/
```

## Common Commands

Native Rust check:

```sh
make check
```

Build and export the no-thread Web build:

```sh
make web-build GODOT_EXPORT_BIN=/path/to/Godot_v4.6.2-stable_linux.x86_64 EMSDK=~/opt/emsdk
```

Serve the exported Web build locally:

```sh
make web-serve
```

Build, export, and serve in one step:

```sh
make web-run GODOT_EXPORT_BIN=/path/to/Godot_v4.6.2-stable_linux.x86_64 EMSDK=~/opt/emsdk
```

Then open:

```text
http://127.0.0.1:8060
```

## Web Export Notes

The Web preset is configured for GDExtension support with thread support disabled. The Rust WASM build uses:

```sh
cargo +nightly build --features nothreads -Zbuild-std --target wasm32-unknown-emscripten
```

Generated exports live in `build/` and are ignored by Git.

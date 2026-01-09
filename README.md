# opus-builder

A Rust-based multi-platform build tool for compiling Xiph “opus-family” libraries (libogg / libopus / libopusenc / libopusfile) and organizing outputs into a unified directory structure.

## Features

- Reads `build_config.toml` to select libraries, platforms, architectures, and library types (static/shared)
- Fetches/reuses upstream source repos and checks out pinned versions
- Apple platforms (macOS / iOS / iOS Simulator): builds universal binaries and packages them into `.xcframework`
- Android: builds per-ABI outputs (`.so` for shared or `.a` for static) and archives them under `build/lib/android/...`

## Supported Platforms & Architectures

| Platform | Config Section | Architectures / ABIs | Output Extension | Notes |
| --- | --- | --- | --- | --- |
| macOS | `platforms.macos` | `arm64`, `x86_64` | `a` / `dylib` | Can produce universal binaries + xcframework |
| iOS Device | `platforms.ios` | `arm64` | `a` / `dylib` | Can produce universal binaries + xcframework |
| iOS Simulator | `platforms.ios-sim` | `arm64`, `x86_64` | `a` / `dylib` | Can produce universal binaries + xcframework |
| Android | `platforms.android` | `arm64-v8a`, `armeabi-v7a`, `x86_64`, `x86` | `a` / `so` | |
| Harmony | `platforms.harmony` | Reuses Android config | `a` / `so` | |

## Known Issues

This project is under active development now. Issues below are being worked on:

- Harmony platform is not implemented in code yet (will error at runtime).
- iOS shared library is not supported yet.

## Prerequisites

- Rust toolchain (stable recommended)
- git
- Autotools toolchain (required by upstream libraries): `autoconf` / `automake` / `libtool`
- Apple platforms: Xcode / Command Line Tools (`xcrun`, `clang`, `xcodebuild`)
- Android: NDK, configured via `ndk_path` in `build_config.toml`

## Quick Start

Build (reads `build_config.toml` from the current directory):

```bash
cargo run -- build
```

More verbose output:

```bash
cargo run -- -v build
```

Clean:

```bash
cargo run -- clean
```

Clean build artifacts only:

```bash
cargo run -- clean -b
```

## Configuration

Build behavior is controlled by `build_config.toml`. Common fields:

- `[general]`
  - `libraries`: libraries to build (e.g. `["libogg"]`)
  - `platforms`: platforms to build (e.g. `["ios", "ios-sim", "android"]`)
  - `keep_intermediate`: whether to keep intermediate build artifacts
- `[platforms.<name>]`
  - `archs`: target architectures / ABIs
  - `lib_type`: `static` or `shared`
- `[libraries.<name>]`
  - `version`: git tag/commit to check out
  - `configure_flags` / `cflags` / `ldflags`: per-library extra flags

Example: build iOS static libraries (device + simulator) only:

```toml
[general]
libraries = ["libogg"]
platforms = ["ios", "ios-sim"]

[platforms.ios]
archs = ["arm64"]
lib_type = "static"

[platforms.ios-sim]
archs = ["arm64", "x86_64"]
lib_type = "static"
```

## Output Layout

The default output directory is `build/`:

- `build/include/<libname>/`: unified headers output
- `build/lib/`
  - `build/lib/darwin/<libname>-<version>.xcframework/`: Apple `.xcframework`
  - `build/lib/android/<abi>/<libname>-<version>/`: archived Android outputs

Lower-level intermediate artifacts live under `build/<platform>/<arch>/<repo>/...` and will be removed automatically when `keep_intermediate=false`.

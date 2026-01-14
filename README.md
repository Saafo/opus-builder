# opus-builder

[![CI](https://github.com/Saafo/opus-builder/actions/workflows/ci.yml/badge.svg)](https://github.com/Saafo/opus-builder/actions/workflows/ci.yml)
[![CD](https://github.com/Saafo/opus-builder/actions/workflows/cd.yml/badge.svg)](https://github.com/Saafo/opus-builder/actions/workflows/cd.yml)
[![GitHub License](https://img.shields.io/github/license/Saafo/opus-builder)](https://github.com/Saafo/opus-builder?tab=MIT-1-ov-file)<br>
[![Host Platforms](https://img.shields.io/badge/Host%20Platforms-macOS%20%7C%20Linux-green)](https://github.com/Saafo/opus-builder)<br>
[![Target Platforms](https://img.shields.io/badge/Target%20Platforms-iOS%20%7C%20macOS%20%7C%20Android%20%7C%20Harmony-blue)](https://github.com/Saafo/opus-builder)<br>

A Rust-based multi-platform build tool for compiling Xiph "opus-family" libraries (libogg / libopus / libopusenc / libopusfile) and organizing outputs into a unified directory structure.

## Features

- Reads `build_config.toml` to select libraries, platforms, architectures, and library types (static/shared)
- Fetches/reuses upstream source repos and checks out pinned versions
- Apple platforms (macOS / iOS / iOS Simulator): builds universal binaries and packages them into `.xcframework`
- Android: builds per-ABI outputs (`.so` for shared or `.a` for static) and archives them under `build/lib/android/...`
- Harmony: builds per-ABI outputs (`.so` for shared or `.a` for static) and archives them under `build/lib/harmony/...`

## Supported Platforms & Architectures

| Platform | Architectures / ABIs | Output Extension | Notes |
| --- | --- | --- | --- |
| macOS | `arm64`, `x86_64` | `a` / `dylib` inside `.xcframework/framework` | dylib not supported yet |
| iOS Device | `arm64` | `a` / `dylib` inside `.xcframework/framework` | dylib not supported yet |
| iOS Simulator | `arm64`, `x86_64` | `a` / `dylib` inside `.xcframework/framework` | dylib not supported yet |
| Android | `arm64-v8a`, `armeabi-v7a`, `x86_64`, `x86` | `a` / `so` | |
| Harmony | `armeabi-v7a`, `arm64-v8a`, `x86_64` | `a` / `so` | |

## Known Issues

This project is under active development now. Issues below are being worked on:

- Darwin shared library is not supported yet (macOS/iOS).

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

## Build on GitHub Actions

If you don't want to build locally, you can run everything on GitHub Actions:

1. Fork this repository.
2. Edit `build_config.toml` in your fork to select (see [Configuration](#configuration))
  3. Push commits to your fork:
   - pushing to the fork's `main` branch triggers CI automatically, or
   - go to GitHub Actions → `CI` → `Run workflow` to trigger it manually.
4. Download build outputs from the workflow run page (Artifacts section).

> Notes:
> - To create a draft GitHub Release with the build artifacts attached, push a git tag that matches `v*` (e.g. `v0.1.0`) to your fork. The `CD` workflow will run and create/update a draft release whose body lists the library versions read from `build_config.toml`.

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
  - `build/lib/harmony/<abi>/<libname>-<version>/`: archived Harmony outputs

Lower-level intermediate artifacts live under `build/<platform>/<arch>/<repo>/...` and will be removed automatically when `keep_intermediate=false`.

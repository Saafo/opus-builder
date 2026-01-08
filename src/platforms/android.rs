use crate::builder::AutotoolsToolchain;
use crate::config::{Arch, Config, LibType, Library};
use anyhow::{Context, Result};
use std::env;
use std::fs;
use std::path::Path;

pub mod build {
    use super::*;

    pub fn arch_dir_name(arch: Arch) -> Result<&'static str> {
        match arch {
            Arch::ArmeabiV7a => Ok("armeabi-v7a"),
            Arch::Arm64V8a => Ok("arm64-v8a"),
            Arch::X86 => Ok("x86"),
            Arch::X86_64 => Ok("x86_64"),
            _ => anyhow::bail!("Unsupported architecture for Android: {:?}", arch),
        }
    }

    fn host_triple(arch: Arch) -> Result<&'static str> {
        match arch {
            Arch::ArmeabiV7a => Ok("armv7-linux-androideabi"),
            Arch::Arm64V8a => Ok("aarch64-linux-android"),
            Arch::X86 => Ok("i686-linux-android"),
            Arch::X86_64 => Ok("x86_64-linux-android"),
            _ => anyhow::bail!("Unsupported architecture for Android: {:?}", arch),
        }
    }

    fn host_platform() -> Result<&'static str> {
        if cfg!(target_os = "macos") {
            Ok("darwin-x86_64")
        } else if cfg!(target_os = "linux") {
            Ok("linux-x86_64")
        } else {
            anyhow::bail!("Unsupported host OS for Android NDK: {}", env::consts::OS)
        }
    }

    pub fn prepare_toolchain(arch: Arch, config: &Config) -> Result<AutotoolsToolchain> {
        let android_config = &config.platforms.android;

        let arch_dir = arch_dir_name(arch)?.to_string();
        let host = host_triple(arch)?.to_string();
        let host_platform = host_platform()?;

        let toolchain_bin = android_config
            .ndk_path
            .join("toolchains/llvm/prebuilt")
            .join(host_platform)
            .join("bin");

        let api_level = android_config.native_api_level;
        let cc_target = format!("{}{}", host, api_level);

        let clang = toolchain_bin.join("clang");
        let clangxx = toolchain_bin.join("clang++");

        let cc = format!("{} --target={}", clang.display(), cc_target);
        let cxx = format!("{} --target={}", clangxx.display(), cc_target);

        let extra_env = vec![
            (
                "AR".to_string(),
                toolchain_bin.join("llvm-ar").display().to_string(),
            ),
            ("AS".to_string(), cc.clone()),
            (
                "LD".to_string(),
                toolchain_bin.join("ld").display().to_string(),
            ),
            (
                "NM".to_string(),
                toolchain_bin.join("llvm-nm").display().to_string(),
            ),
            (
                "RANLIB".to_string(),
                toolchain_bin.join("llvm-ranlib").display().to_string(),
            ),
            (
                "STRIP".to_string(),
                toolchain_bin.join("llvm-strip").display().to_string(),
            ),
        ];

        Ok(AutotoolsToolchain {
            platform_dir: "android".to_string(),
            arch_dir,
            host,
            cc,
            cxx: Some(cxx),
            extra_env,
            base_cflags: config.build.cflags.clone(),
            base_ldflags: config.build.ldflags.clone(),
        })
    }

    pub fn move_android_package(
        build_dir: &Path,
        library: &Library,
        version: &str,
        arch: Arch,
        lib_type: LibType,
    ) -> Result<()> {
        let lib_name = library.name_with_lib_prefix();
        let repo_name = library.repo_name();
        let version = version.trim_start_matches('v');

        let arch_dir = arch_dir_name(arch)?;
        let file_name = format!("{}.{}", lib_name, lib_type.android_harmony_ext());

        let source_lib = build_dir
            .join("android")
            .join(arch_dir)
            .join(repo_name)
            .join("lib")
            .join(&file_name);

        let dest_dir = build_dir
            .join("lib")
            .join("android")
            .join(arch_dir)
            .join(format!("{}-{}", lib_name, version));

        fs::create_dir_all(&dest_dir)?;
        let dest_lib = dest_dir.join(&file_name);

        if source_lib.exists() {
            log::info!(
                "Moving {} from {} to {}",
                lib_name,
                source_lib.display(),
                dest_lib.display()
            );
            fs::copy(&source_lib, &dest_lib).with_context(|| {
                format!(
                    "Failed to copy {} from {} to {}",
                    lib_name,
                    source_lib.display(),
                    dest_lib.display()
                )
            })?;
        } else {
            log::warn!("Library file not found: {}, skipping", source_lib.display());
        }

        Ok(())
    }
}

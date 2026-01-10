use crate::builder::AutotoolsToolchain;
use crate::config::{Arch, Config, LibType, Library};
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

pub mod build {
    use super::*;

    pub fn arch_dir_name(arch: Arch) -> Result<&'static str> {
        match arch {
            Arch::ArmeabiV7a => Ok("armeabi-v7a"),
            Arch::Arm64V8a => Ok("arm64-v8a"),
            Arch::X86_64 => Ok("x86_64"),
            _ => anyhow::bail!("Unsupported architecture for Harmony: {:?}", arch),
        }
    }

    fn clang_target(arch: Arch) -> Result<&'static str> {
        match arch {
            Arch::ArmeabiV7a => Ok("arm-linux-ohos"),
            Arch::Arm64V8a => Ok("aarch64-linux-ohos"),
            Arch::X86_64 => Ok("x86_64-linux-ohos"),
            _ => anyhow::bail!("Unsupported architecture for Harmony: {:?}", arch),
        }
    }

    fn configure_host(arch: Arch) -> Result<&'static str> {
        match arch {
            Arch::ArmeabiV7a => Ok("arm-linux"),
            Arch::Arm64V8a => Ok("aarch64-linux"),
            Arch::X86_64 => Ok("x86_64-linux"),
            _ => anyhow::bail!("Unsupported architecture for Harmony: {:?}", arch),
        }
    }

    // Reference: https://github.com/ohos-rs/ohos-openssl/blob/main/scripts/armeabi-v7a.sh
    fn arch_cflags(arch: Arch) -> Result<&'static str> {
        match arch {
            Arch::ArmeabiV7a => {
                Ok("-D__MUSL__ -march=armv7-a -mfloat-abi=softfp -mtune=generic-armv7-a -mthumb")
            }
            Arch::Arm64V8a | Arch::X86_64 => Ok("-D__MUSL__"),
            _ => anyhow::bail!("Unsupported architecture for Harmony: {:?}", arch),
        }
    }

    fn toolchain_bin(ndk_path: &Path) -> Result<PathBuf> {
        let bin = ndk_path.join("native/llvm/bin");
        if !bin.exists() {
            anyhow::bail!("Harmony toolchain bin not found: {}", bin.to_string_lossy());
        }
        Ok(bin)
    }

    fn sysroot(ndk_path: &Path) -> Result<PathBuf> {
        let sysroot = ndk_path.join("native/sysroot");
        if !sysroot.exists() {
            anyhow::bail!("Harmony sysroot not found: {}", sysroot.to_string_lossy());
        }
        Ok(sysroot)
    }

    pub fn prepare_toolchain(arch: Arch, config: &Config) -> Result<AutotoolsToolchain> {
        let harmony_config = &config.platforms.harmony;

        let arch_dir = arch_dir_name(arch)?.to_string();
        let host = configure_host(arch)?.to_string();
        let target = clang_target(arch)?;
        let arch_flags = arch_cflags(arch)?;

        let toolchain_bin = toolchain_bin(&harmony_config.ndk_path)?;
        let sysroot = sysroot(&harmony_config.ndk_path)?;

        let clang = toolchain_bin.join("clang");
        let clangxx = toolchain_bin.join("clang++");

        let cc = format!("{} --target={}", clang.display(), target);
        let cxx = format!("{} --target={}", clangxx.display(), target);

        let extra_env = vec![
            (
                "AR".to_string(),
                toolchain_bin.join("llvm-ar").display().to_string(),
            ),
            (
                "LD".to_string(),
                toolchain_bin.join("ld.lld").display().to_string(),
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

        let base_cflags = format!(
            "{} --sysroot={} {}",
            config.build.cflags,
            sysroot.display(),
            arch_flags
        );
        let base_ldflags = format!("{} --sysroot={}", config.build.ldflags, sysroot.display());

        Ok(AutotoolsToolchain {
            platform_dir: "harmony".to_string(),
            arch_dir,
            host,
            cc,
            cxx: Some(cxx),
            extra_env,
            base_cflags,
            base_ldflags,
        })
    }

    pub fn move_harmony_package(
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
        let file_name = format!("{}.{}", lib_name, lib_type.linux_ext());

        let source_lib = build_dir
            .join("harmony")
            .join(arch_dir)
            .join(repo_name)
            .join("lib")
            .join(&file_name);

        let dest_dir = build_dir
            .join("lib")
            .join("harmony")
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

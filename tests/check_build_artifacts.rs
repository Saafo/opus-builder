use opus_builder::config::{self, Platform};
use opus_builder::platforms::{android, harmony};
use std::fs;
use std::path::Path;

fn version_no_v(version: &str) -> &str {
    version.strip_prefix('v').unwrap_or(version)
}

fn assert_dir_exists(path: &Path) {
    assert!(
        path.is_dir(),
        "expected directory exists: {}",
        path.display()
    );
}

fn assert_file_exists(path: &Path) {
    assert!(path.is_file(), "expected file exists: {}", path.display());
}

fn has_header_file(dir: &Path) -> bool {
    let Ok(entries) = fs::read_dir(dir) else {
        return false;
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            if has_header_file(&p) {
                return true;
            }
        } else if p.extension().is_some_and(|ext| ext == "h") {
            return true;
        }
    }
    false
}

#[test]
fn check_build_artifacts() {
    let config_path = std::path::PathBuf::from("build_config.toml");
    assert!(config_path.exists(), "build_config.toml must exist");
    let config = config::load_or_create_config(&config_path).expect("load build_config.toml");

    let build_dir = &config.paths.build_dir;

    let has_darwin = config.general.platforms.iter().any(Platform::is_darwin);
    if has_darwin {
        for lib in &config.general.libraries {
            let lib_name = lib.name_with_lib_prefix();
            let version = config.get_library_version(lib).expect("library version");
            let expected = build_dir.join("lib").join("darwin").join(format!(
                "{}-{}.xcframework",
                lib_name,
                version_no_v(version)
            ));
            assert_dir_exists(&expected);
        }
    }

    if config.general.platforms.contains(&Platform::Android) {
        let lib_type = config
            .platforms
            .get_lib_type_for_platform(&Platform::Android);
        let ext = lib_type.linux_ext();
        let archs = config.platforms.get_archs_for_platform(&Platform::Android);

        for lib in &config.general.libraries {
            let lib_name = lib.name_with_lib_prefix();
            let version = config.get_library_version(lib).expect("library version");
            let version = version_no_v(version);
            for arch in archs {
                let abi = android::build::arch_dir_name(*arch).expect("android abi");
                let expected = build_dir
                    .join("lib")
                    .join("android")
                    .join(abi)
                    .join(format!("{lib_name}-{version}"))
                    .join(format!("{lib_name}.{ext}"));
                assert_file_exists(&expected);
            }
        }
    }

    if config.general.platforms.contains(&Platform::Harmony) {
        let lib_type = config
            .platforms
            .get_lib_type_for_platform(&Platform::Harmony);
        let ext = lib_type.linux_ext();
        let archs = config.platforms.get_archs_for_platform(&Platform::Harmony);

        for lib in &config.general.libraries {
            let lib_name = lib.name_with_lib_prefix();
            let version = config.get_library_version(lib).expect("library version");
            let version = version_no_v(version);
            for arch in archs {
                let abi = harmony::build::arch_dir_name(*arch).expect("harmony abi");
                let expected = build_dir
                    .join("lib")
                    .join("harmony")
                    .join(abi)
                    .join(format!("{lib_name}-{version}"))
                    .join(format!("{lib_name}.{ext}"));
                assert_file_exists(&expected);
            }
        }
    }

    for lib in &config.general.libraries {
        let lib_name = lib.name_with_lib_prefix();
        let header_dir = build_dir.join("include").join(&lib_name);
        assert_dir_exists(&header_dir);
        assert!(
            has_header_file(&header_dir),
            "expected at least one .h under {}",
            header_dir.display()
        );
    }
}

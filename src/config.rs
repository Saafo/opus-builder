use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct Config {
    pub general: GeneralConfig,
    pub paths: PathConfig,
    pub build: Build,
    pub platforms: PlatformConfig,
    pub libraries: HashMap<Library, LibraryBuildOptions>,
}

impl Default for Config {
    fn default() -> Self {
        let platforms = PlatformConfig {
            macos: DarwinConfig {
                min_version: "10.13".to_string(),
                archs: vec![Arch::Arm64, Arch::X86_64],
                lib_type: LibType::Static,
            },
            ios: DarwinConfig {
                min_version: "11.0".to_string(),
                archs: vec![Arch::Arm64],
                lib_type: LibType::Static,
            },
            ios_sim: DarwinConfig {
                min_version: "11.0".to_string(),
                archs: vec![Arch::Arm64, Arch::X86_64],
                lib_type: LibType::Static,
            },
            android: AndroidConfig::default(),
            harmony: HarmonyConfig::default(),
        };

        let mut libraries = HashMap::new();
        libraries.insert(
            Library::Libogg,
            LibraryBuildOptions {
                version: Some("v1.3.5".to_string()),
                cflags: None,
                ldflags: None,
                configure_flags: None,
            },
        );
        libraries.insert(
            Library::Libopus,
            LibraryBuildOptions {
                version: Some("v1.5.2".to_string()),
                cflags: None,
                ldflags: None,
                configure_flags: Some(vec![
                    "--enable-float-approx".to_string(),
                    "--disable-extra-programs".to_string(),
                    "--disable-doc".to_string(),
                ]),
            },
        );
        libraries.insert(
            Library::Libopusenc,
            LibraryBuildOptions {
                version: Some("v0.2.1".to_string()),
                cflags: None,
                ldflags: None,
                configure_flags: None,
            },
        );
        libraries.insert(
            Library::Libopusfile,
            LibraryBuildOptions {
                version: Some("v0.12".to_string()),
                cflags: None,
                ldflags: None,
                configure_flags: Some(vec![
                    "--disable-http".to_string(),
                    "--disable-examples".to_string(),
                    "--disable-doc".to_string(),
                ]),
            },
        );

        Self {
            general: GeneralConfig::default(),
            paths: PathConfig::default(),
            build: Build::default(),
            platforms,
            libraries,
        }
    }
}

impl Config {
    pub fn get_library_version(&self, library: &Library) -> Result<&str> {
        let lib_config = self
            .libraries
            .get(library)
            .with_context(|| format!("Library configuration not found for: {library:?}"))?;
        lib_config
            .version
            .as_deref()
            .with_context(|| format!("Version not specified for library: {library:?}"))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Library {
    Libogg,
    Libopus,
    Libopusenc,
    Libopusfile,
    // Libopusurl,
}
impl std::fmt::Display for Library {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl Library {
    pub fn repo_name(&self) -> &'static str {
        match self {
            Library::Libopus => "opus",
            Library::Libopusenc => "libopusenc",
            Library::Libogg => "ogg",
            Library::Libopusfile => "opusfile",
            // Library::Libopusurl => "opusfile",
        }
    }
    /// name without lib prefix
    pub fn name_wo_lib_prefix(&self) -> &'static str {
        match self {
            Library::Libopus => "opus",
            Library::Libopusenc => "opusenc",
            Library::Libogg => "ogg",
            Library::Libopusfile => "opusfile",
            // Library::Libopusurl => "opusurl",
        }
    }
    pub fn name_with_lib_prefix(&self) -> String {
        format!("lib{}", self.name_wo_lib_prefix())
    }
    pub fn include_dir(&self) -> PathBuf {
        match self {
            Library::Libogg => PathBuf::from("include").join("ogg"),
            Library::Libopus | Library::Libopusenc | Library::Libopusfile => {
                PathBuf::from("include").join("opus")
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Platform {
    Ios,
    IosSim,
    Android,
    Harmony,
    Macos,
}
impl std::fmt::Display for Platform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Platform::Ios => write!(f, "ios"),
            Platform::IosSim => write!(f, "ios-sim"),
            Platform::Android => write!(f, "android"),
            Platform::Harmony => write!(f, "harmony"),
            Platform::Macos => write!(f, "macos"),
        }
    }
}

impl Platform {
    pub fn is_darwin(&self) -> bool {
        matches!(self, Platform::Macos | Platform::Ios | Platform::IosSim)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Arch {
    #[serde(rename = "x86_64")]
    X86_64,
    #[serde(rename = "arm64")]
    Arm64,
    #[serde(rename = "armeabi-v7a")]
    ArmeabiV7a,
    #[serde(rename = "arm64-v8a")]
    Arm64V8a,
    #[serde(rename = "x86")]
    X86,
}
impl std::fmt::Display for Arch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LibType {
    Static,
    Shared,
}

impl LibType {
    pub fn linux_ext(&self) -> &'static str {
        match self {
            LibType::Static => "a",
            LibType::Shared => "so",
        }
    }
    pub fn darwin_ext(&self) -> &'static str {
        match self {
            LibType::Static => "a",
            LibType::Shared => "dylib",
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PlatformConfig {
    pub macos: DarwinConfig,
    pub ios: DarwinConfig,
    #[serde(rename = "ios-sim")]
    pub ios_sim: DarwinConfig,
    pub android: AndroidConfig,
    pub harmony: HarmonyConfig,
}

impl PlatformConfig {
    pub fn get_archs_for_platform(&self, platform: &Platform) -> &[Arch] {
        match platform {
            Platform::Macos => &self.macos.archs,
            Platform::Ios => &self.ios.archs,
            Platform::IosSim => &self.ios_sim.archs,
            Platform::Android => &self.android.archs,
            Platform::Harmony => &self.harmony.archs,
        }
    }
    pub fn get_lib_type_for_platform(&self, platform: &Platform) -> LibType {
        match platform {
            Platform::Macos => self.macos.lib_type,
            Platform::Ios => self.ios.lib_type,
            Platform::IosSim => self.ios_sim.lib_type,
            Platform::Android => self.android.lib_type,
            Platform::Harmony => self.harmony.lib_type,
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct DarwinConfig {
    pub min_version: String,
    pub archs: Vec<Arch>,
    pub lib_type: LibType,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AndroidConfig {
    pub native_api_level: u32,
    pub ndk_path: PathBuf,
    pub archs: Vec<Arch>,
    pub lib_type: LibType,
}

impl Default for AndroidConfig {
    fn default() -> Self {
        Self {
            native_api_level: 21,
            ndk_path: PathBuf::from("/usr/local/NDK-r28c"),
            archs: vec![Arch::Arm64V8a, Arch::ArmeabiV7a, Arch::X86_64, Arch::X86],
            lib_type: LibType::Shared,
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct HarmonyConfig {
    pub ndk_path: PathBuf,
    pub archs: Vec<Arch>,
    pub lib_type: LibType,
}

impl Default for HarmonyConfig {
    fn default() -> Self {
        Self {
            ndk_path: PathBuf::from(
                "/usr/local/command-line-tools/sdk/HarmonyOS-NEXT-DB3/openharmony",
            ),
            archs: vec![Arch::ArmeabiV7a, Arch::Arm64V8a, Arch::X86_64],
            lib_type: LibType::Shared,
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct GeneralConfig {
    pub platforms: Vec<Platform>,
    pub libraries: Vec<Library>,
    pub keep_intermediate: bool,
    pub repo_prefix: String,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            platforms: vec![
                Platform::IosSim,
                Platform::Ios,
                Platform::Macos,
                Platform::Android,
            ],
            libraries: vec![
                Library::Libogg,
                Library::Libopus,
                Library::Libopusenc,
                Library::Libopusfile,
            ],
            keep_intermediate: false,
            repo_prefix: "https://gitlab.xiph.org/xiph/".to_string(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct PathConfig {
    pub repo_path: Vec<PathBuf>,
    pub build_dir: PathBuf,
}

impl Default for PathConfig {
    fn default() -> Self {
        Self {
            repo_path: vec![PathBuf::from("repos")],
            build_dir: PathBuf::from("build"),
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct Build {
    pub make_concurrent_jobs: u32,
    pub cflags: String,
    pub ldflags: String,
    pub configure_flags: Vec<String>,
}

impl Default for Build {
    fn default() -> Self {
        Self {
            make_concurrent_jobs: 8,
            cflags: "-O3 -g -DNDEBUG -ffast-math".to_string(),
            ldflags: "-flto -fPIE".to_string(),
            configure_flags: vec!["--with-pic".to_string()],
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct LibraryBuildOptions {
    pub version: Option<String>,
    pub cflags: Option<String>,
    pub ldflags: Option<String>,
    pub configure_flags: Option<Vec<String>>,
}

pub fn load_or_create_config(path: &PathBuf) -> Result<Config> {
    if path.exists() {
        log::info!("Loading config from {:?}", path);
        let config_str = fs::read_to_string(path)?;
        let config: Config = toml::from_str(&config_str)?;
        Ok(config)
    } else {
        log::info!(
            "Config file not found, creating a default one at {:?}",
            path
        );
        let config = Config::default();
        let config_str = toml::to_string_pretty(&config)?;
        fs::write(path, config_str)?;
        Ok(config)
    }
}

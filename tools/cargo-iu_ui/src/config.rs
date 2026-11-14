use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fs, path::Path};

pub const CONFIG_FILE_NAME: &str = "iu_ui.toml";

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Setting {
    pub project: ProjectConfig,
    pub registry: RegistryConfig,
    #[serde(default)]
    pub installed: InstalledConfig,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[allow(clippy::struct_field_names)]
pub struct ProjectConfig {
    pub base_dir: String,
    pub components_dir: String,
    pub theme_dir: String,
    pub tag_dir: String,
    pub utils_dir: String,
    pub assets_dir: String,
}

impl Default for ProjectConfig {
    fn default() -> Self {
        Self {
            base_dir: "src/iu_ui".to_string(),
            components_dir: "src/iu_ui/components".to_string(),
            theme_dir: "src/iu_ui/theme".to_string(),
            tag_dir: "src/iu_ui/tag".to_string(),
            utils_dir: "src/iu_ui/utils".to_string(),
            assets_dir: "src/iu_ui/assets".to_string(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct RegistryConfig {
    pub repo: String,
    pub branch: String,
}

impl Default for RegistryConfig {
    fn default() -> Self {
        Self {
            repo: "eto0202/michiu".to_string(),
            branch: "main".to_string(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct InstalledConfig {
    #[serde(default)]
    pub components: BTreeMap<String, String>,
    #[serde(default)]
    pub themes: BTreeMap<String, String>,
    #[serde(default)]
    pub tags: BTreeMap<String, String>,
    #[serde(default)]
    pub utils: BTreeMap<String, String>,
    #[serde(default)]
    pub assets: BTreeMap<String, AssetInstalled>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct AssetInstalled {
    pub hash: String,
    pub used_by: Vec<String>,
}

impl Setting {
    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        Self::load_from(Path::new(CONFIG_FILE_NAME))
    }

    pub fn load_from(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        if !path.exists() {
            return Err(format!(
                "{CONFIG_FILE_NAME} was not found. Please run `cargo iu_ui init` first.",
            )
            .into());
        }
        let content = fs::read_to_string(path)?;
        let setting: Setting = toml::from_str(&content)?;
        Ok(setting)
    }

    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.save_to(Path::new(CONFIG_FILE_NAME))
    }

    pub fn save_to(&self, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        let content = toml::to_string_pretty(self)?;
        fs::write(path, content)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        path::PathBuf,
        sync::atomic::{AtomicUsize, Ordering},
    };

    struct TestDir {
        path: PathBuf,
    }

    impl TestDir {
        fn new(prefix: &str) -> Self {
            static COUNTER: AtomicUsize = AtomicUsize::new(0);
            let id = COUNTER.fetch_add(1, Ordering::Relaxed);
            let pid = std::process::id();
            let path = std::env::temp_dir().join(format!("iu_ui_cfg_test_{prefix}_{pid}_{id}"));
            if path.exists() {
                let _ = fs::remove_dir_all(&path);
            }
            fs::create_dir_all(&path).expect("failed to create test dir");
            Self { path }
        }

        fn file_path(&self, name: &str) -> PathBuf {
            self.path.join(name)
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn test_defaults() {
        let project = ProjectConfig::default();
        assert_eq!(project.base_dir, "src/iu_ui");
        assert_eq!(project.components_dir, "src/iu_ui/components");
        assert_eq!(project.theme_dir, "src/iu_ui/theme");
        assert_eq!(project.tag_dir, "src/iu_ui/tag");
        assert_eq!(project.utils_dir, "src/iu_ui/utils");
        assert_eq!(project.assets_dir, "src/iu_ui/assets");

        let registry = RegistryConfig::default();
        assert_eq!(registry.repo, "eto0202/michiu");
        assert_eq!(registry.branch, "main");

        let installed = InstalledConfig::default();
        assert!(installed.components.is_empty());
        assert!(installed.themes.is_empty());
        assert!(installed.tags.is_empty());
        assert!(installed.utils.is_empty());
        assert!(installed.assets.is_empty());
    }

    #[test]
    fn test_load_from_missing_file() {
        let dir = TestDir::new("missing");
        let missing = dir.file_path(CONFIG_FILE_NAME);

        let err = Setting::load_from(&missing).unwrap_err();
        assert!(
            err.to_string()
                .contains("iu_ui.toml was not found. Please run `cargo iu_ui init` first."),
            "unexpected error message: {err}"
        );
    }

    #[test]
    fn test_load_from_minimal_toml_uses_defaults_for_installed() {
        let dir = TestDir::new("minimal");
        let path = dir.file_path(CONFIG_FILE_NAME);

        let toml_str = r#"
[project]
base_dir = "src/ui"
components_dir = "src/ui/components"
theme_dir = "src/ui/theme"
tag_dir = "src/ui/tag"
utils_dir = "src/ui/utils"
assets_dir = "src/ui/assets"

[registry]
repo = "example/repo"
branch = "dev"
"#;
        fs::write(&path, toml_str).unwrap();

        let setting = Setting::load_from(&path).unwrap();
        assert_eq!(setting.project.base_dir, "src/ui");
        assert_eq!(setting.registry.repo, "example/repo");
        assert_eq!(setting.registry.branch, "dev");

        // [installed] 省略時は #[serde(default)] によりすべて空になること
        assert!(setting.installed.components.is_empty());
        assert!(setting.installed.themes.is_empty());
        assert!(setting.installed.tags.is_empty());
        assert!(setting.installed.utils.is_empty());
        assert!(setting.installed.assets.is_empty());
    }

    #[test]
    fn test_load_from_invalid_toml_returns_err() {
        let dir = TestDir::new("invalid");

        // 必須セクション不足
        let missing_section = dir.file_path("missing_section.toml");
        fs::write(
            &missing_section,
            r#"
[registry]
repo = "eto0202/michiu"
branch = "main"
"#,
        )
        .unwrap();
        assert!(Setting::load_from(&missing_section).is_err());

        // TOML 構文エラー
        let syntax_err = dir.file_path("syntax_err.toml");
        fs::write(&syntax_err, "[project\nbroken =").unwrap();
        assert!(Setting::load_from(&syntax_err).is_err());
    }

    #[test]
    fn test_save_to_and_load_from_round_trip() {
        let dir = TestDir::new("roundtrip");
        let path = dir.file_path(CONFIG_FILE_NAME);

        let mut installed = InstalledConfig::default();
        installed
            .components
            .insert("button".to_string(), "20260101-abc1234".to_string());
        installed
            .themes
            .insert("dark".to_string(), "20260102-def5678".to_string());
        installed
            .tags
            .insert("badge".to_string(), "20260103-1112223".to_string());
        installed
            .utils
            .insert("cn".to_string(), "20260104-4445556".to_string());
        installed.assets.insert(
            "icon.svg".to_string(),
            AssetInstalled {
                hash: "deadbeef".to_string(),
                used_by: vec!["button".to_string(), "tabs".to_string()],
            },
        );

        let original = Setting {
            project: ProjectConfig::default(),
            registry: RegistryConfig::default(),
            installed,
        };

        original.save_to(&path).unwrap();
        assert!(path.exists());

        let loaded = Setting::load_from(&path).unwrap();
        assert_eq!(loaded.project, original.project);
        assert_eq!(loaded.registry, original.registry);
        assert_eq!(loaded.installed.components, original.installed.components);
        assert_eq!(loaded.installed.themes, original.installed.themes);
        assert_eq!(loaded.installed.tags, original.installed.tags);
        assert_eq!(loaded.installed.utils, original.installed.utils);
        assert_eq!(loaded.installed.assets, original.installed.assets);
    }

    #[test]
    fn test_save_to_nonexistent_parent_returns_err() {
        let dir = TestDir::new("no_parent");
        let bad_path = dir.file_path("nonexistent_dir/iu_ui.toml");

        let setting = Setting {
            project: ProjectConfig::default(),
            registry: RegistryConfig::default(),
            installed: InstalledConfig::default(),
        };

        assert!(setting.save_to(&bad_path).is_err());
    }
}

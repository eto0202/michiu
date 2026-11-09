use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::{fs, path::Path};

pub const CONFIG_FILE_NAME: &str = "iu_ui.toml";

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Setting {
    pub project: ProjectConfig,
    pub registry: RegistryConfig,
    #[serde(default)]
    pub installed: InstalledConfig,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
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

#[derive(Serialize, Deserialize, Debug, Clone)]
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

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AssetInstalled {
    pub hash: String,
    pub used_by: Vec<String>,
}

impl Setting {
    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        let path = Path::new(CONFIG_FILE_NAME);
        if !path.exists() {
            return Err(format!(
                "{} was not found. Please run `cargo iu_ui init` first.",
                CONFIG_FILE_NAME
            )
            .into());
        }
        let content = fs::read_to_string(path)?;
        let setting: Setting = toml::from_str(&content)?;
        Ok(setting)
    }

    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let content = toml::to_string_pretty(self)?;
        fs::write(CONFIG_FILE_NAME, content)?;
        Ok(())
    }
}

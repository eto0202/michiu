use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Deserialize, Debug, Clone)]
pub struct Registry {
    pub name: String,
    pub updated_at: String,
    #[serde(default)]
    pub assets: BTreeMap<String, AssetEntry>,
    #[serde(default)]
    pub components: BTreeMap<String, ComponentEntry>,
    #[serde(default)]
    pub themes: BTreeMap<String, SimpleEntry>,
    #[serde(default)]
    pub tags: BTreeMap<String, SimpleEntry>,
    #[serde(default)]
    pub utils: BTreeMap<String, SimpleEntry>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct AssetEntry {
    pub path: String,
    pub hash: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ComponentEntry {
    #[serde(rename = "type")]
    pub comp_type: String, // "file" or "dir"
    pub version: String,
    pub target_name: Option<String>,
    pub target_dir: Option<String>,
    pub root_dir: Option<String>,
    pub files: Vec<String>,
    #[serde(default)]
    pub assets: Vec<String>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct SimpleEntry {
    pub version: String,
    pub target_name: String,
    pub files: Vec<String>,
}

impl Registry {
    /// Download and parse `registry.json` from GitHub Raw
    pub fn fetch(repo: &str, branch: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let url = format!(
            "https://raw.githubusercontent.com/{}/{}/registry.json",
            repo, branch
        );

        let mut response = ureq::get(&url)
            .header("User-Agent", "cargo-iu_ui")
            .call()
            .map_err(|e| format!("Failed to fetch registry from {}: {}", url, e))?;

        let registry: Registry = response.body_mut().read_json()?;
        Ok(registry)
    }

    /// Retrieving the raw text of a specific file from GitHub Raw
    pub fn fetch_raw_file(
        repo: &str,
        branch: &str,
        file_path: &str,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let url = format!(
            "https://raw.githubusercontent.com/{}/{}/{}",
            repo, branch, file_path
        );

        let mut response = ureq::get(&url)
            .header("User-Agent", "cargo-iu_ui")
            .call()
            .map_err(|e| format!("Failed to fetch file from {}: {}", url, e))?;

        Ok(response.body_mut().read_to_string()?)
    }

    /// Retrieving Binary Files from GitHub Raw
    pub fn fetch_raw_bytes(
        repo: &str,
        branch: &str,
        file_path: &str,
    ) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let url = format!(
            "https://raw.githubusercontent.com/{}/{}/{}",
            repo, branch, file_path
        );

        let response = ureq::get(&url)
            .header("User-Agent", "cargo-iu_ui")
            .call()
            .map_err(|e| format!("Failed to fetch asset from {}: {}", url, e))?;

        let mut reader = response.into_body().into_reader();
        let mut bytes = Vec::new();

        std::io::copy(&mut reader, &mut bytes)?;
        Ok(bytes)
    }
}

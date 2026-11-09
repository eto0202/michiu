use regex::Regex;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use std::process::Command;
use walkdir::WalkDir;

#[derive(Serialize)]
struct Registry {
    name: String,
    updated_at: String,
    assets: BTreeMap<String, AssetEntry>,
    components: BTreeMap<String, ComponentEntry>,
    themes: BTreeMap<String, SimpleEntry>,
    tags: BTreeMap<String, SimpleEntry>,
    utils: BTreeMap<String, SimpleEntry>,
}

#[derive(Serialize)]
struct AssetEntry {
    path: String,
    hash: String,
}

#[derive(Serialize)]
struct ComponentEntry {
    #[serde(rename = "type")]
    comp_type: String, // "file" or "dir"
    version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    root_dir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    target_dir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    target_name: Option<String>,
    files: Vec<String>,
    assets: Vec<String>,
}

#[derive(Serialize)]
struct SimpleEntry {
    version: String,
    target_name: String,
    files: Vec<String>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Scanning repository...");

    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .expect("Failed to find workspace root");

    let assets = collect_assets(workspace_root, &workspace_root.join("iu_ui/src/assets"))?;
    let components =
        collect_components(workspace_root, &workspace_root.join("iu_ui/src/components"))?;
    let themes = collect_simple_modules(workspace_root, &workspace_root.join("iu_ui/src/theme"))?;
    let tags = collect_simple_modules(workspace_root, &workspace_root.join("iu_ui/src/tag"))?;
    let utils = collect_simple_modules(workspace_root, &workspace_root.join("iu_ui/src/utils"))?;

    let registry = Registry {
        name: "iu_ui".to_string(),
        updated_at: get_current_date(),
        assets,
        components,
        themes,
        tags,
        utils,
    };

    let output_path = workspace_root.join("registry.json");
    let json = serde_json::to_string_pretty(&registry)?;
    fs::write(&output_path, json)?;

    println!("Successfully generated registry.json");
    Ok(())
}

// 絶対パスから workspace_root を取り除いて、URL用のスラッシュ(/)区切りにする
fn to_rel_path(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_str()
        .unwrap()
        .replace('\\', "/")
}

// --- Git から "YYYYMMDD-hash" を取得 ---
fn get_git_version(path: &Path) -> String {
    let output = Command::new("git")
        .args([
            "log",
            "-1",
            "--format=%cd-%h",
            "--date=format:%Y%m%d",
            "--",
            path.to_str().unwrap(),
        ])
        .output();

    match output {
        Ok(out) if out.status.success() => {
            let res = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if res.is_empty() {
                "uncommitted".to_string()
            } else {
                res
            }
        }
        _ => "unknown".to_string(),
    }
}

// 今日の日付 (YYYY-MM-DD)
fn get_current_date() -> String {
    let output = Command::new("git")
        .args(["log", "-1", "--format=%cI"])
        .output();
    if let Ok(out) = output {
        let date = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !date.is_empty() {
            return date;
        }
    }
    "uncommitted".to_string()
}

// ファイルの SHA-256 ハッシュを計算
fn calculate_sha256(path: &Path) -> Result<String, Box<dyn std::error::Error>> {
    let bytes = std::fs::read(path)?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let result = hasher.finalize();

    // 各バイトを 2 桁の小文字 16 進数 (02x) に変換して結合
    let hex = result
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect::<String>();
    Ok(hex)
}

// --- Assets 収集 ---
fn collect_assets(
    root: &Path,
    dir_path: &Path,
) -> Result<BTreeMap<String, AssetEntry>, Box<dyn std::error::Error>> {
    let mut assets = BTreeMap::new();
    if !dir_path.exists() {
        return Ok(assets);
    }

    for entry in fs::read_dir(dir_path)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            let filename = path.file_name().unwrap().to_str().unwrap().to_string();
            let hash = calculate_sha256(&path)?;
            assets.insert(
                filename,
                AssetEntry {
                    path: to_rel_path(&path, root),
                    hash,
                },
            );
        }
    }
    Ok(assets)
}

// --- Components 収集 ---
fn collect_components(
    root: &Path,
    dir_path: &Path,
) -> Result<BTreeMap<String, ComponentEntry>, Box<dyn std::error::Error>> {
    let mut components = BTreeMap::new();
    if !dir_path.exists() {
        return Ok(components);
    }

    // コード内の `../assets/xxx` または `../../assets/xxx` を検知する正規表現
    let asset_regex = Regex::new(r#"assets/([^"\s\)]+)"#).unwrap();

    for entry in fs::read_dir(dir_path)? {
        let entry = entry?;
        let path = entry.path();
        let name = path.file_stem().unwrap().to_str().unwrap().to_string();

        if name == "mod" {
            continue; // mod.rs はスキップ
        }

        let version = get_git_version(&path);

        if path.is_file() && path.extension().is_some_and(|ext| ext == "rs") {
            let content = fs::read_to_string(&path).unwrap_or_default();
            let detected_assets = find_assets_in_code(&content, &asset_regex);

            components.insert(
                name.clone(),
                ComponentEntry {
                    comp_type: "file".to_string(),
                    version,
                    root_dir: None,
                    target_dir: None,
                    target_name: Some(format!("{}.rs", name)),
                    files: vec![to_rel_path(&path, root)],
                    assets: detected_assets,
                },
            );
        } else if path.is_dir() {
            // フォルダモジュールコンポーネント (例: tabs/)
            let comp_name = name;
            let mut files = Vec::new();
            let mut detected_assets = BTreeSet::new();

            for file_entry in WalkDir::new(&path).into_iter().filter_map(|e| e.ok()) {
                if file_entry.path().is_file() {
                    let rel_path = file_entry
                        .path()
                        .strip_prefix(&path)?
                        .to_str()
                        .unwrap()
                        .replace('\\', "/");
                    files.push(rel_path);

                    let content = fs::read_to_string(file_entry.path()).unwrap_or_default();
                    for a in find_assets_in_code(&content, &asset_regex) {
                        detected_assets.insert(a);
                    }
                }
            }
            files.sort();

            components.insert(
                comp_name.clone(),
                ComponentEntry {
                    comp_type: "dir".to_string(),
                    version,
                    root_dir: Some(to_rel_path(&path, root)),
                    target_dir: Some(comp_name),
                    target_name: None,
                    files,
                    assets: detected_assets.into_iter().collect(),
                },
            );
        }
    }

    Ok(components)
}

// --- Theme / Tag / Utils 収集 ---
fn collect_simple_modules(
    root: &Path,
    dir_path: &Path,
) -> Result<BTreeMap<String, SimpleEntry>, Box<dyn std::error::Error>> {
    let mut map = BTreeMap::new();
    if !dir_path.exists() {
        return Ok(map);
    }

    for entry in fs::read_dir(dir_path)? {
        let entry = entry?;
        let path = entry.path();
        let stem = path.file_stem().unwrap().to_str().unwrap().to_string();

        if stem == "mod" || !path.is_file() {
            continue;
        }

        let version = get_git_version(&path);
        map.insert(
            stem.clone(),
            SimpleEntry {
                version,
                target_name: format!("{}.rs", stem),
                files: vec![to_rel_path(&path, root)],
            },
        );
    }

    Ok(map)
}

fn find_assets_in_code(content: &str, re: &Regex) -> Vec<String> {
    let mut set = BTreeSet::new();
    for cap in re.captures_iter(content) {
        if let Some(matched) = cap.get(1) {
            set.insert(matched.as_str().to_string());
        }
    }
    set.into_iter().collect()
}

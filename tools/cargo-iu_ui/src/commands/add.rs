use crate::config::{AssetInstalled, Setting};
use crate::registry::{ComponentEntry, Registry};
use colored::*;
use inquire::{Select, Text};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

enum ConflictAction {
    Overwrite,
    Skip,
    Rename,
}

pub fn execute(components: Vec<String>, all: bool) -> Result<(), Box<dyn std::error::Error>> {
    let mut setting = Setting::load()?;
    let registry = Registry::fetch(&setting.registry.repo, &setting.registry.branch)?;

    // 対象コンポーネントのリストアップ
    let target_names: Vec<String> = if all {
        registry.components.keys().cloned().collect()
    } else {
        if components.is_empty() {
            println!(
                "{}",
                "[Warning] Please specify the name of the component you want to add (e.g., cargo iu_ui add simple_button)"
                    .yellow()
            );
            return Ok(());
        }
        components
    };

    let repo = setting.registry.repo.clone();
    let branch = setting.registry.branch.clone();
    let comp_base_dir = PathBuf::from(&setting.project.components_dir);
    let assets_base_dir = PathBuf::from(&setting.project.assets_dir);

    for comp_name in &target_names {
        println!("\nProcessing {}...", comp_name.bold().cyan());

        let comp_def = match registry.components.get(comp_name) {
            Some(def) => def,
            None => {
                println!(
                    "  {} Not found in the catalog. Skipping.",
                    "[Warning] ".yellow()
                );
                continue;
            }
        };

        // インストール処理 (ファイル or ディレクトリ)
        let installed_module_name =
            match install_component(comp_name, comp_def, &comp_base_dir, &repo, &branch)? {
                Some(name) => name,
                None => {
                    println!("  {} Skipped", "↪".dimmed());
                    continue;
                }
            };

        // アセットの解決 (自動ダウンロード・共有判定)
        for assets_name in &comp_def.assets {
            resolve_asset(
                assets_name,
                &installed_module_name,
                &registry,
                &assets_base_dir,
                &mut setting,
                &repo,
                &branch,
            )?;
        }

        register_in_mod_rs(&comp_base_dir, &installed_module_name)?;

        setting
            .installed
            .components
            .insert(comp_name.clone(), comp_def.version.clone());

        println!(
            "  {} {} was added successfully！",
            "[Success]".green(),
            comp_name.bold()
        );
    }

    setting.save()?;
    println!("\n{}", "All selected components processed!".green().bold());
    Ok(())
}

// 単一ファイル、フォルダの配置処理
fn install_component(
    comp_name: &str,
    def: &ComponentEntry,
    dest_base: &Path,
    repo: &str,
    branch: &str,
) -> Result<Option<String>, Box<dyn std::error::Error>> {
    let is_dir = def.comp_type == "dir";
    let default_target_name = if is_dir {
        def.target_dir.as_deref().unwrap_or(comp_name)
    } else {
        def.target_name.as_deref().unwrap_or(comp_name)
    };

    let mut target_name = default_target_name.to_string();
    let mut dest_path = dest_base.join(&target_name);

    // 競合チェック
    if dest_path.exists() {
        println!(
            "  {} '{}' already exists.",
            "[Warning]".yellow(),
            target_name
        );
        let options = vec!["Skip", "Overwrite", "Rename"];
        let choice = Select::new("What would you like to do?", options).prompt()?;

        match choice {
            "Skip" => return Ok(None),
            "Overwrite" => {}
            "Rename" => {
                let new_name = Text::new("Please enter a new name (without a file extension):")
                    .with_default(&format!("{}_custom", comp_name))
                    .prompt()?;
                target_name = if is_dir {
                    new_name.clone()
                } else {
                    format!("{}.rs", new_name)
                };
                dest_path = dest_base.join(&target_name);
            }
            _ => return Ok(None),
        }
    }

    // ダウンロード & 保存
    if is_dir {
        fs::create_dir_all(&dest_path)?;
        let root_dir = def.root_dir.as_deref().unwrap_or("");
        for rel_file in &def.files {
            let file_dest = dest_path.join(rel_file);
            if let Some(parent) = file_dest.parent() {
                fs::create_dir_all(parent)?;
            }
            let remote_file_path = format!("{}/{}", root_dir, rel_file);
            let content = Registry::fetch_raw_file(repo, branch, &remote_file_path)?;
            fs::write(file_dest, content)?;
        }
    } else {
        let remote_file_path = &def.files[0];
        let content = Registry::fetch_raw_file(repo, branch, remote_file_path)?;
        fs::write(&dest_path, content)?;
    }

    let module_name = if is_dir {
        target_name
    } else {
        target_name.trim_end_matches(".rs").to_string()
    };

    Ok(Some(module_name))
}

// アセットの共有・ダウンロード処理
fn resolve_asset(
    assets_name: &str,
    comp_name: &str,
    registry: &Registry,
    assets_dir: &Path,
    setting: &mut Setting,
    repo: &str,
    branch: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let asset_def = match registry.assets.get(assets_name) {
        Some(def) => def,
        None => return Ok(()),
    };

    let local_asset_path = assets_dir.join(assets_name);
    let mut need_download = true;

    if local_asset_path.exists() {
        // ハッシュ値を比較
        let local_bytes = fs::read(&local_asset_path)?;
        let mut hasher = Sha256::default();
        hasher.update(&local_bytes);
        let local_hash = hasher
            .finalize()
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect::<String>();

        if local_hash == asset_def.hash {
            println!(
                "  {} Asset '{}' already exists and contents match. Sharing.",
                "[Note]".blue(),
                assets_name
            );
            need_download = false;
        } else {
            println!(
                "  {} Asset '{}' already exists, but the content differs.",
                "[Warning]".yellow(),
                assets_name
            );
            let choice = Select::new(
                "Please resolve the asset conflicts:",
                vec!["Skip", "Overwrite"],
            )
            .prompt()?;
            if choice == "Skip" {
                need_download = false;
            }
        }
    }

    if need_download {
        println!("  {} Asset '{}' Downloading...", "↓".cyan(), assets_name);
        let bytes = Registry::fetch_raw_bytes(repo, branch, &asset_def.path)?;
        fs::write(&local_asset_path, bytes)?;
    }

    // setting.toml の used_by を更新
    let entry = setting
        .installed
        .assets
        .entry(assets_name.to_string())
        .or_insert_with(|| AssetInstalled {
            hash: asset_def.hash.clone(),
            used_by: Vec::new(),
        });

    if !entry.used_by.contains(&comp_name.to_string()) {
        entry.used_by.push(comp_name.to_string());
    }

    Ok(())
}

// components/mod.rs に `pub mod <module>;` を自動追記
fn register_in_mod_rs(components_dir: &Path, module_name: &str) -> std::io::Result<()> {
    let mod_path = components_dir.join("mod.rs");
    let content = fs::read_to_string(&mod_path).unwrap_or_default();
    let mod_line = format!("pub mod {};", module_name);

    if !content.lines().any(|l| l.trim() == mod_line) {
        let mut new_content = content;
        if !new_content.ends_with('\n') && !new_content.is_empty() {
            new_content.push('\n');
        }
        new_content.push_str(&format!("{}\n", mod_line));
        fs::write(&mod_path, new_content)?;
        println!(
            "  {} Added '{}' to components/mod.rs.",
            "[Success]".green(),
            mod_line
        );
    }
    Ok(())
}

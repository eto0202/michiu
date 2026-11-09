use crate::{
    config::Setting,
    registry::{Registry, SimpleEntry},
};
use colored::*;
use inquire::{Select, Text};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

pub enum ResourceKind {
    Theme,
    Tag,
    Utils,
}

impl ResourceKind {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Theme => "theme",
            Self::Tag => "tag",
            Self::Utils => "utils",
        }
    }
}

pub fn execute(
    kind: ResourceKind,
    targets: Vec<String>,
    all: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut setting = Setting::load()?;
    let registry = Registry::fetch(&setting.registry.repo, &setting.registry.branch)?;

    // 対象レジストリマップの取得
    let catalog_map: &BTreeMap<String, SimpleEntry> = match kind {
        ResourceKind::Theme => &registry.themes,
        ResourceKind::Tag => &registry.tags,
        ResourceKind::Utils => &registry.utils,
    };

    let target_names: Vec<String> = if all {
        catalog_map.keys().cloned().collect()
    } else {
        if targets.is_empty() {
            println!(
                "{}",
                format!(
                    "[Warning] Please specify a name to add (e.g., cargo iu_ui {} <name>))",
                    kind.name()
                )
                .yellow()
            );
            return Ok(());
        }
        targets
    };

    let dest_dir = match kind {
        ResourceKind::Theme => PathBuf::from(&setting.project.theme_dir),
        ResourceKind::Tag => PathBuf::from(&setting.project.tag_dir),
        ResourceKind::Utils => PathBuf::from(&setting.project.utils_dir),
    };

    let repo = setting.registry.repo.clone();
    let branch = setting.registry.branch.clone();

    for name in &target_names {
        println!("\nProcessing {} '{}'...", kind.name().cyan(), name.bold());

        let entry = match catalog_map.get(name) {
            Some(e) => e,
            None => {
                println!(
                    "  {} Not found in the catalog. Skipping.",
                    "[Warning]".yellow()
                );
                continue;
            }
        };

        // ファイル名の決定 & 競合処理
        let mut target_filename = entry.target_name.clone();
        let mut dest_file = dest_dir.join(&target_filename);

        if dest_file.exists() {
            println!(
                "  {} '{}' already exists.",
                "[Warning]".yellow(),
                target_filename
            );
            let options = vec!["Skip", "Overwrite", "Rename"];
            let choice = Select::new("What would you like to do?", options).prompt()?;

            match choice {
                "Skip" => {
                    println!("  {} Skipped", "↪".dimmed());
                    continue;
                }
                "Overwrite" => {}
                "Rename" => {
                    let new_stem = Text::new("Please enter a new name (without a file extension):")
                        .with_default(&format!("{}_custom", name))
                        .prompt()?;
                    target_filename = format!("{}.rs", new_stem);
                    dest_file = dest_dir.join(&target_filename);
                }
                _ => continue,
            }
        }

        // ダウンロード & 保存
        let remote_path = &entry.files[0];
        let content = Registry::fetch_raw_file(&repo, &branch, remote_path)?;
        fs::write(&dest_file, content)?;

        // そのディレクトリの mod.rs に pub mod <name>; を追記
        let mod_name = target_filename.trim_end_matches(".rs");
        register_in_sub_mod(&dest_dir, mod_name)?;

        // setting.toml にバージョン記録
        match kind {
            ResourceKind::Theme => {
                setting
                    .installed
                    .themes
                    .insert(mod_name.to_string(), entry.version.clone());
            }
            ResourceKind::Tag => {
                setting
                    .installed
                    .tags
                    .insert(mod_name.to_string(), entry.version.clone());
            }
            ResourceKind::Utils => {
                setting
                    .installed
                    .utils
                    .insert(mod_name.to_string(), entry.version.clone());
            }
        }

        println!(
            "  {} Successfully added {}!",
            "[Success]".green(),
            mod_name.bold()
        );
    }

    setting.save()?;
    println!(
        "\n{}",
        format!("All {} processed!", kind.name()).green().bold()
    );
    Ok(())
}

fn register_in_sub_mod(dir: &Path, module_name: &str) -> std::io::Result<()> {
    let mod_path = dir.join("mod.rs");
    let content = fs::read_to_string(&mod_path).unwrap_or_default();
    let mod_line = format!("pub mod {};", module_name);

    if !content.lines().any(|l| l.trim() == mod_line) {
        let mut new_content = content;
        if !new_content.ends_with('\n') && !new_content.is_empty() {
            new_content.push('\n');
        }
        new_content.push_str(&format!("{}\n", mod_line));
        fs::write(&mod_path, new_content)?;
        println!("{} Added {} to mod.rs", "[Success]".green(), mod_line);
    }
    Ok(())
}

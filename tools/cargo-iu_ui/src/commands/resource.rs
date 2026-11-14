use crate::{
    config::{ProjectConfig, Setting},
    registry::{Registry, SimpleEntry},
};
use colored::Colorize;
use inquire::{Select, Text};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, PartialEq)]
pub enum ResourceKind {
    Theme,
    Tag,
    Utils,
}

impl ResourceKind {
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Theme => "theme",
            Self::Tag => "tag",
            Self::Utils => "utils",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum ConflictAction {
    Overwrite,
    Skip,
    Rename(String),
}

#[allow(clippy::too_many_lines)]
pub fn execute(
    kind: &ResourceKind,
    targets: Vec<String>,
    all: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut setting = Setting::load()?;
    let registry = Registry::fetch(&setting.registry.repo, &setting.registry.branch)?;

    let processed = add_resources_with(
        kind,
        targets,
        all,
        &mut setting,
        &registry,
        &mut Registry::fetch_raw_file,
        &mut prompt_conflict,
    )?;

    if !processed {
        return Ok(());
    }

    setting.save()?;
    println!(
        "\n{}",
        format!("All {} processed!", kind.name()).green().bold()
    );
    Ok(())
}

fn dest_dir_for(kind: &ResourceKind, project: &ProjectConfig) -> PathBuf {
    match kind {
        ResourceKind::Theme => PathBuf::from(&project.theme_dir),
        ResourceKind::Tag => PathBuf::from(&project.tag_dir),
        ResourceKind::Utils => PathBuf::from(&project.utils_dir),
    }
}

fn add_resources_with<F, C>(
    kind: &ResourceKind,
    targets: Vec<String>,
    all: bool,
    setting: &mut Setting,
    registry: &Registry,
    fetch_file: &mut F,
    on_conflict: &mut C,
) -> Result<bool, Box<dyn std::error::Error>>
where
    F: FnMut(&str, &str, &str) -> Result<String, Box<dyn std::error::Error>>,
    C: FnMut(&str, &str) -> Result<ConflictAction, Box<dyn std::error::Error>>,
{
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
            return Ok(false);
        }
        targets
    };

    let dest_dir = dest_dir_for(kind, &setting.project);
    let repo = setting.registry.repo.clone();
    let branch = setting.registry.branch.clone();

    for name in &target_names {
        println!("\nProcessing {} '{}'...", kind.name().cyan(), name.bold());

        let Some(entry) = catalog_map.get(name) else {
            println!(
                "  {} Not found in the catalog. Skipping.",
                "[Warning]".yellow()
            );
            continue;
        };

        // ファイル名の決定 & 競合処理
        let mut target_filename = entry.target_name.clone();
        let mut dest_file = dest_dir.join(&target_filename);

        if dest_file.exists() {
            match on_conflict(&target_filename, name)? {
                ConflictAction::Skip => {
                    println!("  {} Skipped", "↪".dimmed());
                    continue;
                }
                ConflictAction::Overwrite => {}
                ConflictAction::Rename(new_stem) => {
                    target_filename = format!("{new_stem}.rs");
                    dest_file = dest_dir.join(&target_filename);
                }
            }
        }

        // ダウンロード & 保存
        let remote_path = &entry.files[0];
        let content = fetch_file(&repo, &branch, remote_path)?;
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

    Ok(true)
}

fn prompt_conflict(
    target_filename: &str,
    name: &str,
) -> Result<ConflictAction, Box<dyn std::error::Error>> {
    println!(
        "  {} '{}' already exists.",
        "[Warning]".yellow(),
        target_filename
    );
    let options = vec!["Skip", "Overwrite", "Rename"];
    let choice = Select::new("What would you like to do?", options).prompt()?;

    match choice {
        "Overwrite" => Ok(ConflictAction::Overwrite),
        "Rename" => {
            let new_stem = Text::new("Please enter a new name (without a file extension):")
                .with_default(&format!("{name}_custom"))
                .prompt()?;
            Ok(ConflictAction::Rename(new_stem))
        }
        _ => Ok(ConflictAction::Skip),
    }
}

#[allow(clippy::format_push_string)]
fn register_in_sub_mod(dir: &Path, module_name: &str) -> std::io::Result<()> {
    let mod_path = dir.join("mod.rs");
    let content = fs::read_to_string(&mod_path).unwrap_or_default();
    let mod_line = format!("pub mod {module_name};");

    if !content.lines().any(|l| l.trim() == mod_line) {
        let mut new_content = content;
        if !new_content.ends_with('\n') && !new_content.is_empty() {
            new_content.push('\n');
        }
        new_content.push_str(&format!("{mod_line}\n"));
        fs::write(&mod_path, new_content)?;
        println!("{} Added {} to mod.rs", "[Success]".green(), mod_line);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{InstalledConfig, RegistryConfig};
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct TestDir {
        path: PathBuf,
    }

    impl TestDir {
        fn new(prefix: &str) -> Self {
            static COUNTER: AtomicUsize = AtomicUsize::new(0);
            let id = COUNTER.fetch_add(1, Ordering::Relaxed);
            let pid = std::process::id();
            let path = std::env::temp_dir().join(format!("iu_ui_res_test_{prefix}_{pid}_{id}"));
            if path.exists() {
                let _ = fs::remove_dir_all(&path);
            }
            fs::create_dir_all(&path).expect("failed to create test dir");
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn make_setting(root: &Path) -> Setting {
        let theme_dir = root.join("theme");
        let tag_dir = root.join("tag");
        let utils_dir = root.join("utils");
        fs::create_dir_all(&theme_dir).unwrap();
        fs::create_dir_all(&tag_dir).unwrap();
        fs::create_dir_all(&utils_dir).unwrap();

        Setting {
            project: ProjectConfig {
                base_dir: root.to_string_lossy().into_owned(),
                components_dir: root.join("components").to_string_lossy().into_owned(),
                theme_dir: theme_dir.to_string_lossy().into_owned(),
                tag_dir: tag_dir.to_string_lossy().into_owned(),
                utils_dir: utils_dir.to_string_lossy().into_owned(),
                assets_dir: root.join("assets").to_string_lossy().into_owned(),
            },
            registry: RegistryConfig {
                repo: "owner/repo".to_string(),
                branch: "main".to_string(),
            },
            installed: InstalledConfig::default(),
        }
    }

    fn simple_entry(version: &str, target_name: &str, file: &str) -> SimpleEntry {
        SimpleEntry {
            version: version.to_string(),
            target_name: target_name.to_string(),
            files: vec![file.to_string()],
        }
    }

    fn make_registry() -> Registry {
        let mut themes = BTreeMap::new();
        themes.insert(
            "dark".to_string(),
            simple_entry("20260101-aaa", "dark.rs", "iu_ui/src/theme/dark.rs"),
        );
        themes.insert(
            "light".to_string(),
            simple_entry("20260102-bbb", "light.rs", "iu_ui/src/theme/light.rs"),
        );

        let mut tags = BTreeMap::new();
        tags.insert(
            "badge".to_string(),
            simple_entry("20260103-ccc", "badge.rs", "iu_ui/src/tag/badge.rs"),
        );

        let mut utils = BTreeMap::new();
        utils.insert(
            "cn".to_string(),
            simple_entry("20260104-ddd", "cn.rs", "iu_ui/src/utils/cn.rs"),
        );

        Registry {
            name: "iu_ui".to_string(),
            updated_at: "2026-11-14".to_string(),
            assets: BTreeMap::new(),
            components: BTreeMap::new(),
            themes,
            tags,
            utils,
        }
    }

    #[test]
    fn test_resource_kind_name() {
        assert_eq!(ResourceKind::Theme.name(), "theme");
        assert_eq!(ResourceKind::Tag.name(), "tag");
        assert_eq!(ResourceKind::Utils.name(), "utils");
    }

    #[test]
    fn test_register_in_sub_mod() {
        let dir = TestDir::new("sub_mod");
        let target_dir = dir.path();

        register_in_sub_mod(target_dir, "dark").unwrap();
        assert_eq!(
            fs::read_to_string(target_dir.join("mod.rs")).unwrap(),
            "pub mod dark;\n"
        );

        // 末尾改行なしから追記しても改行が入ること & 重複しないこと
        fs::write(target_dir.join("mod.rs"), "pub mod dark;").unwrap();
        register_in_sub_mod(target_dir, "light").unwrap();
        register_in_sub_mod(target_dir, "dark").unwrap();
        assert_eq!(
            fs::read_to_string(target_dir.join("mod.rs")).unwrap(),
            "pub mod dark;\npub mod light;\n"
        );
    }

    #[test]
    fn test_add_resources_empty_targets_returns_false() {
        let dir = TestDir::new("empty");
        let mut setting = make_setting(dir.path());
        let registry = make_registry();

        let processed = add_resources_with(
            &ResourceKind::Theme,
            vec![],
            false,
            &mut setting,
            &registry,
            &mut |_, _, _| panic!("should not fetch"),
            &mut |_, _| panic!("should not prompt"),
        )
        .unwrap();

        assert!(!processed);
        assert!(setting.installed.themes.is_empty());
    }

    #[test]
    fn test_add_resources_for_each_kind() {
        let dir = TestDir::new("kinds");
        let mut setting = make_setting(dir.path());
        let registry = make_registry();

        // Theme (存在しない名前が混ざってもスキップして続行)
        let processed = add_resources_with(
            &ResourceKind::Theme,
            vec!["missing".to_string(), "dark".to_string()],
            false,
            &mut setting,
            &registry,
            &mut |_, _, path| Ok(format!("// {path}")),
            &mut |_, _| panic!("should not conflict"),
        )
        .unwrap();
        assert!(processed);
        assert!(dir.path().join("theme/dark.rs").exists());
        assert_eq!(
            setting.installed.themes.get("dark").map(String::as_str),
            Some("20260101-aaa")
        );
        assert!(setting.installed.tags.is_empty());
        assert!(setting.installed.utils.is_empty());

        // Tag
        add_resources_with(
            &ResourceKind::Tag,
            vec!["badge".to_string()],
            false,
            &mut setting,
            &registry,
            &mut |_, _, path| Ok(format!("// {path}")),
            &mut |_, _| panic!("should not conflict"),
        )
        .unwrap();
        assert!(dir.path().join("tag/badge.rs").exists());
        assert_eq!(
            setting.installed.tags.get("badge").map(String::as_str),
            Some("20260103-ccc")
        );

        // Utils
        add_resources_with(
            &ResourceKind::Utils,
            vec!["cn".to_string()],
            false,
            &mut setting,
            &registry,
            &mut |_, _, path| Ok(format!("// {path}")),
            &mut |_, _| panic!("should not conflict"),
        )
        .unwrap();
        assert!(dir.path().join("utils/cn.rs").exists());
        assert_eq!(
            setting.installed.utils.get("cn").map(String::as_str),
            Some("20260104-ddd")
        );
    }

    #[test]
    fn test_add_resources_all_flag() {
        let dir = TestDir::new("all");
        let mut setting = make_setting(dir.path());
        let registry = make_registry();

        let processed = add_resources_with(
            &ResourceKind::Theme,
            vec![],
            true,
            &mut setting,
            &registry,
            &mut |_, _, path| Ok(format!("// {path}")),
            &mut |_, _| panic!("should not conflict"),
        )
        .unwrap();

        assert!(processed);
        assert!(dir.path().join("theme/dark.rs").exists());
        assert!(dir.path().join("theme/light.rs").exists());
        assert_eq!(setting.installed.themes.len(), 2);
        assert_eq!(
            fs::read_to_string(dir.path().join("theme/mod.rs")).unwrap(),
            "pub mod dark;\npub mod light;\n"
        );
    }

    #[test]
    fn test_add_resources_conflict_skip_overwrite_rename() {
        let dir = TestDir::new("conflict");
        let mut setting = make_setting(dir.path());
        let registry = make_registry();
        let dark_path = dir.path().join("theme/dark.rs");

        // 既存ファイルを用意
        fs::write(&dark_path, "old_dark").unwrap();

        // Skip -> ファイルも installed も変更されない
        add_resources_with(
            &ResourceKind::Theme,
            vec!["dark".to_string()],
            false,
            &mut setting,
            &registry,
            &mut |_, _, _| panic!("should not fetch on skip"),
            &mut |filename, name| {
                assert_eq!(filename, "dark.rs");
                assert_eq!(name, "dark");
                Ok(ConflictAction::Skip)
            },
        )
        .unwrap();
        assert_eq!(fs::read_to_string(&dark_path).unwrap(), "old_dark");
        assert!(!setting.installed.themes.contains_key("dark"));

        // Rename -> 新しい名前で保存され、mod.rs と installed にも新名前で記録される
        add_resources_with(
            &ResourceKind::Theme,
            vec!["dark".to_string()],
            false,
            &mut setting,
            &registry,
            &mut |_, _, _| Ok("custom_dark_content".to_string()),
            &mut |_, _| Ok(ConflictAction::Rename("custom_dark".to_string())),
        )
        .unwrap();
        assert_eq!(fs::read_to_string(&dark_path).unwrap(), "old_dark");
        assert_eq!(
            fs::read_to_string(dir.path().join("theme/custom_dark.rs")).unwrap(),
            "custom_dark_content"
        );
        assert_eq!(
            fs::read_to_string(dir.path().join("theme/mod.rs")).unwrap(),
            "pub mod custom_dark;\n"
        );
        assert_eq!(
            setting
                .installed
                .themes
                .get("custom_dark")
                .map(String::as_str),
            Some("20260101-aaa")
        );

        // Overwrite -> 既存ファイルが上書きされる
        add_resources_with(
            &ResourceKind::Theme,
            vec!["dark".to_string()],
            false,
            &mut setting,
            &registry,
            &mut |_, _, _| Ok("new_dark_content".to_string()),
            &mut |_, _| Ok(ConflictAction::Overwrite),
        )
        .unwrap();
        assert_eq!(fs::read_to_string(&dark_path).unwrap(), "new_dark_content");
        assert_eq!(
            setting.installed.themes.get("dark").map(String::as_str),
            Some("20260101-aaa")
        );
    }
}

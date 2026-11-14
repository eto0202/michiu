use crate::{
    config::{AssetInstalled, Setting},
    registry::{ComponentEntry, Registry},
};
use colored::Colorize;
use inquire::{Select, Text};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, PartialEq)]
enum ConflictAction {
    Overwrite,
    Skip,
    Rename(String),
}

pub fn execute(components: Vec<String>, all: bool) -> Result<(), Box<dyn std::error::Error>> {
    let mut setting = Setting::load()?;
    let registry = Registry::fetch(&setting.registry.repo, &setting.registry.branch)?;

    let comp_base_dir = PathBuf::from(&setting.project.components_dir);
    let assets_base_dir = PathBuf::from(&setting.project.assets_dir);

    let processed = add_components_with(
        components,
        all,
        &mut setting,
        &registry,
        &comp_base_dir,
        &assets_base_dir,
        &mut Registry::fetch_raw_file,
        &mut Registry::fetch_raw_bytes,
        &mut prompt_component_conflict,
        &mut prompt_asset_conflict,
    )?;

    if !processed {
        return Ok(());
    }

    setting.save()?;
    println!("\n{}", "All selected components processed!".green().bold());
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn add_components_with<F1, F2, C1, C2>(
    components: Vec<String>,
    all: bool,
    setting: &mut Setting,
    registry: &Registry,
    comp_base_dir: &Path,
    assets_base_dir: &Path,
    fetch_file: &mut F1,
    fetch_bytes: &mut F2,
    on_comp_conflict: &mut C1,
    on_asset_conflict: &mut C2,
) -> Result<bool, Box<dyn std::error::Error>>
where
    F1: FnMut(&str, &str, &str) -> Result<String, Box<dyn std::error::Error>>,
    F2: FnMut(&str, &str, &str) -> Result<Vec<u8>, Box<dyn std::error::Error>>,
    C1: FnMut(&str, &str, bool) -> Result<ConflictAction, Box<dyn std::error::Error>>,
    C2: FnMut(&str) -> Result<bool, Box<dyn std::error::Error>>,
{
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
            return Ok(false);
        }
        components
    };

    let repo = setting.registry.repo.clone();
    let branch = setting.registry.branch.clone();

    for comp_name in &target_names {
        println!("\nProcessing {}...", comp_name.bold().cyan());

        let Some(comp_def) = registry.components.get(comp_name) else {
            println!(
                "  {} Not found in the catalog. Skipping.",
                "[Warning] ".yellow()
            );
            continue;
        };

        // インストール処理 (ファイル or ディレクトリ)
        let Some(installed_module_name) = install_component_with(
            comp_name,
            comp_def,
            comp_base_dir,
            &repo,
            &branch,
            fetch_file,
            on_comp_conflict,
        )?
        else {
            println!("  {} Skipped", "↪".dimmed());
            continue;
        };

        // アセットの解決 (自動ダウンロード・共有判定)
        for assets_name in &comp_def.assets {
            resolve_asset_with(
                assets_name,
                &installed_module_name,
                registry,
                assets_base_dir,
                setting,
                &repo,
                &branch,
                fetch_bytes,
                on_asset_conflict,
            )?;
        }

        register_in_mod_rs(comp_base_dir, &installed_module_name)?;

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

    Ok(true)
}

fn prompt_component_conflict(
    target_name: &str,
    comp_name: &str,
    _is_dir: bool,
) -> Result<ConflictAction, Box<dyn std::error::Error>> {
    println!("  {} '{target_name}' already exists.", "[Warning]".yellow());
    let options = vec!["Skip", "Overwrite", "Rename"];
    let choice = Select::new("What would you like to do?", options).prompt()?;

    match choice {
        "Overwrite" => Ok(ConflictAction::Overwrite),
        "Rename" => {
            let new_name = Text::new("Please enter a new name (without a file extension):")
                .with_default(&format!("{comp_name}_custom"))
                .prompt()?;
            Ok(ConflictAction::Rename(new_name))
        }
        _ => Ok(ConflictAction::Skip),
    }
}

fn prompt_asset_conflict(assets_name: &str) -> Result<bool, Box<dyn std::error::Error>> {
    println!(
        "  {} Asset '{assets_name}' already exists, but the content differs.",
        "[Warning]".yellow(),
    );
    let choice = Select::new(
        "Please resolve the asset conflicts:",
        vec!["Skip", "Overwrite"],
    )
    .prompt()?;
    Ok(choice == "Overwrite")
}

// 単一ファイル、フォルダの配置処理
fn install_component_with<F, C>(
    comp_name: &str,
    def: &ComponentEntry,
    dest_base: &Path,
    repo: &str,
    branch: &str,
    fetch_file: &mut F,
    on_conflict: &mut C,
) -> Result<Option<String>, Box<dyn std::error::Error>>
where
    F: FnMut(&str, &str, &str) -> Result<String, Box<dyn std::error::Error>>,
    C: FnMut(&str, &str, bool) -> Result<ConflictAction, Box<dyn std::error::Error>>,
{
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
        match on_conflict(&target_name, comp_name, is_dir)? {
            ConflictAction::Overwrite => {}
            ConflictAction::Rename(new_name) => {
                target_name = if is_dir {
                    new_name
                } else {
                    format!("{new_name}.rs")
                };
                dest_path = dest_base.join(&target_name);
            }
            ConflictAction::Skip => return Ok(None),
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
            let remote_file_path = format!("{root_dir}/{rel_file}");
            let content = fetch_file(repo, branch, &remote_file_path)?;
            fs::write(file_dest, content)?;
        }
    } else {
        let remote_file_path = &def.files[0];
        let content = fetch_file(repo, branch, remote_file_path)?;
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
#[allow(clippy::format_collect, clippy::too_many_arguments)]
fn resolve_asset_with<F, C>(
    assets_name: &str,
    comp_name: &str,
    registry: &Registry,
    assets_dir: &Path,
    setting: &mut Setting,
    repo: &str,
    branch: &str,
    fetch_bytes: &mut F,
    on_conflict: &mut C,
) -> Result<(), Box<dyn std::error::Error>>
where
    F: FnMut(&str, &str, &str) -> Result<Vec<u8>, Box<dyn std::error::Error>>,
    C: FnMut(&str) -> Result<bool, Box<dyn std::error::Error>>,
{
    let Some(asset_def) = registry.assets.get(assets_name) else {
        return Ok(());
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
            .map(|b| format!("{b:02x}"))
            .collect::<String>();

        if local_hash == asset_def.hash {
            println!(
                "  {} Asset '{assets_name}' already exists and contents match. Sharing.",
                "[Note]".blue()
            );
            need_download = false;
        } else if !on_conflict(assets_name)? {
            need_download = false;
        }
    }

    if need_download {
        println!("  {} Asset '{assets_name}' Downloading...", "↓".cyan());
        let bytes = fetch_bytes(repo, branch, &asset_def.path)?;
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
#[allow(clippy::format_push_string)]
fn register_in_mod_rs(components_dir: &Path, module_name: &str) -> std::io::Result<()> {
    let mod_path = components_dir.join("mod.rs");
    let content = fs::read_to_string(&mod_path).unwrap_or_default();
    let mod_line = format!("pub mod {module_name};");

    if !content.lines().any(|l| l.trim() == mod_line) {
        let mut new_content = content;
        if !new_content.ends_with('\n') && !new_content.is_empty() {
            new_content.push('\n');
        }
        new_content.push_str(&format!("{mod_line}\n"));
        fs::write(&mod_path, new_content)?;
        println!(
            "{} Added '{mod_line}' to components/mod.rs.",
            "[Success]".green(),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::{InstalledConfig, ProjectConfig, RegistryConfig},
        registry::AssetEntry,
    };
    use std::{
        collections::BTreeMap,
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
            let path = std::env::temp_dir().join(format!("iu_ui_add_test_{prefix}_{pid}_{id}"));
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

    fn make_setting() -> Setting {
        Setting {
            project: ProjectConfig::default(),
            registry: RegistryConfig {
                repo: "owner/repo".to_string(),
                branch: "main".to_string(),
            },
            installed: InstalledConfig::default(),
        }
    }

    fn make_file_component(remote: &str, target_name: &str, assets: Vec<&str>) -> ComponentEntry {
        ComponentEntry {
            comp_type: "file".to_string(),
            version: "20260101-aaa".to_string(),
            target_name: Some(target_name.to_string()),
            target_dir: None,
            root_dir: None,
            files: vec![remote.to_string()],
            assets: assets.into_iter().map(String::from).collect(),
        }
    }

    fn make_dir_component(
        root_dir: &str,
        target_dir: &str,
        files: Vec<&str>,
        assets: Vec<&str>,
    ) -> ComponentEntry {
        ComponentEntry {
            comp_type: "dir".to_string(),
            version: "20260102-bbb".to_string(),
            target_name: None,
            target_dir: Some(target_dir.to_string()),
            root_dir: Some(root_dir.to_string()),
            files: files.into_iter().map(String::from).collect(),
            assets: assets.into_iter().map(String::from).collect(),
        }
    }

    #[test]
    fn test_register_in_mod_rs() {
        let dir = TestDir::new("mod_rs");
        let comp_dir = dir.path();

        // mod.rs が存在しない状態からの新規作成
        register_in_mod_rs(comp_dir, "button").unwrap();
        assert_eq!(
            fs::read_to_string(comp_dir.join("mod.rs")).unwrap(),
            "pub mod button;\n"
        );

        // 末尾改行なしファイルへ追記しても改行が補われること
        fs::write(comp_dir.join("mod.rs"), "pub mod button;").unwrap();
        register_in_mod_rs(comp_dir, "card").unwrap();
        assert_eq!(
            fs::read_to_string(comp_dir.join("mod.rs")).unwrap(),
            "pub mod button;\npub mod card;\n"
        );

        // 既に存在する場合は重複追記しないこと（前後空白あり行も含む）
        fs::write(comp_dir.join("mod.rs"), "  pub mod button;\n").unwrap();
        register_in_mod_rs(comp_dir, "button").unwrap();
        assert_eq!(
            fs::read_to_string(comp_dir.join("mod.rs")).unwrap(),
            "  pub mod button;\n"
        );
    }

    #[test]
    fn test_install_component_file_and_dir() {
        let dir = TestDir::new("install_basic");
        let dest = dir.path();

        // 単一ファイル
        let file_def = make_file_component("iu_ui/src/components/button.rs", "button.rs", vec![]);
        let mod_name = install_component_with(
            "button",
            &file_def,
            dest,
            "owner/repo",
            "main",
            &mut |_, _, path| {
                assert_eq!(path, "iu_ui/src/components/button.rs");
                Ok("pub fn button() {}".to_string())
            },
            &mut |_, _, _| panic!("should not conflict"),
        )
        .unwrap();
        assert_eq!(mod_name.as_deref(), Some("button"));
        assert_eq!(
            fs::read_to_string(dest.join("button.rs")).unwrap(),
            "pub fn button() {}"
        );

        // ディレクトリ（ネストファイル含む）
        let dir_def = make_dir_component(
            "iu_ui/src/components/tabs",
            "tabs",
            vec!["mod.rs", "sub/item.rs"],
            vec![],
        );
        let mut fetched_paths = Vec::new();
        let mod_name = install_component_with(
            "tabs",
            &dir_def,
            dest,
            "owner/repo",
            "main",
            &mut |_, _, path| {
                fetched_paths.push(path.to_string());
                Ok(format!("// {path}"))
            },
            &mut |_, _, _| panic!("should not conflict"),
        )
        .unwrap();
        assert_eq!(mod_name.as_deref(), Some("tabs"));
        assert_eq!(
            fetched_paths,
            vec![
                "iu_ui/src/components/tabs/mod.rs",
                "iu_ui/src/components/tabs/sub/item.rs"
            ]
        );
        assert_eq!(
            fs::read_to_string(dest.join("tabs/sub/item.rs")).unwrap(),
            "// iu_ui/src/components/tabs/sub/item.rs"
        );
    }

    #[test]
    fn test_install_component_conflict_actions() {
        let dir = TestDir::new("install_conflict");
        let dest = dir.path();
        let def = make_file_component("iu_ui/src/components/button.rs", "button.rs", vec![]);

        // 既存ファイルを用意
        fs::write(dest.join("button.rs"), "old").unwrap();

        // 1. Skip
        let res = install_component_with(
            "button",
            &def,
            dest,
            "owner/repo",
            "main",
            &mut |_, _, _| panic!("should not fetch on skip"),
            &mut |target, _, is_dir| {
                assert_eq!(target, "button.rs");
                assert!(!is_dir);
                Ok(ConflictAction::Skip)
            },
        )
        .unwrap();
        assert_eq!(res, None);
        assert_eq!(fs::read_to_string(dest.join("button.rs")).unwrap(), "old");

        // 2. Rename
        let res = install_component_with(
            "button",
            &def,
            dest,
            "owner/repo",
            "main",
            &mut |_, _, _| Ok("renamed_content".to_string()),
            &mut |_, _, _| Ok(ConflictAction::Rename("my_button".to_string())),
        )
        .unwrap();
        assert_eq!(res.as_deref(), Some("my_button"));
        assert_eq!(fs::read_to_string(dest.join("button.rs")).unwrap(), "old");
        assert_eq!(
            fs::read_to_string(dest.join("my_button.rs")).unwrap(),
            "renamed_content"
        );

        // 3. Overwrite
        let res = install_component_with(
            "button",
            &def,
            dest,
            "owner/repo",
            "main",
            &mut |_, _, _| Ok("new_content".to_string()),
            &mut |_, _, _| Ok(ConflictAction::Overwrite),
        )
        .unwrap();
        assert_eq!(res.as_deref(), Some("button"));
        assert_eq!(
            fs::read_to_string(dest.join("button.rs")).unwrap(),
            "new_content"
        );
    }

    #[test]
    fn test_resolve_asset_download_and_share() {
        let dir = TestDir::new("asset_share");
        let assets_dir = dir.path();

        // "hello" の SHA-256
        let hello_hash = "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824";
        let mut reg_assets = BTreeMap::new();
        reg_assets.insert(
            "icon.svg".to_string(),
            AssetEntry {
                path: "iu_ui/src/assets/icon.svg".to_string(),
                hash: hello_hash.to_string(),
            },
        );
        let registry = Registry {
            name: "iu_ui".to_string(),
            updated_at: "2026-11-14".to_string(),
            assets: reg_assets,
            components: BTreeMap::new(),
            themes: BTreeMap::new(),
            tags: BTreeMap::new(),
            utils: BTreeMap::new(),
        };
        let mut setting = make_setting();

        // 初回：ダウンロードされる
        resolve_asset_with(
            "icon.svg",
            "button",
            &registry,
            assets_dir,
            &mut setting,
            "owner/repo",
            "main",
            &mut |_, _, path| {
                assert_eq!(path, "iu_ui/src/assets/icon.svg");
                Ok(b"hello".to_vec())
            },
            &mut |_| panic!("should not conflict"),
        )
        .unwrap();

        assert_eq!(fs::read(assets_dir.join("icon.svg")).unwrap(), b"hello");
        assert_eq!(setting.installed.assets["icon.svg"].used_by, vec!["button"]);

        // 2回目（別コンポーネント）：ハッシュ一致のため再ダウンロードせず共有
        resolve_asset_with(
            "icon.svg",
            "card",
            &registry,
            assets_dir,
            &mut setting,
            "owner/repo",
            "main",
            &mut |_, _, _| panic!("should share without downloading"),
            &mut |_| panic!("should not conflict when hash matches"),
        )
        .unwrap();

        // 同じコンポーネントの二重登録が起きないことも確認
        resolve_asset_with(
            "icon.svg",
            "card",
            &registry,
            assets_dir,
            &mut setting,
            "owner/repo",
            "main",
            &mut |_, _, _| panic!("should share"),
            &mut |_| panic!("should not conflict"),
        )
        .unwrap();

        assert_eq!(
            setting.installed.assets["icon.svg"].used_by,
            vec!["button", "card"]
        );
    }

    #[test]
    fn test_resolve_asset_hash_mismatch_skip_and_overwrite() {
        let dir = TestDir::new("asset_mismatch");
        let assets_dir = dir.path();

        let hello_hash = "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824";
        let mut reg_assets = BTreeMap::new();
        reg_assets.insert(
            "icon.svg".to_string(),
            AssetEntry {
                path: "iu_ui/src/assets/icon.svg".to_string(),
                hash: hello_hash.to_string(),
            },
        );
        let registry = Registry {
            name: "iu_ui".to_string(),
            updated_at: "2026-11-14".to_string(),
            assets: reg_assets,
            components: BTreeMap::new(),
            themes: BTreeMap::new(),
            tags: BTreeMap::new(),
            utils: BTreeMap::new(),
        };
        let mut setting = make_setting();

        // 異なる内容の既存ファイル
        fs::write(assets_dir.join("icon.svg"), b"different").unwrap();

        // Skip (false)
        resolve_asset_with(
            "icon.svg",
            "button",
            &registry,
            assets_dir,
            &mut setting,
            "owner/repo",
            "main",
            &mut |_, _, _| panic!("should not download on skip"),
            &mut |_| Ok(false),
        )
        .unwrap();
        assert_eq!(fs::read(assets_dir.join("icon.svg")).unwrap(), b"different");

        // Overwrite (true)
        resolve_asset_with(
            "icon.svg",
            "button",
            &registry,
            assets_dir,
            &mut setting,
            "owner/repo",
            "main",
            &mut |_, _, _| Ok(b"hello".to_vec()),
            &mut |_| Ok(true),
        )
        .unwrap();
        assert_eq!(fs::read(assets_dir.join("icon.svg")).unwrap(), b"hello");
    }

    #[test]
    fn test_add_components_with_full_flow_and_all_flag() {
        let dir = TestDir::new("flow");
        let comp_dir = dir.path().join("components");
        let assets_dir = dir.path().join("assets");
        fs::create_dir_all(&comp_dir).unwrap();
        fs::create_dir_all(&assets_dir).unwrap();

        let hello_hash = "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824";
        let mut reg_assets = BTreeMap::new();
        reg_assets.insert(
            "logo.png".to_string(),
            AssetEntry {
                path: "iu_ui/src/assets/logo.png".to_string(),
                hash: hello_hash.to_string(),
            },
        );

        let mut reg_components = BTreeMap::new();
        reg_components.insert(
            "button".to_string(),
            make_file_component(
                "iu_ui/src/components/button.rs",
                "button.rs",
                vec!["logo.png"],
            ),
        );
        reg_components.insert(
            "card".to_string(),
            make_file_component("iu_ui/src/components/card.rs", "card.rs", vec![]),
        );

        let registry = Registry {
            name: "iu_ui".to_string(),
            updated_at: "2026-11-14".to_string(),
            assets: reg_assets,
            components: reg_components,
            themes: BTreeMap::new(),
            tags: BTreeMap::new(),
            utils: BTreeMap::new(),
        };
        let mut setting = make_setting();

        // 空指定 + all=false -> false が返り何もしない
        let processed = add_components_with(
            vec![],
            false,
            &mut setting,
            &registry,
            &comp_dir,
            &assets_dir,
            &mut |_, _, _| panic!("should not run"),
            &mut |_, _, _| panic!("should not run"),
            &mut |_, _, _| panic!("should not run"),
            &mut |_| panic!("should not run"),
        )
        .unwrap();
        assert!(!processed);

        // 存在しないコンポーネントを含む指定 -> 存在する button のみ処理され続行
        let processed = add_components_with(
            vec!["nonexistent".to_string(), "button".to_string()],
            false,
            &mut setting,
            &registry,
            &comp_dir,
            &assets_dir,
            &mut |_, _, path| Ok(format!("// {path}")),
            &mut |_, _, _| Ok(b"hello".to_vec()),
            &mut |_, _, _| Ok(ConflictAction::Overwrite),
            &mut |_| Ok(true),
        )
        .unwrap();
        assert!(processed);
        assert!(comp_dir.join("button.rs").exists());
        assert!(!comp_dir.join("card.rs").exists());
        assert_eq!(
            setting
                .installed
                .components
                .get("button")
                .map(String::as_str),
            Some("20260101-aaa")
        );
        assert!(!setting.installed.components.contains_key("nonexistent"));

        // all=true -> 既に存在する button は Skip しつつ card が追加されること
        let processed = add_components_with(
            vec![],
            true,
            &mut setting,
            &registry,
            &comp_dir,
            &assets_dir,
            &mut |_, _, path| Ok(format!("// {path}")),
            &mut |_, _, _| Ok(b"hello".to_vec()),
            &mut |_, _, _| Ok(ConflictAction::Skip),
            &mut |_| Ok(true),
        )
        .unwrap();
        assert!(processed);
        assert!(comp_dir.join("card.rs").exists());
        assert_eq!(
            fs::read_to_string(comp_dir.join("mod.rs")).unwrap(),
            "pub mod button;\npub mod card;\n"
        );
    }
}

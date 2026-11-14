use regex::Regex;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
    process::Command,
};
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
    let mut cmd = Command::new("git");

    let work_dir = if path.is_dir() {
        Some(path)
    } else {
        path.parent().filter(|p| !p.as_os_str().is_empty())
    };
    if let Some(dir) = work_dir
        && dir.exists()
    {
        cmd.current_dir(dir);
    }

    let target = if path.is_dir() {
        "."
    } else {
        path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_else(|| path.to_str().unwrap())
    };

    let output = cmd
        .args([
            "log",
            "-1",
            "--format=%cd-%h",
            "--date=format:%Y%m%d",
            "--",
            target,
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
#[allow(clippy::format_collect)]
fn calculate_sha256(path: &Path) -> Result<String, Box<dyn std::error::Error>> {
    let bytes = std::fs::read(path)?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let result = hasher.finalize();

    // 各バイトを 2 桁の小文字 16 進数 (02x) に変換して結合
    let hex = result
        .iter()
        .map(|b| format!("{b:02x}"))
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
                    target_name: Some(format!("{name}.rs")),
                    files: vec![to_rel_path(&path, root)],
                    assets: detected_assets,
                },
            );
        } else if path.is_dir() {
            // フォルダモジュールコンポーネント (例: tabs/)
            let comp_name = name;
            let mut files = Vec::new();
            let mut detected_assets = BTreeSet::new();

            for file_entry in WalkDir::new(&path)
                .into_iter()
                .filter_map(std::result::Result::ok)
            {
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

        if stem == "mod" || !path.is_file() || path.extension().is_none_or(|ext| ext != "rs") {
            continue;
        }

        let version = get_git_version(&path);
        map.insert(
            stem.clone(),
            SimpleEntry {
                version,
                target_name: format!("{stem}.rs"),
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
            let path = std::env::temp_dir().join(format!("iu_ui_test_{prefix}_{pid}_{id}"));
            if path.exists() {
                let _ = fs::remove_dir_all(&path);
            }
            fs::create_dir_all(&path).expect("failed to create test dir");
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }

        fn write_file(&self, rel: &str, content: impl AsRef<[u8]>) -> PathBuf {
            let target = self.path.join(rel);
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).expect("failed to create parent dir");
            }
            fs::write(&target, content).expect("failed to write test file");
            target
        }

        fn create_dir(&self, rel: &str) -> PathBuf {
            let target = self.path.join(rel);
            fs::create_dir_all(&target).expect("failed to create sub dir");
            target
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn test_to_rel_path() {
        let root = Path::new("/workspace/project");
        let target = Path::new("/workspace/project/iu_ui/src/assets/logo.png");
        assert_eq!(to_rel_path(target, root), "iu_ui/src/assets/logo.png");

        // root 配下でない場合はそのまま返る
        let outside = Path::new("/other/dir/file.rs");
        assert_eq!(to_rel_path(outside, root), "/other/dir/file.rs");

        // バックスラッシュがスラッシュに変換される
        let win_like = Path::new(r"iu_ui\src\components\button.rs");
        assert_eq!(
            to_rel_path(win_like, root),
            "iu_ui/src/components/button.rs"
        );
    }

    #[test]
    fn test_calculate_sha256() {
        let dir = TestDir::new("sha256");

        let empty_file = dir.write_file("empty.txt", b"");
        assert_eq!(
            calculate_sha256(&empty_file).unwrap(),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );

        let hello_file = dir.write_file("hello.txt", b"hello");
        assert_eq!(
            calculate_sha256(&hello_file).unwrap(),
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );

        // 存在しないファイルはエラー
        let missing = dir.path().join("not_found.txt");
        assert!(calculate_sha256(&missing).is_err());
    }

    #[test]
    fn test_find_assets_in_code() {
        let re = Regex::new(r#"assets/([^"\s\)]+)"#).unwrap();

        let code = r#"
            const ICON: &str = include_str!("../assets/icons/check.svg");
            const LOGO: &[u8] = include_bytes!("../../assets/logo.png");
            const DUP: &str = "../assets/icons/check.svg";
            let url = "url(../assets/bg.webp)";
        "#;

        // 重複排除され辞書順で返ること
        assert_eq!(
            find_assets_in_code(code, &re),
            vec![
                "bg.webp".to_string(),
                "icons/check.svg".to_string(),
                "logo.png".to_string(),
            ]
        );

        // 該当なしの場合
        assert!(find_assets_in_code("fn main() {}", &re).is_empty());
    }

    #[test]
    fn test_collect_assets() {
        let dir = TestDir::new("assets");
        let root = dir.path();

        // 存在しないディレクトリは空
        let missing = root.join("iu_ui/src/assets");
        assert!(collect_assets(root, &missing).unwrap().is_empty());

        // ファイルとサブディレクトリを作成（サブディレクトリは無視される仕様）
        dir.write_file("iu_ui/src/assets/logo.png", b"hello");
        dir.create_dir("iu_ui/src/assets/nested_dir");
        dir.write_file("iu_ui/src/assets/nested_dir/ignored.png", b"ignored");

        let assets = collect_assets(root, &root.join("iu_ui/src/assets")).unwrap();
        assert_eq!(assets.len(), 1);

        let logo = assets.get("logo.png").expect("logo.png should exist");
        assert_eq!(logo.path, "iu_ui/src/assets/logo.png");
        assert_eq!(
            logo.hash,
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
    }

    #[test]
    fn test_collect_components() {
        let dir = TestDir::new("components");
        let root = dir.path();
        let comp_dir = root.join("iu_ui/src/components");

        // 存在しないディレクトリは空
        assert!(collect_components(root, &comp_dir).unwrap().is_empty());

        // スキップ対象
        dir.write_file("iu_ui/src/components/mod.rs", "pub mod button;");
        dir.write_file("iu_ui/src/components/README.md", "# docs");

        // 単一ファイルコンポーネント
        dir.write_file(
            "iu_ui/src/components/button.rs",
            r#"const A: &str = "../assets/btn.svg";"#,
        );

        // ディレクトリコンポーネント（サブディレクトリ含む）
        dir.write_file(
            "iu_ui/src/components/tabs/mod.rs",
            r#"const B: &str = "../../assets/tab.svg";"#,
        );
        dir.write_file(
            "iu_ui/src/components/tabs/sub/panel.rs",
            r#"const C: &str = "../../../assets/panel.png"; const D: &str = "../../../assets/tab.svg";"#,
        );

        let components = collect_components(root, &comp_dir).unwrap();
        assert_eq!(components.len(), 2);
        assert!(!components.contains_key("mod"));
        assert!(!components.contains_key("README"));

        // "file" タイプの検証
        let button = components.get("button").expect("button should exist");
        assert_eq!(button.comp_type, "file");
        assert!(!button.version.is_empty());
        assert_eq!(button.root_dir, None);
        assert_eq!(button.target_dir, None);
        assert_eq!(button.target_name.as_deref(), Some("button.rs"));
        assert_eq!(button.files, vec!["iu_ui/src/components/button.rs"]);
        assert_eq!(button.assets, vec!["btn.svg"]);

        // "dir" タイプの検証
        let tabs = components.get("tabs").expect("tabs should exist");
        assert_eq!(tabs.comp_type, "dir");
        assert!(!tabs.version.is_empty());
        assert_eq!(tabs.root_dir.as_deref(), Some("iu_ui/src/components/tabs"));
        assert_eq!(tabs.target_dir.as_deref(), Some("tabs"));
        assert_eq!(tabs.target_name, None);
        assert_eq!(tabs.files, vec!["mod.rs", "sub/panel.rs"]);
        assert_eq!(tabs.assets, vec!["panel.png", "tab.svg"]);
    }

    #[test]
    fn test_collect_simple_modules() {
        let dir = TestDir::new("simple_modules");
        let root = dir.path();
        let theme_dir = root.join("iu_ui/src/theme");

        // 存在しないディレクトリは空
        assert!(collect_simple_modules(root, &theme_dir).unwrap().is_empty());

        // mod.rs、サブディレクトリ、.rs 以外のファイルはスキップされる
        dir.write_file("iu_ui/src/theme/mod.rs", "pub mod dark;");
        dir.write_file("iu_ui/src/theme/README.md", "# theme docs");
        dir.create_dir("iu_ui/src/theme/extra");
        dir.write_file("iu_ui/src/theme/dark.rs", "pub struct Dark;");

        let themes = collect_simple_modules(root, &theme_dir).unwrap();
        assert_eq!(themes.len(), 1);
        assert!(!themes.contains_key("mod"));
        assert!(!themes.contains_key("README"));
        assert!(!themes.contains_key("extra"));

        let dark = themes.get("dark").expect("dark should exist");
        assert!(!dark.version.is_empty());
        assert_eq!(dark.target_name, "dark.rs");
        assert_eq!(dark.files, vec!["iu_ui/src/theme/dark.rs"]);
    }

    #[test]
    fn test_serialize_component_entry_json() {
        let file_comp = ComponentEntry {
            comp_type: "file".to_string(),
            version: "20260101-abc1234".to_string(),
            root_dir: None,
            target_dir: None,
            target_name: Some("button.rs".to_string()),
            files: vec!["iu_ui/src/components/button.rs".to_string()],
            assets: vec![],
        };

        let val = serde_json::to_value(&file_comp).unwrap();
        assert_eq!(val["type"], "file");
        assert_eq!(val["target_name"], "button.rs");
        // None のフィールドがスキップされていること
        assert!(val.get("root_dir").is_none());
        assert!(val.get("target_dir").is_none());
    }

    #[test]
    fn test_get_git_version() {
        // git が使えない環境ではスキップ
        if Command::new("git").arg("--version").output().is_err() {
            return;
        }

        let dir = TestDir::new("git_ver");
        let init = Command::new("git")
            .args(["init"])
            .current_dir(dir.path())
            .output()
            .unwrap();
        assert!(init.status.success());

        // 初期コミットを作成（コミット0件だと git log 自体がエラーになるため）
        dir.write_file("init.txt", "init");
        Command::new("git")
            .args(["add", "init.txt"])
            .current_dir(dir.path())
            .output()
            .unwrap();
        let first_commit = Command::new("git")
            .args([
                "-c",
                "user.name=Test",
                "-c",
                "user.email=test@example.com",
                "commit",
                "-m",
                "initial",
            ])
            .current_dir(dir.path())
            .output()
            .unwrap();
        assert!(first_commit.status.success());

        // 未コミットファイル -> "uncommitted"
        let uncommitted = dir.write_file("uncommitted.rs", "fn a() {}");
        assert_eq!(get_git_version(&uncommitted), "uncommitted");

        // コミット済みファイル & ディレクトリ -> "YYYYMMDD-<short_hash>"
        let committed = dir.write_file("comp/committed.rs", "fn b() {}");
        Command::new("git")
            .args(["add", "comp/committed.rs"])
            .current_dir(dir.path())
            .output()
            .unwrap();
        let commit = Command::new("git")
            .args([
                "-c",
                "user.name=Test",
                "-c",
                "user.email=test@example.com",
                "commit",
                "-m",
                "add comp",
            ])
            .current_dir(dir.path())
            .output()
            .unwrap();
        assert!(commit.status.success());

        let ver_re = Regex::new(r"^\d{8}-[0-9a-f]+$").unwrap();

        let file_ver = get_git_version(&committed);
        assert!(
            ver_re.is_match(&file_ver),
            "expected YYYYMMDD-hash format for file, got: {file_ver}"
        );

        let dir_ver = get_git_version(&dir.path().join("comp"));
        assert!(
            ver_re.is_match(&dir_ver),
            "expected YYYYMMDD-hash format for dir, got: {dir_ver}"
        );
    }
}

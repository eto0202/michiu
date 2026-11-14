use crate::{config::Setting, registry::Registry};
use colored::Colorize;

#[derive(Debug, Clone, PartialEq, Eq)]
enum CheckStatus {
    UpToDate,
    UpdateAvailable,
    NotInRegistry,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CheckEntry {
    category: &'static str,
    name: String,
    installed: String,
    latest: String,
    status: CheckStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CheckReport {
    entries: Vec<CheckEntry>,
    update_count: usize,
}

pub fn execute() -> Result<(), Box<dyn std::error::Error>> {
    let setting = Setting::load()?;
    println!(
        "{} (repo: {}@{})...",
        "Checking for updates".cyan().bold(),
        setting.registry.repo.bold(),
        setting.registry.branch.bold()
    );

    let registry = Registry::fetch(&setting.registry.repo, &setting.registry.branch)?;
    let report = build_check_report(&setting, &registry);

    println!(
        "\n{:<12} {:<24} {:<18} {:<18} {}",
        "CATEGORY".bold(),
        "NAME".bold(),
        "INSTALLED".bold(),
        "LATEST".bold(),
        "STATUS".bold()
    );
    println!("{}", "-".repeat(88).dimmed());

    for entry in &report.entries {
        let status = match entry.status {
            CheckStatus::UpToDate => "Up to date".green(),
            CheckStatus::UpdateAvailable => "Update available!".yellow().bold(),
            CheckStatus::NotInRegistry => "Not in registry".dimmed(),
        };

        println!(
            "{:<12} {:<24} {:<18} {:<18} {}",
            entry.category,
            entry.name,
            entry.installed.dimmed(),
            entry.latest.dimmed(),
            status
        );
    }

    println!("{}", "-".repeat(88).dimmed());
    if report.update_count > 0 {
        println!(
            "{}",
            format!("{} update(s) available!", report.update_count)
                .yellow()
                .bold()
        );
        println!(
            "Run {} to overwrite with latest files.",
            "cargo iu_ui add <name>".cyan()
        );
    } else {
        println!("{}", "Everything is up to date!".green().bold());
    }

    Ok(())
}

fn short_hash(hash: &str) -> &str {
    &hash[..8.min(hash.len())]
}

fn check_version(
    category: &'static str,
    name: &str,
    local_ver: &str,
    remote_ver: Option<&str>,
    update_count: &mut usize,
) -> CheckEntry {
    let (latest, status) = match remote_ver {
        Some(ver) if ver == local_ver => (ver.to_string(), CheckStatus::UpToDate),
        Some(ver) => {
            *update_count += 1;
            (ver.to_string(), CheckStatus::UpdateAvailable)
        }
        None => ("-".to_string(), CheckStatus::NotInRegistry),
    };

    CheckEntry {
        category,
        name: name.to_string(),
        installed: local_ver.to_string(),
        latest,
        status,
    }
}

fn build_check_report(setting: &Setting, registry: &Registry) -> CheckReport {
    let mut entries = Vec::new();
    let mut update_count = 0;

    for (name, local_ver) in &setting.installed.components {
        let remote_ver = registry.components.get(name).map(|c| c.version.as_str());
        entries.push(check_version(
            "component",
            name,
            local_ver,
            remote_ver,
            &mut update_count,
        ));
    }

    for (name, local_ver) in &setting.installed.themes {
        let remote_ver = registry.themes.get(name).map(|t| t.version.as_str());
        entries.push(check_version(
            "theme",
            name,
            local_ver,
            remote_ver,
            &mut update_count,
        ));
    }

    for (name, local_ver) in &setting.installed.tags {
        let remote_ver = registry.tags.get(name).map(|t| t.version.as_str());
        entries.push(check_version(
            "tag",
            name,
            local_ver,
            remote_ver,
            &mut update_count,
        ));
    }

    for (name, local_ver) in &setting.installed.utils {
        let remote_ver = registry.utils.get(name).map(|u| u.version.as_str());
        entries.push(check_version(
            "utils",
            name,
            local_ver,
            remote_ver,
            &mut update_count,
        ));
    }

    for (name, asset_info) in &setting.installed.assets {
        let (latest, status) = match registry.assets.get(name) {
            Some(asset) if asset.hash == asset_info.hash => {
                (short_hash(&asset.hash).to_string(), CheckStatus::UpToDate)
            }
            Some(asset) => {
                update_count += 1;
                (
                    short_hash(&asset.hash).to_string(),
                    CheckStatus::UpdateAvailable,
                )
            }
            None => ("-".to_string(), CheckStatus::NotInRegistry),
        };

        entries.push(CheckEntry {
            category: "asset",
            name: name.clone(),
            installed: short_hash(&asset_info.hash).to_string(),
            latest,
            status,
        });
    }

    CheckReport {
        entries,
        update_count,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::{AssetInstalled, InstalledConfig, ProjectConfig, RegistryConfig},
        registry::{AssetEntry, ComponentEntry, SimpleEntry},
    };
    use std::collections::BTreeMap;

    fn base_setting() -> Setting {
        Setting {
            project: ProjectConfig::default(),
            registry: RegistryConfig::default(),
            installed: InstalledConfig::default(),
        }
    }

    fn base_registry() -> Registry {
        Registry {
            name: "iu_ui".to_string(),
            updated_at: "2026-11-14".to_string(),
            assets: BTreeMap::new(),
            components: BTreeMap::new(),
            themes: BTreeMap::new(),
            tags: BTreeMap::new(),
            utils: BTreeMap::new(),
        }
    }

    fn comp(version: &str) -> ComponentEntry {
        ComponentEntry {
            comp_type: "file".to_string(),
            version: version.to_string(),
            target_name: Some("x.rs".to_string()),
            target_dir: None,
            root_dir: None,
            files: vec!["x.rs".to_string()],
            assets: vec![],
        }
    }

    fn simple(version: &str) -> SimpleEntry {
        SimpleEntry {
            version: version.to_string(),
            target_name: "x.rs".to_string(),
            files: vec!["x.rs".to_string()],
        }
    }

    #[test]
    fn test_build_check_report_empty() {
        let report = build_check_report(&base_setting(), &base_registry());
        assert!(report.entries.is_empty());
        assert_eq!(report.update_count, 0);
    }

    #[test]
    fn test_build_check_report_all_up_to_date() {
        let mut setting = base_setting();
        setting
            .installed
            .components
            .insert("button".into(), "v1".into());
        setting.installed.themes.insert("dark".into(), "v2".into());
        setting.installed.tags.insert("badge".into(), "v3".into());
        setting.installed.utils.insert("cn".into(), "v4".into());
        setting.installed.assets.insert(
            "logo.png".into(),
            AssetInstalled {
                hash: "1234567890abcdef".into(),
                used_by: vec!["button".into()],
            },
        );

        let mut registry = base_registry();
        registry.components.insert("button".into(), comp("v1"));
        registry.themes.insert("dark".into(), simple("v2"));
        registry.tags.insert("badge".into(), simple("v3"));
        registry.utils.insert("cn".into(), simple("v4"));
        registry.assets.insert(
            "logo.png".into(),
            AssetEntry {
                path: "iu_ui/src/assets/logo.png".into(),
                hash: "1234567890abcdef".into(),
            },
        );

        let report = build_check_report(&setting, &registry);
        assert_eq!(report.update_count, 0);
        assert_eq!(report.entries.len(), 5);
        assert!(
            report
                .entries
                .iter()
                .all(|e| e.status == CheckStatus::UpToDate)
        );

        // asset のハッシュが 8 文字に短縮されていること
        let asset_entry = report.entries.last().unwrap();
        assert_eq!(asset_entry.category, "asset");
        assert_eq!(asset_entry.installed, "12345678");
        assert_eq!(asset_entry.latest, "12345678");
    }

    #[test]
    fn test_build_check_report_updates_and_not_in_registry() {
        let mut setting = base_setting();
        setting
            .installed
            .components
            .insert("button".into(), "v1".into());
        setting
            .installed
            .components
            .insert("local_only".into(), "v1".into());
        setting.installed.themes.insert("dark".into(), "v1".into());
        setting.installed.tags.insert("badge".into(), "v1".into());
        setting.installed.utils.insert("cn".into(), "v1".into());
        // 先頭 8 文字は一致するが全体ハッシュが異なるケース
        setting.installed.assets.insert(
            "icon.svg".into(),
            AssetInstalled {
                hash: "aaaaaaaa11111111".into(),
                used_by: vec![],
            },
        );
        setting.installed.assets.insert(
            "orphan.png".into(),
            AssetInstalled {
                hash: "abc".into(), // 8 文字未満の短いハッシュ
                used_by: vec![],
            },
        );

        let mut registry = base_registry();
        registry.components.insert("button".into(), comp("v2")); // 更新あり
        registry.themes.insert("dark".into(), simple("v2")); // 更新あり
        registry.tags.insert("badge".into(), simple("v1")); // 最新
        registry.utils.insert("cn".into(), simple("v2")); // 更新あり
        registry.assets.insert(
            "icon.svg".into(),
            AssetEntry {
                path: "p".into(),
                hash: "aaaaaaaa22222222".into(), // 更新あり
            },
        );

        let report = build_check_report(&setting, &registry);
        assert_eq!(report.update_count, 4);

        let by_name = |cat: &str, name: &str| {
            report
                .entries
                .iter()
                .find(|e| e.category == cat && e.name == name)
                .unwrap()
                .clone()
        };

        assert_eq!(
            by_name("component", "button"),
            CheckEntry {
                category: "component",
                name: "button".into(),
                installed: "v1".into(),
                latest: "v2".into(),
                status: CheckStatus::UpdateAvailable,
            }
        );
        assert_eq!(
            by_name("component", "local_only"),
            CheckEntry {
                category: "component",
                name: "local_only".into(),
                installed: "v1".into(),
                latest: "-".into(),
                status: CheckStatus::NotInRegistry,
            }
        );
        assert_eq!(by_name("tag", "badge").status, CheckStatus::UpToDate);
        assert_eq!(
            by_name("theme", "dark").status,
            CheckStatus::UpdateAvailable
        );
        assert_eq!(by_name("utils", "cn").status, CheckStatus::UpdateAvailable);
        assert_eq!(
            by_name("asset", "icon.svg"),
            CheckEntry {
                category: "asset",
                name: "icon.svg".into(),
                installed: "aaaaaaaa".into(),
                latest: "aaaaaaaa".into(),
                status: CheckStatus::UpdateAvailable,
            }
        );
        assert_eq!(
            by_name("asset", "orphan.png"),
            CheckEntry {
                category: "asset",
                name: "orphan.png".into(),
                installed: "abc".into(),
                latest: "-".into(),
                status: CheckStatus::NotInRegistry,
            }
        );
    }
}

use crate::{
    config::Setting,
    registry::{Registry, SimpleEntry},
};
use colored::Colorize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ComponentKind {
    Dir,
    File,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ComponentListItem {
    name: String,
    kind: ComponentKind,
    installed: bool,
    version: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SimpleListItem {
    name: String,
    installed: bool,
    version: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CatalogListing {
    components: Vec<ComponentListItem>,
    themes: Vec<SimpleListItem>,
    tags: Vec<SimpleListItem>,
    utils: Vec<SimpleListItem>,
}

pub fn execute() -> Result<(), Box<dyn std::error::Error>> {
    let setting = Setting::load()?;
    println!(
        "{} (repo: {}@{})...",
        "Fetching catalog from GitHub".cyan(),
        setting.registry.repo.bold(),
        setting.registry.branch.bold()
    );

    let registry = Registry::fetch(&setting.registry.repo, &setting.registry.branch)?;
    let listing = build_catalog_listing(&setting, &registry);

    println!("\n{}", "=== Available Components ===".green().bold());
    for item in &listing.components {
        let status = if item.installed {
            "[x] Installed".green()
        } else {
            "[ ] Not installed".dimmed()
        };

        let comp_type = match item.kind {
            ComponentKind::Dir => "(dir)".blue(),
            ComponentKind::File => "(file)".magenta(),
        };

        println!(
            "  {:<24} {:<8} {:<16} v:{}",
            item.name.bold(),
            comp_type,
            status,
            item.version.dimmed()
        );
    }

    if !listing.themes.is_empty() {
        println!("\n{}", "=== Available Themes ===".green().bold());
        for item in &listing.themes {
            let status = if item.installed {
                "[x] Installed".green()
            } else {
                "[ ] Not installed".dimmed()
            };
            println!(
                "  {:<24} {:<16} v:{}",
                item.name.bold(),
                status,
                item.version.dimmed()
            );
        }
    }

    if !listing.tags.is_empty() {
        println!("\n{}", "=== Available Tags ===".green().bold());
        for item in &listing.tags {
            let status = if item.installed {
                "[x] Installed".green()
            } else {
                "[ ] Not installed".dimmed()
            };
            println!(
                "  {:<24} {:<16} v:{}",
                item.name.bold(),
                status,
                item.version.dimmed()
            );
        }
    }

    println!(
        "\nTip: Run {} to install a component.",
        "cargo iu_ui add <name>".cyan()
    );
    Ok(())
}

fn build_simple_items(
    catalog: &BTreeMap<String, SimpleEntry>,
    installed: &BTreeMap<String, String>,
) -> Vec<SimpleListItem> {
    catalog
        .iter()
        .map(|(name, entry)| SimpleListItem {
            name: name.clone(),
            installed: installed.contains_key(name),
            version: entry.version.clone(),
        })
        .collect()
}

fn build_catalog_listing(setting: &Setting, registry: &Registry) -> CatalogListing {
    let components = registry
        .components
        .iter()
        .map(|(name, comp)| ComponentListItem {
            name: name.clone(),
            kind: if comp.comp_type == "dir" {
                ComponentKind::Dir
            } else {
                ComponentKind::File
            },
            installed: setting.installed.components.contains_key(name),
            version: comp.version.clone(),
        })
        .collect();

    let themes = build_simple_items(&registry.themes, &setting.installed.themes);
    let tags = build_simple_items(&registry.tags, &setting.installed.tags);
    let utils = build_simple_items(&registry.utils, &setting.installed.utils);

    CatalogListing {
        components,
        themes,
        tags,
        utils,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::{InstalledConfig, ProjectConfig, RegistryConfig},
        registry::ComponentEntry,
    };

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

    fn make_comp(comp_type: &str, version: &str) -> ComponentEntry {
        ComponentEntry {
            comp_type: comp_type.to_string(),
            version: version.to_string(),
            target_name: None,
            target_dir: None,
            root_dir: None,
            files: vec![],
            assets: vec![],
        }
    }

    fn make_simple(version: &str) -> SimpleEntry {
        SimpleEntry {
            version: version.to_string(),
            target_name: "x.rs".to_string(),
            files: vec![],
        }
    }

    #[test]
    fn test_build_catalog_listing_empty() {
        let listing = build_catalog_listing(&base_setting(), &base_registry());
        assert!(listing.components.is_empty());
        assert!(listing.themes.is_empty());
        assert!(listing.tags.is_empty());
    }

    #[test]
    fn test_build_catalog_listing_installed_status_and_kinds() {
        let mut setting = base_setting();
        setting
            .installed
            .components
            .insert("button".into(), "v1".into());
        // カタログに存在しないローカル項目は一覧に混ざらないこと
        setting
            .installed
            .components
            .insert("local_only".into(), "v1".into());
        setting.installed.themes.insert("dark".into(), "v1".into());
        setting.installed.tags.insert("badge".into(), "v1".into());
        setting.installed.utils.insert("utils".into(), "v1".into());

        let mut registry = base_registry();
        registry
            .components
            .insert("tabs".into(), make_comp("dir", "20260102-bbb"));
        registry
            .components
            .insert("button".into(), make_comp("file", "20260101-aaa"));

        registry
            .themes
            .insert("dark".into(), make_simple("20260103-ccc"));
        registry
            .themes
            .insert("light".into(), make_simple("20260104-ddd"));

        registry
            .tags
            .insert("badge".into(), make_simple("20260105-eee"));

        registry
            .utils
            .insert("utils".into(), make_simple("20260105-fff"));

        let listing = build_catalog_listing(&setting, &registry);

        assert_eq!(
            listing.components,
            vec![
                ComponentListItem {
                    name: "button".into(),
                    kind: ComponentKind::File,
                    installed: true,
                    version: "20260101-aaa".into(),
                },
                ComponentListItem {
                    name: "tabs".into(),
                    kind: ComponentKind::Dir,
                    installed: false,
                    version: "20260102-bbb".into(),
                },
            ]
        );

        assert_eq!(
            listing.themes,
            vec![
                SimpleListItem {
                    name: "dark".into(),
                    installed: true,
                    version: "20260103-ccc".into(),
                },
                SimpleListItem {
                    name: "light".into(),
                    installed: false,
                    version: "20260104-ddd".into(),
                },
            ]
        );

        assert_eq!(
            listing.tags,
            vec![SimpleListItem {
                name: "badge".into(),
                installed: true,
                version: "20260105-eee".into(),
            }]
        );

        assert_eq!(
            listing.utils,
            vec![SimpleListItem {
                name: "utils".into(),
                installed: true,
                version: "20260105-fff".into(),
            }]
        );
    }
}

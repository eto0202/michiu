use crate::{config::Setting, registry::Registry};
use colored::*;

pub fn execute() -> Result<(), Box<dyn std::error::Error>> {
    let setting = Setting::load()?;
    println!(
        "{} (repo: {}@{})...",
        "Checking for updates".cyan().bold(),
        setting.registry.repo.bold(),
        setting.registry.branch.bold()
    );

    let registry = Registry::fetch(&setting.registry.repo, &setting.registry.branch)?;
    let mut update_count = 0;

    println!(
        "\n{:<12} {:<24} {:<18} {:<18} {}",
        "CATEGORY".bold(),
        "NAME".bold(),
        "INSTALLED".bold(),
        "LATEST".bold(),
        "STATUS".bold()
    );
    println!("{}", "-".repeat(88).dimmed());

    for (name, local_ver) in &setting.installed.components {
        let (latest_ver, status) = match registry.components.get(name) {
            Some(comp) => {
                if comp.version == *local_ver {
                    (comp.version.as_str(), "Up to date".green())
                } else {
                    update_count += 1;
                    (comp.version.as_str(), "Update available!".yellow().bold())
                }
            }
            None => ("-", "Not in registry".dimmed()),
        };
        println!(
            "{:<12} {:<24} {:<18} {:<18} {}",
            "component",
            name,
            local_ver.dimmed(),
            latest_ver.dimmed(),
            status
        );
    }

    for (name, local_ver) in &setting.installed.themes {
        let (latest_ver, status) = match registry.themes.get(name) {
            Some(theme) => {
                if theme.version == *local_ver {
                    (theme.version.as_str(), "Up to date".green())
                } else {
                    update_count += 1;
                    (theme.version.as_str(), "Update available!".yellow().bold())
                }
            }
            None => ("-", "Not in registry".dimmed()),
        };
        println!(
            "{:<12} {:<24} {:<18} {:<18} {}",
            "theme",
            name,
            local_ver.dimmed(),
            latest_ver.dimmed(),
            status
        );
    }

    for (name, local_ver) in &setting.installed.tags {
        let (latest_ver, status) = match registry.tags.get(name) {
            Some(tag) => {
                if tag.version == *local_ver {
                    (tag.version.as_str(), "Up to date".green())
                } else {
                    update_count += 1;
                    (tag.version.as_str(), "Update available!".yellow().bold())
                }
            }
            None => ("-", "Not in registry".dimmed()),
        };
        println!(
            "{:<12} {:<24} {:<18} {:<18} {}",
            "tag",
            name,
            local_ver.dimmed(),
            latest_ver.dimmed(),
            status
        );
    }

    for (name, local_ver) in &setting.installed.utils {
        let (latest_ver, status) = match registry.utils.get(name) {
            Some(util) => {
                if util.version == *local_ver {
                    (util.version.as_str(), "Up to date".green())
                } else {
                    update_count += 1;
                    (util.version.as_str(), "Update available!".yellow().bold())
                }
            }
            None => ("-", "Not in registry".dimmed()),
        };
        println!(
            "{:<12} {:<24} {:<18} {:<18} {}",
            "utils",
            name,
            local_ver.dimmed(),
            latest_ver.dimmed(),
            status
        );
    }

    for (name, asset_info) in &setting.installed.assets {
        let (status, is_up) = match registry.assets.get(name) {
            Some(asset) => {
                if asset.hash == asset_info.hash {
                    ("Up to date".green(), true)
                } else {
                    update_count += 1;
                    ("Update available!".yellow().bold(), false)
                }
            }
            None => ("Not in registry".dimmed(), true),
        };
        let hash_short = &asset_info.hash[..8.min(asset_info.hash.len())];
        let latest_hash_short = registry
            .assets
            .get(name)
            .map(|a| &a.hash[..8.min(a.hash.len())])
            .unwrap_or("-");

        println!(
            "{:<12} {:<24} {:<18} {:<18} {}",
            "asset",
            name,
            hash_short.dimmed(),
            latest_hash_short.dimmed(),
            status
        );
    }

    println!("{}", "-".repeat(88).dimmed());
    if update_count > 0 {
        println!(
            "{}",
            format!("{} update(s) available!", update_count)
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

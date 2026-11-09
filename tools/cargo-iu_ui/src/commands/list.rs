use crate::{config::Setting, registry::Registry};
use colored::*;

pub fn execute() -> Result<(), Box<dyn std::error::Error>> {
    let setting = Setting::load()?;
    println!(
        "{} (repo: {}@{})...",
        "Fetching catalog from GitHub".cyan(),
        setting.registry.repo.bold(),
        setting.registry.branch.bold()
    );

    let registry = Registry::fetch(&setting.registry.repo, &setting.registry.branch)?;

    println!("\n{}", "=== Available Components ===".green().bold());
    for (name, comp) in &registry.components {
        let is_installed = setting.installed.components.contains_key(name);
        let status = if is_installed {
            "[x] Installed".green()
        } else {
            "[ ] Not installed".dimmed()
        };

        let comp_type = if comp.comp_type == "dir" {
            "(dir)".blue()
        } else {
            "(file)".magenta()
        };

        println!(
            "  {:<24} {:<8} {:<16} v:{}",
            name.bold(),
            comp_type,
            status,
            comp.version.dimmed()
        );
    }

    if !registry.themes.is_empty() {
        println!("\n{}", "=== Available Themes ===".green().bold());
        for (name, theme) in &registry.themes {
            let is_installed = setting.installed.themes.contains_key(name);
            let status = if is_installed {
                "[x] Installed".green()
            } else {
                "[ ] Not installed".dimmed()
            };
            println!(
                "  {:<24} {:<16} v:{}",
                name.bold(),
                status,
                theme.version.dimmed()
            );
        }
    }

    if !registry.tags.is_empty() {
        println!("\n{}", "=== Available Tags ===".green().bold());
        for (name, tag) in &registry.tags {
            let is_installed = setting.installed.tags.contains_key(name);
            let status = if is_installed {
                "[x] Installed".green()
            } else {
                "[ ] Not installed".dimmed()
            };
            println!(
                "  {:<24} {:<16} v:{}",
                name.bold(),
                status,
                tag.version.dimmed()
            );
        }
    }

    println!(
        "\nTip: Run {} to install a component.",
        "cargo iu_ui add <name>".cyan()
    );
    Ok(())
}

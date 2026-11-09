use crate::config::{CONFIG_FILE_NAME, ProjectConfig, RegistryConfig, Setting};
use colored::*;
use inquire::Text;
use std::{fs, path::Path};

pub fn execute() -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", "Initializing iu_ui project...".cyan().bold());

    if Path::new(CONFIG_FILE_NAME).exists() {
        println!("{}", "[Warning] `iu_ui.toml` already exists.".yellow());
        return Ok(());
    }

    // 対話プロンプトでベースパスを確認
    let default_base = "src/iu_ui";
    let base_dir = Text::new("Where should iu_ui components be installed?")
        .with_default(default_base)
        .with_help_message("Press <Enter> to accept default (src/iu_ui)")
        .prompt()?;

    let project = ProjectConfig {
        base_dir: base_dir.clone(),
        components_dir: format!("{}/components", base_dir),
        theme_dir: format!("{}/theme", base_dir),
        tag_dir: format!("{}/tag", base_dir),
        utils_dir: format!("{}/utils", base_dir),
        assets_dir: format!("{}/assets", base_dir),
    };

    let default_repo = "eto0202/michiu";
    let repo = Text::new("GitHub repository:")
        .with_default(default_repo)
        .with_help_message("Press <Enter> to accept default repository")
        .prompt()?;

    let setting = Setting {
        project: project.clone(),
        registry: RegistryConfig {
            repo,
            branch: "main".to_string(),
        },
        installed: Default::default(),
    };

    println!("Creating directories...");
    fs::create_dir_all(&project.components_dir)?;
    fs::create_dir_all(&project.theme_dir)?;
    fs::create_dir_all(&project.tag_dir)?;
    fs::create_dir_all(&project.utils_dir)?;
    fs::create_dir_all(&project.assets_dir)?;

    // components/mod.rs などの空ファイルを初期化
    let components_mod = Path::new(&project.components_dir).join("mod.rs");
    if !components_mod.exists() {
        fs::write(&components_mod, "// Managed by iu_ui\n")?;
    }
    let theme_mod = Path::new(&project.theme_dir).join("mod.rs");
    if !theme_mod.exists() {
        fs::write(&theme_mod, "// Managed by iu_ui\n")?;
    }
    let tag_mod = Path::new(&project.tag_dir).join("mod.rs");
    if !tag_mod.exists() {
        fs::write(&tag_mod, "// Managed by iu_ui\n")?;
    }
    let utils_mod = Path::new(&project.utils_dir).join("mod.rs");
    if !utils_mod.exists() {
        fs::write(&utils_mod, "// Managed by iu_ui\n")?;
    }

    setting.save()?;

    println!("{}", "Successfully initialized iu_ui!".green().bold());
    println!("Created:");
    println!("  - {}", CONFIG_FILE_NAME.bold());
    println!("  - {}/", project.base_dir.bold());
    println!("\nNext step:");
    println!("  Run {} to browse components!", "cargo iu_ui list".cyan());

    Ok(())
}

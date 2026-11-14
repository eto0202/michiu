#![allow(
    unused,
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic
)]

mod commands;
mod config;
mod registry;

use clap::{Parser, Subcommand};
use colored::Colorize;

use crate::commands::resource::ResourceKind;

#[derive(Debug, Parser)]
#[command(name = "cargo")]
#[command(bin_name = "cargo")]
enum CargoCli {
    #[command(name = "iu_ui")]
    IuUi(IuUiArgs),
}

#[derive(Debug, clap::Args)]
#[command(author, version, about = "michiu component catalog CLI for Rust (iu_ui)", long_about = None)]
struct IuUiArgs {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Initialize the folder structure and `iu_ui.toml`
    Init,
    /// List items in the remote catalog
    List,
    /// Check for updates to installed components
    Check,
    /// Add components
    Add {
        /// Names of components to add (can specify multiple)
        components: Vec<String>,
        /// Add all components
        #[arg(long)]
        all: bool,
    },
    /// Add theme definitions
    Theme {
        /// Names of themes to add (can specify multiple)
        themes: Vec<String>,
        /// Add all themes
        #[arg(long)]
        all: bool,
    },
    /// Add inference tags
    Tag {
        /// Names of tags to add (can specify multiple)
        tags: Vec<String>,
        /// Add all tags
        #[arg(long)]
        all: bool,
    },
    /// Add utilities
    Utils {
        /// Names of utilities to add (can specify multiple)
        utils: Vec<String>,
        /// Add all utilities
        #[arg(long)]
        all: bool,
    },
}

fn main() {
    let CargoCli::IuUi(args) = CargoCli::parse();

    let result = match args.command {
        Commands::Init => commands::init::execute(),
        Commands::List => commands::list::execute(),
        Commands::Add { components, all } => commands::add::execute(components, all),
        Commands::Theme { themes, all } => {
            commands::resource::execute(&ResourceKind::Theme, themes, all)
        }
        Commands::Tag { tags, all } => commands::resource::execute(&ResourceKind::Tag, tags, all),
        Commands::Utils { utils, all } => {
            commands::resource::execute(&ResourceKind::Utils, utils, all)
        }
        Commands::Check => commands::check::execute(),
    };

    if let Err(err) = result {
        eprintln!("{} {err}", "Error:".red().bold());
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn verify_cli_definition() {
        // clap のサブコマンド定義やフラグに矛盾がないか検証
        CargoCli::command().debug_assert();
    }

    #[test]
    fn test_parse_simple_subcommands() {
        let CargoCli::IuUi(init_args) =
            CargoCli::try_parse_from(["cargo", "iu_ui", "init"]).unwrap();
        assert!(matches!(init_args.command, Commands::Init));

        let CargoCli::IuUi(list_args) =
            CargoCli::try_parse_from(["cargo", "iu_ui", "list"]).unwrap();
        assert!(matches!(list_args.command, Commands::List));

        let CargoCli::IuUi(check_args) =
            CargoCli::try_parse_from(["cargo", "iu_ui", "check"]).unwrap();
        assert!(matches!(check_args.command, Commands::Check));
    }

    #[test]
    fn test_parse_add_subcommand() {
        // 複数コンポーネント指定
        let CargoCli::IuUi(args) =
            CargoCli::try_parse_from(["cargo", "iu_ui", "add", "button", "tabs"]).unwrap();
        match args.command {
            Commands::Add { components, all } => {
                assert_eq!(components, vec!["button", "tabs"]);
                assert!(!all);
            }
            other => panic!("unexpected command: {other:?}"),
        }

        // --all 指定
        let CargoCli::IuUi(args_all) =
            CargoCli::try_parse_from(["cargo", "iu_ui", "add", "--all"]).unwrap();
        match args_all.command {
            Commands::Add { components, all } => {
                assert!(components.is_empty());
                assert!(all);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn test_parse_resource_subcommands() {
        // theme
        let CargoCli::IuUi(theme_args) =
            CargoCli::try_parse_from(["cargo", "iu_ui", "theme", "dark", "light"]).unwrap();
        match theme_args.command {
            Commands::Theme { themes, all } => {
                assert_eq!(themes, vec!["dark", "light"]);
                assert!(!all);
            }
            other => panic!("unexpected command: {other:?}"),
        }

        // tag --all
        let CargoCli::IuUi(tag_args) =
            CargoCli::try_parse_from(["cargo", "iu_ui", "tag", "--all"]).unwrap();
        match tag_args.command {
            Commands::Tag { tags, all } => {
                assert!(tags.is_empty());
                assert!(all);
            }
            other => panic!("unexpected command: {other:?}"),
        }

        // utils
        let CargoCli::IuUi(utils_args) =
            CargoCli::try_parse_from(["cargo", "iu_ui", "utils", "cn", "--all"]).unwrap();
        match utils_args.command {
            Commands::Utils { utils, all } => {
                assert_eq!(utils, vec!["cn"]);
                assert!(all);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn test_parse_invalid_arguments_fail() {
        // サブコマンドなし
        assert!(CargoCli::try_parse_from(["cargo", "iu_ui"]).is_err());

        // `iu_ui` サブコマンド名なし（Cargo サブコマンド形式違反）
        assert!(CargoCli::try_parse_from(["cargo", "init"]).is_err());

        // 存在しないサブコマンド
        assert!(CargoCli::try_parse_from(["cargo", "iu_ui", "unknown"]).is_err());

        // 存在しないフラグ
        assert!(CargoCli::try_parse_from(["cargo", "iu_ui", "add", "--invalid"]).is_err());
    }
}

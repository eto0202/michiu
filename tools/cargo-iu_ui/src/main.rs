#![allow(unused)]

mod commands;
mod config;
mod registry;

use clap::{Parser, Subcommand};
use colored::*;

#[derive(Parser)]
#[command(name = "cargo")]
#[command(bin_name = "cargo")]
enum CargoCli {
    #[command(name = "iu_ui")]
    IuUi(IuUiArgs),
}

#[derive(clap::Args)]
#[command(author, version, about = "michiu component catalog CLI for Rust (iu_ui)", long_about = None)]
struct IuUiArgs {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize the folder structure and iu_ui.toml
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
            commands::resource::execute(commands::resource::ResourceKind::Theme, themes, all)
        }
        Commands::Tag { tags, all } => {
            commands::resource::execute(commands::resource::ResourceKind::Tag, tags, all)
        }
        Commands::Utils { utils, all } => {
            commands::resource::execute(commands::resource::ResourceKind::Utils, utils, all)
        }
        Commands::Check => commands::check::execute(),
    };

    if let Err(err) = result {
        eprintln!("{} {}", "Error:".red().bold(), err);
        std::process::exit(1);
    }
}

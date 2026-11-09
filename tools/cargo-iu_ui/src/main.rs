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
    // フォルダ構成と setting.toml を初期化する
    Init,
    // コンポーネントの追加 (実装予定)
    Add {
        // 追加するコンポーネント名
        components: Vec<String>,
        #[arg(long)]
        all: bool,
    },
    // リモートカタログの一覧を表示
    List,
    // 更新を確認する (実装予定)
    Check,
}

fn main() {
    let CargoCli::IuUi(args) = CargoCli::parse();

    let result = match args.command {
        Commands::Init => commands::init::execute(),
        Commands::List => commands::list::execute(),
        Commands::Add { .. } => {
            println!("{}", "Coming soon: add command".yellow());
            Ok(())
        }
        Commands::Check => {
            println!("{}", "Coming soon: check command".yellow());
            Ok(())
        }
    };

    if let Err(err) = result {
        eprintln!("{} {}", "Error:".red().bold(), err);
        std::process::exit(1);
    }
}

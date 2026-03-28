use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(
    name = "patch",
    version,
    about = "Tree-sitter indexed lookups for AI agents"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,

    #[arg(long, global = true)]
    pub json: bool,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Read(ReadArgs),
    #[command(subcommand)]
    Symbol(SymbolCommand),
    #[command(subcommand)]
    Search(SearchCommand),
    Files(FilesArgs),
    Deps(DepsArgs),
    Map(MapArgs),
}

#[derive(Debug, Args)]
pub struct ReadArgs {
    pub path: PathBuf,

    #[arg(long, value_name = "START:END", conflicts_with = "heading")]
    pub lines: Option<String>,

    #[arg(long, conflicts_with = "lines")]
    pub heading: Option<String>,

    #[arg(long)]
    pub full: bool,

    #[arg(long, conflicts_with_all = ["lines", "heading"])]
    pub key: Option<String>,

    #[arg(
        long,
        value_name = "START:END",
        conflicts_with_all = ["lines", "heading"]
    )]
    pub index: Option<String>,

    #[arg(long)]
    pub budget: Option<u64>,
}

#[derive(Debug, Subcommand)]
pub enum SymbolCommand {
    Find(SymbolFindArgs),
    Callers(SymbolCallersArgs),
}

#[derive(Debug, Args)]
pub struct SymbolFindArgs {
    pub query: String,

    #[arg(long, default_value = ".")]
    pub scope: PathBuf,

    #[arg(long, value_enum)]
    pub kind: Option<SymbolFindKind>,

    #[arg(long)]
    pub budget: Option<u64>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
#[value(rename_all = "snake_case")]
pub enum SymbolFindKind {
    Definition,
    Usage,
}

#[derive(Debug, Args)]
pub struct SymbolCallersArgs {
    pub query: String,

    #[arg(long, default_value = ".")]
    pub scope: PathBuf,

    #[arg(long)]
    pub budget: Option<u64>,
}

#[derive(Debug, Subcommand)]
pub enum SearchCommand {
    Text(SearchTextArgs),
    Regex(SearchRegexArgs),
}

#[derive(Debug, Args)]
pub struct SearchTextArgs {
    pub query: String,

    #[arg(long, default_value = ".")]
    pub scope: PathBuf,

    #[arg(long)]
    pub budget: Option<u64>,
}

#[derive(Debug, Args)]
pub struct SearchRegexArgs {
    pub pattern: String,

    #[arg(long, default_value = ".")]
    pub scope: PathBuf,

    #[arg(long)]
    pub budget: Option<u64>,
}

#[derive(Debug, Args)]
pub struct FilesArgs {
    pub pattern: String,

    #[arg(long, default_value = ".")]
    pub scope: PathBuf,

    #[arg(long)]
    pub budget: Option<u64>,
}

#[derive(Debug, Args)]
pub struct DepsArgs {
    pub path: PathBuf,

    #[arg(long, default_value = ".")]
    pub scope: PathBuf,

    #[arg(long)]
    pub budget: Option<u64>,
}

#[derive(Debug, Args)]
pub struct MapArgs {
    #[arg(long, default_value = ".")]
    pub scope: PathBuf,

    #[arg(long, default_value_t = 3)]
    pub depth: usize,

    #[arg(long)]
    pub budget: Option<u64>,
}

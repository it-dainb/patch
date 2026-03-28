use std::io::IsTerminal;

use crate::cli::args::{Cli, Command, SearchCommand, SymbolCommand};
use crate::commands;
use crate::error::DrailError;
use crate::output;

pub fn run(cli: &Cli) -> Result<(), DrailError> {
    let rendered = match cli.command {
        Command::Read(ref args) => commands::read::run(args)?,
        Command::Symbol(SymbolCommand::Find(ref args)) => commands::symbol::find::run(args)?,
        Command::Symbol(SymbolCommand::Callers(ref args)) => commands::symbol::callers::run(args)?,
        Command::Search(SearchCommand::Text(ref args)) => commands::search::text::run(args)?,
        Command::Search(SearchCommand::Regex(ref args)) => commands::search::regex::run(args)?,
        Command::Files(ref args) => commands::files::run(args)?,
        Command::Deps(ref args) => commands::deps::run(args)?,
        Command::Map(ref args) => commands::map::run(args)?,
    };

    output::write(&rendered, cli.json, std::io::stdout().is_terminal());
    Ok(())
}

#[must_use]
pub fn command_name(command: &Command) -> &'static str {
    match command {
        Command::Read(_) => "read",
        Command::Symbol(SymbolCommand::Find(_)) => "symbol.find",
        Command::Symbol(SymbolCommand::Callers(_)) => "symbol.callers",
        Command::Search(SearchCommand::Text(_)) => "search.text",
        Command::Search(SearchCommand::Regex(_)) => "search.regex",
        Command::Files(_) => "files",
        Command::Deps(_) => "deps",
        Command::Map(_) => "map",
    }
}

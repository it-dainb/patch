pub mod args;
pub mod dispatch;

use std::ffi::OsString;
use std::io::IsTerminal;

use clap::Parser;

use crate::error::PatchError;
use crate::output::{self, CommandOutput};

pub fn run() -> Result<(), PatchError> {
    let argv: Vec<OsString> = std::env::args_os().collect();
    let cli = args::Cli::try_parse_from(&argv).map_err(|error| PatchError::Clap {
        message: error.to_string().replace("Usage:", "USAGE:"),
        exit_code: error.exit_code(),
    })?;

    match dispatch::run(&cli) {
        Ok(()) => Ok(()),
        Err(error) => {
            let cli = args::Cli::try_parse_from(&argv).map_err(|parse_error| PatchError::Clap {
                message: parse_error.to_string().replace("Usage:", "USAGE:"),
                exit_code: parse_error.exit_code(),
            })?;

            if cli.json {
                let output =
                    CommandOutput::from_error(dispatch::command_name(&cli.command), &error);
                output::write(&output, true, std::io::stdout().is_terminal());
                Ok(())
            } else {
                let exit_code = error.exit_code();
                let output =
                    CommandOutput::from_error(dispatch::command_name(&cli.command), &error);
                output::write_error(&output, false, std::io::stderr().is_terminal());
                Err(PatchError::AlreadyReported { exit_code })
            }
        }
    }
}

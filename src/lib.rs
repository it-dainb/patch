#![warn(clippy::pedantic)]
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::module_name_repetitions,
    clippy::similar_names,
    clippy::too_many_lines,
    clippy::too_many_arguments,
    clippy::unnecessary_wraps,
    clippy::struct_excessive_bools,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc
)]

pub(crate) mod budget;
pub mod cache;
pub mod cli;
pub mod commands;
pub mod engine;
pub mod error;
#[allow(dead_code)]
pub(crate) mod format;
pub mod index;
pub mod map;
pub(crate) mod minified;
pub mod output;
pub(crate) mod read;
#[allow(dead_code)]
pub(crate) mod search;
#[allow(dead_code)]
pub(crate) mod types;

use crate::cli::args::ReadArgs;
use crate::engine::read::{self, ReadSelector};
use crate::error::PatchError;
use crate::output::CommandOutput;
use crate::output::{json, text};
use crate::types::FileType;
use serde_json::{json, Map, Value};

pub fn run(args: &ReadArgs) -> Result<CommandOutput, PatchError> {
    let selector = parse_selector(args)?;
    let result = read::run(&args.path, selector, args.full, args.budget)?;
    let mut output = CommandOutput::with_parts(
        "read",
        text::read::render(&result),
        json::read::render(&result),
        result.diagnostics,
        true,
    );
    output.meta = meta_for_read(&args.path, &result.data.selector)?;

    Ok(output)
}

fn parse_selector(args: &ReadArgs) -> Result<ReadSelector, PatchError> {
    match (&args.lines, &args.heading) {
        (Some(lines), None) => {
            let (start, end) = parse_lines(lines)?;
            Ok(ReadSelector::Lines { start, end })
        }
        (None, Some(heading)) => Ok(ReadSelector::Heading(heading.clone())),
        (None, None) => Ok(ReadSelector::Full),
        (Some(_), Some(_)) => Err(PatchError::InvalidQuery {
            query: "read".into(),
            reason: "--lines cannot be used with --heading".into(),
        }),
    }
}

fn parse_lines(lines: &str) -> Result<(usize, usize), PatchError> {
    let (start, end) = lines
        .split_once(':')
        .ok_or_else(|| PatchError::InvalidQuery {
            query: lines.into(),
            reason: "expected line range in START:END format".into(),
        })?;

    let start = start
        .trim()
        .parse::<usize>()
        .map_err(|_| PatchError::InvalidQuery {
            query: lines.into(),
            reason: "expected line range in START:END format".into(),
        })?;
    let end = end
        .trim()
        .parse::<usize>()
        .map_err(|_| PatchError::InvalidQuery {
            query: lines.into(),
            reason: "expected line range in START:END format".into(),
        })?;

    if start == 0 || end < start {
        return Err(PatchError::InvalidQuery {
            query: lines.into(),
            reason: "line range must start at 1 and end at or after the start line".into(),
        });
    }

    Ok((start, end))
}

fn meta_for_read(
    path: &std::path::Path,
    selector: &read::ReadSelectorData,
) -> Result<Map<String, Value>, PatchError> {
    let mut meta = Map::new();
    let (selector_kind, selector_display, first_line) = match selector {
        read::ReadSelectorData::Full => ("full", "full".to_string(), 1),
        read::ReadSelectorData::Lines { start, end } => ("lines", format!("{start}:{end}"), *start),
        read::ReadSelectorData::Heading { value } => ("heading", value.clone(), 1),
    };

    meta.insert("path".into(), json!(path.display().to_string()));
    meta.insert("selector_kind".into(), json!(selector_kind));
    meta.insert("selector_display".into(), json!(selector_display));
    meta.insert("file_kind".into(), json!(file_kind_label(path)));
    meta.insert("stability".into(), json!("high"));
    meta.insert("noise".into(), json!("low"));
    meta.insert(
        "heading_aligned".into(),
        json!(match selector {
            read::ReadSelectorData::Heading { .. } => true,
            _ => crate::read::is_markdown_heading_line(path, first_line)?,
        }),
    );

    Ok(meta)
}

fn file_kind_label(path: &std::path::Path) -> &'static str {
    match crate::read::detect_file_type(path) {
        FileType::Markdown => "markdown",
        FileType::StructuredData => "structured_data",
        FileType::Tabular => "tabular",
        FileType::Log => "log",
        FileType::Code(_) => "code",
        FileType::Other => "other",
    }
}

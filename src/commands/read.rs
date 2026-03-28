use crate::cli::args::ReadArgs;
use crate::engine::read::{self, ReadSelector};
use crate::error::PatchError;
use crate::output::json::envelope::NextItem;
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
    output.next = next_for_read(&args.path, &result.data.selector, &output.meta);

    Ok(output)
}

fn next_for_read(
    path: &std::path::Path,
    selector: &read::ReadSelectorData,
    meta: &Map<String, Value>,
) -> Vec<NextItem> {
    let file_kind = meta.get("file_kind").and_then(Value::as_str);
    let heading_aligned = meta
        .get("heading_aligned")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    match selector {
        read::ReadSelectorData::Lines { start, .. }
            if file_kind == Some("markdown") && heading_aligned =>
        {
            let Ok(Some(heading_line)) = crate::read::markdown_heading_line_text(path, *start)
            else {
                return Vec::new();
            };

            vec![crate::output::suggestion(
                format!("Read the full markdown section starting at line {start} with --heading"),
                format!(
                    "patch read {:?} --heading {:?}",
                    path.display().to_string(),
                    heading_line
                ),
            )]
        }
        _ => Vec::new(),
    }
}

fn parse_selector(args: &ReadArgs) -> Result<ReadSelector, PatchError> {
    if args.key.is_some() && !is_json_path(&args.path) {
        return Err(PatchError::InvalidQuery {
            query: "--key".into(),
            reason: "--key is only supported for JSON files".into(),
        });
    }

    if args.index.is_some() && !is_json_path(&args.path) {
        return Err(PatchError::InvalidQuery {
            query: "--index".into(),
            reason: "--index is only supported for JSON files".into(),
        });
    }

    match (&args.lines, &args.heading, &args.key, &args.index) {
        (Some(lines), None, None, None) => {
            let (start, end) = parse_lines(lines)?;
            Ok(ReadSelector::Lines { start, end })
        }
        (None, Some(heading), None, None) => Ok(ReadSelector::Heading(heading.clone())),
        (None, None, Some(key), Some(index)) => {
            let (start, end) = parse_index(index)?;
            Ok(ReadSelector::KeyIndex {
                value: key.clone(),
                start,
                end,
            })
        }
        (None, None, Some(key), None) => Ok(ReadSelector::Key { value: key.clone() }),
        (None, None, None, Some(index)) => {
            let (start, end) = parse_index(index)?;
            Ok(ReadSelector::Index { start, end })
        }
        (None, None, None, None) => Ok(ReadSelector::Full),
        _ => Err(PatchError::InvalidQuery {
            query: "read".into(),
            reason: "invalid selector combination".into(),
        }),
    }
}

fn is_json_path(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
}

fn parse_index(index: &str) -> Result<(usize, usize), PatchError> {
    let (start, end) = index
        .split_once(':')
        .ok_or_else(|| PatchError::InvalidQuery {
            query: index.into(),
            reason: "expected index range in START:END format".into(),
        })?;

    if start.trim().is_empty() || end.trim().is_empty() || end.contains(':') {
        return Err(PatchError::InvalidQuery {
            query: index.into(),
            reason: "expected index range in START:END format".into(),
        });
    }

    let start = start
        .trim()
        .parse::<usize>()
        .map_err(|_| PatchError::InvalidQuery {
            query: index.into(),
            reason: "expected index range in START:END format".into(),
        })?;
    let end = end
        .trim()
        .parse::<usize>()
        .map_err(|_| PatchError::InvalidQuery {
            query: index.into(),
            reason: "expected index range in START:END format".into(),
        })?;

    Ok((start, end))
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
        read::ReadSelectorData::Key { value } => ("key", value.clone(), 1),
        read::ReadSelectorData::Index { start, end } => ("index", format!("{start}:{end}"), 1),
        read::ReadSelectorData::KeyIndex { value, start, end } => {
            ("key_index", format!("{value} @ {start}:{end}"), 1)
        }
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

use std::path::Path;

use serde::Serialize;

use crate::cache::OutlineCache;
use crate::error::DrailError;
use crate::output::json::envelope::{Diagnostic, DiagnosticLevel};

#[derive(Debug, Clone)]
pub enum ReadSelector {
    Full,
    Lines {
        start: usize,
        end: usize,
    },
    Heading(String),
    Key {
        value: String,
    },
    Index {
        start: usize,
        end: usize,
    },
    KeyIndex {
        value: String,
        start: usize,
        end: usize,
    },
}

#[derive(Debug, Clone, Serialize)]
pub enum ReadSelectorData {
    #[serde(rename = "full")]
    Full,
    #[serde(rename = "lines")]
    Lines {
        start: usize,
        end: usize,
    },
    #[serde(rename = "heading")]
    Heading {
        value: String,
    },
    Key {
        value: String,
    },
    Index {
        start: usize,
        end: usize,
    },
    KeyIndex {
        value: String,
        start: usize,
        end: usize,
    },
}

#[derive(Debug, Clone, Serialize)]
pub struct ReadResultData {
    pub path: String,
    pub selector: ReadSelectorData,
    pub content: String,
}

#[derive(Debug, Clone)]
pub struct ReadCommandResult {
    pub data: ReadResultData,
    pub diagnostics: Vec<Diagnostic>,
}

pub fn run(
    path: &Path,
    selector: ReadSelector,
    full: bool,
    budget: Option<u64>,
) -> Result<ReadCommandResult, DrailError> {
    let (content, minified_fallback_used) = read_content(path, &selector, full, budget)?;

    let selector = match selector {
        ReadSelector::Full => ReadSelectorData::Full,
        ReadSelector::Lines { start, end } => ReadSelectorData::Lines { start, end },
        ReadSelector::Heading(value) => ReadSelectorData::Heading { value },
        ReadSelector::Key { value } => ReadSelectorData::Key { value },
        ReadSelector::Index { start, end } => ReadSelectorData::Index { start, end },
        ReadSelector::KeyIndex { value, start, end } => {
            ReadSelectorData::KeyIndex { value, start, end }
        }
    };

    let mut diagnostics = Vec::new();
    if minified_fallback_used {
        diagnostics.push(Diagnostic {
            level: DiagnosticLevel::Warning,
            code: "minified_fallback_used".into(),
            message: "content appears minified; showing a bounded preview instead of raw content"
                .into(),
            suggestion: None,
        });
    }

    Ok(ReadCommandResult {
        data: ReadResultData {
            path: path.display().to_string(),
            selector,
            content,
        },
        diagnostics,
    })
}

fn read_content(
    path: &Path,
    selector: &ReadSelector,
    full: bool,
    budget: Option<u64>,
) -> Result<(String, bool), DrailError> {
    if matches!(
        selector,
        ReadSelector::Key { .. } | ReadSelector::Index { .. } | ReadSelector::KeyIndex { .. }
    ) && !is_json_file(path)
    {
        let (query, reason) = match selector {
            ReadSelector::Key { .. } => ("--key", "--key is only supported for JSON files"),
            ReadSelector::Index { .. } | ReadSelector::KeyIndex { .. } => {
                ("--index", "--index is only supported for JSON files")
            }
            _ => unreachable!(),
        };
        return Err(DrailError::InvalidQuery {
            query: query.into(),
            reason: reason.into(),
        });
    }

    let cache = OutlineCache::new();

    let section_selector = match selector {
        ReadSelector::Lines { start, end } => Some(crate::read::SectionSelector::Lines {
            start: *start,
            end: *end,
        }),
        ReadSelector::Heading(heading) => {
            let file_type = crate::read::detect_file_type(path);
            if !matches!(file_type, crate::types::FileType::Markdown) {
                return Err(DrailError::InvalidQuery {
                    query: heading.clone(),
                    reason: "heading selectors are only supported for markdown files".into(),
                });
            }
            Some(crate::read::SectionSelector::Heading(heading.clone()))
        }
        _ => None,
    };

    let json_selector = match selector {
        ReadSelector::Full => Some(crate::read::JsonSelector::Full),
        ReadSelector::Key { value } => Some(crate::read::JsonSelector::Key(value.clone())),
        ReadSelector::Index { start, end } => Some(crate::read::JsonSelector::Index {
            start: *start,
            end: *end,
        }),
        ReadSelector::KeyIndex { value, start, end } => Some(crate::read::JsonSelector::KeyIndex {
            key: value.clone(),
            start: *start,
            end: *end,
        }),
        _ => None,
    };

    let output = crate::read::read_file(
        path,
        section_selector.as_ref(),
        json_selector.as_ref(),
        full,
        &cache,
        false,
    )?;

    let content = match budget {
        Some(budget) => crate::budget::apply(&output.content, budget),
        None => output.content,
    };

    Ok((content, output.minified_fallback_used))
}

fn is_json_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
}

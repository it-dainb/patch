use std::path::Path;

use serde::Serialize;

use crate::cache::OutlineCache;
use crate::error::PatchError;
use crate::output::json::envelope::Diagnostic;

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
) -> Result<ReadCommandResult, PatchError> {
    let content = read_content(path, &selector, full, budget)?;

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

    Ok(ReadCommandResult {
        data: ReadResultData {
            path: path.display().to_string(),
            selector,
            content,
        },
        diagnostics: Vec::new(),
    })
}

fn read_content(
    path: &Path,
    selector: &ReadSelector,
    full: bool,
    budget: Option<u64>,
) -> Result<String, PatchError> {
    let content = if should_render_json_as_toon(path, selector) {
        read_json_content(path, selector)?
    } else {
        let cache = OutlineCache::new();
        let legacy_section = match selector {
            ReadSelector::Full
            | ReadSelector::Key { .. }
            | ReadSelector::Index { .. }
            | ReadSelector::KeyIndex { .. } => None,
            ReadSelector::Lines { start, end } => Some(format!("{start}-{end}")),
            ReadSelector::Heading(heading) => {
                let file_type = crate::read::detect_file_type(path);
                if !matches!(file_type, crate::types::FileType::Markdown) {
                    return Err(PatchError::InvalidQuery {
                        query: heading.clone(),
                        reason: "heading selectors are only supported for markdown files".into(),
                    });
                }
                Some(heading.clone())
            }
        };
        crate::read::read_file(path, legacy_section.as_deref(), full, &cache, false)?
    };

    Ok(match budget {
        Some(budget) => crate::budget::apply(&content, budget),
        None => content,
    })
}

fn should_render_json_as_toon(path: &Path, selector: &ReadSelector) -> bool {
    is_json_file(path)
        && !matches!(
            selector,
            ReadSelector::Lines { .. } | ReadSelector::Heading(_)
        )
}

fn is_json_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
}

fn read_json_content(path: &Path, selector: &ReadSelector) -> Result<String, PatchError> {
    let source = std::fs::read_to_string(path).map_err(|source| PatchError::IoError {
        path: path.to_path_buf(),
        source,
    })?;

    let parsed = crate::read::json::parse_json(&source)
        .map_err(|error| map_json_error(path, error.to_string()))?;

    let selected = match selector {
        ReadSelector::Full => parsed,
        ReadSelector::Key { value } => crate::read::json::resolve_path(&parsed, value)
            .map(|resolved| resolved.clone())
            .map_err(|error| map_json_error(path, error.to_string()))?,
        ReadSelector::Index { start, end } => {
            let range = format!("{start}:{end}");
            let window = crate::read::json::slice_array(&parsed, &range)
                .map_err(|error| map_json_error(path, error.to_string()))?;
            serde_json::Value::Array(window.to_vec())
        }
        ReadSelector::KeyIndex { value, start, end } => {
            let range = format!("{start}:{end}");
            let resolved = crate::read::json::resolve_path(&parsed, value)
                .map_err(|error| map_json_error(path, error.to_string()))?;
            let window = crate::read::json::slice_array(resolved, &range)
                .map_err(|error| map_json_error(path, error.to_string()))?;
            serde_json::Value::Array(window.to_vec())
        }
        ReadSelector::Lines { .. } | ReadSelector::Heading(_) => unreachable!(),
    };

    crate::read::json::encode_to_toon(&selected)
        .map_err(|error| map_json_error(path, error.to_string()))
}

fn map_json_error(path: &Path, reason: String) -> PatchError {
    PatchError::ParseError {
        path: path.to_path_buf(),
        reason,
    }
}

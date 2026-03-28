use std::path::Path;

use serde::Serialize;

use crate::error::PatchError;
use crate::output::json::envelope::{Diagnostic, DiagnosticLevel};

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SearchMode {
    Text,
    Regex,
}

#[derive(Debug, Clone, Serialize)]
pub struct SearchMatch {
    pub path: String,
    pub line: usize,
    pub text: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SearchData {
    pub query: String,
    pub scope: String,
    pub mode: SearchMode,
    pub matches: Vec<SearchMatch>,
}

#[derive(Debug, Clone)]
pub struct SearchCommandResult {
    pub data: SearchData,
    pub diagnostics: Vec<Diagnostic>,
}

pub fn run_text(
    query: &str,
    scope: &Path,
    budget: Option<u64>,
) -> Result<SearchCommandResult, PatchError> {
    run(query, scope, SearchMode::Text, budget)
}

pub fn run_regex(
    pattern: &str,
    scope: &Path,
    budget: Option<u64>,
) -> Result<SearchCommandResult, PatchError> {
    run(pattern, scope, SearchMode::Regex, budget)
}

fn run(
    query: &str,
    scope: &Path,
    mode: SearchMode,
    budget: Option<u64>,
) -> Result<SearchCommandResult, PatchError> {
    let scope = crate::engine::resolve_scope(scope);
    let result = match mode {
        SearchMode::Text => crate::search::search_content_raw(query, &scope)?,
        SearchMode::Regex => crate::search::search_regex_raw(query, &scope)?,
    };

    let mut command_result = SearchCommandResult {
        data: SearchData {
            query: query.to_string(),
            scope: scope.display().to_string(),
            mode,
            matches: result
                .matches
                .into_iter()
                .map(|entry| SearchMatch {
                    path: entry
                        .path
                        .strip_prefix(&scope)
                        .unwrap_or(entry.path.as_path())
                        .display()
                        .to_string(),
                    line: entry.line as usize,
                    text: entry.text,
                })
                .collect(),
        },
        diagnostics: diagnostics(query, &scope, mode, result.total_found),
    };

    if let Some(budget) = budget {
        while serde_json::to_string(&command_result.data)
            .expect("search data should serialize")
            .len() as u64
            > budget
            && !command_result.data.matches.is_empty()
        {
            command_result.data.matches.pop();
        }
    }

    Ok(command_result)
}

fn diagnostics(query: &str, scope: &Path, mode: SearchMode, total_found: usize) -> Vec<Diagnostic> {
    if total_found == 0 {
        let suggestion = match mode {
            SearchMode::Text if looks_like_slash_delimited_regex(query) => Some(format!(
                "Try: patch search regex {:?} --scope {}",
                &query[1..query.len() - 1],
                scope.display()
            )),
            _ => None,
        };

        return vec![Diagnostic {
            level: DiagnosticLevel::Hint,
            code: "no_search_matches".into(),
            message: format!("no search matches found for \"{query}\""),
            suggestion,
        }];
    }

    Vec::new()
}

fn looks_like_slash_delimited_regex(query: &str) -> bool {
    query.starts_with('/') && query.ends_with('/') && query.len() > 2
}

use std::path::Path;

use serde::Serialize;

use crate::cli::args::SymbolFindKind;
use crate::error::PatchError;
use crate::output::json::envelope::{Diagnostic, DiagnosticLevel};

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SymbolMatchKind {
    Definition,
    Usage,
}

#[derive(Debug, Clone, Serialize)]
pub struct SymbolRange {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct SymbolMatch {
    pub path: String,
    pub kind: SymbolMatchKind,
    pub range: SymbolRange,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snippet: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SymbolFindData {
    pub query: String,
    pub scope: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<SymbolMatchKind>,
    pub matches: Vec<SymbolMatch>,
}

#[derive(Debug, Clone)]
pub struct SymbolFindCommandResult {
    pub data: SymbolFindData,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SymbolCaller {
    pub path: String,
    pub line: usize,
    pub caller: String,
    pub call_text: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SymbolCallerImpact {
    pub path: String,
    pub line: usize,
    pub caller: String,
    pub via: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SymbolCallersData {
    pub query: String,
    pub scope: String,
    pub callers: Vec<SymbolCaller>,
    pub impact: Vec<SymbolCallerImpact>,
}

#[derive(Debug, Clone)]
pub struct SymbolCallersCommandResult {
    pub data: SymbolCallersData,
    pub truncated: bool,
    pub diagnostics: Vec<Diagnostic>,
}

pub fn run(
    query: &str,
    scope: &Path,
    kind: Option<SymbolFindKind>,
    budget: Option<u64>,
) -> Result<SymbolFindCommandResult, PatchError> {
    let scope = scope.canonicalize().unwrap_or_else(|_| scope.to_path_buf());
    let result = crate::search::search_symbol_raw(query, &scope)?;

    let kind_filter = kind.map(SymbolMatchKind::from);
    let matches = result
        .matches
        .into_iter()
        .filter(|candidate| match kind_filter {
            Some(SymbolMatchKind::Definition) => candidate.is_definition,
            Some(SymbolMatchKind::Usage) => !candidate.is_definition,
            None => true,
        })
        .map(|candidate| SymbolMatch {
            path: candidate
                .path
                .strip_prefix(&scope)
                .unwrap_or(candidate.path.as_path())
                .display()
                .to_string(),
            kind: if candidate.is_definition {
                SymbolMatchKind::Definition
            } else {
                SymbolMatchKind::Usage
            },
            range: match candidate.def_range {
                Some((start, end)) => SymbolRange {
                    start: start as usize,
                    end: end as usize,
                },
                None => SymbolRange {
                    start: candidate.line as usize,
                    end: candidate.line as usize,
                },
            },
            snippet: Some(candidate.text),
        })
        .collect::<Vec<_>>();

    let data = SymbolFindData {
        query: query.to_string(),
        scope: scope.display().to_string(),
        kind: kind_filter,
        matches,
    };

    let diagnostics = if data.matches.is_empty() {
        vec![Diagnostic {
            level: DiagnosticLevel::Hint,
            code: "no_symbol_matches".into(),
            message: format!("no symbol matches found for \"{query}\""),
            suggestion: Some(format!(
                "Try: patch search text {query:?} --scope {}",
                scope.display()
            )),
        }]
    } else {
        Vec::new()
    };

    let mut command_result = SymbolFindCommandResult { data, diagnostics };

    if let Some(budget) = budget {
        while serde_json::to_string(&command_result.data)
            .expect("symbol data should serialize")
            .len() as u64
            > budget
            && !command_result.data.matches.is_empty()
        {
            command_result.data.matches.pop();
        }
    }

    Ok(command_result)
}

pub fn run_callers(
    query: &str,
    scope: &Path,
    budget: Option<u64>,
) -> Result<SymbolCallersCommandResult, PatchError> {
    let scope = scope.canonicalize().unwrap_or_else(|_| scope.to_path_buf());
    let bloom = crate::index::bloom::BloomFilterCache::new();
    let result = crate::search::callers::search_callers_structured(query, &scope, &bloom, None)?;

    let mut command_result = SymbolCallersCommandResult {
        data: SymbolCallersData {
            query: query.to_string(),
            scope: scope.display().to_string(),
            callers: result
                .callers
                .into_iter()
                .map(|caller| SymbolCaller {
                    path: caller
                        .path
                        .strip_prefix(&scope)
                        .unwrap_or(caller.path.as_path())
                        .display()
                        .to_string(),
                    line: caller.line as usize,
                    caller: caller.caller,
                    call_text: caller.call_text,
                })
                .collect(),
            impact: result
                .impact
                .into_iter()
                .map(|entry| SymbolCallerImpact {
                    path: entry
                        .path
                        .strip_prefix(&scope)
                        .unwrap_or(entry.path.as_path())
                        .display()
                        .to_string(),
                    line: entry.line as usize,
                    caller: entry.caller,
                    via: entry.via,
                })
                .collect(),
        },
        truncated: false,
        diagnostics: callers_diagnostics(query, &scope)?,
    };

    if let Some(budget) = budget {
        let original_callers = command_result.data.callers.len();
        let original_impact = command_result.data.impact.len();

        while serde_json::to_string(&command_result.data)
            .expect("symbol callers data should serialize")
            .len() as u64
            > budget
            && !command_result.data.impact.is_empty()
        {
            command_result.data.impact.pop();
        }

        while serde_json::to_string(&command_result.data)
            .expect("symbol callers data should serialize")
            .len() as u64
            > budget
            && !command_result.data.callers.is_empty()
        {
            command_result.data.callers.pop();
        }

        command_result.truncated = command_result.data.callers.len() < original_callers
            || command_result.data.impact.len() < original_impact;
    }

    Ok(command_result)
}

fn callers_diagnostics(query: &str, scope: &Path) -> Result<Vec<Diagnostic>, PatchError> {
    let symbol_result = crate::search::search_symbol_raw(query, scope)?;
    let definition_snippets = symbol_result
        .matches
        .iter()
        .filter(|candidate| candidate.is_definition)
        .map(|candidate| candidate.text.as_str())
        .collect::<Vec<_>>();

    if !definition_snippets.is_empty()
        && definition_snippets
            .iter()
            .all(|snippet| !is_callable_definition(snippet))
    {
        return Ok(vec![Diagnostic {
            level: DiagnosticLevel::Warning,
            code: "callers_relation_not_meaningful".into(),
            message: format!(
                "\"{query}\" is not a callable symbol, so callers may be empty or misleading"
            ),
            suggestion: Some(format!(
                "Try: patch symbol find {query:?} --scope {}",
                scope.display()
            )),
        }]);
    }

    if symbol_result.matches.is_empty() {
        return Ok(vec![Diagnostic {
            level: DiagnosticLevel::Hint,
            code: "no_symbol_matches".into(),
            message: format!("no symbol matches found for \"{query}\""),
            suggestion: Some(format!(
                "Try: patch search text {query:?} --scope {}",
                scope.display()
            )),
        }]);
    }

    Ok(Vec::new())
}

fn is_callable_definition(snippet: &str) -> bool {
    let snippet = snippet.trim_start();
    snippet.starts_with("fn ")
        || snippet.starts_with("pub fn ")
        || snippet.starts_with("pub(crate) fn ")
        || snippet.starts_with("async fn ")
        || snippet.starts_with("pub async fn ")
        || snippet.starts_with("function ")
        || snippet.starts_with("def ")
}

impl From<SymbolFindKind> for SymbolMatchKind {
    fn from(value: SymbolFindKind) -> Self {
        match value {
            SymbolFindKind::Definition => Self::Definition,
            SymbolFindKind::Usage => Self::Usage,
        }
    }
}

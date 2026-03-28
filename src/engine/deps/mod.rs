use std::collections::BTreeMap;
use std::path::Path;

use serde::Serialize;

use crate::error::DrailError;
use crate::output::json::envelope::Diagnostic;

#[derive(Debug, Clone, Serialize)]
pub struct LocalDependency {
    pub path: String,
    pub symbols: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReverseDependencyCaller {
    pub caller: String,
    pub line: usize,
    pub symbols: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReverseDependency {
    pub path: String,
    pub is_test: bool,
    pub callers: Vec<ReverseDependencyCaller>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DepsData {
    pub path: String,
    pub scope: String,
    pub uses_local: Vec<LocalDependency>,
    pub uses_external: Vec<String>,
    pub used_by: Vec<ReverseDependency>,
    pub truncated_dependents: usize,
}

#[derive(Debug, Clone)]
pub struct DepsCommandResult {
    pub data: DepsData,
    pub diagnostics: Vec<Diagnostic>,
}

pub fn run(path: &Path, scope: &Path) -> Result<DepsCommandResult, DrailError> {
    if !path.exists() {
        return Err(DrailError::NotFound {
            path: path.to_path_buf(),
            suggestion: None,
        });
    }

    let scope = crate::engine::resolve_scope(scope);
    let cache = crate::cache::OutlineCache::new();
    let bloom = crate::index::bloom::BloomFilterCache::new();
    let result = crate::search::deps::analyze_deps(path, &scope, &cache, &bloom)?;
    let truncated_dependents = result.total_dependents.saturating_sub(result.used_by.len());

    Ok(DepsCommandResult {
        data: DepsData {
            path: result
                .target
                .strip_prefix(&scope)
                .unwrap_or(result.target.as_path())
                .display()
                .to_string(),
            scope: scope.display().to_string(),
            uses_local: result
                .uses_local
                .into_iter()
                .map(|dependency| LocalDependency {
                    path: dependency
                        .path
                        .strip_prefix(&scope)
                        .unwrap_or(dependency.path.as_path())
                        .display()
                        .to_string(),
                    symbols: dependency.symbols,
                })
                .collect(),
            uses_external: result.uses_external,
            used_by: result
                .used_by
                .into_iter()
                .map(|dependency| {
                    let mut callers = dependency
                        .symbols
                        .into_iter()
                        .fold(
                            BTreeMap::<(usize, String), Vec<String>>::new(),
                            |mut grouped, item| {
                                let (caller, symbol, line) = item;
                                grouped
                                    .entry((line as usize, caller))
                                    .or_default()
                                    .push(symbol);
                                grouped
                            },
                        )
                        .into_iter()
                        .map(|((line, caller), mut symbols)| {
                            symbols.sort();
                            symbols.dedup();
                            ReverseDependencyCaller {
                                caller,
                                line,
                                symbols,
                            }
                        })
                        .collect::<Vec<_>>();

                    callers.sort_by(|left, right| {
                        left.line
                            .cmp(&right.line)
                            .then_with(|| left.caller.cmp(&right.caller))
                    });

                    ReverseDependency {
                        path: dependency
                            .path
                            .strip_prefix(&scope)
                            .unwrap_or(dependency.path.as_path())
                            .display()
                            .to_string(),
                        is_test: dependency.is_test,
                        callers,
                    }
                })
                .collect(),
            truncated_dependents,
        },
        diagnostics: Vec::new(),
    })
}

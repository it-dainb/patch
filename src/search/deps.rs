//! File-level dependency analysis: what a file imports and what imports it.
//! Used by the explicit `deps` command for blast-radius checks before breaking changes.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::cache::OutlineCache;
use crate::error::DrailError;
use crate::read::detect_file_type;
use crate::read::imports::{is_external, is_import_line, resolve_related_files_with_content};
use crate::read::outline::code::extract_import_source;
use crate::search::callees::{extract_callee_names, get_outline_entries, resolve_callees};
use crate::search::callers::find_callers_batch;
use crate::types::{is_test_file, FileType, OutlineKind};

const MAX_EXPORTED_SYMBOLS: usize = 25;
const MAX_DEPENDENTS: usize = 15;

pub struct DepsResult {
    pub target: PathBuf,
    pub uses_local: Vec<LocalDep>,
    pub uses_external: Vec<String>,
    pub used_by: Vec<Dependent>,
    pub total_dependents: usize,
}

pub struct LocalDep {
    pub path: PathBuf,
    pub symbols: Vec<String>,
}

pub struct Dependent {
    pub path: PathBuf,
    pub symbols: Vec<(String, String, u32)>,
    pub is_test: bool,
}

pub fn analyze_deps(
    path: &Path,
    scope: &Path,
    cache: &OutlineCache,
    bloom: &crate::index::bloom::BloomFilterCache,
) -> Result<DepsResult, DrailError> {
    let path = &path.canonicalize().map_err(|e| DrailError::IoError {
        path: path.to_path_buf(),
        source: e,
    })?;

    let content = fs::read_to_string(path).map_err(|e| DrailError::IoError {
        path: path.clone(),
        source: e,
    })?;

    let FileType::Code(lang) = detect_file_type(path) else {
        return Ok(DepsResult {
            target: path.clone(),
            uses_local: Vec::new(),
            uses_external: Vec::new(),
            used_by: Vec::new(),
            total_dependents: 0,
        });
    };

    let entries = get_outline_entries(&content, lang);
    let _ = cache;

    let mut all_names: Vec<String> = Vec::new();
    for entry in &entries {
        if matches!(entry.kind, OutlineKind::Import | OutlineKind::Export) {
            continue;
        }
        collect_symbol_names(entry, &mut all_names);
    }

    all_names.sort();
    all_names.dedup();
    all_names.retain(|name| !is_placeholder_name(name));

    if all_names.len() > MAX_EXPORTED_SYMBOLS {
        all_names.sort_by(|left, right| right.len().cmp(&left.len()).then_with(|| left.cmp(right)));
        all_names.truncate(MAX_EXPORTED_SYMBOLS);
    }

    let callee_names = extract_callee_names(&content, lang, None);
    let resolved = resolve_callees(&callee_names, path, &content, cache, bloom);

    let mut local_by_file: HashMap<PathBuf, Vec<String>> = HashMap::new();
    for callee in resolved {
        if callee.file != *path {
            local_by_file
                .entry(callee.file)
                .or_default()
                .push(callee.name);
        }
    }

    for import_path in resolve_related_files_with_content(path, &content) {
        local_by_file.entry(import_path).or_default();
    }

    let mut uses_local: Vec<LocalDep> = local_by_file
        .into_iter()
        .map(|(dep_path, mut symbols)| {
            symbols.sort();
            symbols.dedup();
            LocalDep {
                path: dep_path,
                symbols,
            }
        })
        .collect();
    uses_local.sort_by(|left, right| left.path.cmp(&right.path));

    let mut external_set: HashSet<String> = HashSet::new();
    for line in content.lines() {
        if !is_import_line(line, lang) {
            continue;
        }

        let source = extract_import_source(line);
        if source.is_empty() {
            continue;
        }

        if is_external(&source, lang) && !is_stdlib(&source, lang) && is_valid_module_path(&source)
        {
            external_set.insert(source.clone());
        }
    }
    let mut uses_external: Vec<String> = external_set.into_iter().collect();
    uses_external.sort();

    let mut used_by = if all_names.is_empty() {
        Vec::new()
    } else {
        let symbols_set: HashSet<String> = all_names.iter().cloned().collect();
        let raw_matches = find_callers_batch(&symbols_set, scope, bloom)?;

        let mut by_file: HashMap<PathBuf, Vec<(String, String, u32)>> = HashMap::new();
        for (matched_symbol, caller_match) in raw_matches {
            if caller_match.path == *path {
                continue;
            }

            by_file.entry(caller_match.path).or_default().push((
                caller_match.calling_function,
                matched_symbol,
                caller_match.line,
            ));
        }

        let mut dependents: Vec<Dependent> = by_file
            .into_iter()
            .map(|(dep_path, mut symbols)| {
                symbols.sort();
                symbols.dedup();
                Dependent {
                    is_test: is_test_file(&dep_path),
                    path: dep_path,
                    symbols,
                }
            })
            .collect();

        dependents.sort_by(|left, right| {
            left.path
                .cmp(&right.path)
                .then_with(|| left.is_test.cmp(&right.is_test))
        });

        dependents
    };

    let total_dependents = used_by.len();
    used_by.truncate(MAX_DEPENDENTS);

    Ok(DepsResult {
        target: path.clone(),
        uses_local,
        uses_external,
        used_by,
        total_dependents,
    })
}

fn collect_symbol_names(entry: &crate::types::OutlineEntry, out: &mut Vec<String>) {
    out.push(entry.name.clone());
    for child in &entry.children {
        if !matches!(child.kind, OutlineKind::Import | OutlineKind::Export) {
            out.push(child.name.clone());
        }
    }
}

fn is_placeholder_name(name: &str) -> bool {
    if name == "<anonymous>" || name.starts_with('<') || name.starts_with("impl ") {
        return true;
    }

    name.chars().count() == 1
}

fn is_stdlib(source: &str, lang: crate::types::Lang) -> bool {
    use crate::types::Lang;

    match lang {
        Lang::Rust => {
            source.starts_with("std::")
                || source.starts_with("core::")
                || source.starts_with("alloc::")
        }
        Lang::Python => {
            matches!(
                source.split('.').next().unwrap_or(""),
                "os" | "sys"
                    | "re"
                    | "json"
                    | "math"
                    | "time"
                    | "datetime"
                    | "pathlib"
                    | "typing"
                    | "collections"
                    | "functools"
                    | "itertools"
                    | "abc"
                    | "io"
                    | "logging"
                    | "unittest"
                    | "dataclasses"
                    | "enum"
                    | "copy"
                    | "hashlib"
                    | "subprocess"
                    | "threading"
                    | "asyncio"
            )
        }
        Lang::Go => source.starts_with("fmt") || !source.contains('.'),
        _ => false,
    }
}

fn is_valid_module_path(source: &str) -> bool {
    if source.contains(' ') {
        return false;
    }

    source
        .chars()
        .next()
        .is_some_and(|c| c.is_alphanumeric() || c == '@' || c == '.')
}

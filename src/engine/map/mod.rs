use std::path::Path;

use serde::Serialize;

use crate::cache::OutlineCache;
use crate::error::PatchError;
use crate::output::json::envelope::Diagnostic;

#[derive(Debug, Clone, Serialize)]
pub struct MapEntry {
    pub path: String,
    pub tokens: u64,
    pub symbols: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MapData {
    pub scope: String,
    pub depth: usize,
    pub total_files: usize,
    pub total_tokens: u64,
    pub entries: Vec<MapEntry>,
    pub tree_text: String,
}

#[derive(Debug, Clone)]
pub struct MapCommandResult {
    pub data: MapData,
    pub truncated: bool,
    pub diagnostics: Vec<Diagnostic>,
}

pub fn run(
    scope: &Path,
    depth: usize,
    budget: Option<u64>,
) -> Result<MapCommandResult, PatchError> {
    let cache = OutlineCache::new();
    let scope = crate::engine::resolve_scope(scope);
    let full_map = crate::map::generate(&scope, depth, None, &cache);
    let full_tree_text = full_map.text;
    let tree_text = match budget {
        Some(budget) => crate::budget::apply(&full_tree_text, budget),
        None => full_tree_text.clone(),
    };

    let entries = parse_map_entries(&tree_text);
    let total_files = full_map.total_files;
    let total_tokens = full_map.total_tokens;

    let truncated = tree_text != full_tree_text;

    Ok(MapCommandResult {
        data: MapData {
            scope: scope.display().to_string(),
            depth,
            total_files,
            total_tokens,
            entries,
            tree_text,
        },
        truncated,
        diagnostics: Vec::new(),
    })
}

fn parse_map_entries(tree_text: &str) -> Vec<MapEntry> {
    let mut entries = Vec::new();
    for line in tree_text.lines() {
        let trimmed = line.trim();
        // Skip the header line
        if trimmed.starts_with("# Map:") || trimmed.is_empty() {
            continue;
        }
        if trimmed.starts_with("... truncated (") {
            continue;
        }
        // Skip directory lines (ending with /)
        if trimmed.ends_with('/') {
            continue;
        }
        // Parse file entries: "name: symbols" or "name (~N tokens)"
        if let Some(tokens_entry) = parse_tokens_entry(trimmed) {
            entries.push(tokens_entry);
        } else if let Some(symbols_entry) = parse_symbols_entry(trimmed) {
            entries.push(symbols_entry);
        }
    }
    entries.sort_by(|a, b| a.path.cmp(&b.path));
    entries
}

fn parse_tokens_entry(line: &str) -> Option<MapEntry> {
    // Format: "name (~N tokens)"
    let idx = line.find(" (~")?;
    let name = line[..idx].trim().to_string();
    let rest = &line[idx + 3..];
    let tok_end = rest.find(" tokens)")?;
    let tokens: u64 = rest[..tok_end].parse().ok()?;
    Some(MapEntry {
        path: name,
        tokens,
        symbols: vec![],
    })
}

fn parse_symbols_entry(line: &str) -> Option<MapEntry> {
    // Format: "name: sym1, sym2, ..."
    let idx = line.find(": ")?;
    let name = line[..idx].trim().to_string();
    let syms_str = &line[idx + 2..];
    let symbols: Vec<String> = syms_str
        .split(", ")
        .map(|s| s.trim_end_matches("...").trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    Some(MapEntry {
        path: name,
        tokens: 0,
        symbols,
    })
}

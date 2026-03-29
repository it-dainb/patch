use std::collections::HashSet;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use streaming_iterator::StreamingIterator;

use super::treesitter::{extract_definition_name, DEFINITION_KINDS};

use crate::cache::OutlineCache;
use crate::error::DrailError;
use crate::minified;
use crate::read::detect_file_type;
use crate::read::outline::code::outline_language;
use crate::types::FileType;

const MAX_MATCHES: usize = 10;
/// Stop walking once we have this many raw matches. Generous headroom for dedup + ranking.
const EARLY_QUIT_THRESHOLD: usize = 30;
/// Max unique caller functions to trace for 2nd hop. Above this = wide fan-out, skip.
const IMPACT_FANOUT_THRESHOLD: usize = 10;
/// Max 2nd-hop results to display.
const IMPACT_MAX_RESULTS: usize = 15;
/// Early quit for batch caller search.
const BATCH_EARLY_QUIT: usize = 50;

/// A single caller match — a call site of a target symbol.
#[allow(dead_code)]
#[derive(Debug)]
pub struct CallerMatch {
    pub path: PathBuf,
    pub line: u32,
    pub calling_function: String,
    pub call_text: String,
    /// Line range of the calling function (for expand).
    pub caller_range: Option<(u32, u32)>,
    /// File content, already read during `find_callers` — avoids re-reading during expand.
    /// Shared across all call sites in the same file via reference counting.
    pub content: Arc<String>,
}

#[derive(Debug, Clone)]
pub struct CallerResult {
    pub path: PathBuf,
    pub line: u32,
    pub caller: String,
    pub call_text: String,
}

#[derive(Debug, Clone)]
pub struct ImpactResult {
    pub path: PathBuf,
    pub line: u32,
    pub caller: String,
    pub via: String,
}

#[derive(Debug, Clone)]
pub struct CallerSearchResult {
    pub callers: Vec<CallerResult>,
    pub impact: Vec<ImpactResult>,
    pub text_fallback_used: bool,
}

/// Find all call sites of a target symbol across the codebase using tree-sitter.
pub fn find_callers(
    target: &str,
    scope: &Path,
    bloom: &crate::index::bloom::BloomFilterCache,
) -> Result<(Vec<CallerMatch>, bool), DrailError> {
    let matches: Mutex<Vec<CallerMatch>> = Mutex::new(Vec::new());
    let found_count = AtomicUsize::new(0);
    let text_fallback_used = AtomicBool::new(false);
    let needle = target.as_bytes();

    let walker = super::walker(scope);

    walker.run(|| {
        let matches = &matches;
        let found_count = &found_count;
        let text_fallback_used = &text_fallback_used;

        Box::new(move |entry| {
            // Early termination: enough callers found
            if found_count.load(Ordering::Relaxed) >= EARLY_QUIT_THRESHOLD {
                return ignore::WalkState::Quit;
            }

            let Ok(entry) = entry else {
                return ignore::WalkState::Continue;
            };

            if !entry.file_type().is_some_and(|ft| ft.is_file()) {
                return ignore::WalkState::Continue;
            }

            let path = entry.path();

            // Single metadata call: capture mtime
            let (_file_len, mtime) = match std::fs::metadata(path) {
                Ok(meta) => (
                    meta.len(),
                    meta.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH),
                ),
                Err(_) => return ignore::WalkState::Continue,
            };

            // Single read: read file once, use buffer for both check and parse
            let Ok(content) = fs::read_to_string(path) else {
                return ignore::WalkState::Continue;
            };

            // Bloom pre-filter: skip if target is definitely not in file
            if !bloom.contains(path, mtime, &content, target) {
                return ignore::WalkState::Continue;
            }

            // Fast byte check via memchr::memmem (SIMD) — skip files without the symbol
            if memchr::memmem::find(content.as_bytes(), needle).is_none() {
                return ignore::WalkState::Continue;
            }

            // Only process files with tree-sitter grammars
            let file_type = detect_file_type(path);
            let lang = match file_type {
                FileType::Code(lang) => Some(lang),
                _ => None,
            };

            if super::fallback::should_use_text_fallback(content.len() as u64, &content, lang) {
                let shared_content: Arc<String> = Arc::new(content);
                let file_callers =
                    super::fallback::query_centered_matches(shared_content.as_ref(), target)
                        .into_iter()
                        .map(|(line, call_text)| CallerMatch {
                            path: path.to_path_buf(),
                            line,
                            calling_function: "<text-fallback>".to_string(),
                            call_text,
                            caller_range: None,
                            content: Arc::clone(&shared_content),
                        })
                        .collect::<Vec<_>>();

                if !file_callers.is_empty() {
                    text_fallback_used.store(true, Ordering::Relaxed);
                    found_count.fetch_add(file_callers.len(), Ordering::Relaxed);
                    let mut all = matches
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    all.extend(file_callers);
                }

                return ignore::WalkState::Continue;
            }

            let FileType::Code(lang) = file_type else {
                return ignore::WalkState::Continue;
            };

            let Some(ts_lang) = outline_language(lang) else {
                return ignore::WalkState::Continue;
            };

            let file_callers = find_callers_treesitter(path, target, &ts_lang, &content, lang);

            if !file_callers.is_empty() {
                found_count.fetch_add(file_callers.len(), Ordering::Relaxed);
                let mut all = matches
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                all.extend(file_callers);
            }

            ignore::WalkState::Continue
        })
    });

    let all = matches
        .into_inner()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    Ok((all, text_fallback_used.load(Ordering::Relaxed)))
}

/// Tree-sitter call site detection.
fn find_callers_treesitter(
    path: &Path,
    target: &str,
    ts_lang: &tree_sitter::Language,
    content: &str,
    lang: crate::types::Lang,
) -> Vec<CallerMatch> {
    // Get the query string for this language
    let Some(query_str) = super::callees::callee_query_str(lang) else {
        return Vec::new();
    };

    let mut parser = tree_sitter::Parser::new();
    if parser.set_language(ts_lang).is_err() {
        return Vec::new();
    }

    let Some(tree) = parser.parse(content, None) else {
        return Vec::new();
    };

    let content_bytes = content.as_bytes();
    let lines: Vec<&str> = content.lines().collect();

    // One Arc per file — all call sites share the same allocation.
    let shared_content: Arc<String> = Arc::new(content.to_string());

    let Some(callers) = super::callees::with_callee_query(ts_lang, query_str, |query| {
        let Some(callee_idx) = query.capture_index_for_name("callee") else {
            return Vec::new();
        };

        let mut cursor = tree_sitter::QueryCursor::new();
        let mut matches = cursor.matches(query, tree.root_node(), content_bytes);
        let mut callers = Vec::new();

        while let Some(m) = matches.next() {
            for cap in m.captures {
                if cap.index != callee_idx {
                    continue;
                }

                // Check if the captured text matches our target symbol
                let Ok(text) = cap.node.utf8_text(content_bytes) else {
                    continue;
                };

                if text != target {
                    continue;
                }

                // Found a call site! Now walk up to find the calling function
                let line = cap.node.start_position().row as u32 + 1;

                // Get the call text (the whole call expression, not just the callee)
                let call_node = cap.node.parent().unwrap_or(cap.node);
                let same_line = call_node.start_position().row == call_node.end_position().row;
                let call_text: String = if same_line {
                    let row = call_node.start_position().row;
                    if row < lines.len() {
                        lines[row].trim().to_string()
                    } else {
                        text.to_string()
                    }
                } else {
                    text.to_string()
                };

                // Walk up the tree to find the enclosing function
                let (calling_function, caller_range) = find_enclosing_function(cap.node, &lines);

                callers.push(CallerMatch {
                    path: path.to_path_buf(),
                    line,
                    calling_function,
                    call_text,
                    caller_range,
                    content: Arc::clone(&shared_content),
                });
            }
        }

        callers
    }) else {
        return Vec::new();
    };

    callers
}

/// Find all call sites of any symbol in `targets` across the codebase using a single walk.
/// Returns tuples of (`target_name`, match) so callers know which symbol was matched.
pub(crate) fn find_callers_batch(
    targets: &HashSet<String>,
    scope: &Path,
    bloom: &crate::index::bloom::BloomFilterCache,
) -> Result<Vec<(String, CallerMatch)>, DrailError> {
    let matches: Mutex<Vec<(String, CallerMatch)>> = Mutex::new(Vec::new());
    let found_count = AtomicUsize::new(0);

    let walker = super::walker(scope);

    walker.run(|| {
        let matches = &matches;
        let found_count = &found_count;

        Box::new(move |entry| {
            // Early termination: enough callers found
            if found_count.load(Ordering::Relaxed) >= BATCH_EARLY_QUIT {
                return ignore::WalkState::Quit;
            }

            let Ok(entry) = entry else {
                return ignore::WalkState::Continue;
            };

            if !entry.file_type().is_some_and(|ft| ft.is_file()) {
                return ignore::WalkState::Continue;
            }

            let path = entry.path();

            // Single metadata call: check size and capture mtime together
            let (file_len, mtime) = match std::fs::metadata(path) {
                Ok(meta) => (
                    meta.len(),
                    meta.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH),
                ),
                Err(_) => return ignore::WalkState::Continue,
            };
            if file_len > minified::TREE_SITTER_FILE_SIZE_CAP {
                return ignore::WalkState::Continue;
            }

            // Single read: read file once, use buffer for both check and parse
            let Ok(content) = fs::read_to_string(path) else {
                return ignore::WalkState::Continue;
            };

            // Bloom pre-filter: skip if none of the targets are definitely in the file
            if !targets
                .iter()
                .any(|t| bloom.contains(path, mtime, &content, t))
            {
                return ignore::WalkState::Continue;
            }

            // Fast byte check via memchr::memmem (SIMD) — skip files without any target symbol
            if !targets
                .iter()
                .any(|t| memchr::memmem::find(content.as_bytes(), t.as_bytes()).is_some())
            {
                return ignore::WalkState::Continue;
            }

            // Only process files with tree-sitter grammars
            let file_type = detect_file_type(path);
            let FileType::Code(lang) = file_type else {
                return ignore::WalkState::Continue;
            };

            let Some(ts_lang) = outline_language(lang) else {
                return ignore::WalkState::Continue;
            };

            let file_callers =
                find_callers_treesitter_batch(path, targets, &ts_lang, &content, lang);

            if !file_callers.is_empty() {
                found_count.fetch_add(file_callers.len(), Ordering::Relaxed);
                let mut all = matches
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                all.extend(file_callers);
            }

            ignore::WalkState::Continue
        })
    });

    Ok(matches
        .into_inner()
        .unwrap_or_else(std::sync::PoisonError::into_inner))
}

/// Tree-sitter call site detection for a set of target symbols.
/// Returns tuples of (`matched_target_name`, `CallerMatch`).
fn find_callers_treesitter_batch(
    path: &Path,
    targets: &HashSet<String>,
    ts_lang: &tree_sitter::Language,
    content: &str,
    lang: crate::types::Lang,
) -> Vec<(String, CallerMatch)> {
    // Get the query string for this language
    let Some(query_str) = super::callees::callee_query_str(lang) else {
        return Vec::new();
    };

    let mut parser = tree_sitter::Parser::new();
    if parser.set_language(ts_lang).is_err() {
        return Vec::new();
    }

    let Some(tree) = parser.parse(content, None) else {
        return Vec::new();
    };

    let content_bytes = content.as_bytes();
    let lines: Vec<&str> = content.lines().collect();

    // One Arc per file — all call sites share the same allocation.
    let shared_content: Arc<String> = Arc::new(content.to_string());

    let Some(callers) = super::callees::with_callee_query(ts_lang, query_str, |query| {
        let Some(callee_idx) = query.capture_index_for_name("callee") else {
            return Vec::new();
        };

        let mut cursor = tree_sitter::QueryCursor::new();
        let mut matches = cursor.matches(query, tree.root_node(), content_bytes);
        let mut callers = Vec::new();

        while let Some(m) = matches.next() {
            for cap in m.captures {
                if cap.index != callee_idx {
                    continue;
                }

                // Check if the captured text matches any of our target symbols
                let Ok(text) = cap.node.utf8_text(content_bytes) else {
                    continue;
                };

                if !targets.contains(text) {
                    continue;
                }

                let matched_target = text.to_string();

                // Found a call site! Now walk up to find the calling function
                let line = cap.node.start_position().row as u32 + 1;

                // Get the call text (the whole call expression, not just the callee)
                let call_node = cap.node.parent().unwrap_or(cap.node);
                let same_line = call_node.start_position().row == call_node.end_position().row;
                let call_text: String = if same_line {
                    let row = call_node.start_position().row;
                    if row < lines.len() {
                        lines[row].trim().to_string()
                    } else {
                        matched_target.clone()
                    }
                } else {
                    matched_target.clone()
                };

                // Walk up the tree to find the enclosing function
                let (calling_function, caller_range) = find_enclosing_function(cap.node, &lines);

                callers.push((
                    matched_target,
                    CallerMatch {
                        path: path.to_path_buf(),
                        line,
                        calling_function,
                        call_text,
                        caller_range,
                        content: Arc::clone(&shared_content),
                    },
                ));
            }
        }

        callers
    }) else {
        return Vec::new();
    };

    callers
}

/// Walk up the AST from a node to find the enclosing function definition.
/// Returns (`function_name`, `line_range`).
/// Type-like node kinds that can enclose a function definition.
const TYPE_KINDS: &[&str] = &[
    "class_declaration",
    "class_definition",
    "struct_item",
    "impl_item",
    "interface_declaration",
    "trait_item",
    "trait_declaration",
    "type_declaration",
    "enum_item",
    "enum_declaration",
    "module",
    "mod_item",
    "namespace_definition",
];

fn find_enclosing_function(
    node: tree_sitter::Node,
    lines: &[&str],
) -> (String, Option<(u32, u32)>) {
    // Walk up the tree until we find a definition node
    let mut current = Some(node);

    while let Some(n) = current {
        let kind = n.kind();

        if DEFINITION_KINDS.contains(&kind) {
            let name =
                extract_definition_name(n, lines).unwrap_or_else(|| "<anonymous>".to_string());
            let range = Some((
                n.start_position().row as u32 + 1,
                n.end_position().row as u32 + 1,
            ));

            // Walk further up to find an enclosing type and qualify the name
            let mut parent = n.parent();
            while let Some(p) = parent {
                if TYPE_KINDS.contains(&p.kind()) {
                    if let Some(type_name) = extract_definition_name(p, lines) {
                        return (format!("{type_name}.{name}"), range);
                    }
                }
                parent = p.parent();
            }

            return (name, range);
        }

        current = n.parent();
    }

    // No enclosing function found — top-level call
    ("<top-level>".to_string(), None)
}

/// Format and rank caller search results with optional expand.
pub fn search_callers_expanded(
    target: &str,
    scope: &Path,
    _cache: &OutlineCache,
    bloom: &crate::index::bloom::BloomFilterCache,
    _expand: usize,
    context: Option<&Path>,
) -> Result<String, DrailError> {
    let result = search_callers_structured(target, scope, bloom, context)?;

    if result.callers.is_empty() {
        return Ok(format!(
            "# Callers of \"{}\" in {} — no call sites found\n\n\
             Tip: the symbol may be called via interface/trait dispatch. \
             Try symbol search instead.",
            target,
            scope.display()
        ));
    }

    let total = result.callers.len();

    // Format the output
    let mut output = format!(
        "# Callers of \"{}\" in {} — {} call site{}\n",
        target,
        scope.display(),
        total,
        if total == 1 { "" } else { "s" }
    );

    for caller in &result.callers {
        let _ = write!(
            output,
            "\n## {}:{} [caller: {}]\n",
            caller
                .path
                .strip_prefix(scope)
                .unwrap_or(&caller.path)
                .display(),
            caller.line,
            caller.caller
        );
        let _ = writeln!(output, "→ {}", caller.call_text);
    }

    if !result.impact.is_empty() {
        output.push_str("\n── impact (2nd hop) ──\n");

        for entry in &result.impact {
            let rel_path = entry
                .path
                .strip_prefix(scope)
                .unwrap_or(&entry.path)
                .display();
            let _ = writeln!(
                output,
                "  {:<20} {}:{}  → {}",
                entry.caller, rel_path, entry.line, entry.via
            );
        }

        let _ = writeln!(
            output,
            "\n{} functions affected across 2 hops.",
            result.callers.len() + result.impact.len()
        );
    }

    let tokens = crate::types::estimate_tokens(output.len() as u64);
    let token_str = if tokens >= 1000 {
        format!("~{}.{}k", tokens / 1000, (tokens % 1000) / 100)
    } else {
        format!("~{tokens}")
    };
    let _ = write!(output, "\n\n({token_str} tokens)");
    Ok(output)
}

pub fn search_callers_structured(
    target: &str,
    scope: &Path,
    bloom: &crate::index::bloom::BloomFilterCache,
    context: Option<&Path>,
) -> Result<CallerSearchResult, DrailError> {
    let (callers, text_fallback_used) = find_callers(target, scope, bloom)?;

    if callers.is_empty() {
        return Ok(CallerSearchResult {
            callers: Vec::new(),
            impact: Vec::new(),
            text_fallback_used,
        });
    }

    let mut sorted_callers = callers;
    rank_callers(&mut sorted_callers, scope, context);

    let all_caller_names: HashSet<String> = sorted_callers
        .iter()
        .filter(|c| c.calling_function != "<top-level>" && c.calling_function != "<text-fallback>")
        .map(|c| c.calling_function.clone())
        .collect();

    sorted_callers.truncate(MAX_MATCHES);

    let callers = sorted_callers
        .iter()
        .map(|caller| CallerResult {
            path: caller.path.clone(),
            line: caller.line,
            caller: caller.calling_function.clone(),
            call_text: caller.call_text.clone(),
        })
        .collect::<Vec<_>>();

    let impact = if !text_fallback_used
        && !all_caller_names.is_empty()
        && all_caller_names.len() <= IMPACT_FANOUT_THRESHOLD
    {
        match find_callers_batch(&all_caller_names, scope, bloom) {
            Ok(hop2) => {
                let hop1_locations: HashSet<(PathBuf, u32)> = sorted_callers
                    .iter()
                    .map(|c| (c.path.clone(), c.line))
                    .collect();

                let mut deduped = hop2
                    .into_iter()
                    .filter(|(_, m)| !hop1_locations.contains(&(m.path.clone(), m.line)))
                    .map(|(via, m)| ImpactResult {
                        path: m.path,
                        line: m.line,
                        caller: m.calling_function,
                        via,
                    })
                    .collect::<Vec<_>>();

                deduped.sort_by(|a, b| {
                    a.path
                        .cmp(&b.path)
                        .then_with(|| a.line.cmp(&b.line))
                        .then_with(|| a.caller.cmp(&b.caller))
                        .then_with(|| a.via.cmp(&b.via))
                });
                deduped.dedup_by(|a, b| {
                    a.path == b.path && a.line == b.line && a.caller == b.caller && a.via == b.via
                });
                deduped.truncate(IMPACT_MAX_RESULTS);
                deduped
            }
            Err(_) => Vec::new(),
        }
    } else {
        Vec::new()
    };

    Ok(CallerSearchResult {
        callers,
        impact,
        text_fallback_used,
    })
}

/// Simple ranking: context file first, then by path length (proximity heuristic).
fn rank_callers(callers: &mut [CallerMatch], scope: &Path, context: Option<&Path>) {
    callers.sort_by(|a, b| {
        // Context file wins
        if let Some(ctx) = context {
            match (a.path == ctx, b.path == ctx) {
                (true, false) => return std::cmp::Ordering::Less,
                (false, true) => return std::cmp::Ordering::Greater,
                _ => {}
            }
        }

        // Shorter paths (more similar to scope) rank higher
        let a_rel = a.path.strip_prefix(scope).unwrap_or(&a.path);
        let b_rel = b.path.strip_prefix(scope).unwrap_or(&b.path);
        a_rel
            .components()
            .count()
            .cmp(&b_rel.components().count())
            .then_with(|| a.path.cmp(&b.path))
            .then_with(|| a.line.cmp(&b.line))
    });
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::index::bloom::BloomFilterCache;

    use super::find_callers;

    #[test]
    fn find_callers_detects_simple_rust_fixture_callsite() {
        let bloom = BloomFilterCache::new();
        let scope = Path::new("tests/fixtures/drailignore");

        let (callers, text_fallback_used) =
            find_callers("visible_api", scope, &bloom).expect("caller search succeeds");

        assert!(
            !text_fallback_used,
            "did not expect text fallback for simple rust fixture"
        );

        assert!(
            callers
                .iter()
                .any(|caller| caller.path.ends_with("visible_caller.rs")),
            "expected visible_caller.rs in callers: {callers:#?}"
        );
        assert!(
            callers
                .iter()
                .all(|caller| !caller.path.ends_with("ignored-dir/ignored_caller.rs")),
            "expected ignored caller to be excluded: {callers:#?}"
        );
    }
}

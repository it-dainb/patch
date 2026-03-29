use crate::minified;
use crate::read::outline::code::outline_language;
use crate::types::Lang;

const TEXT_FALLBACK_SNIPPET_BUDGET: usize = 120;

pub fn should_use_text_fallback(file_len: u64, content: &str, lang: Option<Lang>) -> bool {
    file_len > minified::TREE_SITTER_FILE_SIZE_CAP
        || minified::profile(content).is_likely_minified()
        || parse_is_unreliable(content, lang)
}

pub fn query_centered_matches(content: &str, query: &str) -> Vec<(u32, String)> {
    content
        .lines()
        .enumerate()
        .filter_map(|(idx, line)| {
            if line.contains(query) {
                Some(((idx + 1) as u32, query_centered_snippet(line, query)))
            } else {
                None
            }
        })
        .collect()
}

fn parse_is_unreliable(content: &str, lang: Option<Lang>) -> bool {
    let Some(lang) = lang else {
        return false;
    };
    let Some(ts_lang) = outline_language(lang) else {
        return false;
    };

    let mut parser = tree_sitter::Parser::new();
    if parser.set_language(&ts_lang).is_err() {
        return true;
    }

    let Some(tree) = parser.parse(content, None) else {
        return true;
    };

    tree.root_node().has_error()
}

fn query_centered_snippet(line: &str, query: &str) -> String {
    if line.len() <= TEXT_FALLBACK_SNIPPET_BUDGET {
        return line.trim().to_string();
    }

    let Some(query_idx) = line.find(query) else {
        return trim_with_ellipsis(line, 0, TEXT_FALLBACK_SNIPPET_BUDGET);
    };

    let keep = TEXT_FALLBACK_SNIPPET_BUDGET.saturating_sub(6);
    let query_end = query_idx + query.len();
    let mut start = query_idx.saturating_sub(keep / 2);
    let mut end = start + keep;

    if end < query_end {
        end = query_end;
        start = end.saturating_sub(keep);
    }

    if end > line.len() {
        end = line.len();
        start = end.saturating_sub(keep);
    }

    trim_with_ellipsis(line, start, end)
}

fn trim_with_ellipsis(line: &str, raw_start: usize, raw_end: usize) -> String {
    let start = floor_char_boundary(line, raw_start.min(line.len()));
    let end = ceil_char_boundary(line, raw_end.min(line.len()));

    let mut snippet = String::new();
    if start > 0 {
        snippet.push_str("...");
    }
    snippet.push_str(line[start..end].trim());
    if end < line.len() {
        snippet.push_str("...");
    }
    snippet
}

fn floor_char_boundary(s: &str, mut idx: usize) -> usize {
    while idx > 0 && !s.is_char_boundary(idx) {
        idx -= 1;
    }
    idx
}

fn ceil_char_boundary(s: &str, mut idx: usize) -> usize {
    while idx < s.len() && !s.is_char_boundary(idx) {
        idx += 1;
    }
    idx.min(s.len())
}

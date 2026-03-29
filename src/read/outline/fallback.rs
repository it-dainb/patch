use std::fmt::Write;

const PREVIEW_MAX_LINES: usize = 40;
const PREVIEW_MAX_CHARS: usize = 4_000;

/// Shared read fallback contract for minified/unreliable content previews.
///
/// Contract:
/// - derived from source text only,
/// - always rendered as multi-line preview (never a raw single source line),
/// - bounded by line and char budgets,
/// - omission markers emitted when content is skipped.
pub fn minified_preview(content: &str) -> String {
    let total_source_chars = content.chars().count();
    let mut lines = Vec::new();
    let mut line = String::new();
    let mut chars_used = 0;
    let mut source_chars = 0;

    let flush_line = |line: &mut String, lines: &mut Vec<String>, chars_used: &mut usize| -> bool {
        if line.trim().is_empty() {
            line.clear();
            return false;
        }

        if lines.len() >= PREVIEW_MAX_LINES {
            line.clear();
            return true;
        }

        let projected = chars_used.saturating_add(line.len());
        if projected > PREVIEW_MAX_CHARS {
            line.clear();
            return true;
        }

        *chars_used = projected;
        lines.push(std::mem::take(line));
        false
    };

    for ch in content.chars() {
        source_chars += 1;
        line.push(ch);

        let safe_break = matches!(ch, ';' | '{' | '}' | ',' | ')');
        let hard_wrap = line.chars().count() >= 80;
        let source_break = ch == '\n';

        if (safe_break || hard_wrap || source_break)
            && flush_line(&mut line, &mut lines, &mut chars_used)
        {
            break;
        }
    }

    let reached_limits = lines.len() >= PREVIEW_MAX_LINES || chars_used >= PREVIEW_MAX_CHARS;

    if !line.trim().is_empty() && !reached_limits {
        let _ = flush_line(&mut line, &mut lines, &mut chars_used);
    }

    if lines.is_empty() {
        return "... omitted ...".to_string();
    }

    let mut rendered = lines.join("\n");
    if reached_limits || source_chars < total_source_chars {
        rendered.push_str("\n... omitted ...");
    }
    rendered
}

/// Unknown file types: first 50 lines + last 10 lines.
pub fn head_tail(content: &str) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let total = lines.len();

    if total <= 60 {
        return content.to_string();
    }

    let omitted = total - 60;
    let mut result = lines[..50].join("\n");
    let _ = write!(result, "\n\n... {total} lines total, {omitted} omitted\n\n");
    result.push_str(&lines[total - 10..].join("\n"));
    result
}

/// Log files: first 10 lines + last 5 lines + total line count.
pub fn log_view(content: &str) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let total = lines.len();

    if total <= 15 {
        return content.to_string();
    }

    let mut result = lines[..10].join("\n");
    let omitted = total - 15;
    let _ = write!(result, "\n\n... {total} lines total, {omitted} omitted\n\n");
    result.push_str(&lines[total - 5..].join("\n"));
    result
}

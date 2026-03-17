pub mod common;
pub mod deps;
pub mod files;
pub mod map;
pub mod read;
pub mod search;
pub mod symbol;

use std::fmt::Write as _;

use crate::output::json::envelope::{Diagnostic, DiagnosticLevel, NextItem};
use crate::output::CommandOutput;

pub fn write(output: &CommandOutput, is_tty: bool) {
    common::emit(&render_output(output), is_tty);
}

pub fn write_error(output: &CommandOutput, is_tty: bool) {
    common::emit_error(&render_output(output), is_tty);
}

#[must_use]
pub fn render(
    command: &str,
    meta: &serde_json::Map<String, serde_json::Value>,
    evidence: &str,
    next: &[NextItem],
    diagnostics: &[Diagnostic],
) -> String {
    let mut rendered = String::new();
    let _ = write!(rendered, "# {command}\n\n## Meta\n");
    render_meta(&mut rendered, meta);

    rendered.push_str("\n\n## Evidence\n");
    if evidence.is_empty() {
        rendered.push_str("(none)");
    } else {
        rendered.push_str(evidence.trim_end());
    }

    rendered.push_str("\n\n## Next\n");
    render_next(&mut rendered, next);

    rendered.push_str("\n\n## Diagnostics\n");
    render_diagnostics(&mut rendered, diagnostics);

    rendered
}

fn render_output(output: &CommandOutput) -> String {
    render(
        output.command,
        &output.meta,
        &output.text,
        &output.next,
        &output.diagnostics,
    )
}

fn render_meta(rendered: &mut String, meta: &serde_json::Map<String, serde_json::Value>) {
    if meta.is_empty() {
        rendered.push_str("(none)");
        return;
    }

    for (index, (key, value)) in meta.iter().enumerate() {
        if index > 0 {
            rendered.push('\n');
        }

        let value = match value {
            serde_json::Value::String(string) => string.clone(),
            other => other.to_string(),
        };

        let _ = write!(rendered, "- {key}: {value}");
    }
}

fn render_next(rendered: &mut String, next: &[NextItem]) {
    if next.is_empty() {
        rendered.push_str("(none)");
        return;
    }

    for (index, item) in next.iter().enumerate() {
        if index > 0 {
            rendered.push('\n');
        }

        let _ = write!(
            rendered,
            "- {} (command: {}; confidence: {})",
            item.message, item.command, item.confidence
        );
    }
}

fn render_diagnostics(rendered: &mut String, diagnostics: &[Diagnostic]) {
    let diagnostics = visible_diagnostics(diagnostics);

    if diagnostics.is_empty() {
        rendered.push_str("(none)");
        return;
    }

    for (index, diagnostic) in sort_diagnostics(&diagnostics).iter().enumerate() {
        if index > 0 {
            rendered.push('\n');
        }

        let _ = write!(
            rendered,
            "- {}: {}",
            diagnostic_level_label(&diagnostic.level),
            diagnostic.message
        );

        if !diagnostic.code.is_empty() {
            let _ = write!(rendered, " [code: {}]", diagnostic.code);
        }

        if let Some(suggestion) = &diagnostic.suggestion {
            let _ = write!(rendered, " [suggestion: {}]", suggestion);
        }
    }
}

fn visible_diagnostics(diagnostics: &[Diagnostic]) -> Vec<Diagnostic> {
    diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code != "no_diagnostics")
        .cloned()
        .collect()
}

fn diagnostic_level_label(level: &DiagnosticLevel) -> &'static str {
    match level {
        DiagnosticLevel::Error => "error",
        DiagnosticLevel::Warning => "warning",
        DiagnosticLevel::Hint => "hint",
    }
}

fn sort_diagnostics(diagnostics: &[Diagnostic]) -> Vec<Diagnostic> {
    let mut sorted = diagnostics.to_vec();
    sorted.sort_by_key(|diagnostic| match diagnostic.level {
        DiagnosticLevel::Error => 0,
        DiagnosticLevel::Warning => 1,
        DiagnosticLevel::Hint => 2,
    });
    sorted
}

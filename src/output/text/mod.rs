pub mod common;
pub mod deps;
pub mod files;
pub mod map;
pub mod read;
pub mod search;
pub mod symbol;

use std::fmt::Write as _;

use crate::output::CommandOutput;

pub fn write(output: &CommandOutput, is_tty: bool) {
    common::emit(
        &render(
            output.command,
            &output.text,
            &output.next,
            &output.diagnostics,
        ),
        is_tty,
    );
}

#[must_use]
pub fn render(
    command: &str,
    evidence: &str,
    next: &[crate::output::json::envelope::NextItem],
    diagnostics: &[crate::output::json::envelope::Diagnostic],
) -> String {
    let mut rendered = String::new();
    let _ = write!(rendered, "# {command}\n\n## Meta\n(none)\n\n## Evidence\n");
    if evidence.is_empty() {
        rendered.push_str("(none)");
    } else {
        rendered.push_str(evidence.trim_end());
    }
    rendered.push_str("\n\n## Next\n");

    if next.is_empty() {
        rendered.push_str("(none)");
    } else {
        for (index, item) in next.iter().enumerate() {
            if index > 0 {
                rendered.push('\n');
            }
            let _ = write!(
                rendered,
                "- [{}] {} => {}",
                item.confidence, item.message, item.command
            );
        }
    }

    rendered.push_str("\n\n## Diagnostics\n");

    if diagnostics.is_empty() {
        rendered.push_str("(none)");
        return rendered;
    }

    for (index, diagnostic) in sort_diagnostics(diagnostics).iter().enumerate() {
        if index > 0 {
            rendered.push('\n');
        }
        let _ = write!(
            rendered,
            "[{}]",
            match diagnostic.level {
                crate::output::json::envelope::DiagnosticLevel::Error => "error",
                crate::output::json::envelope::DiagnosticLevel::Warning => "warning",
                crate::output::json::envelope::DiagnosticLevel::Hint => "hint",
            }
        );

        let _ = write!(rendered, " {}", diagnostic.message);

        if let Some(suggestion) = &diagnostic.suggestion {
            let _ = write!(rendered, " => {suggestion}");
        }
    }

    rendered
}

fn sort_diagnostics(
    diagnostics: &[crate::output::json::envelope::Diagnostic],
) -> Vec<crate::output::json::envelope::Diagnostic> {
    let mut sorted = diagnostics.to_vec();
    sorted.sort_by_key(|diagnostic| match diagnostic.level {
        crate::output::json::envelope::DiagnosticLevel::Error => 0,
        crate::output::json::envelope::DiagnosticLevel::Warning => 1,
        crate::output::json::envelope::DiagnosticLevel::Hint => 2,
    });
    sorted
}

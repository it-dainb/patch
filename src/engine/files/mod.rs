use std::path::Path;

use serde::Serialize;

use crate::error::DrailError;
use crate::output::json::envelope::{Diagnostic, DiagnosticLevel};

#[derive(Debug, Clone, Serialize)]
pub struct FileMatch {
    pub path: String,
    pub preview: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct FilesData {
    pub pattern: String,
    pub scope: String,
    pub files: Vec<FileMatch>,
}

#[derive(Debug, Clone)]
pub struct FilesCommandResult {
    pub data: FilesData,
    pub diagnostics: Vec<Diagnostic>,
}

pub fn run(
    pattern: &str,
    scope: &Path,
    budget: Option<u64>,
) -> Result<FilesCommandResult, DrailError> {
    let scope = crate::engine::resolve_scope(scope);
    let result = crate::search::glob::search(pattern, &scope)?;

    let mut command_result = FilesCommandResult {
        data: FilesData {
            pattern: pattern.to_string(),
            scope: scope.display().to_string(),
            files: result
                .files
                .into_iter()
                .map(|entry| FileMatch {
                    path: entry
                        .path
                        .strip_prefix(&scope)
                        .unwrap_or(entry.path.as_path())
                        .display()
                        .to_string(),
                    preview: entry.preview.unwrap_or_else(|| "(no preview)".into()),
                })
                .collect(),
        },
        diagnostics: diagnostics(
            pattern,
            &scope,
            &result.available_extensions,
            result.total_found,
        ),
    };

    if let Some(budget) = budget {
        while serde_json::to_string(&command_result.data)
            .expect("files data should serialize")
            .len() as u64
            > budget
            && !command_result.data.files.is_empty()
        {
            command_result.data.files.pop();
        }
    }

    Ok(command_result)
}

fn diagnostics(
    pattern: &str,
    scope: &Path,
    available_extensions: &[String],
    total_found: usize,
) -> Vec<Diagnostic> {
    if total_found == 0 {
        let suggestion = if available_extensions.is_empty() {
            None
        } else {
            Some(format!(
                "Try: drail files \"*.{}\" --scope {}",
                available_extensions[0],
                scope.display()
            ))
        };

        return vec![Diagnostic {
            level: DiagnosticLevel::Hint,
            code: "no_file_matches".into(),
            message: format!("no file matches found for \"{pattern}\""),
            suggestion,
        }];
    }

    Vec::new()
}

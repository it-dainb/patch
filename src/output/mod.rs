pub mod json;
pub mod text;

use serde_json::{Map, Value};

use crate::error::PatchError;
use crate::output::json::envelope::{
    Diagnostic, DiagnosticLevel, Envelope, EnvelopeData, NextItem,
};

pub struct CommandOutput {
    pub command: &'static str,
    pub text: String,
    pub data: Value,
    pub meta: Map<String, Value>,
    pub next: Vec<NextItem>,
    pub diagnostics: Vec<Diagnostic>,
    pub ok: bool,
}

impl CommandOutput {
    #[must_use]
    pub fn with_parts(
        command: &'static str,
        text: String,
        data: Value,
        diagnostics: Vec<Diagnostic>,
        ok: bool,
    ) -> Self {
        let next = next_from_diagnostics(&diagnostics);
        let diagnostics = visible_diagnostics(diagnostics);

        Self {
            command,
            text,
            data,
            meta: Map::new(),
            next,
            diagnostics,
            ok,
        }
    }

    #[must_use]
    pub fn from_error(command: &'static str, error: &PatchError) -> Self {
        Self {
            command,
            text: text::render(command, "", &[diagnostic_from_error(error)]),
            data: serde_json::json!({}),
            meta: Map::new(),
            next: next_from_error(error),
            diagnostics: vec![diagnostic_from_error(error)],
            ok: false,
        }
    }
}

pub fn write(output: &CommandOutput, json_mode: bool, is_tty: bool) {
    if json_mode {
        println!("{}", json::render(output));
    } else {
        text::write(output, is_tty);
    }
}

fn diagnostic_from_error(error: &PatchError) -> Diagnostic {
    match error {
        PatchError::NotFound { suggestion, .. } => Diagnostic {
            level: DiagnosticLevel::Error,
            code: "not_found".into(),
            message: error.to_string(),
            suggestion: suggestion.clone(),
        },
        PatchError::PermissionDenied { .. } => Diagnostic {
            level: DiagnosticLevel::Error,
            code: "permission_denied".into(),
            message: error.to_string(),
            suggestion: None,
        },
        PatchError::InvalidQuery { .. } => Diagnostic {
            level: DiagnosticLevel::Error,
            code: "invalid_query".into(),
            message: error.to_string(),
            suggestion: None,
        },
        PatchError::IoError { .. } => Diagnostic {
            level: DiagnosticLevel::Error,
            code: "io_error".into(),
            message: error.to_string(),
            suggestion: None,
        },
        PatchError::ParseError { .. } => Diagnostic {
            level: DiagnosticLevel::Error,
            code: "parse_error".into(),
            message: error.to_string(),
            suggestion: None,
        },
        PatchError::Clap { .. } => Diagnostic {
            level: DiagnosticLevel::Error,
            code: "clap".into(),
            message: error.to_string(),
            suggestion: None,
        },
    }
}

fn visible_diagnostics(diagnostics: Vec<Diagnostic>) -> Vec<Diagnostic> {
    diagnostics
        .into_iter()
        .filter(|diagnostic| {
            !(diagnostic.level == DiagnosticLevel::Hint
                && diagnostic.code == "no_diagnostics"
                && diagnostic.message == "no diagnostics"
                && diagnostic.suggestion.is_none())
        })
        .collect()
}

fn next_from_diagnostics(diagnostics: &[Diagnostic]) -> Vec<NextItem> {
    diagnostics
        .iter()
        .filter_map(|diagnostic| {
            diagnostic.suggestion.as_ref().map(|command| NextItem {
                kind: "suggestion".into(),
                message: diagnostic.message.clone(),
                command: command.clone(),
                confidence: "high".into(),
            })
        })
        .collect()
}

fn next_from_error(error: &PatchError) -> Vec<NextItem> {
    match error {
        PatchError::NotFound {
            suggestion: Some(command),
            ..
        } => vec![NextItem {
            kind: "suggestion".into(),
            message: error.to_string(),
            command: command.clone(),
            confidence: "high".into(),
        }],
        _ => Vec::new(),
    }
}

fn wrap_data(meta: Map<String, Value>, data: Value) -> EnvelopeData<Value> {
    let payload = match data {
        Value::Object(map) => map,
        other => {
            let mut map = Map::new();
            map.insert("value".into(), other);
            map
        }
    };

    EnvelopeData {
        meta,
        payload: Value::Object(payload),
    }
}

#[must_use]
pub fn envelope(output: &CommandOutput) -> Envelope<EnvelopeData<Value>> {
    Envelope {
        command: output.command.to_string(),
        schema_version: 2,
        ok: output.ok,
        data: wrap_data(output.meta.clone(), output.data.clone()),
        next: output.next.clone(),
        diagnostics: output.diagnostics.clone(),
    }
}

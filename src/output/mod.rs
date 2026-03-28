pub mod json;
pub mod text;

use serde_json::{Map, Value};

use crate::error::DrailError;
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

#[must_use]
pub fn suggestion(message: impl Into<String>, command: impl Into<String>) -> NextItem {
    NextItem {
        kind: "suggestion".into(),
        message: message.into(),
        command: command.into(),
        confidence: "high".into(),
    }
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
        Self {
            command,
            text,
            data,
            meta: Map::new(),
            next: Vec::new(),
            diagnostics,
            ok,
        }
    }

    #[must_use]
    pub fn from_error(command: &'static str, error: &DrailError) -> Self {
        let diagnostic = diagnostic_from_error(error);
        Self {
            command,
            text: String::new(),
            data: serde_json::json!({}),
            meta: Map::new(),
            next: Vec::new(),
            diagnostics: vec![diagnostic],
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

pub fn write_error(output: &CommandOutput, json_mode: bool, is_tty: bool) {
    if json_mode {
        println!("{}", json::render(output));
    } else {
        text::write_error(output, is_tty);
    }
}

fn diagnostic_from_error(error: &DrailError) -> Diagnostic {
    match error {
        DrailError::AlreadyReported { .. } => {
            unreachable!("AlreadyReported is an internal control-flow signal")
        }
        DrailError::NotFound { suggestion, .. } => Diagnostic {
            level: DiagnosticLevel::Error,
            code: "not_found".into(),
            message: error.to_string(),
            suggestion: suggestion.clone(),
        },
        DrailError::PermissionDenied { .. } => Diagnostic {
            level: DiagnosticLevel::Error,
            code: "permission_denied".into(),
            message: error.to_string(),
            suggestion: None,
        },
        DrailError::InvalidQuery { .. } => Diagnostic {
            level: DiagnosticLevel::Error,
            code: "invalid_query".into(),
            message: error.to_string(),
            suggestion: None,
        },
        DrailError::IoError { .. } => Diagnostic {
            level: DiagnosticLevel::Error,
            code: "io_error".into(),
            message: error.to_string(),
            suggestion: None,
        },
        DrailError::ParseError { .. } => Diagnostic {
            level: DiagnosticLevel::Error,
            code: "parse_error".into(),
            message: error.to_string(),
            suggestion: None,
        },
        DrailError::Clap { .. } => Diagnostic {
            level: DiagnosticLevel::Error,
            code: "clap".into(),
            message: error.to_string(),
            suggestion: None,
        },
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

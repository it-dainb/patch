use std::fmt;

use serde_json::Value;
use toon_format::{encode, EncodeOptions};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum JsonReadError {
    InvalidJson(String),
    MissingKeySegment(String),
    ExpectedNumericArrayIndex(String),
    InvalidIndexRangeSyntax,
    DescendingIndexRange { start: usize, end: usize },
    IndexRangeStartsAt { start: usize, len: usize },
    ToonEncodeFailed(String),
}

impl fmt::Display for JsonReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidJson(detail) => write!(f, "invalid JSON: {detail}"),
            Self::MissingKeySegment(segment) => write!(f, "missing key segment: {segment}"),
            Self::ExpectedNumericArrayIndex(segment) => {
                write!(f, "expected numeric array index: {segment}")
            }
            Self::InvalidIndexRangeSyntax => {
                write!(f, "expected index range in START:END format")
            }
            Self::DescendingIndexRange { start, end } => write!(
                f,
                "index range end must be greater than or equal to start: {start}:{end}"
            ),
            Self::IndexRangeStartsAt { start, len } => {
                write!(f, "index range starts at {start} but array length is {len}")
            }
            Self::ToonEncodeFailed(detail) => write!(f, "failed to encode JSON as TOON: {detail}"),
        }
    }
}

impl std::error::Error for JsonReadError {}

pub(crate) fn parse_json(input: &str) -> Result<Value, JsonReadError> {
    serde_json::from_str(input).map_err(|err| JsonReadError::InvalidJson(err.to_string()))
}

pub(crate) fn resolve_path<'a>(value: &'a Value, path: &str) -> Result<&'a Value, JsonReadError> {
    if path.is_empty() {
        return Ok(value);
    }

    let mut current = value;
    for segment in path.split('.') {
        if segment.is_empty() {
            return Err(JsonReadError::MissingKeySegment(segment.to_string()));
        }

        match current {
            Value::Object(map) => {
                current = map
                    .get(segment)
                    .ok_or_else(|| JsonReadError::MissingKeySegment(segment.to_string()))?;
            }
            Value::Array(items) => {
                let index = segment
                    .parse::<usize>()
                    .map_err(|_| JsonReadError::ExpectedNumericArrayIndex(segment.to_string()))?;
                current = items
                    .get(index)
                    .ok_or_else(|| JsonReadError::MissingKeySegment(segment.to_string()))?;
            }
            _ => return Err(JsonReadError::MissingKeySegment(segment.to_string())),
        }
    }

    Ok(current)
}

pub(crate) fn slice_array<'a>(value: &'a Value, range: &str) -> Result<&'a [Value], JsonReadError> {
    let items = value
        .as_array()
        .ok_or_else(|| JsonReadError::MissingKeySegment(range.to_string()))?;

    let (start_raw, end_raw) = range
        .split_once(':')
        .ok_or(JsonReadError::InvalidIndexRangeSyntax)?;

    if start_raw.is_empty() || end_raw.is_empty() || end_raw.contains(':') {
        return Err(JsonReadError::InvalidIndexRangeSyntax);
    }

    let start = start_raw
        .parse::<usize>()
        .map_err(|_| JsonReadError::InvalidIndexRangeSyntax)?;
    let end = end_raw
        .parse::<usize>()
        .map_err(|_| JsonReadError::InvalidIndexRangeSyntax)?;

    if end < start {
        return Err(JsonReadError::DescendingIndexRange { start, end });
    }

    let len = items.len();
    if start > len {
        return Err(JsonReadError::IndexRangeStartsAt { start, len });
    }
    if end > len {
        return Err(JsonReadError::IndexRangeStartsAt { start: end, len });
    }

    Ok(&items[start..end])
}

pub(crate) fn encode_to_toon(value: &Value) -> Result<String, JsonReadError> {
    let options = EncodeOptions::new().with_key_folding(toon_format::types::KeyFoldingMode::Safe);
    encode(value, &options).map_err(|err| JsonReadError::ToonEncodeFailed(err.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_root() -> Value {
        serde_json::json!({
            "users": [
                {
                    "accounts": [
                        { "id": 1, "name": "alpha" },
                        { "id": 2, "name": "beta" },
                        { "id": 3, "name": "gamma" }
                    ]
                }
            ]
        })
    }

    #[test]
    fn parse_json_accepts_valid_input() {
        let parsed = parse_json(r#"{"users":[{"accounts":[{"id":1}]}]}"#).unwrap();
        assert!(parsed.get("users").is_some());
    }

    #[test]
    fn resolve_path_walks_object_and_array_segments() {
        let root = sample_root();
        let subtree = resolve_path(&root, "users.0.accounts").unwrap();
        assert!(subtree.is_array());
        assert_eq!(subtree.as_array().unwrap().len(), 3);
    }

    #[test]
    fn resolve_path_rejects_missing_key() {
        let root = sample_root();
        let err = resolve_path(&root, "users.0.missing").unwrap_err();
        assert!(err.to_string().contains("missing key segment"));
    }

    #[test]
    fn resolve_path_rejects_non_numeric_array_segment() {
        let root = sample_root();
        let err = resolve_path(&root, "users.zero.accounts").unwrap_err();
        assert!(err.to_string().contains("expected numeric array index"));
    }

    #[test]
    fn slice_array_returns_expected_window() {
        let root = sample_root();
        let accounts = resolve_path(&root, "users.0.accounts").unwrap();
        let window = slice_array(accounts, "1:3").unwrap();
        assert_eq!(window.len(), 2);
        assert_eq!(window[0].get("id").unwrap().as_i64(), Some(2));
        assert_eq!(window[1].get("id").unwrap().as_i64(), Some(3));
    }

    #[test]
    fn slice_array_rejects_invalid_range() {
        let root = sample_root();
        let accounts = resolve_path(&root, "users.0.accounts").unwrap();
        let err = slice_array(accounts, "1-3").unwrap_err();
        assert!(err
            .to_string()
            .contains("expected index range in START:END format"));
    }

    #[test]
    fn slice_array_rejects_descending_range() {
        let root = sample_root();
        let accounts = resolve_path(&root, "users.0.accounts").unwrap();
        let err = slice_array(accounts, "3:1").unwrap_err();
        assert!(err
            .to_string()
            .contains("index range end must be greater than or equal to start"));
    }

    #[test]
    fn slice_array_rejects_out_of_bounds_start() {
        let root = sample_root();
        let accounts = resolve_path(&root, "users.0.accounts").unwrap();
        let err = slice_array(accounts, "5:5").unwrap_err();
        assert!(err.to_string().contains("index range starts at"));
    }

    #[test]
    fn encode_to_toon_emits_compact_output() {
        let value = serde_json::json!({
            "root": {
                "branch": {
                    "leaf": "value"
                }
            }
        });
        let rendered = encode_to_toon(&value).unwrap();
        assert!(!rendered.trim().is_empty());
        assert!(rendered.contains("root.branch.leaf:"));
    }
}

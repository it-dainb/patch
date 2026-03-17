use std::ffi::OsStr;
use std::process::Output;

use assert_cmd::Command;
use serde_json::Value;

const PATCHIGNORE_SCOPE: &str = "tests/fixtures/patchignore";

fn run_patch<I, S>(args: I) -> Output
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    Command::cargo_bin("patch")
        .expect("patch binary should build for integration tests")
        .args(args)
        .output()
        .expect("patch should execute")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "expected success, got status {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        stdout(output),
        stderr(output)
    );
}

fn assert_failure(output: &Output) {
    assert!(
        !output.status.success(),
        "expected failure, got status {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        stdout(output),
        stderr(output)
    );
}

fn run_patch_json<I, S>(args: I) -> Value
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = run_patch(args);
    assert_success(&output);
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "expected valid json stdout, got error: {error}\nstdout:\n{}\nstderr:\n{}",
            stdout(&output),
            stderr(&output)
        )
    })
}

fn run_patch_json_failure<I, S>(args: I) -> Value
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = run_patch(args);
    assert_failure(&output);
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "expected valid json stdout, got error: {error}\nstdout:\n{}\nstderr:\n{}",
            stdout(&output),
            stderr(&output)
        )
    })
}

fn diagnostics(value: &Value) -> &[Value] {
    value["diagnostics"].as_array().unwrap_or_else(|| {
        panic!(
            "expected diagnostics array, got:\n{}",
            serde_json::to_string_pretty(value).expect("json value should serialize")
        )
    })
}

fn match_paths(value: &Value) -> Vec<&str> {
    value["data"]["matches"]
        .as_array()
        .unwrap_or_else(|| {
            panic!(
                "expected matches array, got:\n{}",
                serde_json::to_string_pretty(value).expect("json value should serialize")
            )
        })
        .iter()
        .map(|entry| {
            entry["path"].as_str().unwrap_or_else(|| {
                panic!(
                    "expected path string, got:\n{}",
                    serde_json::to_string_pretty(entry).expect("json value should serialize")
                )
            })
        })
        .collect()
}

#[test]
fn search_text_returns_typed_matches() {
    let value = run_patch_json([
        "search",
        "text",
        "symbol callers",
        "--scope",
        "src",
        "--json",
    ]);

    assert_eq!(value["command"], "search.text");
    assert_eq!(value["schema_version"], 2);
    assert_eq!(value["ok"], true);
    assert!(value["data"]["meta"].is_object());
    assert!(value["next"].is_array());

    let matches = value["data"]["matches"].as_array().unwrap_or_else(|| {
        panic!(
            "expected search.text matches array, got:\n{}",
            serde_json::to_string_pretty(&value).expect("json value should serialize")
        )
    });

    assert!(
        !matches.is_empty(),
        "expected at least one text search match: {value:#}"
    );
    let first = &matches[0];
    assert!(first["path"].is_string(), "expected path string: {first:#}");
    assert!(first["line"].is_u64(), "expected line number: {first:#}");
    assert!(first["text"].is_string(), "expected text string: {first:#}");

    let meta = value["data"]["meta"].as_object().unwrap_or_else(|| {
        panic!(
            "expected search.text meta object, got:\n{}",
            serde_json::to_string_pretty(&value).expect("json value should serialize")
        )
    });
    assert!(meta.get("query").is_some_and(Value::is_string));
    assert!(meta.get("scope").is_some_and(Value::is_string));
    assert!(meta.get("matches").is_some_and(Value::is_u64));
    assert!(meta.get("mode").is_some_and(Value::is_string));
    assert!(meta.get("stability").is_some_and(Value::is_string));
    assert!(meta.get("noise").is_some_and(Value::is_string));
}

#[test]
fn search_regex_returns_typed_matches() {
    let value = run_patch_json([
        "search",
        "regex",
        "symbol\\s+callers",
        "--scope",
        "src",
        "--json",
    ]);

    assert_eq!(value["command"], "search.regex");
    assert_eq!(value["schema_version"], 2);
    assert_eq!(value["ok"], true);
    assert!(value["data"]["meta"].is_object());
    assert!(value["next"].is_array());

    let matches = value["data"]["matches"].as_array().unwrap_or_else(|| {
        panic!(
            "expected search.regex matches array, got:\n{}",
            serde_json::to_string_pretty(&value).expect("json value should serialize")
        )
    });

    assert!(
        !matches.is_empty(),
        "expected at least one regex search match: {value:#}"
    );

    let meta = value["data"]["meta"].as_object().unwrap_or_else(|| {
        panic!(
            "expected search.regex meta object, got:\n{}",
            serde_json::to_string_pretty(&value).expect("json value should serialize")
        )
    });
    assert!(meta.get("query").is_some_and(Value::is_string));
    assert!(meta.get("scope").is_some_and(Value::is_string));
    assert!(meta.get("matches").is_some_and(Value::is_u64));
    assert!(meta.get("mode").is_some_and(Value::is_string));
    assert!(meta.get("stability").is_some_and(Value::is_string));
    assert!(meta.get("noise").is_some_and(Value::is_string));
}

#[test]
fn search_regex_invalid_pattern_returns_single_error_diagnostic() {
    let value = run_patch_json_failure(["search", "regex", "(", "--scope", "src", "--json"]);
    let diagnostics = diagnostics(&value);

    assert_eq!(value["command"], "search.regex");
    assert_eq!(value["schema_version"], 2);
    assert_eq!(value["ok"], false);
    assert!(value["data"]["meta"].is_object());
    assert!(value["next"].is_array());
    assert_eq!(diagnostics.len(), 1, "expected one diagnostic: {value:#}");
    assert_eq!(diagnostics[0]["level"], "error");
}

#[test]
fn search_text_treats_regex_like_input_as_literal_and_hints_about_regex_command() {
    let value = run_patch_json([
        "search",
        "text",
        "/symbol\\s+callers/",
        "--scope",
        "src",
        "--json",
    ]);
    let matches = value["data"]["matches"].as_array().unwrap_or_else(|| {
        panic!(
            "expected search.text matches array, got:\n{}",
            serde_json::to_string_pretty(&value).expect("json value should serialize")
        )
    });
    let next = value["next"].as_array().unwrap_or_else(|| {
        panic!(
            "expected next array, got:\n{}",
            serde_json::to_string_pretty(&value).expect("json value should serialize")
        )
    });

    assert_eq!(value["command"], "search.text");
    assert!(
        matches.is_empty(),
        "expected regex-like literal to stay literal: {value:#}"
    );
    assert!(
        next.iter().any(|item| {
            item["kind"] == "suggestion"
                && item["command"]
                    .as_str()
                    .is_some_and(|command| command.contains("search regex"))
        }),
        "expected regex guidance in next items: {value:#}"
    );
}

#[test]
fn search_text_respects_patchignore_and_not_gitignore() {
    let ignored = run_patch_json([
        "search",
        "text",
        "IGNORED_TEXT_MARKER",
        "--scope",
        PATCHIGNORE_SCOPE,
        "--json",
    ]);
    let gitignored = run_patch_json([
        "search",
        "text",
        "GITONLY_KEEP_MARKER",
        "--scope",
        PATCHIGNORE_SCOPE,
        "--json",
    ]);

    assert!(
        match_paths(&ignored).is_empty(),
        "expected ignored text matches to be excluded from traversal: {ignored:#}"
    );
    assert!(
        match_paths(&gitignored).contains(&"gitignored-only.rs"),
        "expected .gitignore-only file to remain searchable: {gitignored:#}"
    );
}

#[test]
fn search_regex_respects_patchignore() {
    let value = run_patch_json([
        "search",
        "regex",
        "IGNORED_[A-Z_]+_MARKER",
        "--scope",
        PATCHIGNORE_SCOPE,
        "--json",
    ]);

    assert!(
        match_paths(&value).is_empty(),
        "expected ignored regex matches to be excluded from traversal: {value:#}"
    );
}

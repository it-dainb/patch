use std::ffi::OsStr;
use std::process::Output;

use assert_cmd::Command;
use serde_json::Value;

fn run_drail<I, S>(args: I) -> Output
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    Command::cargo_bin("drail")
        .expect("drail binary should build for integration tests")
        .args(args)
        .output()
        .expect("drail should execute")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn combined_output(output: &Output) -> String {
    format!("{}{}", stdout(output), stderr(output))
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

fn assert_contains(haystack: &str, needle: &str) {
    assert!(
        haystack.contains(needle),
        "expected to find {needle:?} in:\n{haystack}"
    );
}

fn parse_json_stdout(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "expected valid json stdout, got error: {error}\nstdout:\n{}\nstderr:\n{}",
            stdout(output),
            stderr(output)
        )
    })
}

#[test]
fn read_requires_path_arg() {
    let output = run_drail(["read"]);

    assert_failure(&output);
    assert_contains(&stderr(&output), "USAGE:");
}

#[test]
fn symbol_find_is_a_valid_command() {
    let output = run_drail(["symbol", "find", "main", "--scope", "src"]);

    assert_success(&output);
}

#[test]
fn help_lists_only_current_command_families() {
    let output = run_drail(["--help"]);
    let text = combined_output(&output);

    assert_success(&output);
    assert_contains(&text, "Commands:");
    assert_contains(&text, "read");
    assert_contains(&text, "symbol");
    assert_contains(&text, "search");
    assert_contains(&text, "files");
    assert_contains(&text, "deps");
    assert_contains(&text, "map");
    assert!(
        !text.contains("install") && !text.contains("mcp"),
        "expected help text to stay CLI-only, got:\n{text}"
    );
}

#[test]
fn symbol_help_lists_find_and_callers_subcommands() {
    let output = run_drail(["symbol", "--help"]);
    let text = combined_output(&output);

    assert_success(&output);
    assert_contains(&text, "find");
    assert_contains(&text, "callers");
}

#[test]
fn search_help_lists_text_and_regex_subcommands() {
    let output = run_drail(["search", "--help"]);
    let text = combined_output(&output);

    assert_success(&output);
    assert_contains(&text, "text");
    assert_contains(&text, "regex");
}

#[test]
fn bare_unknown_command_is_rejected() {
    let output = run_drail(["foo"]);

    assert_failure(&output);
    assert_contains(&stderr(&output), "unrecognized subcommand 'foo'");
}

#[test]
fn read_section_flag_is_rejected() {
    let output = run_drail(["read", "x", "--section", "1-9"]);

    assert_failure(&output);
    assert_contains(&stderr(&output), "unexpected argument '--section'");
}

#[test]
fn unknown_command_json_uses_v2_error_envelope() {
    let output = run_drail(["foo", "--json"]);

    assert_failure(&output);
    let value = parse_json_stdout(&output);
    assert_eq!(value["command"], "cli");
    assert_eq!(value["schema_version"], 2);
    assert_eq!(value["ok"], false);
    assert!(value["data"]["meta"].is_object());
    assert!(value["next"].as_array().is_some_and(|next| next.is_empty()));
    let diagnostics = value["diagnostics"].as_array().unwrap();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0]["level"], "error");
    assert_eq!(diagnostics[0]["code"], "clap");
}

#[test]
fn missing_path_json_uses_v2_error_envelope() {
    let output = run_drail(["read", "--json"]);

    assert_failure(&output);
    let value = parse_json_stdout(&output);
    assert_eq!(value["command"], "cli");
    assert_eq!(value["schema_version"], 2);
    assert_eq!(value["ok"], false);
    assert!(value["data"]["meta"].is_object());
    assert!(value["next"].as_array().is_some_and(|next| next.is_empty()));
    let diagnostics = value["diagnostics"].as_array().unwrap();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0]["level"], "error");
    assert_eq!(diagnostics[0]["code"], "clap");
}

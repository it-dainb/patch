use std::ffi::OsStr;
use std::process::Output;

use assert_cmd::Command;
use serde_json::Value;

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

fn assert_contains(haystack: &str, needle: &str) {
    assert!(
        haystack.contains(needle),
        "expected to find {needle:?} in:\n{haystack}"
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

#[test]
fn read_path_renders_file_contents() {
    let output = run_patch(["read", "src/commands/read.rs"]);

    assert_success(&output);
    assert_contains(&stdout(&output), "pub fn run(args: &ReadArgs)");
}

#[test]
fn read_lines_renders_only_requested_range() {
    let output = run_patch(["read", "README.md", "--lines", "1:5"]);
    let text = stdout(&output);

    assert_success(&output);
    assert_contains(&text, "1 │ # patch");
    assert_contains(&text, "5 │ The product goal is simple");
    assert!(
        !text.contains("## Command families"),
        "expected later README content to be excluded, got:\n{text}"
    );
}

#[test]
fn read_json_data_contains_v2_meta() {
    let value = run_patch_json(["read", "README.md", "--lines", "1:5", "--json"]);

    assert_eq!(value["command"], "read");
    assert_eq!(value["schema_version"], 2);
    assert!(
        value["data"]["meta"].is_object(),
        "expected read meta object: {value:#}"
    );
    assert!(
        value["next"].is_array(),
        "expected read next array: {value:#}"
    );

    let meta = value["data"]["meta"].as_object().unwrap_or_else(|| {
        panic!(
            "expected read meta object, got:\n{}",
            serde_json::to_string_pretty(&value).expect("json value should serialize")
        )
    });
    assert!(meta.get("path").is_some_and(Value::is_string));
    assert!(meta.get("selector_kind").is_some_and(Value::is_string));
    assert!(meta.get("selector_display").is_some_and(Value::is_string));
    assert!(meta.get("file_kind").is_some_and(Value::is_string));
    assert!(meta.get("stability").is_some_and(Value::is_string));
    assert!(meta.get("noise").is_some_and(Value::is_string));
    assert!(meta.get("heading_aligned").is_some_and(Value::is_boolean));
}

#[test]
fn read_heading_renders_markdown_section() {
    let output = run_patch(["read", "README.md", "--heading", "## Installation"]);
    let text = stdout(&output);

    assert_success(&output);
    assert_contains(&text, "## Installation");
    assert_contains(&text, "./install.sh --dry-run");
    assert!(
        !text.contains("## Build and test"),
        "expected section to stop before next top-level heading, got:\n{text}"
    );
}

#[test]
fn read_markdown_lines_suggests_heading_when_heading_aligned() {
    let value = run_patch_json(["read", "README.md", "--lines", "10:14", "--json"]);

    let next = value["next"].as_array().unwrap_or_else(|| {
        panic!(
            "expected read next array, got:\n{}",
            serde_json::to_string_pretty(&value).expect("json value should serialize")
        )
    });
    assert!(
        next.iter().any(|item| {
            item["kind"] == "suggestion"
                && item["confidence"] == "high"
                && item["command"]
                    .as_str()
                    .is_some_and(|command| command.contains("--heading"))
        }),
        "expected heading-aligned markdown range to suggest --heading follow-up: {value:#}"
    );
}

#[test]
fn read_markdown_lines_without_heading_alignment_has_no_heading_hint() {
    let value = run_patch_json(["read", "README.md", "--lines", "11:14", "--json"]);

    let next = value["next"].as_array().unwrap_or_else(|| {
        panic!(
            "expected read next array, got:\n{}",
            serde_json::to_string_pretty(&value).expect("json value should serialize")
        )
    });
    assert!(
        next.iter().all(|item| {
            item["command"]
                .as_str()
                .is_none_or(|command| !command.contains("--heading"))
        }),
        "expected no heading suggestion when range starts inside body text: {value:#}"
    );
}

#[test]
fn read_markdown_lines_starting_before_heading_has_no_heading_hint() {
    let value = run_patch_json(["read", "README.md", "--lines", "9:12", "--json"]);

    let next = value["next"].as_array().unwrap_or_else(|| {
        panic!(
            "expected read next array, got:\n{}",
            serde_json::to_string_pretty(&value).expect("json value should serialize")
        )
    });
    assert!(
        next.iter().all(|item| {
            item["command"]
                .as_str()
                .is_none_or(|command| !command.contains("--heading"))
        }),
        "expected no heading suggestion when selected range starts before heading: {value:#}"
    );
}

#[test]
fn read_rejects_lines_and_heading_together() {
    let output = run_patch([
        "read",
        "README.md",
        "--lines",
        "1:4",
        "--heading",
        "## Installation",
    ]);

    assert_failure(&output);
    assert_contains(&stderr(&output), "cannot be used with");
}

#[test]
fn read_rejects_heading_for_non_markdown_files() {
    let output = run_patch([
        "read",
        "src/commands/read.rs",
        "--heading",
        "## Installation",
    ]);

    assert_failure(&output);
    assert_contains(&stderr(&output), "heading");
    assert_contains(&stderr(&output), "markdown");
}

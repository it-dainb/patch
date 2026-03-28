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

fn assert_not_contains(haystack: &str, needle: &str) {
    assert!(
        !haystack.contains(needle),
        "expected to not find {needle:?} in:\n{haystack}"
    );
}

fn evidence_block(text: &str) -> &str {
    text.split("## Evidence\n")
        .nth(1)
        .and_then(|rest| rest.split("\n\n## Next\n").next())
        .unwrap_or_else(|| panic!("expected output with Evidence and Next sections:\n{text}"))
}

fn assert_toon_success_baseline(text: &str) {
    assert_contains(text, "# read");
    let evidence = evidence_block(text);
    assert_contains(evidence, "[toon]");
    assert_not_contains(evidence, "{\n");
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

fn assert_has_high_confidence_heading_suggestion(next: &[Value], context: &Value) {
    assert!(
        next.iter().any(|item| {
            item["kind"] == "suggestion"
                && item["confidence"] == "high"
                && item["command"]
                    .as_str()
                    .is_some_and(|command| command.contains("--heading"))
        }),
        "expected heading-aligned markdown range to suggest --heading follow-up: {context:#}"
    );
}

fn assert_has_no_heading_suggestion(next: &[Value], context: &Value) {
    assert!(
        next.iter().all(|item| {
            item["command"]
                .as_str()
                .is_none_or(|command| !command.contains("--heading"))
        }),
        "expected no heading suggestion for this selected range: {context:#}"
    );
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
    let value = run_patch_json(["read", "README.md", "--lines", "19:22", "--json"]);

    let next = value["next"].as_array().unwrap_or_else(|| {
        panic!(
            "expected read next array, got:\n{}",
            serde_json::to_string_pretty(&value).expect("json value should serialize")
        )
    });
    assert_has_high_confidence_heading_suggestion(next, &value);
}

#[test]
fn read_markdown_lines_without_heading_alignment_has_no_heading_hint() {
    let value = run_patch_json(["read", "README.md", "--lines", "21:24", "--json"]);

    let next = value["next"].as_array().unwrap_or_else(|| {
        panic!(
            "expected read next array, got:\n{}",
            serde_json::to_string_pretty(&value).expect("json value should serialize")
        )
    });
    assert_has_no_heading_suggestion(next, &value);
}

#[test]
fn read_markdown_lines_starting_before_heading_has_no_heading_hint() {
    let value = run_patch_json(["read", "README.md", "--lines", "17:21", "--json"]);

    let next = value["next"].as_array().unwrap_or_else(|| {
        panic!(
            "expected read next array, got:\n{}",
            serde_json::to_string_pretty(&value).expect("json value should serialize")
        )
    });
    assert_has_no_heading_suggestion(next, &value);
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

#[test]
fn read_explicit_patchignored_path_still_succeeds() {
    let output = run_patch([
        "read",
        "tests/fixtures/patchignore/ignored-dir/ignored_api.rs",
    ]);
    let text = stdout(&output);

    assert_success(&output);
    assert_contains(&text, "pub fn ignored_api() -> &'static str");
    assert_contains(&text, "IGNORED_TEXT_MARKER");
}

#[test]
fn read_json_renders_toon_for_full_file() {
    let output = run_patch(["read", "tests/fixtures/json/users.json"]);
    let text = stdout(&output);

    assert_success(&output);
    assert_toon_success_baseline(&text);
    assert_contains(&text, "users[");
    assert_contains(&text, "meta:");
}

#[test]
fn read_json_full_flag_still_renders_toon() {
    let output = run_patch(["read", "tests/fixtures/json/users.json", "--full"]);
    let text = stdout(&output);

    assert_success(&output);
    assert_toon_success_baseline(&text);
    assert_contains(&text, "users[");
    assert_contains(&text, "meta:");
}

#[test]
fn read_json_key_renders_selected_subtree() {
    let output = run_patch(["read", "tests/fixtures/json/users.json", "--key", "meta"]);
    let text = stdout(&output);

    assert_success(&output);
    assert_toon_success_baseline(&text);
    assert_contains(&text, "generated_at:");
    assert_contains(&text, "integration-test");
    assert_not_contains(&text, "Ada");
}

#[test]
fn read_json_key_and_index_slice_selected_array() {
    let output = run_patch([
        "read",
        "tests/fixtures/json/users.json",
        "--key",
        "users",
        "--index",
        "0:1",
    ]);
    let text = stdout(&output);

    assert_success(&output);
    assert_toon_success_baseline(&text);
    assert_not_contains(&text, "[\n");
    assert_contains(&text, "id:");
    assert_contains(&text, "Ada");
    assert_not_contains(&text, "Lin");
}

#[test]
fn read_json_nested_numeric_key_path_resolves() {
    let output = run_patch([
        "read",
        "tests/fixtures/json/users.json",
        "--key",
        "users.0.accounts.1",
    ]);
    let text = stdout(&output);

    assert_success(&output);
    assert_toon_success_baseline(&text);
    assert_contains(&text, "sav-1");
    assert_not_contains(&text, "chk-2");
}

#[test]
fn read_json_root_array_index_slice_renders_toon() {
    let output = run_patch([
        "read",
        "tests/fixtures/json/root-array.json",
        "--index",
        "1:3",
    ]);
    let text = stdout(&output);

    assert_success(&output);
    assert_toon_success_baseline(&text);
    assert_not_contains(&text, "[\n");
    assert_contains(&text, "id,kind");
    assert_contains(&text, "b2");
    assert_contains(&text, "c3");
    assert_not_contains(&text, "a1");
}

#[test]
fn read_rejects_key_with_lines_or_heading() {
    let lines_output = run_patch([
        "read",
        "tests/fixtures/json/users.json",
        "--key",
        "users",
        "--lines",
        "1:2",
    ]);
    assert_failure(&lines_output);
    assert_contains(
        &stderr(&lines_output),
        "--key <KEY>' cannot be used with '--lines",
    );

    let heading_output = run_patch([
        "read",
        "tests/fixtures/json/users.json",
        "--key",
        "users",
        "--heading",
        "## anything",
    ]);
    assert_failure(&heading_output);
    assert_contains(
        &stderr(&heading_output),
        "--key <KEY>' cannot be used with '--heading",
    );
}

#[test]
fn read_rejects_index_with_lines_or_heading() {
    let lines_output = run_patch([
        "read",
        "tests/fixtures/json/users.json",
        "--index",
        "0:1",
        "--lines",
        "1:2",
    ]);
    assert_failure(&lines_output);
    assert_contains(
        &stderr(&lines_output),
        "--index <START:END>' cannot be used with '--lines",
    );

    let heading_output = run_patch([
        "read",
        "tests/fixtures/json/users.json",
        "--index",
        "0:1",
        "--heading",
        "## anything",
    ]);
    assert_failure(&heading_output);
    assert_contains(
        &stderr(&heading_output),
        "--index <START:END>' cannot be used with '--heading",
    );
}

#[test]
fn read_rejects_key_for_non_json_files() {
    let output = run_patch(["read", "README.md", "--key", "users"]);

    assert_failure(&output);
    assert_contains(&stderr(&output), "--key is only supported for JSON files");
}

#[test]
fn read_rejects_index_for_non_json_files() {
    let output = run_patch(["read", "README.md", "--index", "0:1"]);

    assert_failure(&output);
    assert_contains(&stderr(&output), "--index is only supported for JSON files");
}

#[test]
fn read_json_invalid_key_path_fails() {
    let output = run_patch([
        "read",
        "tests/fixtures/json/users.json",
        "--key",
        "users.0.missing",
    ]);

    assert_failure(&output);
    assert_contains(&stderr(&output), "missing key segment");
}

#[test]
fn read_json_invalid_index_syntax_fails() {
    let output = run_patch([
        "read",
        "tests/fixtures/json/root-array.json",
        "--index",
        "1-3",
    ]);

    assert_failure(&output);
    assert_contains(&stderr(&output), "expected index range in START:END format");
}

#[test]
fn read_json_out_of_range_index_fails() {
    let output = run_patch([
        "read",
        "tests/fixtures/json/users.json",
        "--key",
        "users",
        "--index",
        "10:12",
    ]);

    assert_failure(&output);
    assert_contains(&stderr(&output), "index range starts at");
}

#[test]
fn read_json_invalid_parse_fails() {
    let output = run_patch(["read", "tests/fixtures/json/invalid.json"]);

    assert_failure(&output);
    assert_contains(&stderr(&output), "invalid JSON");
}

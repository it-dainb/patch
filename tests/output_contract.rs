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

fn run_patch_json<I, S>(args: I) -> Value
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = run_patch(args);
    assert_success(&output);
    parse_json_stdout(&output)
}

fn run_patch_json_failure<I, S>(args: I) -> Value
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = run_patch(args);
    assert_failure(&output);
    parse_json_stdout(&output)
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

fn next_items<'a>(value: &'a Value, command: &str) -> &'a [Value] {
    value["next"].as_array().unwrap_or_else(|| {
        panic!(
            "expected {command} next array, got:\n{}",
            serde_json::to_string_pretty(value).expect("json value should serialize")
        )
    })
}

fn diagnostics<'a>(value: &'a Value, command: &str) -> &'a [Value] {
    value["diagnostics"].as_array().unwrap_or_else(|| {
        panic!(
            "expected {command} diagnostics array, got:\n{}",
            serde_json::to_string_pretty(value).expect("json value should serialize")
        )
    })
}

fn meta<'a>(value: &'a Value, command: &str) -> &'a serde_json::Map<String, Value> {
    value["data"]["meta"].as_object().unwrap_or_else(|| {
        panic!(
            "expected {command} data.meta object, got:\n{}",
            serde_json::to_string_pretty(value).expect("json value should serialize")
        )
    })
}

fn assert_v2_envelope(value: &Value, command: &str) {
    assert_eq!(value["command"], command);
    assert_eq!(value["schema_version"], 2, "expected V2 schema: {value:#}");
    assert!(value["ok"].is_boolean(), "expected ok boolean: {value:#}");
    assert!(value["data"].is_object(), "expected data object: {value:#}");
    assert!(
        value["data"]["meta"].is_object(),
        "expected data.meta object: {value:#}"
    );
    assert!(value["next"].is_array(), "expected next array: {value:#}");
    assert!(
        value["diagnostics"].is_array(),
        "expected diagnostics array: {value:#}"
    );
}

fn assert_v2_next_item_shape(item: &Value) {
    let object = item.as_object().unwrap_or_else(|| {
        panic!(
            "expected next item object, got:\n{}",
            serde_json::to_string_pretty(item).expect("json value should serialize")
        )
    });

    assert_eq!(
        object.get("kind").and_then(Value::as_str),
        Some("suggestion"),
        "expected next.kind to equal 'suggestion': {item:#}"
    );
    assert!(
        object.get("message").and_then(Value::as_str).is_some(),
        "expected next.message string: {item:#}"
    );
    assert!(
        object.get("command").and_then(Value::as_str).is_some(),
        "expected next.command string: {item:#}"
    );
    assert_eq!(
        object.get("confidence").and_then(Value::as_str),
        Some("high"),
        "expected next.confidence to equal 'high': {item:#}"
    );
}

fn assert_text_section_order_v2(text: &str) {
    let mut lines = text.lines();
    let first_line = lines.next().expect("expected at least one output line");
    assert!(
        first_line.starts_with("# "),
        "expected summary header first, got:\n{text}"
    );

    let meta_index = text
        .find("## Meta")
        .unwrap_or_else(|| panic!("expected Meta block in output:\n{text}"));
    let evidence_index = text
        .find("## Evidence")
        .unwrap_or_else(|| panic!("expected Evidence block in output:\n{text}"));
    let next_index = text
        .find("## Next")
        .unwrap_or_else(|| panic!("expected Next block in output:\n{text}"));
    let diagnostics_index = text
        .find("## Diagnostics")
        .unwrap_or_else(|| panic!("expected Diagnostics block in output:\n{text}"));

    assert!(
        meta_index < evidence_index
            && evidence_index < next_index
            && next_index < diagnostics_index,
        "expected Meta, Evidence, Next, Diagnostics ordering:\n{text}"
    );
}

fn assert_empty_success_sections_are_none(text: &str) {
    let next_block = text
        .split("## Next\n")
        .nth(1)
        .unwrap_or_else(|| panic!("expected Next section:\n{text}"));
    let next_body = next_block
        .split("\n\n## Diagnostics\n")
        .next()
        .unwrap_or_else(|| panic!("expected Diagnostics section after Next:\n{text}"));
    assert_eq!(
        next_body.trim(),
        "(none)",
        "expected empty Next to render as (none):\n{text}"
    );

    let diagnostics_body = text
        .split("## Diagnostics\n")
        .nth(1)
        .unwrap_or_else(|| panic!("expected Diagnostics section:\n{text}"));
    assert_eq!(
        diagnostics_body.trim(),
        "(none)",
        "expected empty Diagnostics to render as (none):\n{text}"
    );
}

#[test]
fn schema_version_is_2_for_all_json_commands() {
    let cases = [
        (
            vec!["read", "README.md", "--lines", "1:4", "--json"],
            "read",
        ),
        (
            vec!["symbol", "find", "main", "--scope", "src", "--json"],
            "symbol.find",
        ),
        (
            vec![
                "symbol",
                "callers",
                "render",
                "--scope",
                "src/output",
                "--json",
            ],
            "symbol.callers",
        ),
        (
            vec![
                "search",
                "text",
                "symbol callers",
                "--scope",
                "src",
                "--json",
            ],
            "search.text",
        ),
        (
            vec![
                "search",
                "regex",
                "symbol\\s+callers",
                "--scope",
                "src",
                "--json",
            ],
            "search.regex",
        ),
        (
            vec!["files", "*.rs", "--scope", "src/output", "--json"],
            "files",
        ),
        (
            vec!["deps", "src/commands/deps.rs", "--scope", "src", "--json"],
            "deps",
        ),
        (vec!["map", "--scope", "src", "--json"], "map"),
    ];

    for (args, command) in cases {
        let value = run_patch_json(args);
        assert_v2_envelope(&value, command);
    }
}

#[test]
fn text_output_uses_v2_section_order() {
    let cases = [
        vec!["read", "README.md", "--lines", "1:4"],
        vec!["files", "*.rs", "--scope", "src/output"],
        vec!["search", "text", "symbol callers", "--scope", "src"],
        vec!["symbol", "find", "main", "--scope", "src"],
        vec!["deps", "src/commands/deps.rs", "--scope", "src"],
        vec!["map", "--scope", "src"],
    ];

    for args in cases {
        let output = run_patch(args);
        assert_success(&output);
        assert_text_section_order_v2(&stdout(&output));
    }
}

#[test]
fn next_items_follow_v2_object_shape() {
    let cases = [
        (
            vec![
                "files",
                "*.definitelymissingxyz",
                "--scope",
                "src",
                "--json",
            ],
            "files",
        ),
        (
            vec![
                "symbol",
                "find",
                "definitely_missing_symbol_xyz",
                "--scope",
                "src",
                "--json",
            ],
            "symbol.find",
        ),
        (
            vec!["read", "README.md", "--lines", "19:22", "--json"],
            "read",
        ),
    ];

    for (args, command) in cases {
        let value = run_patch_json(args);
        assert_v2_envelope(&value, command);
        let next = next_items(&value, command);
        assert!(
            !next.is_empty(),
            "expected at least one next item for {command}: {value:#}"
        );
        for item in next {
            assert_v2_next_item_shape(item);
        }
    }
}

#[test]
fn successful_commands_do_not_emit_placeholder_success_diagnostics() {
    let cases = [
        (
            vec!["read", "README.md", "--lines", "1:4", "--json"],
            "read",
        ),
        (
            vec!["files", "*.rs", "--scope", "src/output", "--json"],
            "files",
        ),
        (vec!["map", "--scope", "src", "--json"], "map"),
    ];

    for (args, command) in cases {
        let value = run_patch_json(args);
        let diagnostics = diagnostics(&value, command);
        assert!(
            diagnostics.is_empty(),
            "expected no placeholder success diagnostics for {command}: {value:#}"
        );
    }
}

#[test]
fn text_output_renders_none_for_empty_next_and_diagnostics() {
    let output = run_patch(["map", "--scope", "src"]);

    assert_success(&output);
    let text = stdout(&output);
    assert_text_section_order_v2(&text);
    assert_empty_success_sections_are_none(&text);
    assert!(
        !text.contains("[hint]"),
        "expected placeholder success hint to be absent:\n{text}"
    );
}

#[test]
fn json_errors_use_schema_version_2() {
    let value = run_patch_json_failure(["search", "regex", "(", "--scope", "src", "--json"]);
    assert_v2_envelope(&value, "search.regex");
    assert_eq!(
        value["ok"], false,
        "expected failing error envelope: {value:#}"
    );

    let diagnostics = diagnostics(&value, "search.regex");
    assert_eq!(
        diagnostics.len(),
        1,
        "expected exactly one error diagnostic: {value:#}"
    );
    assert_eq!(diagnostics[0]["level"], "error");
}

#[test]
fn json_errors_emit_empty_next_and_meta_objects_when_needed() {
    let value = run_patch_json_failure(["deps", "render", "--scope", "src", "--json"]);
    assert_v2_envelope(&value, "deps");
    assert_eq!(
        value["ok"], false,
        "expected invalid deps path to be reported as error"
    );
    assert!(
        meta(&value, "deps").is_empty(),
        "expected empty error meta object: {value:#}"
    );
    assert!(
        next_items(&value, "deps").is_empty(),
        "expected empty next array when no high-confidence recovery command exists: {value:#}"
    );
}

#[test]
fn invalid_query_text_output_uses_v2_error_sections() {
    let output = run_patch(["read", "README.md", "--lines", "4:1"]);
    assert_failure(&output);

    let text = stderr(&output);
    assert_text_section_order_v2(&text);
    assert!(
        text.contains("## Evidence\n(none)"),
        "expected error output to render empty evidence: {text}"
    );
    assert!(
        text.contains("## Next\n(none)"),
        "expected error output to render empty next: {text}"
    );
    assert!(
        text.contains("## Diagnostics\n- error: invalid query \"4:1\": line range must start at 1 and end at or after the start line [code: invalid_query]"),
        "expected Diagnostics section to include an error entry: {text}"
    );
}

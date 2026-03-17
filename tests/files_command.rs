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

fn files(value: &Value) -> &[Value] {
    value["data"]["files"].as_array().unwrap_or_else(|| {
        panic!(
            "expected files array, got:\n{}",
            serde_json::to_string_pretty(value).expect("json value should serialize")
        )
    })
}

#[test]
fn files_returns_typed_matches() {
    let value = run_patch_json(["files", "*.rs", "--scope", "src/output", "--json"]);

    assert_eq!(value["command"], "files");
    assert_eq!(value["schema_version"], 2);
    assert_eq!(value["ok"], true);
    assert!(value["data"]["meta"].is_object());
    assert!(value["next"].is_array());

    let files = files(&value);
    assert!(
        !files.is_empty(),
        "expected at least one file match: {value:#}"
    );

    let first = &files[0];
    assert!(first["path"].is_string(), "expected path string: {first:#}");
    assert!(
        first["preview"].is_string(),
        "expected preview string: {first:#}"
    );

    let meta = value["data"]["meta"].as_object().unwrap_or_else(|| {
        panic!(
            "expected files meta object, got:\n{}",
            serde_json::to_string_pretty(&value).expect("json value should serialize")
        )
    });
    assert!(meta.get("pattern").is_some_and(Value::is_string));
    assert!(meta.get("scope").is_some_and(Value::is_string));
    assert!(meta.get("files").is_some_and(Value::is_u64));
    assert!(meta.get("stability").is_some_and(Value::is_string));
    assert!(meta.get("noise").is_some_and(Value::is_string));
}

#[test]
fn files_no_match_reports_single_recovery_hint() {
    let value = run_patch_json([
        "files",
        "*.definitelymissingxyz",
        "--scope",
        "src",
        "--json",
    ]);

    assert_eq!(value["command"], "files");
    assert_eq!(
        value["ok"], true,
        "expected no-match to stay non-fatal: {value:#}"
    );
    assert_eq!(files(&value).len(), 0);

    let next = value["next"].as_array().unwrap_or_else(|| {
        panic!(
            "expected next array, got:\n{}",
            serde_json::to_string_pretty(&value).expect("json value should serialize")
        )
    });

    assert_eq!(
        next.len(),
        1,
        "expected exactly one recovery next step: {value:#}"
    );
    assert_eq!(next[0]["kind"], "suggestion");
    assert_eq!(next[0]["confidence"], "high");
    assert!(next[0]["message"].is_string());
    assert!(next[0]["command"].is_string());
}

#[test]
fn files_multiple_matches_use_stable_ordering() {
    let value = run_patch_json(["files", "*.rs", "--scope", "src/output", "--json"]);
    let files = files(&value);

    let paths: Vec<&str> = files
        .iter()
        .map(|entry| {
            entry["path"].as_str().unwrap_or_else(|| {
                panic!(
                    "expected path string, got:\n{}",
                    serde_json::to_string_pretty(entry).expect("json value should serialize")
                )
            })
        })
        .collect();

    let mut sorted = paths.clone();
    sorted.sort_unstable();
    assert_eq!(
        paths, sorted,
        "expected stable path ordering for multiple matches"
    );
}

#[test]
fn files_text_no_match_includes_single_next_step_hint() {
    let output = run_patch(["files", "*.definitelymissingxyz", "--scope", "src"]);
    let text = stdout(&output);

    assert_success(&output);
    assert!(text.contains("## Next"), "expected next block: {text}");
    assert!(
        text.contains("patch files")
            || text.contains("patch search")
            || text.contains("patch symbol"),
        "expected a next-step suggestion in text output: {text}"
    );
}

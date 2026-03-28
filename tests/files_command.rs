use std::ffi::OsStr;
use std::path::Path;
use std::path::PathBuf;
use std::process::Output;

use assert_cmd::Command;
use serde_json::Value;

const DRAILIGNORE_SCOPE: &str = "tests/fixtures/drailignore";

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

fn run_drail_from<I, S>(args: I, cwd: &Path) -> Output
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    Command::cargo_bin("drail")
        .expect("drail binary should build for integration tests")
        .current_dir(cwd)
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

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "expected success, got status {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        stdout(output),
        stderr(output)
    );
}

fn run_drail_json<I, S>(args: I) -> Value
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = run_drail(args);
    assert_success(&output);
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "expected valid json stdout, got error: {error}\nstdout:\n{}\nstderr:\n{}",
            stdout(&output),
            stderr(&output)
        )
    })
}

fn run_drail_json_from<I, S>(args: I, cwd: &Path) -> Value
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = run_drail_from(args, cwd);
    assert_success(&output);
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "expected valid json stdout, got error: {error}\nstdout:\n{}\nstderr:\n{}",
            stdout(&output),
            stderr(&output)
        )
    })
}

fn fixture_dir_from_repo(relative_path: &str) -> PathBuf {
    std::env::current_dir()
        .expect("integration test process should have a current dir")
        .join(relative_path)
}

fn files(value: &Value) -> &[Value] {
    value["data"]["files"].as_array().unwrap_or_else(|| {
        panic!(
            "expected files array, got:\n{}",
            serde_json::to_string_pretty(value).expect("json value should serialize")
        )
    })
}

fn file_paths(value: &Value) -> Vec<&str> {
    files(value)
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
fn files_returns_typed_matches() {
    let value = run_drail_json(["files", "*.rs", "--scope", "src/output", "--json"]);

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
    let value = run_drail_json([
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

    assert!(
        next.iter().any(|item| {
            item["kind"] == "suggestion"
                && item["confidence"] == "high"
                && item["message"].is_string()
                && item["command"].is_string()
        }),
        "expected at least one high-confidence recovery next step: {value:#}"
    );
}

#[test]
fn files_multiple_matches_use_stable_ordering() {
    let value = run_drail_json(["files", "*.rs", "--scope", "src/output", "--json"]);
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
    let output = run_drail(["files", "*.definitelymissingxyz", "--scope", "src"]);
    let text = stdout(&output);

    assert_success(&output);
    assert!(text.contains("## Next"), "expected next block: {text}");
    assert!(
        text.contains("drail files")
            || text.contains("drail search")
            || text.contains("drail symbol"),
        "expected a next-step suggestion in text output: {text}"
    );
}

#[test]
fn files_no_match_guidance_renders_in_next_section() {
    let output = run_drail(["files", "*.definitelymissingxyz", "--scope", "src"]);
    let text = stdout(&output);

    assert_success(&output);

    let evidence = text
        .split("## Evidence\n")
        .nth(1)
        .and_then(|section| section.split("\n\n## Next\n").next())
        .unwrap_or_else(|| panic!("expected Evidence section: {text}"));
    let next = text
        .split("## Next\n")
        .nth(1)
        .and_then(|section| section.split("\n\n## Diagnostics\n").next())
        .unwrap_or_else(|| panic!("expected Next section: {text}"));

    assert!(
        !evidence.contains("Try:"),
        "expected Evidence to stay evidentiary only: {text}"
    );
    assert!(
        next.contains("drail files"),
        "expected recovery guidance in Next section: {text}"
    );
}

#[test]
fn files_respects_drailignore_patterns_but_not_gitignore() {
    let value = run_drail_json(["files", "*.rs", "--scope", DRAILIGNORE_SCOPE, "--json"]);
    let paths = file_paths(&value);

    assert!(
        paths.contains(&"gitignored-only.rs"),
        "expected .gitignore-only file to remain visible: {value:#}"
    );
    assert!(
        paths.contains(&"ignored-dir/reincluded.rs"),
        "expected negated .drailignore path to be re-included: {value:#}"
    );
    assert!(
        paths.contains(&"nested/root-only.rs"),
        "expected nested path to survive root-relative ignore rule: {value:#}"
    );
    assert!(
        !paths.contains(&"generated.gen.rs"),
        "expected ignored glob match to be excluded: {value:#}"
    );
    assert!(
        !paths.contains(&"root-only.rs"),
        "expected root-relative ignored file to be excluded: {value:#}"
    );
    assert!(
        !paths.contains(&"ignored-dir/ignored_api.rs"),
        "expected ignored directory file to be excluded: {value:#}"
    );
}

#[test]
fn files_scope_parent_relative_uses_invoking_cwd() {
    let fixture_nested_dir = fixture_dir_from_repo("tests/fixtures/drailignore/nested");
    let value = run_drail_json_from(
        ["files", "*.rs", "--scope", "..", "--json"],
        &fixture_nested_dir,
    );
    let paths = file_paths(&value);

    assert!(
        paths.contains(&"visible_api.rs"),
        "expected visible_api.rs relative to resolved parent scope: {value:#}"
    );
    assert!(
        paths.contains(&"nested/root-only.rs"),
        "expected nested/root-only.rs relative to resolved parent scope: {value:#}"
    );
}

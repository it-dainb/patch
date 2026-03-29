use std::env;
use std::ffi::OsStr;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Output;
use std::time::{SystemTime, UNIX_EPOCH};

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

fn matches(value: &Value) -> &[Value] {
    value["data"]["matches"].as_array().unwrap_or_else(|| {
        panic!(
            "expected symbol.find matches array, got:\n{}",
            serde_json::to_string_pretty(value).expect("json value should serialize")
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

fn evidence_block(text: &str) -> &str {
    text.split("## Evidence\n")
        .nth(1)
        .and_then(|section| section.split("\n\n## Next\n").next())
        .unwrap_or_else(|| panic!("expected Evidence section: {text}"))
}

fn diagnostics_block(text: &str) -> &str {
    text.split("## Diagnostics\n")
        .nth(1)
        .unwrap_or_else(|| panic!("expected Diagnostics section: {text}"))
}

fn unique_temp_dir(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after unix epoch")
        .as_nanos();
    env::temp_dir().join(format!(
        "drail-symbol-find-{label}-{}-{nanos}",
        std::process::id()
    ))
}

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(label: &str) -> Self {
        let path = unique_temp_dir(label);
        fs::create_dir_all(&path).expect("temp dir should be created");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn write_oversized_minified_fixture(dir: &Path, query: &str) -> (PathBuf, String) {
    let path = dir.join("bundle.min.js");
    let mut full_line = String::new();
    while full_line.len() <= 550_000 {
        full_line.push_str("const fillerValue=1234567890;");
    }
    full_line.push_str(&format!(
        "export function {query}(n){{return n+1}}{query}(5);"
    ));
    fs::write(&path, format!("{full_line}\n")).expect("oversized fixture should be written");
    (path, full_line)
}

#[test]
fn symbol_find_returns_definitions_before_usages() {
    let value = run_drail_json(["symbol", "find", "main", "--scope", "src", "--json"]);
    let matches = matches(&value);

    assert_eq!(value["schema_version"], 2);
    assert!(value["data"]["meta"].is_object());
    assert!(value["next"].is_array());
    assert!(
        matches.len() >= 2,
        "expected at least two matches: {value:#}"
    );
    assert_eq!(matches[0]["kind"], "definition");
    assert_eq!(matches[1]["kind"], "usage");
}

#[test]
fn symbol_find_kind_definition_filters_to_definitions_only() {
    let value = run_drail_json([
        "symbol",
        "find",
        "common",
        "--scope",
        "src/output",
        "--kind",
        "definition",
        "--json",
    ]);

    let matches = matches(&value);
    assert!(
        !matches.is_empty(),
        "expected definition matches: {value:#}"
    );
    assert!(matches.iter().all(|entry| entry["kind"] == "definition"));

    let meta = value["data"]["meta"].as_object().unwrap_or_else(|| {
        panic!(
            "expected symbol.find meta object, got:\n{}",
            serde_json::to_string_pretty(&value).expect("json value should serialize")
        )
    });
    assert!(meta.get("query").is_some_and(Value::is_string));
    assert!(meta.get("scope").is_some_and(Value::is_string));
    assert!(meta.get("definitions").is_some_and(Value::is_u64));
    assert!(meta.get("usages").is_some_and(Value::is_u64));
    assert!(meta.get("stability").is_some_and(Value::is_string));
    assert!(meta.get("noise").is_some_and(Value::is_string));
}

#[test]
fn symbol_find_kind_usage_filters_to_usages_only() {
    let value = run_drail_json([
        "symbol",
        "find",
        "common",
        "--scope",
        "src/output",
        "--kind",
        "usage",
        "--json",
    ]);

    let matches = matches(&value);
    assert!(!matches.is_empty(), "expected usage matches: {value:#}");
    assert!(matches.iter().all(|entry| entry["kind"] == "usage"));
}

#[test]
fn symbol_find_no_match_reports_one_recovery_suggestion() {
    let value = run_drail_json([
        "symbol",
        "find",
        "definitely_missing_symbol_xyz",
        "--scope",
        "src",
        "--json",
    ]);

    assert_eq!(
        value["ok"], true,
        "expected no-match to stay non-fatal: {value:#}"
    );
    assert_eq!(matches(&value).len(), 0);

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
fn symbol_find_multiple_definition_matches_use_stable_ordering() {
    let value = run_drail_json([
        "symbol",
        "find",
        "render",
        "--scope",
        "src/output",
        "--kind",
        "definition",
        "--json",
    ]);

    let matches = matches(&value);
    let paths: Vec<&str> = matches
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
fn symbol_find_text_no_match_includes_single_next_step_hint() {
    let output = run_drail([
        "symbol",
        "find",
        "definitely_missing_symbol_xyz",
        "--scope",
        "src",
    ]);
    let text = stdout(&output);

    assert_success(&output);
    assert!(text.contains("## Next"), "expected Next block: {text}");
    assert!(
        text.contains("drail symbol find")
            || text.contains("drail files")
            || text.contains("drail search"),
        "expected a next-step suggestion in text output: {text}"
    );
}

#[test]
fn symbol_find_no_match_guidance_renders_in_next_section() {
    let output = run_drail([
        "symbol",
        "find",
        "definitely_missing_symbol_xyz",
        "--scope",
        "src",
    ]);
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
        next.contains("drail search text"),
        "expected recovery guidance in Next section: {text}"
    );
}

#[test]
fn symbol_find_excludes_drailignored_definitions() {
    let value = run_drail_json([
        "symbol",
        "find",
        "ignored_api",
        "--scope",
        DRAILIGNORE_SCOPE,
        "--kind",
        "definition",
        "--json",
    ]);

    assert!(
        matches(&value).is_empty(),
        "expected ignored definitions to be excluded from traversal: {value:#}"
    );
}

#[test]
fn symbol_find_scope_dot_uses_invoking_cwd() {
    let fixture_dir = fixture_dir_from_repo("tests/fixtures/drailignore");
    let value = run_drail_json_from(
        ["symbol", "find", "visible_api", "--scope", ".", "--json"],
        &fixture_dir,
    );
    let matches = matches(&value);

    assert!(
        matches
            .iter()
            .any(|entry| entry["kind"] == "definition" && entry["path"] == "visible_api.rs"),
        "expected visible_api definition in scope-relative results: {value:#}"
    );
    assert!(
        matches
            .iter()
            .any(|entry| { entry["kind"] == "usage" && entry["path"] == "visible_caller.rs" }),
        "expected visible_caller usage in scope-relative results: {value:#}"
    );
}

#[test]
fn symbol_find_minified_fallback_returns_usage_only_snippets() {
    let value = run_drail_json([
        "symbol",
        "find",
        "stableEntryPoint",
        "--scope",
        "tests/fixtures/minified",
        "--json",
    ]);
    let fixture = include_str!("fixtures/minified/app.min.js").trim_end();
    let matches = matches(&value);

    assert_eq!(value["ok"], true);
    assert!(
        !matches.is_empty(),
        "expected text fallback results for minified fixture: {value:#}"
    );
    assert!(
        matches.iter().all(|entry| entry["kind"] == "usage"),
        "expected fallback matches to keep usage kind only: {value:#}"
    );

    for entry in matches {
        let snippet = entry["snippet"].as_str().unwrap_or_else(|| {
            panic!(
                "expected fallback snippet text string, got:\n{}",
                serde_json::to_string_pretty(entry).expect("json value should serialize")
            )
        });
        assert!(
            snippet.contains("stableEntryPoint"),
            "expected fallback snippet to contain query token: {entry:#}"
        );
        assert!(
            snippet.len() < fixture.len(),
            "expected fallback snippet shorter than original one-line fixture"
        );
    }

    let fallback_warnings = diagnostics(&value)
        .iter()
        .filter(|diag| diag["level"] == "warning" && diag["code"] == "text_fallback_used")
        .count();
    assert_eq!(
        fallback_warnings, 1,
        "expected one text fallback warning: {value:#}"
    );
}

#[test]
fn symbol_find_oversized_minified_bundle_uses_text_fallback() {
    let temp_dir = TempDir::new("oversized-minified-fallback");
    let (_, full_line) = write_oversized_minified_fixture(temp_dir.path(), "oversizedEntryPoint");

    let value = run_drail_json_from(
        [
            "symbol",
            "find",
            "oversizedEntryPoint",
            "--scope",
            ".",
            "--json",
        ],
        temp_dir.path(),
    );
    let matches = matches(&value);

    assert_eq!(value["ok"], true);
    assert!(
        !matches.is_empty(),
        "expected oversized minified fallback result instead of silent skip: {value:#}"
    );
    assert!(
        matches.iter().all(|entry| entry["kind"] == "usage"),
        "expected fallback matches to keep usage kind only: {value:#}"
    );

    for entry in matches {
        let snippet = entry["snippet"].as_str().unwrap_or_else(|| {
            panic!(
                "expected fallback snippet text string, got:\n{}",
                serde_json::to_string_pretty(entry).expect("json value should serialize")
            )
        });
        assert!(
            snippet.contains("oversizedEntryPoint"),
            "expected fallback snippet to contain query token: {entry:#}"
        );
        assert!(
            snippet.len() < full_line.len(),
            "expected fallback snippet shorter than original one-line oversized fixture"
        );
    }

    let fallback_warnings = diagnostics(&value)
        .iter()
        .filter(|diag| diag["level"] == "warning" && diag["code"] == "text_fallback_used")
        .count();
    assert_eq!(
        fallback_warnings, 1,
        "expected one text fallback warning: {value:#}"
    );
}

#[test]
fn symbol_find_text_output_minified_fallback_hides_raw_line_and_warns() {
    let output = run_drail([
        "symbol",
        "find",
        "stableEntryPoint",
        "--scope",
        "tests/fixtures/minified",
    ]);
    let text = stdout(&output);
    let fixture = include_str!("fixtures/minified/app.min.js").trim_end();

    assert_success(&output);

    let evidence = evidence_block(&text);
    assert!(
        evidence.contains("stableEntryPoint"),
        "expected fallback evidence to include query token: {text}"
    );
    assert!(
        !evidence.contains(fixture),
        "expected fallback evidence to avoid full raw minified line: {text}"
    );

    let diagnostics = diagnostics_block(&text);
    assert!(
        diagnostics.contains("text_fallback_used"),
        "expected diagnostics to include text_fallback_used warning code: {text}"
    );
}

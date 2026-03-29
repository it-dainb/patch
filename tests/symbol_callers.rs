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

fn callers(value: &Value) -> &[Value] {
    value["data"]["callers"].as_array().unwrap_or_else(|| {
        panic!(
            "expected symbol.callers callers array, got:\n{}",
            serde_json::to_string_pretty(value).expect("json value should serialize")
        )
    })
}

fn impact(value: &Value) -> &[Value] {
    value["data"]["impact"].as_array().unwrap_or_else(|| {
        panic!(
            "expected symbol.callers impact array, got:\n{}",
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
        "drail-symbol-callers-{label}-{}-{nanos}",
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
    full_line.push_str(&format!("function outer(){{{query}(5)}}outer();"));
    fs::write(&path, format!("{full_line}\n")).expect("oversized fixture should be written");
    (path, full_line)
}

#[test]
fn symbol_callers_returns_callers_in_stable_order() {
    let value = run_drail_json([
        "symbol",
        "callers",
        "render",
        "--scope",
        "src/output",
        "--json",
    ]);
    let callers = callers(&value);

    assert_eq!(value["command"], "symbol.callers");
    assert_eq!(value["schema_version"], 2);
    assert!(value["data"]["meta"].is_object());
    assert!(value["next"].is_array());
    assert!(
        !callers.is_empty(),
        "expected at least one caller: {value:#}"
    );

    let locations: Vec<(&str, u64)> = callers
        .iter()
        .map(|caller| {
            (
                caller["path"].as_str().unwrap_or_else(|| {
                    panic!(
                        "expected caller path string, got:\n{}",
                        serde_json::to_string_pretty(caller).expect("json value should serialize")
                    )
                }),
                caller["line"].as_u64().unwrap_or_else(|| {
                    panic!(
                        "expected caller line number, got:\n{}",
                        serde_json::to_string_pretty(caller).expect("json value should serialize")
                    )
                }),
            )
        })
        .collect();

    // Verify stable ordering: callers sorted by (path, line)
    for pair in locations.windows(2) {
        assert!(
            pair[0] <= pair[1],
            "expected callers in stable (path, line) order: {pair:?}"
        );
    }

    let meta = value["data"]["meta"].as_object().unwrap_or_else(|| {
        panic!(
            "expected symbol.callers meta object, got:\n{}",
            serde_json::to_string_pretty(&value).expect("json value should serialize")
        )
    });
    assert!(meta.get("query").is_some_and(Value::is_string));
    assert!(meta.get("scope").is_some_and(Value::is_string));
    assert!(meta.get("direct_call_sites").is_some_and(Value::is_u64));
    assert!(meta.get("second_hop_sites").is_some_and(Value::is_u64));
    assert!(meta.get("stability").is_some_and(Value::is_string));
    assert!(meta.get("noise").is_some_and(Value::is_string));
    assert!(meta.get("truncated").is_some_and(Value::is_boolean));
}

#[test]
fn symbol_callers_preserves_second_hop_results_in_typed_output() {
    let value = run_drail_json([
        "symbol",
        "callers",
        "render",
        "--scope",
        "src/output",
        "--json",
    ]);
    let impact = impact(&value);

    assert!(
        !impact.is_empty(),
        "expected second-hop impact entries: {value:#}"
    );

    let first = &impact[0];
    assert!(
        first["path"].is_string(),
        "expected impact path string: {first:#}"
    );
    assert!(
        first["line"].is_u64(),
        "expected impact line number: {first:#}"
    );
    assert!(
        first["caller"].is_string(),
        "expected impact caller string: {first:#}"
    );
    assert!(
        first["via"].is_string(),
        "expected impact via string: {first:#}"
    );
}

#[test]
fn symbol_callers_reports_warning_for_symbols_without_meaningful_callers_relation() {
    let value = run_drail_json([
        "symbol",
        "callers",
        "SymbolCommand",
        "--scope",
        "src",
        "--json",
    ]);
    let callers = callers(&value);
    let diagnostics = value["diagnostics"].as_array().unwrap_or_else(|| {
        panic!(
            "expected diagnostics array, got:\n{}",
            serde_json::to_string_pretty(&value).expect("json value should serialize")
        )
    });
    let next = value["next"].as_array().unwrap_or_else(|| {
        panic!(
            "expected next array, got:\n{}",
            serde_json::to_string_pretty(&value).expect("json value should serialize")
        )
    });

    assert!(
        callers.is_empty(),
        "expected no call sites for non-callable relation: {value:#}"
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic["level"] == "warning"),
        "expected warning diagnostic for non-meaningful callers relation: {value:#}"
    );
    assert!(
        next.iter().any(|item| {
            item["kind"] == "suggestion"
                && item["confidence"] == "high"
                && item["command"]
                    .as_str()
                    .is_some_and(|command| command.contains("drail symbol find"))
        }),
        "expected follow-up next suggestion for non-meaningful callers relation: {value:#}"
    );
}

#[test]
fn symbol_callers_excludes_drailignored_call_sites() {
    let value = run_drail_json([
        "symbol",
        "callers",
        "visible_api",
        "--scope",
        DRAILIGNORE_SCOPE,
        "--json",
    ]);
    let callers = callers(&value);

    let caller_paths: Vec<&str> = callers
        .iter()
        .map(|caller| {
            caller["path"].as_str().unwrap_or_else(|| {
                panic!(
                    "expected caller path string, got:\n{}",
                    serde_json::to_string_pretty(caller).expect("json value should serialize")
                )
            })
        })
        .collect();

    assert!(
        caller_paths.contains(&"visible_caller.rs"),
        "expected visible caller to remain: {value:#}"
    );
    assert!(
        !caller_paths.contains(&"ignored-dir/ignored_caller.rs"),
        "expected ignored caller to be excluded from traversal: {value:#}"
    );
}

#[test]
fn symbol_callers_scope_dot_uses_invoking_cwd() {
    let fixture_dir = fixture_dir_from_repo("tests/fixtures/drailignore");
    let value = run_drail_json_from(
        ["symbol", "callers", "visible_api", "--scope", ".", "--json"],
        &fixture_dir,
    );

    let caller_paths: Vec<&str> = callers(&value)
        .iter()
        .map(|caller| {
            caller["path"].as_str().unwrap_or_else(|| {
                panic!(
                    "expected caller path string, got:\n{}",
                    serde_json::to_string_pretty(caller).expect("json value should serialize")
                )
            })
        })
        .collect();

    assert!(
        caller_paths.contains(&"visible_caller.rs"),
        "expected direct caller to remain scope-relative: {value:#}"
    );
}

#[test]
fn symbol_callers_minified_fallback_returns_placeholder_caller_and_empty_impact() {
    let value = run_drail_json([
        "symbol",
        "callers",
        "stableEntryPoint",
        "--scope",
        "tests/fixtures/minified",
        "--json",
    ]);
    let fixture = include_str!("fixtures/minified/app.min.js").trim_end();
    let callers = callers(&value);

    assert_eq!(value["ok"], true);
    assert!(
        !callers.is_empty(),
        "expected fallback caller entries for minified fixture: {value:#}"
    );

    for caller in callers {
        let mut keys: Vec<&str> = caller
            .as_object()
            .unwrap_or_else(|| {
                panic!(
                    "expected caller entry object, got:\n{}",
                    serde_json::to_string_pretty(caller).expect("json value should serialize")
                )
            })
            .keys()
            .map(String::as_str)
            .collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            vec!["call_text", "caller", "line", "path"],
            "expected fallback caller rows to contain existing fields only: {caller:#}"
        );
        assert!(
            caller["path"].is_string() && caller["line"].is_u64(),
            "expected fallback caller to keep existing structured fields: {caller:#}"
        );
        assert_eq!(caller["caller"], "<text-fallback>");

        let call_text = caller["call_text"].as_str().unwrap_or_else(|| {
            panic!(
                "expected call_text string, got:\n{}",
                serde_json::to_string_pretty(caller).expect("json value should serialize")
            )
        });
        assert!(
            call_text.contains("stableEntryPoint"),
            "expected fallback call_text to include query token: {caller:#}"
        );
        assert!(
            call_text.len() < fixture.len(),
            "expected fallback call_text shorter than original one-line fixture"
        );
    }

    assert!(
        impact(&value).is_empty(),
        "expected no second-hop impact for fallback-only callers: {value:#}"
    );

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
fn symbol_callers_oversized_minified_bundle_uses_text_fallback() {
    let temp_dir = TempDir::new("oversized-minified-fallback");
    let (_, full_line) = write_oversized_minified_fixture(temp_dir.path(), "oversizedEntryPoint");

    let value = run_drail_json_from(
        [
            "symbol",
            "callers",
            "oversizedEntryPoint",
            "--scope",
            ".",
            "--json",
        ],
        temp_dir.path(),
    );
    let callers = callers(&value);

    assert_eq!(value["ok"], true);
    assert!(
        !callers.is_empty(),
        "expected oversized minified fallback caller entries instead of skip: {value:#}"
    );

    for caller in callers {
        let mut keys: Vec<&str> = caller
            .as_object()
            .unwrap()
            .keys()
            .map(|k| k.as_str())
            .collect();
        keys.sort_unstable();
        assert_eq!(keys, vec!["call_text", "caller", "line", "path"]);

        assert!(
            caller["path"].is_string() && caller["line"].is_u64(),
            "expected fallback caller to keep existing structured fields: {caller:#}"
        );
        assert_eq!(caller["caller"], "<text-fallback>");
        let call_text = caller["call_text"].as_str().unwrap_or_else(|| {
            panic!(
                "expected call_text string, got:\n{}",
                serde_json::to_string_pretty(caller).expect("json value should serialize")
            )
        });
        assert!(
            call_text.contains("oversizedEntryPoint"),
            "expected fallback call_text to include query token: {caller:#}"
        );
        assert!(
            call_text.len() < full_line.len(),
            "expected fallback call_text shorter than original one-line oversized fixture"
        );
    }

    assert!(
        impact(&value).is_empty(),
        "expected no second-hop impact for fallback-only callers: {value:#}"
    );

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
fn symbol_callers_text_output_minified_fallback_hides_raw_line_and_warns() {
    let output = run_drail([
        "symbol",
        "callers",
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

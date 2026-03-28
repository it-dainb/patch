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

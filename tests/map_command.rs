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
fn map_returns_deterministic_output_shape() {
    let output = run_patch(["map", "--scope", "src"]);
    assert_success(&output);

    let text = stdout(&output);
    assert!(
        text.contains("# map"),
        "expected map header in output:\n{text}"
    );
}

#[test]
fn map_json_returns_typed_data() {
    let value = run_patch_json(["map", "--scope", "src", "--json"]);

    assert_eq!(value["command"], "map");
    assert_eq!(value["schema_version"], 2);
    assert!(value["ok"].as_bool().unwrap_or(false));
    assert!(value["data"].is_object());
    assert!(value["data"]["meta"].is_object());
    assert!(value["next"].is_array());

    let data = &value["data"];
    assert!(data["depth"].is_number());
    assert!(data["total_files"].is_number());
    assert!(data["total_tokens"].is_number());
    assert!(data["entries"].is_array());
    assert!(data["tree_text"].is_string());

    let meta = value["data"]["meta"].as_object().unwrap_or_else(|| {
        panic!(
            "expected map meta object, got:\n{}",
            serde_json::to_string_pretty(&value).expect("json value should serialize")
        )
    });
    assert!(meta.get("scope").is_some_and(Value::is_string));
    assert!(meta.get("depth").is_some_and(Value::is_u64));
    assert!(meta.get("total_files").is_some_and(Value::is_u64));
    assert!(meta.get("total_tokens").is_some_and(Value::is_u64));
    assert!(meta.get("stability").is_some_and(Value::is_string));
    assert!(meta.get("noise").is_some_and(Value::is_string));
    assert!(meta.get("truncated").is_some_and(Value::is_boolean));

    assert!(
        data["total_files"].as_u64().unwrap_or(0) > 0,
        "expected src map to report nonzero total_files: {value:#}"
    );
    assert!(
        data["total_tokens"].as_u64().unwrap_or(0) > 0,
        "expected src map to report nonzero total_tokens: {value:#}"
    );
}

#[test]
fn map_budget_preserves_total_counts_and_marks_truncation() {
    let full = run_patch_json(["map", "--scope", "src", "--json"]);
    let budgeted = run_patch_json(["map", "--scope", "src", "--budget", "400", "--json"]);

    assert_eq!(
        budgeted["data"]["total_files"], full["data"]["total_files"],
        "expected budgeted map to preserve total_files: budgeted={budgeted:#} full={full:#}"
    );
    assert_eq!(
        budgeted["data"]["total_tokens"], full["data"]["total_tokens"],
        "expected budgeted map to preserve total_tokens: budgeted={budgeted:#} full={full:#}"
    );
    assert_eq!(
        budgeted["data"]["meta"]["truncated"], true,
        "expected budgeted map meta.truncated to be true: {budgeted:#}"
    );

    let entries = budgeted["data"]["entries"].as_array().unwrap_or_else(|| {
        panic!(
            "expected budgeted map entries array, got:\n{}",
            serde_json::to_string_pretty(&budgeted).expect("json value should serialize")
        )
    });
    assert!(
        entries.iter().all(|entry| {
            entry["path"]
                .as_str()
                .is_none_or(|path| !path.starts_with("... truncated ("))
        }),
        "expected budget truncation marker to stay out of parsed entries: {budgeted:#}"
    );
}

#[test]
fn map_depth_controls_traversal_depth() {
    let shallow = run_patch_json(["map", "--scope", "src", "--depth", "1", "--json"]);
    let deep = run_patch_json(["map", "--scope", "src", "--depth", "3", "--json"]);

    let shallow_files = shallow["data"]["total_files"].as_u64().unwrap_or(0);
    let deep_files = deep["data"]["total_files"].as_u64().unwrap_or(0);

    assert!(
        deep_files >= shallow_files,
        "deeper map should find >= files: shallow={shallow_files}, deep={deep_files}"
    );
}

#[test]
fn map_scope_restricts_to_given_directory() {
    let value = run_patch_json(["map", "--scope", "src", "--json"]);

    let scope = value["data"]["scope"].as_str().unwrap_or("");
    assert!(
        scope.contains("src"),
        "expected scope to contain 'src', got: {scope}"
    );
}

#[test]
fn map_respects_patchignore_patterns() {
    let value = run_patch_json(["map", "--scope", PATCHIGNORE_SCOPE, "--json"]);
    let total_files = value["data"]["total_files"].as_u64().unwrap_or_else(|| {
        panic!(
            "expected map total_files number, got:\n{}",
            serde_json::to_string_pretty(&value).expect("json value should serialize")
        )
    });
    let tree_text = value["data"]["tree_text"].as_str().unwrap_or_else(|| {
        panic!(
            "expected map tree_text string, got:\n{}",
            serde_json::to_string_pretty(&value).expect("json value should serialize")
        )
    });
    let entry_paths: Vec<&str> = value["data"]["entries"]
        .as_array()
        .unwrap_or_else(|| {
            panic!(
                "expected map entries array, got:\n{}",
                serde_json::to_string_pretty(&value).expect("json value should serialize")
            )
        })
        .iter()
        .filter_map(|entry| entry["path"].as_str())
        .collect();

    assert_eq!(
        total_files, 12,
        "expected map totals to exclude patchignored files: {value:#}"
    );
    assert_eq!(
        entry_paths.len(),
        12,
        "expected parsed map entries to exclude patchignored files: {value:#}"
    );

    assert!(
        tree_text.contains("gitignored-only.rs"),
        "expected .gitignore-only file to remain visible in map: {value:#}"
    );
    assert!(
        tree_text.contains("reincluded.rs") && tree_text.contains("reincluded_symbol"),
        "expected negated path to remain visible in map: {value:#}"
    );
    assert!(
        !tree_text.contains("ignored-dir/ignored_api.rs"),
        "expected ignored directory file to be excluded from map: {value:#}"
    );
    assert!(
        !tree_text.contains("generated.gen.rs"),
        "expected ignored glob match to be excluded from map: {value:#}"
    );
    assert!(
        !tree_text.contains("\nroot-only.rs:"),
        "expected root-relative ignored file to be excluded from top-level map output: {value:#}"
    );
    assert!(
        tree_text.contains("\nnested/\n  root-only.rs:"),
        "expected nested root-only file to remain visible in map output: {value:#}"
    );
    assert!(
        !entry_paths.contains(&"generated.gen.rs"),
        "expected ignored glob match to be excluded from map entries: {value:#}"
    );
    assert!(
        !entry_paths.contains(&"ignored_api.rs")
            && !entry_paths.contains(&"ignored_caller.rs")
            && !entry_paths.contains(&"ignored_dependent.rs"),
        "expected ignored directory files to be excluded from map entries: {value:#}"
    );
}

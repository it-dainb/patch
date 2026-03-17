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

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

fn diagnostics(value: &Value) -> &[Value] {
    value["diagnostics"].as_array().unwrap_or_else(|| {
        panic!(
            "expected diagnostics array, got:\n{}",
            serde_json::to_string_pretty(value).expect("json value should serialize")
        )
    })
}

fn local_uses(value: &Value) -> &[Value] {
    value["data"]["uses_local"].as_array().unwrap_or_else(|| {
        panic!(
            "expected uses_local array, got:\n{}",
            serde_json::to_string_pretty(value).expect("json value should serialize")
        )
    })
}

fn external_uses(value: &Value) -> &[Value] {
    value["data"]["uses_external"]
        .as_array()
        .unwrap_or_else(|| {
            panic!(
                "expected uses_external array, got:\n{}",
                serde_json::to_string_pretty(value).expect("json value should serialize")
            )
        })
}

fn used_by(value: &Value) -> &[Value] {
    value["data"]["used_by"].as_array().unwrap_or_else(|| {
        panic!(
            "expected used_by array, got:\n{}",
            serde_json::to_string_pretty(value).expect("json value should serialize")
        )
    })
}

#[test]
fn deps_returns_typed_reverse_dependency_data() {
    let value = run_patch_json(["deps", "src/commands/deps.rs", "--scope", "src", "--json"]);

    assert_eq!(value["command"], "deps");
    assert_eq!(value["schema_version"], 2);
    assert_eq!(value["ok"], true);
    assert!(value["data"]["meta"].is_object());
    assert!(value["next"].is_array());
    assert_eq!(value["data"]["path"], "commands/deps.rs");
    assert!(
        value["data"]["scope"].is_string(),
        "expected scope string: {value:#}"
    );
    assert!(
        value["data"]["uses_local"].is_array(),
        "expected uses_local array: {value:#}"
    );
    assert!(
        value["data"]["uses_external"].is_array(),
        "expected uses_external array: {value:#}"
    );
    assert!(
        value["data"]["used_by"].is_array(),
        "expected used_by array: {value:#}"
    );
    assert!(
        value["data"]["truncated_dependents"].is_u64(),
        "expected truncated_dependents count: {value:#}"
    );

    let local = local_uses(&value);
    assert!(
        !local.is_empty(),
        "expected at least one local dependency entry: {value:#}"
    );
    assert!(
        local[0]["path"].is_string(),
        "expected local path string: {:#}",
        local[0]
    );
    assert!(
        local[0]["symbols"].is_array(),
        "expected local symbols array: {:#}",
        local[0]
    );

    let dependents = used_by(&value);
    assert!(
        !dependents.is_empty(),
        "expected at least one reverse dependency entry: {value:#}"
    );
    let first = &dependents[0];
    assert!(
        first["path"].is_string(),
        "expected dependent path string: {first:#}"
    );
    assert!(
        first["is_test"].is_boolean(),
        "expected dependent is_test flag: {first:#}"
    );
    let callers = first["callers"].as_array().unwrap_or_else(|| {
        panic!(
            "expected dependent callers array, got:\n{}",
            serde_json::to_string_pretty(first).expect("json value should serialize")
        )
    });
    assert!(
        !callers.is_empty(),
        "expected at least one caller detail: {first:#}"
    );
    assert!(
        callers[0]["caller"].is_string(),
        "expected caller name string: {:#}",
        callers[0]
    );
    assert!(
        callers[0]["line"].is_u64(),
        "expected caller line number: {:#}",
        callers[0]
    );
    assert!(
        callers[0]["symbols"].is_array(),
        "expected caller symbol list: {:#}",
        callers[0]
    );

    let meta = value["data"]["meta"].as_object().unwrap_or_else(|| {
        panic!(
            "expected deps meta object, got:\n{}",
            serde_json::to_string_pretty(&value).expect("json value should serialize")
        )
    });
    assert!(meta.get("path").is_some_and(Value::is_string));
    assert!(meta.get("local_uses").is_some_and(Value::is_u64));
    assert!(meta.get("external_uses").is_some_and(Value::is_u64));
    assert!(meta.get("dependents").is_some_and(Value::is_u64));
    assert!(meta.get("stability").is_some_and(Value::is_string));
    assert!(meta.get("noise").is_some_and(Value::is_string));
    assert!(meta.get("truncated").is_some_and(Value::is_boolean));
}

#[test]
fn deps_reverse_dependencies_use_stable_path_ordering() {
    let value = run_patch_json(["deps", "src/commands/deps.rs", "--scope", "src", "--json"]);
    let dependents = used_by(&value);

    let paths: Vec<&str> = dependents
        .iter()
        .map(|entry| {
            entry["path"].as_str().unwrap_or_else(|| {
                panic!(
                    "expected dependent path string, got:\n{}",
                    serde_json::to_string_pretty(entry).expect("json value should serialize")
                )
            })
        })
        .collect();

    let mut sorted = paths.clone();
    sorted.sort_unstable();
    assert_eq!(
        paths, sorted,
        "expected stable alphabetical dependent ordering"
    );
}

#[test]
fn deps_rejects_symbol_like_input_without_path_interpretation() {
    let output = run_patch(["deps", "render", "--scope", "src", "--json"]);

    assert_success(&output);
    let value: Value = serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "expected valid json stdout, got error: {error}\nstdout:\n{}\nstderr:\n{}",
            stdout(&output),
            stderr(&output)
        )
    });

    assert_eq!(value["command"], "deps");
    assert_eq!(value["schema_version"], 2);
    assert_eq!(
        value["ok"], false,
        "expected invalid deps path to be reported as an error"
    );
    assert!(value["data"]["meta"].is_object());
    assert!(value["next"].is_array());

    let diagnostics = diagnostics(&value);
    assert_eq!(
        diagnostics.len(),
        1,
        "expected a single strict diagnostic: {value:#}"
    );
    assert_eq!(diagnostics[0]["level"], "error");
    assert!(
        diagnostics[0]["message"]
            .as_str()
            .is_some_and(|message| message.contains("render") && message.contains("not found")),
        "expected missing-path diagnostic mentioning the exact input: {value:#}"
    );
    assert!(
        value["next"].as_array().is_some_and(|next| next.is_empty()),
        "expected no fallback next command for deps path input: {value:#}"
    );
}

#[test]
fn deps_text_output_includes_structured_sections() {
    let output = run_patch(["deps", "src/commands/deps.rs", "--scope", "src"]);
    let text = stdout(&output);

    assert_success(&output);
    assert!(
        text.contains("Uses (local)"),
        "expected local uses section: {text}"
    );
    assert!(
        text.contains("Uses (external)"),
        "expected explicit external uses section even when empty: {text}"
    );
    assert!(
        text.contains("Used by"),
        "expected reverse dependency section: {text}"
    );
}

#[test]
fn deps_external_dependencies_use_stable_ordering() {
    let value = run_patch_json(["deps", "src/search/deps.rs", "--scope", "src", "--json"]);
    let externals = external_uses(&value);

    let modules: Vec<&str> = externals
        .iter()
        .map(|entry| {
            entry.as_str().unwrap_or_else(|| {
                panic!(
                    "expected external dependency string, got:\n{}",
                    serde_json::to_string_pretty(entry).expect("json value should serialize")
                )
            })
        })
        .collect();

    let mut sorted = modules.clone();
    sorted.sort_unstable();
    assert_eq!(
        modules, sorted,
        "expected stable external dependency ordering"
    );
}

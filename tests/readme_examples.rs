use std::env;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::{Command as ProcessCommand, Output};
use std::time::{SystemTime, UNIX_EPOCH};

use assert_cmd::Command;
use serde_json::Value;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

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

fn unique_temp_dir(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after unix epoch")
        .as_nanos();
    env::temp_dir().join(format!(
        "patch-readme-example-{label}-{}-{nanos}",
        std::process::id()
    ))
}

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(label: &str) -> Self {
        let path = unique_temp_dir(label);
        std::fs::create_dir_all(&path).expect("temp dir should be created");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn run_install(home: &Path, path_env: &str, dry_run: bool) -> Output {
    let mut command = ProcessCommand::new("bash");
    command.arg(repo_root().join("install.sh"));
    command.current_dir(repo_root());
    command.env("HOME", home);
    command.env("PATH", path_env);
    if dry_run {
        command.env("PATCH_INSTALL_DRY_RUN", "1");
    }
    command.output().expect("install.sh should execute")
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

fn parse_json_stdout(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "expected valid json stdout, got error: {error}\nstdout:\n{}\nstderr:\n{}",
            stdout(output),
            stderr(output)
        )
    })
}

fn read_repo_file(path: &str) -> String {
    std::fs::read_to_string(repo_root().join(path))
        .unwrap_or_else(|error| panic!("expected to read {path}, got error: {error}"))
}

const PATCHIGNORE_SCOPE: &str = "tests/fixtures/patchignore";

#[test]
fn readme_and_cli_contract_document_v2_output_contract() {
    let readme = read_repo_file("README.md");
    let contract = read_repo_file("docs/cli-contract.md");

    for text in [&readme, &contract] {
        assert_contains(text, "schema_version\": 2");
        assert_contains(text, "\"next\": []");
        assert_contains(text, "\"meta\": {}");
        assert_contains(text, "Meta");
        assert_contains(text, "Evidence");
        assert_contains(text, "Next");
        assert_contains(text, "Diagnostics");
        assert_contains(text, "(none)");
        assert_contains(text, "stderr");
    }

    assert_contains(&readme, "cargo run -- read README.md --lines 7:17");
    assert_contains(&readme, "JSON files always render as TOON text");
    assert_contains(
        &readme,
        "cargo run -- read tests/fixtures/json/users.json --key users.0.accounts",
    );
    assert_contains(
        &readme,
        "cargo run -- read tests/fixtures/json/root-array.json --index 0:1",
    );
    assert_contains(
        &readme,
        "cargo run -- read tests/fixtures/json/users.json --key users.0.accounts --index 0:1",
    );
    assert_contains(&readme, "`--key` and `--index` are JSON-only selectors");
    assert_contains(
        &readme,
        "cargo run -- files \"*.definitely-nope\" --scope src --json",
    );
    assert_contains(&readme, "cargo run -- search regex \"(\" --scope src");
    assert_contains(&readme, ".patchignore");
    assert_contains(&readme, "active scope root");
    assert_contains(&readme, ".gitignore is not read");
    assert_contains(&readme, "read still works for ignored paths");
    assert_contains(
        &readme,
        "deps accepts an ignored target path but filters traversal-derived results",
    );
    assert_contains(
        &readme,
        "cargo run -- files \"*.rs\" --scope tests/fixtures/patchignore",
    );
    assert_contains(&contract, "heading_aligned");
    assert_contains(&contract, "`read` supports these selectors");
    assert_contains(
        &contract,
        "`selector_kind`: `full`, `lines`, `heading`, `key`, `index`, `key_index`",
    );
    assert_contains(&contract, "`selector_display` uses raw selector text");
    assert_contains(&contract, "`{\"Key\": {\"value\": \"users.0.accounts\"}}`");
    assert_contains(&contract, "`{\"Index\": {\"start\": 0, \"end\": 1}}`");
    assert_contains(
        &contract,
        "`{\"KeyIndex\": {\"value\": \"users.0.accounts\", \"start\": 0, \"end\": 1}}`",
    );
    assert_contains(&contract, "JSON `read` content is TOON text");
    assert_contains(&contract, "`invalid JSON`");
    assert_contains(&contract, "`missing key segment`");
    assert_contains(&contract, "`expected numeric array index`");
    assert_contains(&contract, "`expected index range in START:END format`");
    assert_contains(
        &contract,
        "`index range end must be greater than or equal to start`",
    );
    assert_contains(&contract, "`index range starts at`");
    assert_contains(&contract, "`failed to encode JSON as TOON`");
    assert_contains(&contract, "first selected line itself");
    assert_contains(&contract, ".patchignore");
    assert_contains(&contract, "only one .patchignore at the scope root is read");
    assert_contains(&contract, "Traversal commands honor that file");
    assert_contains(&contract, ".gitignore is not read");
    assert_contains(&contract, "read still accepts an explicit ignored path");
    assert_contains(
        &contract,
        "deps accepts an explicit ignored target path but filters traversal-derived results",
    );
}

#[test]
fn quick_start_commands_from_readme_stay_valid() {
    let symbol_find = run_patch(["symbol", "find", "main", "--scope", "src"]);
    let symbol_text = stdout(&symbol_find);
    assert_success(&symbol_find);
    assert_contains(&symbol_text, "# symbol.find");
    assert_contains(&symbol_text, "## Meta");
    assert_contains(&symbol_text, "## Evidence");
    assert_contains(&symbol_text, "## Next");
    assert_contains(&symbol_text, "## Diagnostics");
    assert_contains(&symbol_text, "main.rs:");
    assert_contains(&symbol_text, "[definition]");

    let files = run_patch(["files", "*.rs", "--scope", "src"]);
    let files_text = stdout(&files);
    assert_success(&files);
    assert_contains(&files_text, "# files");
    assert_contains(&files_text, "files \"*.rs\"");
    assert_contains(&files_text, "## Next\n(none)");

    let deps = run_patch(["deps", "src/main.rs"]);
    let deps_text = stdout(&deps);
    assert_success(&deps);
    assert_contains(&deps_text, "# deps");
    assert_contains(&deps_text, "deps \"src/main.rs\"");
    assert_contains(&deps_text, "## Diagnostics");

    let map = run_patch(["map", "--scope", "src"]);
    assert_success(&map);
    assert_contains(&stdout(&map), "# Map:");
}

#[test]
fn read_command_examples_from_readme_stay_valid() {
    let lines = run_patch(["read", "README.md", "--lines", "7:17"]);
    let lines_text = stdout(&lines);
    assert_success(&lines);
    assert_contains(&lines_text, "## Meta");
    assert_contains(&lines_text, "heading_aligned: true");
    assert_contains(&lines_text, "## Why patch exists");
    assert_contains(
        &lines_text,
        "patch read \"README.md\" --heading \"## Why patch exists\"",
    );

    let heading = run_patch(["read", "README.md", "--heading", "## Command families"]);
    assert_success(&heading);
    assert_contains(&stdout(&heading), "## Command families");

    let json_key = run_patch([
        "read",
        "tests/fixtures/json/users.json",
        "--key",
        "users.0.accounts",
    ]);
    let json_key_text = stdout(&json_key);
    assert_success(&json_key);
    assert_contains(&json_key_text, "selector_kind: key");
    assert_contains(&json_key_text, "selector_display: users.0.accounts");
    assert_contains(&json_key_text, "[2]{id,type,balance}:");

    let json_index = run_patch([
        "read",
        "tests/fixtures/json/root-array.json",
        "--index",
        "0:1",
    ]);
    let json_index_text = stdout(&json_index);
    assert_success(&json_index);
    assert_contains(&json_index_text, "selector_kind: index");
    assert_contains(&json_index_text, "selector_display: 0:1");
    assert_contains(&json_index_text, "[1]{id,kind}:");

    let json_key_index = run_patch([
        "read",
        "tests/fixtures/json/users.json",
        "--key",
        "users.0.accounts",
        "--index",
        "0:1",
    ]);
    let json_key_index_text = stdout(&json_key_index);
    assert_success(&json_key_index);
    assert_contains(&json_key_index_text, "selector_kind: key_index");
    assert_contains(
        &json_key_index_text,
        "selector_display: users.0.accounts @ 0:1",
    );
    assert_contains(&json_key_index_text, "[1]{id,type,balance}:");
}

#[test]
fn patchignore_example_from_readme_stays_valid() {
    let files = run_patch(["files", "*.rs", "--scope", PATCHIGNORE_SCOPE]);
    let files_text = stdout(&files);

    assert_success(&files);
    assert_contains(&files_text, "# files");
    assert_contains(&files_text, "gitignored-only.rs");
    assert_contains(&files_text, "ignored-dir/reincluded.rs");
    assert_contains(&files_text, "nested/root-only.rs");
    assert!(
        !files_text.contains("generated.gen.rs"),
        "expected ignored glob match to stay out of README example output:\n{files_text}"
    );
    assert!(
        !files_text.contains("\n- root-only.rs\n"),
        "expected root-only ignored file to stay out of README example output:\n{files_text}"
    );
}

#[test]
fn no_match_and_error_examples_from_readme_stay_valid() {
    let no_match = run_patch(["files", "*.definitely-nope", "--scope", "src", "--json"]);
    assert_success(&no_match);
    let no_match_json = parse_json_stdout(&no_match);
    assert_eq!(no_match_json["schema_version"], 2);
    assert_eq!(no_match_json["ok"], true);
    assert!(no_match_json["data"]["meta"].is_object());
    assert_eq!(no_match_json["data"]["meta"]["files"], 0);
    assert_eq!(no_match_json["diagnostics"][0]["level"], "hint");
    assert_eq!(no_match_json["next"][0]["kind"], "suggestion");
    assert_eq!(no_match_json["next"][0]["confidence"], "high");
    assert_contains(
        no_match_json["next"][0]["command"]
            .as_str()
            .expect("next command should be a string"),
        "patch files \"*.rs\" --scope",
    );

    let error = run_patch(["search", "regex", "(", "--scope", "src"]);
    assert_failure(&error);
    let error_stderr = stderr(&error);
    assert_contains(&error_stderr, "# search.regex");
    assert_contains(&error_stderr, "## Meta");
    assert_contains(&error_stderr, "## Evidence\n(none)");
    assert_contains(&error_stderr, "## Next\n(none)");
    assert_contains(&error_stderr, "## Diagnostics");
    assert_contains(&error_stderr, "[code: invalid_query]");
}

#[test]
fn installer_dry_run_example_from_readme_stays_valid() {
    let temp = TempDir::new("install-dry-run");
    let home = temp.path().join("home");
    std::fs::create_dir_all(&home).expect("home should exist");

    let output = run_install(&home, "/usr/bin", true);
    let text = stdout(&output);

    assert_success(&output);
    assert_contains(&text, &format!("{}/.local/bin/patch", home.display()));
    assert_contains(&text, "Add this directory to your PATH");
}

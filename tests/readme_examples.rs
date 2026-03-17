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
    assert_contains(
        &readme,
        "cargo run -- files \"*.definitely-nope\" --scope src --json",
    );
    assert_contains(&readme, "cargo run -- search regex \"(\" --scope src");
    assert_contains(&contract, "heading_aligned");
    assert_contains(&contract, "first selected line itself");
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

use std::env;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::{Command as ProcessCommand, Output};
use std::time::{SystemTime, UNIX_EPOCH};

use assert_cmd::Command;

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

fn assert_contains(haystack: &str, needle: &str) {
    assert!(
        haystack.contains(needle),
        "expected to find {needle:?} in:\n{haystack}"
    );
}

#[test]
fn quick_start_commands_from_readme_stay_valid() {
    let symbol_find = run_patch(["symbol", "find", "main", "--scope", "src"]);
    let symbol_text = stdout(&symbol_find);
    assert_success(&symbol_find);
    assert_contains(&symbol_text, "# symbol.find");
    assert_contains(&symbol_text, "main.rs:");
    assert_contains(&symbol_text, "[definition]");

    let files = run_patch(["files", "*.rs", "--scope", "src"]);
    let files_text = stdout(&files);
    assert_success(&files);
    assert_contains(&files_text, "# files");
    assert_contains(&files_text, "files \"*.rs\"");

    let deps = run_patch(["deps", "src/main.rs"]);
    let deps_text = stdout(&deps);
    assert_success(&deps);
    assert_contains(&deps_text, "# deps");
    assert_contains(&deps_text, "deps \"src/main.rs\"");

    let map = run_patch(["map", "--scope", "src"]);
    assert_success(&map);
    assert_contains(&stdout(&map), "# Map:");
}

#[test]
fn read_command_examples_from_readme_stay_valid() {
    let lines = run_patch(["read", "README.md", "--lines", "1:20"]);
    assert_success(&lines);
    assert_contains(&stdout(&lines), "# patch");

    let heading = run_patch(["read", "README.md", "--heading", "## Command families"]);
    assert_success(&heading);
    assert_contains(&stdout(&heading), "## Command families");
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

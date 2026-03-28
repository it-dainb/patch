use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn unique_temp_dir(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after unix epoch")
        .as_nanos();
    env::temp_dir().join(format!(
        "drail-version-sync-test-{label}-{}-{nanos}",
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

fn write_fixture(root: &Path, cargo_version: &str, npm_version: &str) {
    fs::create_dir_all(root.join("npm")).expect("npm dir should be created");
    fs::create_dir_all(root.join("scripts")).expect("scripts dir should be created");

    fs::write(
        root.join("Cargo.toml"),
        format!(
            "[package]\nname = \"drail\"\nversion = \"{}\"\nedition = \"2021\"\n",
            cargo_version
        ),
    )
    .expect("Cargo.toml should be written");

    fs::write(
        root.join("npm/package.json"),
        format!(
            concat!(
                "{{\n",
                "  \"name\": \"drail\",\n",
                "  \"version\": \"{}\"\n",
                "}}\n"
            ),
            npm_version
        ),
    )
    .expect("package.json should be written");

    fs::copy(
        repo_root().join("scripts/sync-npm-version.js"),
        root.join("scripts/sync-npm-version.js"),
    )
    .expect("sync script should be copied");
}

fn run_sync(root: &Path, args: &[&str]) -> std::process::Output {
    let mut command = Command::new("node");
    command.arg("scripts/sync-npm-version.js");
    command.args(args);
    command.current_dir(root);
    command.output().expect("sync script should execute")
}

fn stdout(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn assert_success(output: &std::process::Output) {
    assert!(
        output.status.success(),
        "expected success, got status {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        stdout(output),
        stderr(output)
    );
}

#[test]
fn sync_script_updates_npm_package_version_from_cargo() {
    let temp = TempDir::new("updates-package-json");
    write_fixture(temp.path(), "1.2.3", "0.0.1");

    let output = run_sync(temp.path(), &[]);
    assert_success(&output);

    let package_json = fs::read_to_string(temp.path().join("npm/package.json"))
        .expect("package.json should remain readable");

    assert!(stdout(&output).contains("updated to 1.2.3"));
    assert!(package_json.contains("\"version\": \"1.2.3\""));
}

#[test]
fn sync_script_prints_cargo_version_without_mutating_files() {
    let temp = TempDir::new("prints-version");
    write_fixture(temp.path(), "2.0.0", "0.5.0");

    let before = fs::read_to_string(temp.path().join("npm/package.json"))
        .expect("package.json should be readable before print");
    let output = run_sync(temp.path(), &["--print"]);
    let after = fs::read_to_string(temp.path().join("npm/package.json"))
        .expect("package.json should be readable after print");

    assert_success(&output);
    assert_eq!(stdout(&output), "2.0.0\n");
    assert_eq!(after, before);
}

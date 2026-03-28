use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn script_path() -> PathBuf {
    repo_root().join("install.sh")
}

fn unique_temp_dir(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after unix epoch")
        .as_nanos();
    env::temp_dir().join(format!(
        "drail-installer-test-{label}-{}-{nanos}",
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

fn run_install(home: &Path, path_env: &str, dry_run: bool, extra_env: &[(&str, &str)]) -> Output {
    let mut command = Command::new("bash");
    command.arg(script_path());
    command.current_dir(repo_root());
    command.env("HOME", home);
    command.env("PATH", path_env);
    if dry_run {
        command.env("DRAIL_INSTALL_DRY_RUN", "1");
    }

    for (key, value) in extra_env {
        command.env(key, value);
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

fn assert_not_contains(text: &str, needle: &str) {
    assert!(
        !text.contains(needle),
        "did not expect to find {needle:?} in:\n{text}"
    );
}

#[test]
fn dry_run_defaults_to_home_local_bin_and_prints_path_guidance_when_missing() {
    let temp = TempDir::new("default-target");
    let home = temp.path().join("home");
    fs::create_dir_all(&home).expect("home should exist");

    let output = run_install(&home, "/usr/bin", true, &[]);
    let text = stdout(&output);

    assert_success(&output);
    assert!(text.contains(&format!("{}/.local/bin/drail", home.display())));
    assert!(
        text.contains("Add this directory to your PATH"),
        "stdout:\n{text}"
    );
    assert_not_contains(&text, "MCP");
    assert_not_contains(&text, "edit mode");
    assert_not_contains(&text, ".claude.json");
    assert!(
        !home.join(".config").exists(),
        "dry-run should not create .config"
    );
    assert!(
        !home.join(".claude.json").exists(),
        "dry-run should not create .claude.json"
    );
}

#[test]
fn dry_run_honors_install_dir_override_and_omits_path_guidance_when_already_present() {
    let temp = TempDir::new("override-target");
    let home = temp.path().join("home");
    let install_dir = temp.path().join("bin");
    fs::create_dir_all(&home).expect("home should exist");

    let path_env = format!("{}:/usr/bin", install_dir.display());
    let install_dir_value = install_dir.to_string_lossy().into_owned();
    let output = run_install(
        &home,
        &path_env,
        true,
        &[("DRAIL_INSTALL_DIR", install_dir_value.as_str())],
    );
    let text = stdout(&output);

    assert_success(&output);
    assert!(text.contains(&format!("{}/drail", install_dir.display())));
    assert_not_contains(&text, "Add this directory to your PATH");
    assert_not_contains(&text, "MCP");
}

#[test]
fn dry_run_is_side_effect_free_even_when_target_exists() {
    let temp = TempDir::new("existing-target");
    let home = temp.path().join("home");
    let install_dir = home.join(".local/bin");
    let target = install_dir.join("drail");
    fs::create_dir_all(&install_dir).expect("install dir should exist");
    fs::write(&target, "old").expect("existing target should be created");

    let output = run_install(&home, "/usr/bin", true, &[]);
    let text = stdout(&output);

    assert_success(&output);
    assert!(text.contains(&format!("{}/.local/bin/drail", home.display())));
    assert_eq!(
        fs::read_to_string(&target).expect("target should remain readable"),
        "old"
    );
}

#[test]
fn rerunning_replaces_existing_target_idempotently() {
    let temp = TempDir::new("replace-target");
    let home = temp.path().join("home");
    let install_dir = home.join(".local/bin");
    let source_one = temp.path().join("drail-one");
    let source_two = temp.path().join("drail-two");
    let target = install_dir.join("drail");

    fs::create_dir_all(&home).expect("home should exist");
    fs::write(&source_one, "first-binary").expect("first source should exist");
    fs::write(&source_two, "second-binary").expect("second source should exist");

    let source_one_value = source_one.to_string_lossy().into_owned();
    let first = run_install(
        &home,
        "/usr/bin",
        false,
        &[("DRAIL_INSTALL_SOURCE", source_one_value.as_str())],
    );
    assert_success(&first);
    assert_eq!(
        fs::read_to_string(&target).expect("first install target should exist"),
        "first-binary"
    );

    let source_two_value = source_two.to_string_lossy().into_owned();
    let second = run_install(
        &home,
        "/usr/bin",
        false,
        &[("DRAIL_INSTALL_SOURCE", source_two_value.as_str())],
    );
    assert_success(&second);
    assert_eq!(
        fs::read_to_string(&target).expect("second install target should exist"),
        "second-binary"
    );
}

use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

fn rustx_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_rustx"))
}

fn temp_dir(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = env::temp_dir().join(format!(
        "rustx-test-{}-{}-{}",
        name,
        std::process::id(),
        nanos
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn write_script(dir: &Path, name: &str, contents: &str) -> PathBuf {
    let path = dir.join(name);
    fs::write(&path, contents).unwrap();

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(&path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&path, permissions).unwrap();
    }

    path
}

fn run_rustx(cache_dir: &Path, script: &Path, args: &[&str]) -> Output {
    let mut command = rustx_command(cache_dir);
    command.arg(script).args(args).output().unwrap()
}

fn rustx_command(cache_dir: &Path) -> Command {
    let mut command = Command::new(rustx_bin());
    command
        .env("RUSTX_CACHE_DIR", cache_dir)
        .env_remove("CARGO");
    command
}

fn with_fake_cargo_path(command: &mut Command, fake_bin: &Path) {
    command.env("PATH", prepend_to_path(fake_bin));
}

fn prepend_to_path(dir: &Path) -> OsString {
    let path = env::var_os("PATH").unwrap_or_default();
    let mut paths = vec![dir.to_path_buf()];
    paths.extend(env::split_paths(&path));
    env::join_paths(paths).unwrap()
}

fn cache_entries(cache_dir: &Path) -> Vec<PathBuf> {
    let mut entries = fs::read_dir(cache_dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.is_dir())
        .collect::<Vec<_>>();
    entries.sort();
    entries
}

#[test]
fn no_args_prints_usage() {
    let dir = temp_dir("usage");
    let cache = dir.join("cache");

    let output = rustx_command(&cache).output().unwrap();

    assert!(!output.status.success());
    let stderr = stderr(&output);
    assert!(stderr.contains("missing script path"), "stderr: {}", stderr);
    assert!(stderr.contains("Usage:"), "stderr: {}", stderr);
}

#[test]
fn runs_simple_script() {
    let dir = temp_dir("simple");
    let cache = dir.join("cache");
    let script = write_script(
        &dir,
        "hello.rs",
        r#"#!/usr/bin/env rustx

fn main() {
    println!("hello");
}
"#,
    );

    let output = run_rustx(&cache, &script, &[]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert_eq!(stdout(&output), "hello\n");
}

#[cfg(unix)]
#[test]
fn executes_shebang_script_directly() {
    let dir = temp_dir("shebang-direct");
    let cache = dir.join("cache");
    let script = write_script(
        &dir,
        "direct.rs",
        r#"#!/usr/bin/env rustx

fn main() {
    println!("direct");
}
"#,
    );

    let rustx_dir = rustx_bin().parent().unwrap().to_path_buf();
    let output = Command::new(&script)
        .env("RUSTX_CACHE_DIR", &cache)
        .env_remove("CARGO")
        .env("PATH", prepend_to_path(&rustx_dir))
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert_eq!(stdout(&output), "direct\n");
}

#[test]
fn forwards_args_and_sets_argv0_on_unix() {
    let dir = temp_dir("args");
    let cache = dir.join("cache");
    let script = write_script(
        &dir,
        "args.rs",
        r#"fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    println!("{}", args.join("|"));
}
"#,
    );

    let output = run_rustx(&cache, &script, &["one", "--two"]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let text = stdout(&output);
    assert!(text.ends_with("|one|--two\n"), "stdout: {}", text);

    #[cfg(unix)]
    assert!(
        text.starts_with(&script.to_string_lossy().to_string()),
        "stdout: {}",
        text
    );
}

#[test]
fn supports_embedded_manifest() {
    let dir = temp_dir("manifest");
    let cache = dir.join("cache");
    let script = write_script(
        &dir,
        "manifest.rs",
        r#"#!/usr/bin/env rustx
---cargo
[package]
edition = "2021"
---

fn main() {
    println!("manifest");
}
"#,
    );

    let output = run_rustx(&cache, &script, &[]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert_eq!(stdout(&output), "manifest\n");
}

#[test]
fn propagates_script_exit_code() {
    let dir = temp_dir("exit");
    let cache = dir.join("cache");
    let script = write_script(
        &dir,
        "exit.rs",
        r#"fn main() {
    std::process::exit(37);
}
"#,
    );

    let output = run_rustx(&cache, &script, &[]);

    assert_eq!(output.status.code(), Some(37));
}

#[test]
fn cache_hit_does_not_invoke_cargo() {
    let dir = temp_dir("cache-hit");
    let cache = dir.join("cache");
    let fake_bin = dir.join("fake-bin");
    fs::create_dir_all(&fake_bin).unwrap();
    let script = write_script(
        &dir,
        "cache.rs",
        r#"fn main() {
    println!("cached");
}
"#,
    );

    let first = run_rustx(&cache, &script, &[]);
    assert!(first.status.success(), "stderr: {}", stderr(&first));
    assert_eq!(cache_entries(&cache).len(), 1);

    write_fake_cargo(&fake_bin);

    let mut command = rustx_command(&cache);
    command.arg(&script);
    with_fake_cargo_path(&mut command, &fake_bin);
    let second = command.output().unwrap();

    assert!(second.status.success(), "stderr: {}", stderr(&second));
    assert_eq!(stdout(&second), "cached\n");
    assert_eq!(cache_entries(&cache).len(), 1);
}

#[test]
fn updated_script_rebuilds_and_runs_new_binary() {
    let dir = temp_dir("update-success");
    let cache = dir.join("cache");
    let script = write_script(
        &dir,
        "update.rs",
        r#"fn main() {
    println!("version-one");
}
"#,
    );

    let first = run_rustx(&cache, &script, &[]);
    assert!(first.status.success(), "stderr: {}", stderr(&first));
    assert_eq!(stdout(&first), "version-one\n");

    fs::write(
        &script,
        r#"fn main() {
    println!("version-two");
}
"#,
    )
    .unwrap();

    let second = run_rustx(&cache, &script, &[]);
    assert!(second.status.success(), "stderr: {}", stderr(&second));
    assert_eq!(stdout(&second), "version-two\n");
    assert_eq!(cache_entries(&cache).len(), 2);
}

#[test]
fn updated_script_attempts_rebuild_instead_of_using_stale_cache() {
    let dir = temp_dir("update-fake-cargo");
    let cache = dir.join("cache");
    let fake_bin = dir.join("fake-bin");
    fs::create_dir_all(&fake_bin).unwrap();
    let script = write_script(
        &dir,
        "stale.rs",
        r#"fn main() {
    println!("old");
}
"#,
    );

    let first = run_rustx(&cache, &script, &[]);
    assert!(first.status.success(), "stderr: {}", stderr(&first));

    fs::write(
        &script,
        r#"fn main() {
    println!("new");
}
"#,
    )
    .unwrap();
    write_fake_cargo(&fake_bin);

    let mut command = rustx_command(&cache);
    command.arg(&script);
    with_fake_cargo_path(&mut command, &fake_bin);
    let second = command.output().unwrap();

    assert_eq!(second.status.code(), Some(77));
    assert!(
        stderr(&second).contains("fake cargo invoked"),
        "stderr: {}",
        stderr(&second)
    );
}

#[test]
fn force_rebuild_invokes_cargo_even_when_cache_is_fresh() {
    let dir = temp_dir("force");
    let cache = dir.join("cache");
    let fake_bin = dir.join("fake-bin");
    fs::create_dir_all(&fake_bin).unwrap();
    let script = write_script(
        &dir,
        "force.rs",
        r#"fn main() {
    println!("fresh");
}
"#,
    );

    let first = run_rustx(&cache, &script, &[]);
    assert!(first.status.success(), "stderr: {}", stderr(&first));
    write_fake_cargo(&fake_bin);

    let mut command = rustx_command(&cache);
    command.arg("--force").arg(&script);
    with_fake_cargo_path(&mut command, &fake_bin);
    let second = command.output().unwrap();

    assert_eq!(second.status.code(), Some(77));
    assert!(
        stderr(&second).contains("fake cargo invoked"),
        "stderr: {}",
        stderr(&second)
    );
}

#[test]
fn print_generated_writes_package_without_building() {
    let dir = temp_dir("print-generated");
    let cache = dir.join("cache");
    let fake_bin = dir.join("fake-bin");
    fs::create_dir_all(&fake_bin).unwrap();
    write_fake_cargo(&fake_bin);
    let script = write_script(
        &dir,
        "generated.rs",
        r#"#!/usr/bin/env rustx
---cargo
[package]
edition = "2021"
---

fn main() {
    println!("generated");
}
"#,
    );

    let mut command = rustx_command(&cache);
    command.arg("--print-generated").arg(&script);
    with_fake_cargo_path(&mut command, &fake_bin);
    let output = command.output().unwrap();

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let generated = PathBuf::from(stdout(&output).trim());
    assert!(generated.join("Cargo.toml").is_file());
    assert!(generated.join("src").join("main.rs").is_file());
    assert!(!generated.join("target").exists());
}

#[test]
fn clear_cache_removes_only_rustx_cache_root() {
    let dir = temp_dir("clear-cache");
    let cache = dir.join("cache");
    let script = write_script(
        &dir,
        "clear.rs",
        r#"fn main() {
    println!("clear");
}
"#,
    );

    let first = run_rustx(&cache, &script, &[]);
    assert!(first.status.success(), "stderr: {}", stderr(&first));
    assert!(cache.exists());

    let output = rustx_command(&cache).arg("--clear-cache").output().unwrap();

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert!(!cache.exists());
    assert!(dir.exists());
}

#[test]
fn print_cache_dir_uses_env_override() {
    let dir = temp_dir("print-cache");
    let cache = dir.join("cache");

    let output = rustx_command(&cache)
        .arg("--print-cache-dir")
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert_eq!(stdout(&output), format!("{}\n", cache.display()));
}

#[test]
fn invalid_embedded_manifest_marker_fails_before_cargo() {
    let dir = temp_dir("invalid-marker");
    let cache = dir.join("cache");
    let fake_bin = dir.join("fake-bin");
    fs::create_dir_all(&fake_bin).unwrap();
    write_fake_cargo(&fake_bin);
    let script = write_script(
        &dir,
        "invalid.rs",
        r#"---cargo
[dependencies]
fn main() {}
"#,
    );

    let mut command = rustx_command(&cache);
    command.arg(&script);
    with_fake_cargo_path(&mut command, &fake_bin);
    let output = command.output().unwrap();

    assert!(!output.status.success());
    let stderr = stderr(&output);
    assert!(stderr.contains("missing closing"), "stderr: {}", stderr);
    assert!(!stderr.contains("fake cargo invoked"), "stderr: {}", stderr);
}

#[test]
fn rust_compile_errors_return_cargo_failure() {
    let dir = temp_dir("compile-error");
    let cache = dir.join("cache");
    let script = write_script(&dir, "broken.rs", "fn main() { let = ; }\n");

    let output = run_rustx(&cache, &script, &[]);

    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("error"),
        "stderr: {}",
        stderr(&output)
    );
}

#[test]
fn release_build_uses_separate_cache_entry() {
    let dir = temp_dir("release");
    let cache = dir.join("cache");
    let script = write_script(
        &dir,
        "profile.rs",
        r#"fn main() {
    println!("profile");
}
"#,
    );

    let debug = run_rustx(&cache, &script, &[]);
    assert!(debug.status.success(), "stderr: {}", stderr(&debug));

    let release = rustx_command(&cache)
        .arg("--release")
        .arg(&script)
        .output()
        .unwrap();
    assert!(release.status.success(), "stderr: {}", stderr(&release));
    assert_eq!(stdout(&release), "profile\n");
    assert_eq!(cache_entries(&cache).len(), 2);
}

#[cfg(unix)]
fn write_fake_cargo(dir: &Path) {
    use std::os::unix::fs::PermissionsExt;

    let path = dir.join("cargo");
    fs::write(&path, "#!/bin/sh\necho fake cargo invoked >&2\nexit 77\n").unwrap();
    let mut permissions = fs::metadata(&path).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).unwrap();
}

#[cfg(not(unix))]
fn write_fake_cargo(dir: &Path) {
    fs::write(
        dir.join("cargo.bat"),
        "@echo off\r\necho fake cargo invoked 1>&2\r\nexit /b 77\r\n",
    )
    .unwrap();
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

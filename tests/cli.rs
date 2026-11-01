//! End-to-end checks of the binary's non-interactive paths: every case here
//! exits before the terminal UI starts, so none of them can hang on a TTY.

use std::path::PathBuf;
use std::process::{Command, Output, Stdio};

fn udiff(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_udiff"))
        .args(args)
        .stdin(Stdio::null())
        .output()
        .expect("run the udiff binary")
}

/// Writes `contents` to a unique file under the system temp dir.
fn fixture(name: &str, contents: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("udiff-cli-{}-{name}", std::process::id()));
    std::fs::write(&path, contents).expect("write the fixture");
    path
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn help_exits_0() {
    let output = udiff(&["--help"]);
    assert_eq!(output.status.code(), Some(0));
    assert!(stdout(&output).contains("exit codes"));
}

#[test]
fn version_exits_0() {
    let output = udiff(&["--version"]);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(stdout(&output), "udiff 0.1.0\n");
}

#[test]
fn bad_arity_exits_2() {
    let output = udiff(&["a", "b"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).starts_with("usage:"));
}

#[test]
fn missing_file_exits_1() {
    let output = udiff(&["/nonexistent/x"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).contains("cannot read"));
}

#[test]
fn no_conflicts_exits_1() {
    let path = fixture("plain", "plain\n");
    let output = udiff(&[path.to_str().unwrap()]);
    std::fs::remove_file(&path).unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).contains("no conflicts"));
}

#[test]
fn malformed_exits_1_with_line() {
    let path = fixture("malformed", "<<<<<<< a\nx\n");
    let output = udiff(&[path.to_str().unwrap()]);
    std::fs::remove_file(&path).unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).contains("line 1:"));
}

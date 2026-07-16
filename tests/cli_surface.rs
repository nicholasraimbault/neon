//! Regression tests for the supported release CLI surface.

use std::process::{Command, Output};

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_neon"))
        .args(args)
        .output()
        .expect("spawn neon binary")
}

fn run_help(args: &[&str]) -> String {
    let output = run(args);
    assert!(output.status.success(), "help command failed: {output:?}");
    String::from_utf8(output.stdout).expect("help output is UTF-8")
}

#[test]
fn root_help_excludes_experimental_stream_command() {
    let help = run_help(&["--help"]);
    assert!(
        !help
            .lines()
            .any(|line| line.trim_start().starts_with("stream ")),
        "release CLI unexpectedly exposes `stream`: {help}"
    );
}

#[test]
fn doctor_help_excludes_experimental_bridge_option() {
    let help = run_help(&["doctor", "--help"]);
    assert!(
        !help.contains("--bridge"),
        "release CLI unexpectedly exposes `doctor --bridge`: {help}"
    );
}

#[test]
fn parser_rejects_experimental_stream_command() {
    let output = run(&["stream"]);
    assert_eq!(
        output.status.code(),
        Some(2),
        "unexpected result: {output:?}"
    );
}

#[test]
fn parser_rejects_experimental_doctor_bridge_option() {
    let output = run(&["doctor", "--bridge"]);
    assert_eq!(
        output.status.code(),
        Some(2),
        "unexpected result: {output:?}"
    );
}

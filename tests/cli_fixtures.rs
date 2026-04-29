use regex::Regex;
use std::io::Write;
use std::process::{Command, Stdio};

fn strip_ansi(input: &str) -> String {
    let ansi_pattern = Regex::new(r"\x1b\[[0-9;]*m").expect("valid ansi regex");
    ansi_pattern.replace_all(input, "").into_owned()
}

fn run_pretty_log(input: &str) -> String {
    let mut child = Command::new(env!("CARGO_BIN_EXE_pretty-log"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn pretty-log binary");

    child
        .stdin
        .as_mut()
        .expect("stdin handle")
        .write_all(input.as_bytes())
        .expect("write fixture input");

    let output = child.wait_with_output().expect("wait for pretty-log");
    assert!(output.status.success(), "binary should exit successfully");

    strip_ansi(&String::from_utf8(output.stdout).expect("utf8 stdout"))
}

#[test]
fn formats_sample_log_fixture() {
    let output = run_pretty_log(include_str!("../test/logs.txt"));
    let lines: Vec<&str> = output.lines().collect();

    assert_eq!(lines.len(), include_str!("../test/logs.txt").lines().count());
    assert!(lines[0].contains("TRACE request completed"));
    assert!(lines[0].contains("req={id:8127, method:\"POST\", url:\"/graphql\"}"));
    assert!(lines[4].contains("ERROR request completed"));
}

#[test]
fn formats_pino_fixture() {
    let output = run_pretty_log(include_str!("../test/logs_pino.txt"));
    let lines: Vec<&str> = output.lines().collect();

    assert!(lines.len() >= include_str!("../test/logs_pino.txt").lines().count());
    assert!(lines[0].contains("INFO hello world"));
    assert!(lines[0].contains("pid=2505893"));
    assert!(output.contains("stack=\"Error: an error"));
    assert!(lines.iter().any(|line| line.contains("ERROR an error")));
    assert!(lines.iter().any(|line| line.contains("UNKNOWN (70) this is at unknown level")));
}
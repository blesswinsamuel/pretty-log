use regex::Regex;
use std::io::Write;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

fn strip_ansi(input: &str) -> String {
    let ansi_pattern = Regex::new(r"\x1b\[[0-9;]*m").expect("valid ansi regex");
    ansi_pattern.replace_all(input, "").into_owned()
}

fn wait_with_timeout(child: &mut Child, timeout: Duration) -> ExitStatus {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child.try_wait().expect("poll pretty-log") {
            return status;
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            panic!("pretty-log did not exit within {timeout:?}");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn exits_cleanly_when_stdout_closes_early() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_pretty-log"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn pretty-log binary");

    // Simulate a consumer such as `head` going away: close the read end of stdout.
    drop(child.stdout.take());

    let mut stdin = child.stdin.take().expect("stdin handle");
    let line = "{\"time\":1624829360868,\"level\":30,\"message\":\"line\"}\n";
    // Ignore the write error: the child may already have exited.
    let _ = stdin.write_all(line.repeat(10_000).as_bytes());
    drop(stdin);

    let status = wait_with_timeout(&mut child, Duration::from_secs(10));
    assert!(status.success(), "pretty-log should exit cleanly after a broken pipe, got {status:?}");
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
    assert!(lines[0].contains("service=\"api-gateway\""));
    assert!(lines[0].contains("req={id:\"req-8127\", method:\"POST\", url:\"/graphql\"}"));
    assert!(lines[4].contains("ERROR request failed"));
}

#[test]
fn formats_pino_fixture() {
    let output = run_pretty_log(include_str!("../test/logs_pino.txt"));
    let lines: Vec<&str> = output.lines().collect();

    assert!(lines.len() >= include_str!("../test/logs_pino.txt").lines().count());
    assert!(lines[0].contains("INFO worker started"));
    assert!(lines[0].contains("service=\"billing-worker\""));
    assert!(output.contains("stack:\n  Error: failed to charge invoice"));
    assert!(lines.iter().any(|line| line.contains("ERROR charge failed")));
    assert!(lines.iter().any(|line| line.contains("UNKNOWN (70) this is at unknown level")));
}

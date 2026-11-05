use regex::Regex;
use std::fs::File;
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

#[cfg(unix)]
#[test]
fn terminates_on_sigint() {
    use std::os::unix::process::ExitStatusExt;

    let mut child = Command::new(env!("CARGO_BIN_EXE_pretty-log"))
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn pretty-log binary");

    // Keep stdin open so the process stays running and waiting for input.
    let _stdin = child.stdin.take().expect("stdin handle");

    std::thread::sleep(Duration::from_millis(500));
    let kill = Command::new("kill")
        .arg("-INT")
        .arg(child.id().to_string())
        .status()
        .expect("send SIGINT to pretty-log");
    assert!(kill.success(), "failed to send SIGINT");

    let status = wait_with_timeout(&mut child, Duration::from_secs(10));
    assert_eq!(status.signal(), Some(2), "pretty-log should terminate on SIGINT, got {status:?}");
}

#[cfg(unix)]
#[test]
fn exits_nonzero_when_stdin_cannot_be_read() {
    // A directory can be opened but not read as a byte stream.
    let dir = File::open(std::env::temp_dir()).expect("open temp dir");

    let output = Command::new(env!("CARGO_BIN_EXE_pretty-log"))
        .stdin(Stdio::from(dir))
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .expect("run pretty-log");

    assert_eq!(output.status.code(), Some(1), "a read failure should fail the run");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("<pretty-log> Error: "), "{stderr:?}");
    assert!(!stderr.contains("Os {"), "diagnostics should use Display formatting: {stderr:?}");
}

#[test]
fn applies_utc_and_time_format_flags() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_pretty-log"))
        .args(["--utc", "--time-format", "%Y-%m-%d %H:%M:%S%.3f", "--color", "never"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn pretty-log binary");

    child
        .stdin
        .as_mut()
        .expect("stdin handle")
        .write_all(b"{\"time\":1624829360868,\"level\":30,\"message\":\"x\"}\n")
        .expect("write input");

    let output = child.wait_with_output().expect("wait for pretty-log");
    assert!(output.status.success(), "binary should exit successfully");
    assert_eq!(
        String::from_utf8(output.stdout).expect("utf8 stdout").trim_end(),
        "2021-06-27 21:29:20.868  INFO x"
    );
}

#[test]
fn rejects_invalid_time_format() {
    let output = Command::new(env!("CARGO_BIN_EXE_pretty-log"))
        .args(["--time-format", "%Q"])
        .stdin(Stdio::null())
        .output()
        .expect("run pretty-log");

    assert_eq!(output.status.code(), Some(2), "usage errors should exit 2");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("invalid --time-format"), "{stderr:?}");
}

fn run_pretty_log_with_args(args: &[&str], input: &[u8]) -> String {
    let mut child = Command::new(env!("CARGO_BIN_EXE_pretty-log"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn pretty-log binary");

    child.stdin.as_mut().expect("stdin handle").write_all(input).expect("write fixture input");

    let output = child.wait_with_output().expect("wait for pretty-log");
    assert!(output.status.success(), "binary should exit successfully");

    strip_ansi(&String::from_utf8(output.stdout).expect("utf8 stdout"))
}

#[test]
fn applies_include_exclude_and_order_flags() {
    let input = b"{\"level\":30,\"message\":\"x\",\"service\":\"api\",\"req\":{\"id\":\"r1\",\"method\":\"POST\"},\"pid\":1}\n";

    let included = run_pretty_log_with_args(&["--include-fields", "req.id,service"], input);
    assert_eq!(included, "INFO x service=\"api\" req={id:\"r1\"}\n");

    let excluded = run_pretty_log_with_args(&["--exclude-fields", "req.method,pid"], input);
    assert_eq!(excluded, "INFO x service=\"api\" req={id:\"r1\"}\n");

    let ordered = run_pretty_log_with_args(&["--field-order", "req.method,req"], input);
    assert_eq!(ordered, "INFO x req={method:\"POST\", id:\"r1\"} service=\"api\" pid=1\n");
}

fn run_pretty_log_bytes(input: &[u8]) -> Vec<u8> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_pretty-log"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn pretty-log binary");

    child.stdin.as_mut().expect("stdin handle").write_all(input).expect("write fixture input");

    let output = child.wait_with_output().expect("wait for pretty-log");
    assert!(output.status.success(), "binary should exit successfully");
    assert!(output.stderr.is_empty(), "unexpected stderr: {:?}", String::from_utf8_lossy(&output.stderr));

    output.stdout
}

fn run_pretty_log(input: &str) -> String {
    strip_ansi(&String::from_utf8(run_pretty_log_bytes(input.as_bytes())).expect("utf8 stdout"))
}

#[test]
fn passes_through_lines_that_are_not_valid_utf8() {
    let mut input = Vec::new();
    input.extend_from_slice(b"{\"time\":1624829360868,\"level\":30,\"message\":\"before\"}\n");
    input.extend_from_slice(b"plain \xff\xfe text\n");
    input.extend_from_slice(b"{\"time\":1624829360868,\"level\":30,\"message\":\"after\"}\n");

    let output = String::from_utf8(run_pretty_log_bytes(&input)).expect("utf8 stdout");
    let lines: Vec<&str> = output.lines().collect();

    assert_eq!(lines.len(), 3);
    assert!(lines[0].contains("before"));
    assert_eq!(lines[1], "plain \u{FFFD}\u{FFFD} text");
    assert!(lines[2].contains("after"));
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

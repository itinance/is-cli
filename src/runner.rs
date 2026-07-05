use crate::stream::{parse_line, Event};
use std::io::{BufRead, BufReader, Read};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

pub struct RunOutcome {
    pub final_text: Option<String>,
    pub is_error: bool,
    pub timed_out: bool,
    pub stderr: String,
    pub status_ok: bool,
}

pub enum SpawnError {
    NotFound,
    Io(std::io::Error),
}

pub fn run_claude(
    program: &str,
    args: &[String],
    timeout: Duration,
    trace: bool,
) -> Result<RunOutcome, SpawnError> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                SpawnError::NotFound
            } else {
                SpawnError::Io(e)
            }
        })?;

    let stdout = child.stdout.take().expect("stdout is piped");
    let stderr_pipe = child.stderr.take().expect("stderr is piped");

    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let Ok(line) = line else { break };
            if tx.send(line).is_err() {
                break;
            }
        }
    });
    let stderr_thread = std::thread::spawn(move || {
        let mut buf = String::new();
        let _ = BufReader::new(stderr_pipe).read_to_string(&mut buf);
        buf
    });

    let deadline = Instant::now() + timeout;
    let mut final_text = None;
    let mut is_error = false;
    let mut timed_out = false;

    loop {
        let now = Instant::now();
        if now >= deadline {
            timed_out = true;
            break;
        }
        match rx.recv_timeout(deadline - now) {
            Ok(line) => {
                for event in parse_line(&line) {
                    match event {
                        Event::ToolUse { name, detail } => {
                            if trace {
                                if detail.is_empty() {
                                    eprintln!("  ⤷ {name}");
                                } else {
                                    eprintln!("  ⤷ {detail}");
                                }
                            }
                        }
                        Event::Result { text, is_error: err } => {
                            final_text = Some(text);
                            is_error = err;
                        }
                    }
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                timed_out = true;
                break;
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }

    let status_ok = if timed_out {
        let _ = child.kill();
        let _ = child.wait();
        false
    } else {
        child.wait().map(|s| s.success()).unwrap_or(false)
    };
    let stderr = stderr_thread.join().unwrap_or_default();

    Ok(RunOutcome { final_text, is_error, timed_out, stderr, status_ok })
}

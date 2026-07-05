use crate::stream::{parse_line, Event};
use std::io::{BufRead, BufReader, Read};
use std::process::{Child, Command, Stdio};
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

/// How long we allow a child to exit gracefully after we've already captured a
/// final `result` event. It's still doing work (e.g. cleanup) but we already
/// have our answer, so we don't want to burn the rest of the user's `--timeout`
/// waiting on it.
const RESULT_GRACE: Duration = Duration::from_millis(500);

/// Poll interval used while bounded-reaping the child (see `reap`).
const REAP_POLL_INTERVAL: Duration = Duration::from_millis(25);

/// Floor applied to the stderr collection wait so that, even when the stream
/// loop exits immediately (e.g. a result short-circuit with no time left on
/// the deadline), we still give the stderr thread a brief window to hand off
/// whatever it's already buffered rather than giving up with zero wait.
const STDERR_WAIT_FLOOR: Duration = Duration::from_millis(100);

/// Extra wait after a process-group kill to pick up stderr that the kill just
/// released (the descendant holding the pipe open should die promptly once
/// signalled).
const STDERR_POST_KILL_WAIT: Duration = Duration::from_millis(250);

pub fn run_claude(
    program: &str,
    args: &[String],
    timeout: Duration,
    trace: bool,
) -> Result<RunOutcome, SpawnError> {
    let mut command = Command::new(program);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    // Put the child in its own process group so that, on timeout/kill, we can
    // signal the whole group (child + any subprocesses it spawns) rather than
    // just the direct child. Without this, a descendant that inherited the
    // piped stderr fd can keep it open after we kill the direct child, and
    // the stderr-reader thread's `read_to_string` (handed off over a channel
    // below, not joined) would never see EOF.
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }

    let mut child = command.spawn().map_err(|e| {
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
    // Hand stderr off over a channel rather than joining the reader thread
    // directly: a descendant of the child that inherited the piped stderr fd
    // (see the `process_group` comment above) can keep that fd open long
    // after the direct child has exited, so `read_to_string` may not return
    // for a long time -- possibly never, if the descendant outlives the
    // whole `is` invocation. Sending over a channel lets the main thread wait
    // with a bound and move on, leaving the reader thread to finish (or not)
    // on its own, detached, rather than blocking `run_claude`'s return on it.
    let (stderr_tx, stderr_rx) = mpsc::channel();
    let _stderr_thread = std::thread::spawn(move || {
        let mut buf = String::new();
        let _ = BufReader::new(stderr_pipe).read_to_string(&mut buf);
        let _ = stderr_tx.send(buf);
    });

    let deadline = Instant::now() + timeout;
    let mut final_text = None;
    let mut is_error = false;
    let mut got_result = false;

    // Stream loop: exits when the deadline passes, the stdout channel
    // disconnects (child closed stdout, or the reader thread bailed on a
    // non-UTF8 line), or we've captured a `result` event.
    loop {
        let now = Instant::now();
        if now >= deadline {
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
                        Event::Result {
                            text,
                            is_error: err,
                        } => {
                            final_text = Some(text);
                            is_error = err;
                            got_result = true;
                        }
                    }
                }
                if got_result {
                    // We have our answer; stop reading further lines and move
                    // straight to a short, bounded reap of the child.
                    break;
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => break,
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }

    // Bounded reap: never fall into an unbounded blocking `wait()`. If we
    // already have a result, give the child a short grace period to exit on
    // its own; otherwise use whatever's left of the original deadline. If it
    // doesn't exit in time, kill the whole process group and reap.
    let reap_deadline = if got_result {
        let remaining = deadline.saturating_duration_since(Instant::now());
        Instant::now() + RESULT_GRACE.min(remaining)
    } else {
        deadline
    };
    let (wait_status, expired) = reap(&mut child, reap_deadline);

    let timed_out = expired && !got_result;
    let status_ok = if got_result && expired {
        // We already had a result but the child didn't exit within the grace
        // period, so we killed it. A forced kill's exit status is expected to
        // look like a failure, but that shouldn't turn a good answer into
        // one — the result event's `is_error` is already the real
        // success/failure signal here.
        true
    } else {
        wait_status.map(|s| s.success()).unwrap_or(false)
    };

    // Bounded stderr collection (see the channel-handoff comment above): wait
    // for the reader thread to hand off its buffer, bounded by whatever's
    // left of `reap_deadline` (with a small floor so a result short-circuit
    // with little/no time left still gets a chance to collect stderr that's
    // already sitting in the pipe). If that times out, a descendant is
    // likely still holding the fd open -- kill the whole process group (a
    // no-op if `reap` already did this) and give the reader thread one more
    // short window to flush what the kill just released. If it still hasn't
    // shown up, proceed with empty stderr; the reader thread is left to die
    // on its own (or with the process) rather than blocking us further.
    let stderr_budget = reap_deadline
        .saturating_duration_since(Instant::now())
        .max(STDERR_WAIT_FLOOR);
    let stderr = match stderr_rx.recv_timeout(stderr_budget) {
        Ok(buf) => buf,
        Err(_) => {
            kill_group(&mut child);
            stderr_rx
                .recv_timeout(STDERR_POST_KILL_WAIT)
                .unwrap_or_default()
        }
    };

    Ok(RunOutcome {
        final_text,
        is_error,
        timed_out,
        stderr,
        status_ok,
    })
}

/// Poll `child.try_wait()` until it exits or `deadline` passes. Returns the
/// exit status (if reaped) and whether the deadline expired. On expiry, kills
/// the child's process group (falling back to killing just the child) and
/// does a final blocking `wait()` to reap it (bounded, since it's already
/// been signalled to die).
fn reap(child: &mut Child, deadline: Instant) -> (Option<std::process::ExitStatus>, bool) {
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return (Some(status), false),
            Ok(None) => {}
            Err(_) => return (None, false),
        }
        if Instant::now() >= deadline {
            kill_group(child);
            let status = child.wait().ok();
            return (status, true);
        }
        std::thread::sleep(REAP_POLL_INTERVAL);
    }
}

/// Kill the child's whole process group (it was placed into its own group at
/// spawn time on unix) so descendants that inherited the piped stderr fd are
/// also signalled. Killing them closes their copy of the pipe's write end,
/// which lets the stderr-reader thread's `read_to_string` hit EOF and hand
/// its buffer off over the mpsc channel, so the bounded `recv_timeout` on the
/// receiving end can complete instead of blocking indefinitely. Falls back
/// to killing just the direct child if that fails, or on non-unix targets.
#[cfg(unix)]
fn kill_group(child: &mut Child) {
    let pid = child.id() as i32;
    // Safety: `libc::killpg` is a plain syscall wrapper; `pid` is a valid,
    // still-live process (group) id owned by `child`.
    let result = unsafe { libc::killpg(pid, libc::SIGKILL) };
    if result != 0 {
        let _ = child.kill();
    }
}

#[cfg(not(unix))]
fn kill_group(child: &mut Child) {
    let _ = child.kill();
}

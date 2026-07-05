use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

/// Writes a fake `claude` shim into `dir` that replays fixtures / simulates failures.
fn write_fake_claude(dir: &Path) {
    let script = "#!/usr/bin/env bash\n\
case \"${FAKE_CLAUDE_MODE:-ok}\" in\n\
  ok)   cat \"$FAKE_CLAUDE_FIXTURE\" ;;\n\
  hang) sleep 30 ;;\n\
  auth) echo \"Invalid API key. Please run /login.\" >&2; exit 1 ;;\n\
  descendant)\n\
    cat \"$FAKE_CLAUDE_FIXTURE\"\n\
    # Spawn a detached background process that inherits stdout/stderr (not\n\
    # redirected away) and outlives us, then keep running a little longer\n\
    # ourselves so the runner's post-result grace period actually elapses\n\
    # while we're still alive -- exercising the deadline-triggered\n\
    # process-group kill rather than a normal exit/reap.\n\
    sleep 30 &\n\
    sleep 5\n\
    ;;\n\
esac\n";
    let path = dir.join("claude");
    fs::write(&path, script).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    }
}

/// An `is` command wired to the fake shim and an isolated config dir.
fn is_cmd(sandbox: &tempfile::TempDir) -> Command {
    write_fake_claude(sandbox.path());
    let path = format!(
        "{}:{}",
        sandbox.path().display(),
        std::env::var("PATH").unwrap()
    );
    let mut cmd = Command::cargo_bin("is").unwrap();
    cmd.env("PATH", path)
        .env("XDG_CONFIG_HOME", sandbox.path().join("xdg"));
    cmd
}

fn fixture(name: &str) -> PathBuf {
    fixtures_dir().join(name)
}

#[test]
fn yes_exits_zero_with_one_line_answer() {
    let sandbox = tempfile::tempdir().unwrap();
    is_cmd(&sandbox)
        .env("FAKE_CLAUDE_FIXTURE", fixture("yes.jsonl"))
        .args(["this", "branch", "already", "merged"])
        .assert()
        .code(0)
        .stdout("yes — feature/login is in origin/main (a1b2c3d)\n");
}

#[test]
fn no_exits_one() {
    let sandbox = tempfile::tempdir().unwrap();
    is_cmd(&sandbox)
        .env("FAKE_CLAUDE_FIXTURE", fixture("no.jsonl"))
        .args(["this", "merged"])
        .assert()
        .code(1)
        .stdout(predicate::str::starts_with("no — "));
}

#[test]
fn unknown_exits_two() {
    let sandbox = tempfile::tempdir().unwrap();
    is_cmd(&sandbox)
        .env("FAKE_CLAUDE_FIXTURE", fixture("unknown.jsonl"))
        .args(["this", "merged"])
        .assert()
        .code(2)
        .stdout(predicate::str::starts_with("unknown — "));
}

#[test]
fn prose_wrapped_verdict_is_recovered() {
    let sandbox = tempfile::tempdir().unwrap();
    is_cmd(&sandbox)
        .env("FAKE_CLAUDE_FIXTURE", fixture("prose.jsonl"))
        .args(["today", "saturday"])
        .assert()
        .code(0)
        .stdout("yes — it is Saturday\n");
}

#[test]
fn unparseable_verdict_prints_raw_text_and_exits_two() {
    let sandbox = tempfile::tempdir().unwrap();
    is_cmd(&sandbox)
        .env("FAKE_CLAUDE_FIXTURE", fixture("garbage.jsonl"))
        .args(["this", "merged"])
        .assert()
        .code(2)
        .stdout("I could not settle on a verdict, sorry.\n");
}

#[test]
fn json_flag_emits_machine_readable_object() {
    let sandbox = tempfile::tempdir().unwrap();
    let out = is_cmd(&sandbox)
        .env("FAKE_CLAUDE_FIXTURE", fixture("yes.jsonl"))
        .args(["--json", "this", "merged"])
        .assert()
        .code(0)
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["verdict"], "yes");
    assert_eq!(v["answer"], "feature/login is in origin/main (a1b2c3d)");
}

#[test]
fn no_trace_on_stderr_when_not_a_tty() {
    let sandbox = tempfile::tempdir().unwrap();
    is_cmd(&sandbox)
        .env("FAKE_CLAUDE_FIXTURE", fixture("yes.jsonl"))
        .args(["this", "merged"])
        .assert()
        .code(0)
        .stderr(predicate::str::contains("⤷").not());
}

#[test]
fn using_model_is_stripped_and_persisted() {
    let sandbox = tempfile::tempdir().unwrap();
    is_cmd(&sandbox)
        .env("FAKE_CLAUDE_FIXTURE", fixture("yes.jsonl"))
        .args(["this", "merged", "using", "sonnet"])
        .assert()
        .code(0)
        .stderr(predicate::str::contains("model set to sonnet"));
    let stored =
        fs::read_to_string(sandbox.path().join("xdg/is/config.toml")).unwrap();
    assert!(stored.contains("model = \"sonnet\""), "got: {stored}");
}

#[test]
fn timeout_kills_child_and_exits_two() {
    let sandbox = tempfile::tempdir().unwrap();
    is_cmd(&sandbox)
        .env("FAKE_CLAUDE_MODE", "hang")
        .env("FAKE_CLAUDE_FIXTURE", fixture("yes.jsonl"))
        .args(["--timeout", "1", "this", "merged"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("couldn't determine within 1s"));
}

#[test]
fn auth_failure_exits_three_with_login_hint() {
    let sandbox = tempfile::tempdir().unwrap();
    is_cmd(&sandbox)
        .env("FAKE_CLAUDE_MODE", "auth")
        .env("FAKE_CLAUDE_FIXTURE", fixture("yes.jsonl"))
        .args(["this", "merged"])
        .assert()
        .code(3)
        .stderr(predicate::str::contains("log"));
}

#[test]
fn missing_claude_binary_exits_three_with_install_hint() {
    let sandbox = tempfile::tempdir().unwrap();
    // PATH contains ONLY an empty dir: no `claude` anywhere.
    let empty = sandbox.path().join("empty");
    fs::create_dir(&empty).unwrap();
    Command::cargo_bin("is")
        .unwrap()
        .env("PATH", empty.display().to_string())
        .env("XDG_CONFIG_HOME", sandbox.path().join("xdg"))
        .args(["this", "merged"])
        .assert()
        .code(3)
        .stderr(predicate::str::contains("not found on PATH"));
}

/// A descendant of the fake shim (a detached `sleep 30 &`) inherits the piped
/// stdout/stderr fds and keeps them open well after the shim itself is done.
/// If `is` only killed the direct child, the pipe-reader threads would block
/// on that descendant for the full 30s. `is` must kill the whole process
/// group so the invocation still returns promptly, with the exit code
/// determined by the result event that was already captured.
#[test]
fn descendant_outliving_shim_is_reaped_via_process_group_kill() {
    let sandbox = tempfile::tempdir().unwrap();
    let start = Instant::now();
    is_cmd(&sandbox)
        .env("FAKE_CLAUDE_MODE", "descendant")
        .env("FAKE_CLAUDE_FIXTURE", fixture("yes.jsonl"))
        .args(["this", "merged"])
        .assert()
        .code(0)
        .stdout("yes — feature/login is in origin/main (a1b2c3d)\n");
    let elapsed = start.elapsed();
    assert!(
        elapsed < Duration::from_secs(10),
        "expected is to return well under the descendant's 30s sleep, took {elapsed:?}"
    );
}

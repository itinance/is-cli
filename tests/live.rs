use assert_cmd::Command;

/// Real end-to-end run against the user's local `claude`.
/// Costs tokens; opt in with: IS_LIVE_TESTS=1 cargo test --test live -- --ignored
#[test]
#[ignore = "hits the real claude CLI; set IS_LIVE_TESTS=1 and run with --ignored"]
fn live_is_today_a_day_of_the_week() {
    if std::env::var("IS_LIVE_TESTS").as_deref() != Ok("1") {
        eprintln!("IS_LIVE_TESTS not set; skipping");
        return;
    }
    // "is today a weekday or a weekend day" is always answerable → never exit 3.
    let assert = Command::cargo_bin("is")
        .unwrap()
        .args([
            "--timeout",
            "120",
            "today",
            "either",
            "a",
            "weekday",
            "or",
            "a",
            "weekend",
            "day",
        ])
        .assert();
    let code = assert.get_output().status.code().unwrap();
    assert!(
        code == 0 || code == 1,
        "expected a verdict, got exit {code}"
    );
}

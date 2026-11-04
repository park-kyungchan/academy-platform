//! A subreaper owns a process, not a Rust test thread. Each ownership test runs
//! in its own harness process so parallel tests cannot adopt/reap one another's
//! children. The outer harness stays parallel, including between whole suites.
pub fn run(name: &str) -> bool {
    if std::env::var("S11_ISOLATED_TEST").as_deref() == Ok(name) {
        return false;
    }
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", name, "--nocapture", "--test-threads=1"])
        .env("S11_ISOLATED_TEST", name)
        .status()
        .unwrap();
    assert!(status.success(), "isolated ownership test failed: {name}");
    true
}

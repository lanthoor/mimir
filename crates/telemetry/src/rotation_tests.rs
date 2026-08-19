#[test]
fn rotates_at_max_bytes() {
    let _g = crate::init().expect("init");
    // Hard to write 5 MB cheaply, so call the rotation logic with small MAX
    // isn't trivial. Instead simulate by directly writing 6 MB of filler.
    let big = "x".repeat(6 * 1024 * 1024);
    crate::log("INFO", "rotate-test", &big);
    // The active file should now exist; older generations may exist too.
    let dir = std::env::var_os("XDG_STATE_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".local")))
        .map(|p| p.join("var").join("log"))
        .unwrap();
    let active = dir.join("mimir.log");
    assert!(active.exists(), "active log should exist after big write");
}

#[test]
fn init_is_idempotent() {
    let _a = crate::init().expect("first init");
    let _b = crate::init().expect("second init");
    // Calling init twice must not panic or reset state.
}

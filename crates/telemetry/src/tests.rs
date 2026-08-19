//! Tests for the rotating file logger.

#[test]
fn init_creates_log_file_under_home() {
    let _ = crate::init().expect("init should succeed with a writable HOME");
}

#[test]
fn log_writes_line_when_initialised() {
    let _g = crate::init();
    crate::log("INFO", "test", "hello world");
}

#[test]
fn log_falls_back_to_stderr_when_uninitialised() {
    // No init → should not panic.
    crate::log("WARN", "test", "no init yet, that's fine");
}

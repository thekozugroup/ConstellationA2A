use std::process::Command;

#[test]
fn binary_prints_help() {
    let exe = env!("CARGO_BIN_EXE_constellation");
    let output = Command::new(exe).arg("--help").output().expect("run");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    for verb in [
        "init",
        "serve",
        "peers",
        "send",
        "wait",
        "inbox",
        "respond",
        "card",
        "install-service",
    ] {
        assert!(stdout.contains(verb), "help missing verb: {verb}");
    }
}

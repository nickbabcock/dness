use assert_cmd::Command;

#[test]
fn resolve_wan_on_no_arguments() {
    let mut cmd = Command::cargo_bin(env!("CARGO_PKG_NAME")).unwrap();
    match cmd.ok() {
        Ok(output) => {
            let stdout = std::str::from_utf8(&output.stdout).unwrap();
            assert!(stdout.contains("resolved address to"));
        }
        Err(e) => {
            let output = e.as_output().unwrap();
            let stdout = std::str::from_utf8(&output.stdout).unwrap();
            assert!(stdout.contains("no records found for Query"));
        }
    }
}

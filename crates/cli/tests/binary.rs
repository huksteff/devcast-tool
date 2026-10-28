use std::process::Command;

#[test]
fn usage_errors_exit_with_2_on_stderr() {
    for args in [&[][..], &["nonexistent"], &["--frobnicate"]] {
        let out = Command::new(env!("CARGO_BIN_EXE_devcast"))
            .args(args)
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(2), "{args:?}");
        assert!(out.stdout.is_empty(), "{args:?}");
        let stderr = String::from_utf8(out.stderr).unwrap();
        assert!(stderr.contains("devcast help"), "{args:?}: {stderr}");
    }
}

//! A long flat run builds a deep LZW dictionary: lossy lookup must not recurse on the stack.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers: a failed step must fail the test"
)]

#[test]
fn a_large_flat_gif_does_not_overflow_the_compressor_stack() {
    const CHILD: &str = "DEVCAST_TEST_FLAT_GIF_CHILD";
    if std::env::var_os(CHILD).is_none() {
        // A stack overflow aborts the process, so isolate it even under plain cargo test.
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "a_large_flat_gif_does_not_overflow_the_compressor_stack",
                "--nocapture",
            ])
            .env(CHILD, "1")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "status: {}; stderr: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    let (collector, writer) = gifski::new(gifski::Settings {
        width: Some(2560),
        height: Some(1440),
        quality: 90,
        fast: false,
        repeat: gifski::Repeat::Infinite,
    })
    .unwrap();
    let bytes = std::thread::scope(|scope| {
        let written = scope.spawn(move || {
            let mut bytes = Vec::new();
            writer
                .write(&mut bytes, &mut gifski::progress::NoProgress {})
                .unwrap();
            bytes
        });
        let pixels = vec![rgb::RGBA8::new(240, 240, 240, 255); 2560 * 1440];
        collector
            .add_frame_rgba(0, imgref::ImgVec::new(pixels, 2560, 1440), 0.0)
            .unwrap();
        drop(collector);
        written.join().unwrap()
    });
    assert_eq!(&bytes[..6], b"GIF89a");
    assert_eq!(u16::from_le_bytes([bytes[6], bytes[7]]), 2560);
    assert_eq!(u16::from_le_bytes([bytes[8], bytes[9]]), 1440);
    assert_eq!(bytes.last(), Some(&b';'));
    let temp = tempfile::tempdir().unwrap();
    let file = temp.path().join("однотонный.gif");
    std::fs::write(&file, bytes).unwrap();
    let decoded = dc_media::decode_picture(&file).unwrap();
    assert_eq!(decoded.frames.len(), 1);
    let rgba = &decoded.frames[0].rgba;
    assert_eq!(rgba.len(), 2560 * 1440 * 4);
    assert!(
        rgba.as_chunks::<4>()
            .0
            .iter()
            .all(|px| *px == [240, 240, 240, 255])
    );
}

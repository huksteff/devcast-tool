//! Pictures of the system cursors: read from their bitmaps, through PNG and back.

use windows::Graphics::Imaging::{
    BitmapAlphaMode, BitmapDecoder, BitmapPixelFormat, BitmapTransform, ColorManagementMode,
    ExifOrientationMode,
};
use windows::Storage::Streams::{DataWriter, InMemoryRandomAccessStream};
use windows::Win32::UI::WindowsAndMessaging::{IDC_ARROW, IDC_IBEAM, LoadCursorW};

use super::*;

fn system(id: PCWSTR) -> isize {
    // SAFETY: a shared system cursor; it needs no release.
    unsafe { LoadCursorW(None, id) }.unwrap().0 as isize
}

/// RGBA pixels with straight alpha of a PNG.
fn decode(png: &[u8]) -> Vec<u8> {
    let stream = InMemoryRandomAccessStream::new().unwrap();
    let writer = DataWriter::CreateDataWriter(&stream).unwrap();
    writer.WriteBytes(png).unwrap();
    writer.StoreAsync().unwrap().join().unwrap();
    writer.DetachStream().unwrap();
    stream.Seek(0).unwrap();
    let decoder = BitmapDecoder::CreateAsync(&stream).unwrap().join().unwrap();
    decoder
        .GetPixelDataTransformedAsync(
            BitmapPixelFormat::Rgba8,
            BitmapAlphaMode::Straight,
            &BitmapTransform::new().unwrap(),
            ExifOrientationMode::IgnoreExifOrientation,
            ColorManagementMode::DoNotColorManage,
        )
        .unwrap()
        .join()
        .unwrap()
        .DetachPixelData()
        .unwrap()
        .to_vec()
}

fn count(image: &CursorImage, rgba: [u8; 4]) -> usize {
    image
        .rgba
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|p| **p == rgba)
        .count()
}

#[test]
fn system_cursors_are_recognised() {
    let cursors = SystemCursors::load().unwrap();
    assert_eq!(cursors.kind(system(IDC_ARROW)), CursorKind::Arrow);
    assert_eq!(cursors.kind(system(IDC_IBEAM)), CursorKind::Text);
    assert_eq!(cursors.kind(12345), CursorKind::Custom);
}

#[test]
fn arrow_and_ibeam_pictures_survive_png() {
    let arrow = picture(system(IDC_ARROW)).unwrap();
    let ibeam = picture(system(IDC_IBEAM)).unwrap();
    for (name, image) in [("arrow", &arrow), ("ibeam", &ibeam)] {
        let visible = image
            .rgba
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|p| p[3] > 0)
            .count();
        println!(
            "{name}: {}×{}, hotspot {:?}, {visible} visible, {} black, {} white",
            image.width,
            image.height,
            image.hotspot,
            count(image, [0, 0, 0, 255]),
            count(image, [255, 255, 255, 255])
        );
        assert!(visible > 20, "{name} has no picture");
        let back = decode(&encode_png(image).unwrap());
        assert_eq!(back.len(), image.rgba.len());
        for (a, b) in back
            .as_chunks::<4>()
            .0
            .iter()
            .zip(image.rgba.as_chunks::<4>().0.iter())
        {
            assert_eq!(a[3], b[3]);
            if b[3] == 255 {
                assert_eq!(a, b);
            }
        }
    }
    assert_eq!(arrow.hotspot, (0, 0));
    // The I-beam is monochrome «invert» pixels: black with a white outline, 1 px larger.
    assert!(count(&ibeam, [0, 0, 0, 255]) > 10);
    assert!(count(&ibeam, [255, 255, 255, 255]) > count(&ibeam, [0, 0, 0, 255]));
    assert_eq!(ibeam.width % 2, 0);
}

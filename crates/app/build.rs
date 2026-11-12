//! Puts the DevCast icon (`branding/devcast.ico`) into `devcast-app.exe`: Explorer, the taskbar
//! and the shortcuts the installer creates take it from the exe (task 8.1). The windows and the
//! tray take its 48 × 48 picture as RGBA from `$OUT_DIR/icon.rgba` (`logo.rs`), decoded here so
//! the program never decodes it at run time.

use std::error::Error;
use std::io::Cursor;
use std::path::Path;

const ICO: &str = "../../branding/devcast.ico";
/// Side of the picture the windows and the tray use, px (`logo::SIDE`).
const SIDE: u8 = 48;

fn main() -> Result<(), Box<dyn Error>> {
    println!("cargo:rerun-if-changed={ICO}");
    embed_resource::compile("devcast-app.rc", embed_resource::NONE).manifest_optional()?;
    let ico = std::fs::read(ICO)?;
    let rgba = decode(picture(&ico, SIDE)?)?;
    let out = std::env::var("OUT_DIR")?;
    std::fs::write(Path::new(&out).join("icon.rgba"), rgba)?;
    Ok(())
}

/// The PNG of the `side` × `side` entry of an .ico file (ICONDIR, then 16-byte ICONDIRENTRY).
fn picture(ico: &[u8], side: u8) -> Result<&[u8], Box<dyn Error>> {
    let broken = || format!("{ICO} не читается как .ico: пересобери значок с PNG {SIDE} × {SIDE}");
    let count = ico.get(4..6).ok_or_else(broken)?;
    let count = u16::from_le_bytes([count[0], count[1]]) as usize;
    let entries = ico.get(6..6 + 16 * count).ok_or_else(broken)?;
    let word = |entry: &[u8; 16], at: usize| {
        u32::from_le_bytes([entry[at], entry[at + 1], entry[at + 2], entry[at + 3]]) as usize
    };
    let entry = entries
        .as_chunks::<16>()
        .0
        .iter()
        .find(|entry| entry[0] == side && entry[1] == side)
        .ok_or_else(|| format!("в {ICO} нет картинки {side} × {side}: добавь её в значок"))?;
    let (size, at) = (word(entry, 8), word(entry, 12));
    Ok(ico.get(at..at + size).ok_or_else(broken)?)
}

fn decode(png: &[u8]) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut reader = png::Decoder::new(Cursor::new(png)).read_info()?;
    let mut rgba = vec![
        0;
        reader
            .output_buffer_size()
            .ok_or("картинка значка слишком большая")?
    ];
    let info = reader.next_frame(&mut rgba)?;
    if info.width != u32::from(SIDE)
        || info.height != u32::from(SIDE)
        || info.bit_depth != png::BitDepth::Eight
    {
        return Err(format!("картинка в {ICO} должна быть PNG {SIDE} × {SIDE}, 8 бит").into());
    }
    rgba.truncate(info.buffer_size());
    match info.color_type {
        png::ColorType::Rgba => Ok(rgba),
        png::ColorType::Rgb => Ok(rgba
            .as_chunks::<3>()
            .0
            .iter()
            .flat_map(|rgb| [rgb[0], rgb[1], rgb[2], 255])
            .collect()),
        _ => Err(format!("картинка {SIDE} × {SIDE} в {ICO} должна быть PNG RGB или RGBA").into()),
    }
}

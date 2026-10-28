//! `cargo xtask deps`: native build dependencies (FFmpeg, libclang, CMake) in `.deps/`, verified by SHA-256
//! (docs/decisions/003-video-decode.md). Downloads and unpacks with the system `curl` and `tar`.

use std::path::{Path, PathBuf};
use std::process::Command;

use sha2::{Digest, Sha256};

enum Unpack {
    /// The archive has one top-level folder; its contents become the destination folder.
    StripTopDir,
    /// Only one member of the archive is needed; it is placed into the destination folder.
    SingleFile { member: &'static str },
}

struct Archive {
    name: &'static str,
    url: &'static str,
    file: &'static str,
    sha256: &'static str,
    unpack: Unpack,
}

const ARCHIVES: [Archive; 3] = [
    Archive {
        name: "ffmpeg",
        url: "https://github.com/GyanD/codexffmpeg/releases/download/9.0.2/ffmpeg-9.0.2-full_build-shared.zip",
        file: "ffmpeg-9.0.2-full_build-shared.zip",
        sha256: "8d31e162f1616e37aab3fa2db991b97e1b8dbeb1c8465fd81a81be16ff91b328",
        unpack: Unpack::StripTopDir,
    },
    Archive {
        name: "libclang",
        url: "https://files.pythonhosted.org/packages/0b/2d/3f480b1e1d31eb3d6de5e3ef641954e5c67430d5ac93b7fa7e07589576c7/libclang-18.1.1-py2.py3-none-win_amd64.whl",
        file: "libclang-18.1.1-py2.py3-none-win_amd64.whl",
        sha256: "4dd2d3b82fab35e2bf9ca717d7b63ac990a3519c7e312f19fa8e86dcc712f7fb",
        unpack: Unpack::SingleFile {
            member: "libclang-18.1.1.data/platlib/clang/native/libclang.dll",
        },
    },
    // Builds whisper.cpp inside whisper-rs (decision 010); hash from the official
    // cmake-4.4.5-SHA-256.txt of the release.
    Archive {
        name: "cmake",
        url: "https://github.com/Kitware/CMake/releases/download/v4.4.5/cmake-4.4.5-windows-x86_64.zip",
        file: "cmake-4.4.5-windows-x86_64.zip",
        sha256: "326c826486d46a23cf9d5a1c9ee3a567d427bdd02c4a34071937a2a675cff004",
        unpack: Unpack::StripTopDir,
    },
];

/// Marker inside a ready destination folder: the SHA-256 of the archive it came from.
const MARKER: &str = ".sha256";

pub fn run(root: &Path) -> Result<(), String> {
    let deps = root.join(".deps");
    for archive in &ARCHIVES {
        ensure(&deps, archive)?;
    }
    copy_ffmpeg_dlls(root)?;
    println!(
        "deps: OK — .deps/ffmpeg, .deps/libclang; DLL FFmpeg скопированы в target/debug и target/release"
    );
    Ok(())
}

fn ensure(deps: &Path, archive: &Archive) -> Result<(), String> {
    let dest = deps.join(archive.name);
    let ready =
        std::fs::read_to_string(dest.join(MARKER)).is_ok_and(|s| s.trim() == archive.sha256);
    if ready {
        println!("{}: уже есть", archive.name);
        return Ok(());
    }
    let download = deps.join("downloads").join(archive.file);
    let cached = download.exists() && sha256_file(&download)? == archive.sha256;
    if !cached {
        println!("{}: скачиваю {}", archive.name, archive.url);
        create_dir(download.parent().unwrap_or(deps))?;
        run_tool(
            Command::new("curl")
                .args(["-fsSL", "--retry", "3", "-o"])
                .arg(&download)
                .arg(archive.url),
        )?;
        let actual = sha256_file(&download)?;
        if actual != archive.sha256 {
            return Err(format!(
                "{}: SHA-256 не совпал (ожидался {}, получен {actual}).\n  → Файл повреждён или подменён. Удали {} и повтори; если повторяется — не используй этот архив.",
                archive.name,
                archive.sha256,
                download.display()
            ));
        }
    }
    unpack(deps, archive, &download, &dest)?;
    std::fs::write(dest.join(MARKER), archive.sha256)
        .map_err(|e| format!("не удалось записать маркер: {e}"))
}

fn unpack(deps: &Path, archive: &Archive, download: &Path, dest: &Path) -> Result<(), String> {
    let staging = deps.join(format!("unpack-{}", archive.name));
    remove_dir(&staging)?;
    create_dir(&staging)?;
    let mut tar = Command::new("tar");
    tar.arg("-xf").arg(download).arg("-C").arg(&staging);
    if let Unpack::SingleFile { member } = archive.unpack {
        tar.arg(member);
    }
    run_tool(&mut tar)?;
    remove_dir(dest)?;
    match archive.unpack {
        Unpack::StripTopDir => {
            let top = single_child(&staging)?;
            std::fs::rename(&top, dest)
                .map_err(|e| format!("не удалось переместить {}: {e}", top.display()))?;
        }
        Unpack::SingleFile { member } => {
            create_dir(dest)?;
            let from = staging.join(member);
            let name = from.file_name().ok_or("пустое имя файла в архиве")?;
            std::fs::rename(&from, dest.join(name))
                .map_err(|e| format!("не удалось переместить {}: {e}", from.display()))?;
        }
    }
    remove_dir(&staging)
}

/// FFmpeg DLLs next to the built executables, so `cargo run` / tests start without PATH tweaks.
fn copy_ffmpeg_dlls(root: &Path) -> Result<(), String> {
    let bin = root.join(".deps").join("ffmpeg").join("bin");
    let entries =
        std::fs::read_dir(&bin).map_err(|e| format!("нет папки {}: {e}", bin.display()))?;
    let dlls: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "dll"))
        .collect();
    for profile in ["debug", "release"] {
        let target = root.join("target").join(profile);
        create_dir(&target)?;
        for dll in &dlls {
            let name = dll.file_name().ok_or("пустое имя DLL")?;
            std::fs::copy(dll, target.join(name))
                .map_err(|e| format!("не удалось скопировать {}: {e}", dll.display()))?;
        }
    }
    Ok(())
}

pub fn sha256_file(path: &Path) -> Result<String, String> {
    let bytes =
        std::fs::read(path).map_err(|e| format!("не удалось прочитать {}: {e}", path.display()))?;
    Ok(hex(&Sha256::digest(&bytes)))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn single_child(dir: &Path) -> Result<PathBuf, String> {
    let mut children = std::fs::read_dir(dir)
        .map_err(|e| format!("не удалось прочитать {}: {e}", dir.display()))?;
    match (children.next(), children.next()) {
        (Some(Ok(only)), None) => Ok(only.path()),
        _ => Err(format!(
            "в архиве ожидалась одна верхняя папка: {}",
            dir.display()
        )),
    }
}

fn run_tool(command: &mut Command) -> Result<(), String> {
    let shown = format!("{command:?}");
    let status = command
        .status()
        .map_err(|e| format!("не удалось запустить {shown}: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{shown} завершился с ошибкой ({status})"))
    }
}

fn create_dir(path: &Path) -> Result<(), String> {
    std::fs::create_dir_all(path).map_err(|e| format!("не удалось создать {}: {e}", path.display()))
}

fn remove_dir(path: &Path) -> Result<(), String> {
    if path.exists() {
        std::fs::remove_dir_all(path)
            .map_err(|e| format!("не удалось удалить {}: {e}", path.display()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_known_input() {
        assert_eq!(
            hex(&Sha256::digest(b"abc")),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn every_archive_has_a_full_hash() {
        for archive in &ARCHIVES {
            assert_eq!(archive.sha256.len(), 64, "{}", archive.name);
            assert!(
                archive.url.ends_with(archive.file),
                "{}: имя файла должно совпадать с URL",
                archive.name
            );
        }
    }
}

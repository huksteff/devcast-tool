//! `cargo xtask package`: the Velopack installer for Windows (task 8.1, decision 013). Builds the
//! release programs, gathers them with the FFmpeg and Vulkan DLLs and the fonts into a package
//! folder, as `target/release` holds them after `xtask deps` (decision 018), and calls `vpk pack`.
//! Everything lands in `<build folder>/velopack`, outside the repository.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::native::{copy_files, create_dir, remove_dir, run_tool};

/// The installed app's id: Velopack finds the installation and its updates by it, never change it.
const PACK_ID: &str = "DevCast";
const TITLE: &str = "DevCast";
const AUTHORS: &str = "Kefir Games";
/// The exe Velopack starts, puts on the shortcuts and runs the install hooks with.
const MAIN_EXE: &str = "devcast-app.exe";
/// The editor starts recognition through `devcast.exe` next to it (decision 018).
const EXES: [&str; 2] = [MAIN_EXE, "devcast.exe"];
const PACKAGES: [&str; 2] = ["dc-app", "dc-cli"];
/// The folder next to the exe the programs load the built-in fonts from
/// (`dc_render::text::fonts::BUILTIN_FONTS`).
const FONTS_DIR: &str = "fonts";
/// Files copied verbatim into the package folder (`xtask/package/`).
const PACKAGE_FILES: &str = "xtask/package";
/// The FFmpeg licence under a name that says whose it is (GPL v3, decision 018).
const FFMPEG_LICENSE: &str = "FFmpeg-LICENSE.txt";
/// The same version as the `velopack` crate in `crates/update` (decision 013).
const VPK_VERSION: &str = "1.2.161";

pub fn run(root: &Path) -> Result<(), String> {
    check_vpk()?;
    crate::native::run(root)?;
    let target = crate::metadata::Workspace::load()?.target;
    build_release()?;
    let out = target.join("velopack");
    let pack = out.join("pack");
    let releases = out.join("releases");
    stage(root, &target.join("release"), &pack)?;
    create_dir(&releases)?;
    let icon = root.join("branding").join("devcast.ico");
    run_tool(Command::new("vpk").args(pack_args(
        env!("CARGO_PKG_VERSION"),
        &pack,
        &releases,
        &icon,
    )))?;
    println!(
        "package: OK — установщик DevCast-win-Setup.exe и пакеты обновлений в {}",
        releases.display()
    );
    Ok(())
}

/// Fails before the long release build when `vpk` cannot be started.
fn check_vpk() -> Result<(), String> {
    match Command::new("vpk").arg("--help").output() {
        Ok(_) => Ok(()),
        Err(error) => Err(vpk_missing(&error)),
    }
}

fn vpk_missing(error: &std::io::Error) -> String {
    format!(
        "не удалось запустить vpk ({error}).\n  → Поставь .NET SDK 10 (LTS) и vpk {VPK_VERSION} без прав администратора: \
         docs/SETUP.md, строка «.NET SDK 10 (LTS) и vpk» (`dotnet tool install -g vpk --version {VPK_VERSION}`), \
         затем перезапусти терминал, чтобы `vpk` нашёлся в PATH."
    )
}

fn build_release() -> Result<(), String> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
    let mut command = Command::new(cargo);
    command.args(["build", "--release", "--locked"]);
    for package in PACKAGES {
        command.args(["-p", package]);
    }
    run_tool(&mut command)
}

/// A fresh package folder: the programs from `release`, the DLLs, their licences and the fonts
/// from `.deps/`, the files of `xtask/package/`.
fn stage(root: &Path, release: &Path, pack: &Path) -> Result<(), String> {
    remove_dir(pack)?;
    let exes: Vec<PathBuf> = EXES.iter().map(|exe| release.join(exe)).collect();
    copy_files(&exes, pack)?;
    copy_files(&crate::native::runtime_dlls(root)?, pack)?;
    copy_files(&package_files(root)?, pack)?;
    copy_files(&[crate::native::vulkan_license(root)], pack)?;
    let ffmpeg = ffmpeg_license(root);
    std::fs::copy(&ffmpeg, pack.join(FFMPEG_LICENSE)).map_err(|e| {
        format!(
            "не удалось скопировать {}: {e}.
  → Запусти `cargo xtask deps`.",
            ffmpeg.display()
        )
    })?;
    crate::fonts::copy_into(&root.join(".deps"), &pack.join(FONTS_DIR))
}

fn ffmpeg_license(root: &Path) -> PathBuf {
    root.join(".deps").join("ffmpeg").join("LICENSE")
}

/// The files of `xtask/package/` (the third-party notices).
fn package_files(root: &Path) -> Result<Vec<PathBuf>, String> {
    let dir = root.join(PACKAGE_FILES);
    let entries =
        std::fs::read_dir(&dir).map_err(|e| format!("нет папки {}: {e}", dir.display()))?;
    let mut files: Vec<PathBuf> = entries.filter_map(Result::ok).map(|e| e.path()).collect();
    files.sort();
    Ok(files)
}

/// Arguments of `vpk pack`: a per-user install (Velopack's only mode on Windows) with shortcuts
/// on the desktop and in the Start menu.
fn pack_args(version: &str, pack: &Path, releases: &Path, icon: &Path) -> Vec<OsString> {
    let mut args: Vec<OsString> = Vec::new();
    let mut push = |key: &str, value: OsString| {
        args.push(key.into());
        args.push(value);
    };
    push("--packId", PACK_ID.into());
    push("--packVersion", version.into());
    push("--packTitle", TITLE.into());
    push("--packAuthors", AUTHORS.into());
    push("--packDir", pack.into());
    push("--mainExe", MAIN_EXE.into());
    push("--icon", icon.into());
    push("--shortcuts", "Desktop,StartMenuRoot".into());
    push("--outputDir", releases.into());
    let mut command: Vec<OsString> = vec!["pack".into()];
    command.extend(args);
    command
}

#[cfg(test)]
mod tests {
    use super::*;

    fn value<'a>(args: &'a [OsString], key: &str) -> Option<&'a OsString> {
        let at = args.iter().position(|arg| arg == key)?;
        args.get(at + 1)
    }

    #[test]
    fn packs_the_app_with_its_icon_and_shortcuts() {
        let args = pack_args(
            "0.1.0",
            Path::new("D:/dct/velopack/pack"),
            Path::new("D:/dct/velopack/releases"),
            Path::new("D:/repo/branding/devcast.ico"),
        );
        assert_eq!(args.first().map(OsString::as_os_str), Some("pack".as_ref()));
        assert_eq!(value(&args, "--packId"), Some(&"DevCast".into()));
        assert_eq!(value(&args, "--packVersion"), Some(&"0.1.0".into()));
        assert_eq!(value(&args, "--mainExe"), Some(&"devcast-app.exe".into()));
        assert_eq!(
            value(&args, "--packDir"),
            Some(&"D:/dct/velopack/pack".into())
        );
        assert_eq!(
            value(&args, "--outputDir"),
            Some(&"D:/dct/velopack/releases".into())
        );
        assert_eq!(
            value(&args, "--icon"),
            Some(&"D:/repo/branding/devcast.ico".into())
        );
        assert_eq!(
            value(&args, "--shortcuts"),
            Some(&"Desktop,StartMenuRoot".into())
        );
    }

    #[test]
    fn the_main_exe_goes_into_the_package() {
        assert!(EXES.contains(&MAIN_EXE));
    }

    #[test]
    fn the_icon_is_in_the_repository() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let icon = std::fs::read(root.join("branding").join("devcast.ico"));
        assert!(
            icon.is_ok_and(|bytes| bytes.starts_with(&[0, 0, 1, 0])),
            "branding/devcast.ico должен быть файлом .ico"
        );
    }

    #[test]
    fn vpk_version_matches_the_crate() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let manifest = std::fs::read_to_string(root.join("crates/update/Cargo.toml"));
        assert!(
            manifest.is_ok_and(|text| text.contains(&format!("velopack = \"={VPK_VERSION}\""))),
            "версия vpk должна совпадать с крейтом velopack (решение 013)"
        );
    }

    fn root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
    }

    fn names(files: &[PathBuf]) -> Vec<String> {
        files
            .iter()
            .filter_map(|file| file.file_name())
            .map(|name| name.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn the_package_gets_the_third_party_notices() {
        let files = package_files(&root());
        assert!(
            files.is_ok_and(|files| names(&files).contains(&"THIRD-PARTY-NOTICES.txt".to_owned())),
            "xtask/package/THIRD-PARTY-NOTICES.txt должен попадать в пакет"
        );
    }

    #[test]
    fn notices_name_the_bundled_ffmpeg_and_its_licence_files() {
        let notices =
            std::fs::read_to_string(root().join(PACKAGE_FILES).join("THIRD-PARTY-NOTICES.txt"))
                .unwrap_or_default();
        let url = crate::native::archive_url("ffmpeg").unwrap_or_default();
        assert!(
            url.contains("/9.0.2/"),
            "обнови версию FFmpeg в THIRD-PARTY-NOTICES.txt"
        );
        for needed in [
            "FFmpeg 9.0.2",
            "GPL v3",
            "https://github.com/FFmpeg/FFmpeg/commit/946fcce07b",
            FFMPEG_LICENSE,
            "VulkanRT-License.txt",
            "Open Font License",
        ] {
            assert!(
                notices.contains(needed),
                "в THIRD-PARTY-NOTICES.txt нет «{needed}»"
            );
        }
    }

    #[test]
    fn licences_come_from_the_deps_folder() {
        let root = Path::new("D:/repo");
        assert_eq!(ffmpeg_license(root), root.join(".deps/ffmpeg/LICENSE"));
        assert_eq!(
            crate::native::vulkan_license(root),
            root.join(".deps")
                .join("vulkan")
                .join("VulkanRT-License.txt")
        );
    }

    #[test]
    fn missing_vpk_says_how_to_install_it() {
        let error = std::io::Error::from(std::io::ErrorKind::NotFound);
        let message = vpk_missing(&error);
        assert!(message.contains("dotnet tool install -g vpk --version 1.2.161"));
        assert!(message.contains("docs/SETUP.md"));
    }
}

//! Spike 1.13: Velopack install + update (full and delta) from a local release folder,
//! and settings surviving the update (settings live outside the install folder).
//!
//! Usage (installed copy): spike-velopack            → record a launch
//!                         spike-velopack update <releases_dir>

use std::io::Write;
use std::path::PathBuf;

use velopack::sources::FileSource;
use velopack::{UpdateCheck, UpdateManager, VelopackApp};

fn data_dir() -> PathBuf {
    let base = std::env::var("LOCALAPPDATA").unwrap_or_else(|_| ".".to_owned());
    PathBuf::from(base).join("DevCastSpikeData")
}

fn log(line: &str) {
    let dir = data_dir();
    let _ = std::fs::create_dir_all(&dir);
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(dir.join("log.txt")) {
        let _ = writeln!(f, "{line}");
    }
    println!("{line}");
}

/// Settings file outside the install folder: a launch counter that must survive updates.
fn bump_launches() -> u32 {
    let path = data_dir().join("settings.txt");
    let n = std::fs::read_to_string(&path).ok().and_then(|s| s.trim().parse().ok()).unwrap_or(0) + 1;
    let _ = std::fs::create_dir_all(data_dir());
    let _ = std::fs::write(&path, n.to_string());
    n
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    VelopackApp::build().run();
    let launches = bump_launches();
    let args: Vec<String> = std::env::args().skip(1).collect();
    log(&format!("start: binary {} launches={launches} args={args:?}", env!("CARGO_PKG_VERSION")));
    if args.first().map(String::as_str) == Some("update") {
        let dir = args.get(1).cloned().unwrap_or_default();
        let manager = UpdateManager::new(FileSource::new(&dir), None, None)?;
        log(&format!("installed version {}", manager.get_current_version_as_string()));
        match manager.check_for_updates()? {
            UpdateCheck::UpdateAvailable(info) => {
                log(&format!("update available: {} (delta packages: {})", info.TargetFullRelease.Version, info.DeltasToTarget.len()));
                manager.download_updates(&info, None)?;
                log("downloaded, applying and restarting");
                manager.apply_updates_and_restart_with_args(&info.TargetFullRelease, ["after-update"])?;
            }
            UpdateCheck::NoUpdateAvailable => log("no update available"),
            UpdateCheck::RemoteIsEmpty => log("release feed is empty"),
        }
    }
    Ok(())
}

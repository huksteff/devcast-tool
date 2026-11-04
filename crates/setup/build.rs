//! Puts the DevCast icon into `devcast-setup.exe` and the Velopack installer into the program:
//! `cargo xtask package` builds this crate after `vpk pack` with `DEVCAST_VELOPACK_SETUP` set to
//! `DevCast-win-Setup.exe`. Without it (`cargo check`, tests) the program carries nothing and
//! says so when started.

use std::error::Error;
use std::path::Path;

/// The Velopack installer to carry, set by `cargo xtask package`.
const SETUP_ENV: &str = "DEVCAST_VELOPACK_SETUP";

fn main() -> Result<(), Box<dyn Error>> {
    println!("cargo:rerun-if-changed=../../branding/devcast.ico");
    println!("cargo:rerun-if-changed=devcast-setup.manifest");
    println!("cargo:rerun-if-env-changed={SETUP_ENV}");
    embed_resource::compile("devcast-setup.rc", embed_resource::NONE).manifest_optional()?;
    let out = Path::new(&std::env::var("OUT_DIR")?).join("velopack-setup.exe");
    match std::env::var_os(SETUP_ENV) {
        Some(setup) => {
            println!("cargo:rerun-if-changed={}", Path::new(&setup).display());
            std::fs::copy(&setup, &out)?;
        }
        None => std::fs::write(&out, [])?,
    }
    Ok(())
}

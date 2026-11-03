//! Puts the DevCast icon (`branding/devcast.ico`) into `devcast-app.exe`: Explorer, the taskbar
//! and the shortcuts the installer creates take it from the exe (task 8.1).

fn main() -> Result<(), embed_resource::CompilationResult> {
    println!("cargo:rerun-if-changed=../../branding/devcast.ico");
    embed_resource::compile("devcast-app.rc", embed_resource::NONE).manifest_optional()
}

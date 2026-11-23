//! Build script for the Astra GUI host.
//!
//! On Windows, embeds `assets/alloy-icon.ico` into the executable's PE header
//! so the `.exe` shows the Astra icon in Explorer and the taskbar. (mocida-sys's
//! own build script handles linking + DLL staging; this only adds the icon.)
//!
//! On other platforms this is a no-op — macOS gets its icon from the `.app`
//! bundle produced by `packaging/make-app.sh`, not from the binary.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    #[cfg(windows)]
    {
        println!("cargo:rerun-if-changed=assets/astra-icon.ico");
        let ico = std::path::Path::new("assets/astra-icon.ico"); // rendered from astra-icon-win.png
        if ico.is_file() {
            let mut res = winresource::WindowsResource::new();
            res.set_icon("assets/astra-icon.ico");
            if let Err(e) = res.compile() {
                println!("cargo:warning=failed to embed the Astra icon: {e}");
            }
        } else {
            println!(
                "cargo:warning=assets/astra-icon.ico missing — generate it from the PNG \
                 (assets/astra-icon.png) to embed the icon in the .exe"
            );
        }
    }
}

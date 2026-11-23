//! Build script for the Astra GUI host.
//!
//! On Windows, embeds `assets/astra-icon.ico` into the PE header.
//! `backend.mika` is the source of truth for app logic; keep `src/backend_gen.rs`
//! in sync (regenerate with mikasa when you change backend.mika).

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=backend.mika");
    println!("cargo:rerun-if-changed=astra.mika");

    #[cfg(windows)]
    {
        println!("cargo:rerun-if-changed=assets/astra-icon.ico");
        let ico = std::path::Path::new("assets/astra-icon.ico");
        if ico.is_file() {
            let mut res = winresource::WindowsResource::new();
            res.set_icon("assets/astra-icon.ico");
            if let Err(e) = res.compile() {
                println!("cargo:warning=failed to embed the Astra icon: {e}");
            }
        } else {
            println!(
                "cargo:warning=assets/astra-icon.ico missing — generate it from the PNG \
                 to embed the icon in the .exe"
            );
        }
    }
}

//! `# mika_import!("file.mika")`
//!
//! Compile-time include of a Mikasa source file as Rust. The macro:
//! 1. Resolves the path relative to the calling `.rs` file (fallback:
//!    `CARGO_MANIFEST_DIR`).
//! 2. Expands nested `import("…")` inside that `.mika`.
//! 3. Transpiles Mikasa → Rust and pastes the items at the call site
//!    (top-level `fn`/`struct`/`enum`/`trait` are `pub`).
//!
//! ```ignore
//! // in Rust
//! mika_import!("logic.mika");
//!
//! fn main() {
//!     greeter(); // defined in logic.mika
//! }
//! ```
//!
//! Inside `.mika` the same idea is spelled without the macro:
//! ```text
//! import("logic.mika")
//! ```

use proc_macro::TokenStream;
use quote::quote;
use std::path::{Path, PathBuf};
use syn::{parse_macro_input, LitStr};

#[proc_macro]
pub fn mika_import(input: TokenStream) -> TokenStream {
    let lit = parse_macro_input!(input as LitStr);
    let raw = lit.value();
    let span = lit.span();

    let path = match resolve_path(&raw, span) {
        Ok(p) => p,
        Err(msg) => {
            return syn::Error::new(span, msg).to_compile_error().into();
        }
    };

    let src = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) => {
            return syn::Error::new(
                span,
                format!("mika_import!({raw:?}): could not read {}: {e}", path.display()),
            )
            .to_compile_error()
            .into();
        }
    };

    // Absolute path so `include_bytes!` (resolved relative to the calling
    // `.rs`) doesn't double-prefix `src/`. Forces rebuild when the .mika changes.
    let abs = path
        .canonicalize()
        .unwrap_or_else(|_| path.clone());
    let path_str = abs.to_string_lossy().replace('\\', "/");
    let watch = quote! { const _: &[u8] = include_bytes!(#path_str); };

    let rust = match mikasa_parser::transpile_as_module(&src, &path) {
        Ok(r) => r,
        Err(e) => {
            return syn::Error::new(span, format!("mika_import!({raw:?}): {e}"))
                .to_compile_error()
                .into();
        }
    };

    let items: syn::File = match syn::parse_str(&rust) {
        Ok(f) => f,
        Err(e) => {
            return syn::Error::new(
                span,
                format!(
                    "mika_import!({raw:?}): transpiled Rust failed to parse ({e}).\n---\n{rust}\n---"
                ),
            )
            .to_compile_error()
            .into();
        }
    };

    let out = quote! {
        #watch
        #items
    };
    out.into()
}

fn resolve_path(raw: &str, span: proc_macro2::Span) -> Result<PathBuf, String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err("mika_import!(\"\"): empty path".into());
    }
    // Prefer the call-site source file's directory (Rust 1.88+).
    let call_dir = proc_macro::Span::call_site()
        .local_file()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()));

    let candidates: Vec<PathBuf> = match call_dir {
        Some(dir) => vec![dir],
        None => {
            let manifest = std::env::var_os("CARGO_MANIFEST_DIR")
                .map(PathBuf::from)
                .ok_or_else(|| {
                    "mika_import!: cannot resolve path (no call-site file, no CARGO_MANIFEST_DIR)"
                        .to_string()
                })?;
            vec![manifest]
        }
    };

    for base in &candidates {
        if let Ok(p) = mikasa_syntax::file_import::resolve_import_path(base, raw) {
            return Ok(p);
        }
        // Also try as-is under the base (macro callers often pass "src/foo.mika").
        let direct = base.join(raw);
        if direct.is_file() {
            return Ok(direct);
        }
    }

    // Absolute path pass-through.
    let abs = Path::new(raw);
    if abs.is_absolute() && abs.is_file() {
        return Ok(abs.to_path_buf());
    }

    let _ = span;
    Err(format!(
        "mika_import!({raw:?}): file not found (looked next to the calling .rs / CARGO_MANIFEST_DIR)"
    ))
}

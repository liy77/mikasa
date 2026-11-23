//! File extensions for the Mikasa / Astra toolchain.
//!
//! The current names are the recommended ones. The originals keep working so
//! nothing already written breaks, but new code and new output use the new
//! names:
//!
//! | Role | Recommended | Still accepted |
//! | --- | --- | --- |
//! | Source | `.mika` | `.crs` |
//! | Astra bytecode | `.ast` | `.loy` |
//! | Library | `.mklib` | — |
//! | Package | `.mipkg` | — |
//!
//! Detection never keys off the extension alone where the bytes already say
//! what a file is: an Astra artifact is recognised by its `ALLOYBC` magic, so
//! `.ast` and `.loy` load through the same path.

/// Recommended extension for Mikasa source, without the dot.
pub const SOURCE: &str = "mika";
/// Original source extension. Accepted everywhere, no longer the default.
pub const SOURCE_LEGACY: &str = "crs";

/// Recommended extension for an Astra bytecode artifact.
pub const ARTIFACT: &str = "ast";
/// Original artifact extension (`alloy build` used to write this).
pub const ARTIFACT_LEGACY: &str = "loy";

/// A Mikasa library.
pub const LIBRARY: &str = "mklib";
/// A Mikasa package.
pub const PACKAGE: &str = "mipkg";

/// A MocidaUI document. Markup-only files still use this; a `.mika` that
/// contains `view` or `App()` is a MocidaUI document too.
pub const MUI: &str = "mui";
/// The old "Mikasa + Mocida in one file" extension. Retired: `.mika` does that
/// itself now. Still recognised so existing files open, with a warning.
pub const MUI_LEGACY: &str = "crm";

/// Source extensions, recommended first.
pub const SOURCE_EXTS: &[&str] = &[SOURCE, SOURCE_LEGACY];
/// Artifact extensions, recommended first.
pub const ARTIFACT_EXTS: &[&str] = &[ARTIFACT, ARTIFACT_LEGACY];

/// True for a Mikasa source file (`.mika`, or the legacy `.crs`).
pub fn is_source(ext: &str) -> bool {
    let ext = ext.trim_start_matches('.').to_ascii_lowercase();
    SOURCE_EXTS.iter().any(|e| *e == ext)
}

/// True for an Astra artifact extension (`.ast`, or the legacy `.loy`).
///
/// The extension is only a hint — the loader confirms an artifact by its
/// magic header, so a renamed file still runs.
pub fn is_artifact(ext: &str) -> bool {
    let ext = ext.trim_start_matches('.').to_ascii_lowercase();
    ARTIFACT_EXTS.iter().any(|e| *e == ext)
}

/// True for a Mikasa library (`.mklib`).
pub fn is_library(ext: &str) -> bool {
    ext.trim_start_matches('.').eq_ignore_ascii_case(LIBRARY)
}

/// True for a Mikasa package (`.mipkg`).
pub fn is_package(ext: &str) -> bool {
    ext.trim_start_matches('.').eq_ignore_ascii_case(PACKAGE)
}

/// True for a markup-only MocidaUI file (`.mui`).
pub fn is_mui(ext: &str) -> bool {
    ext.trim_start_matches('.').eq_ignore_ascii_case(MUI)
}

/// True for the retired `.crm` extension.
pub fn is_mui_legacy(ext: &str) -> bool {
    ext.trim_start_matches('.').eq_ignore_ascii_case(MUI_LEGACY)
}

/// True when a Mikasa source is really a MocidaUI document: it declares a
/// `view` or an `App()`. A plain `.mika` has neither, so the two stay apart
/// without a second extension.
pub fn source_is_mui(src: &str) -> bool {
    let mut line_start = true;
    let bytes = src.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if c == b'\n' {
            line_start = true;
            i += 1;
            continue;
        }
        if line_start && c.is_ascii_whitespace() {
            i += 1;
            continue;
        }
        if line_start {
            line_start = false;
            if src[i..].starts_with("view ") || src[i..].starts_with("view\t") {
                return true;
            }
            if src[i..].starts_with("App()") || src[i..].starts_with("App ()") {
                return true;
            }
        }
        i += 1;
    }
    false
}

/// The warning `mikasa build` and `mikasa run` print for a legacy extension.
///
/// The file still builds and runs — the old spelling is deprecated, not
/// removed — but the recommended name is stated so it gets renamed.
pub fn deprecation_warning(ext: &str) -> Option<String> {
    let ext = ext.trim_start_matches('.').to_ascii_lowercase();
    let recommended = match ext.as_str() {
        SOURCE_LEGACY => SOURCE,
        ARTIFACT_LEGACY => ARTIFACT,
        MUI_LEGACY => SOURCE,
        _ => return None,
    };
    Some(format!(
        "Deprecated: .{ext} is no longer recommended. Rename it to .{recommended}."
    ))
}

/// A note for when a legacy extension was used, so the user learns the new
/// name without the old one stopping working. Empty for a recommended one.
pub fn legacy_note(ext: &str) -> Option<String> {
    let ext = ext.trim_start_matches('.').to_ascii_lowercase();
    match ext.as_str() {
        SOURCE_LEGACY => Some(format!(
            ".{SOURCE_LEGACY} still works, but .{SOURCE} is the recommended extension for Mikasa source"
        )),
        ARTIFACT_LEGACY => Some(format!(
            ".{ARTIFACT_LEGACY} still works, but .{ARTIFACT} is the recommended extension for Astra bytecode"
        )),
        MUI_LEGACY => Some(format!(
            ".{MUI_LEGACY} is retired — a .{SOURCE} file supports MocidaUI directly (rename it to .{SOURCE})"
        )),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recommended_and_legacy_both_match() {
        assert!(is_source("mika"));
        assert!(is_source(".crs"));
        assert!(is_source("CRS"));
        assert!(!is_source("rs"));

        assert!(is_artifact("ast"));
        assert!(is_artifact(".loy"));
        assert!(!is_artifact("mika"));

        assert!(is_library("mklib"));
        assert!(is_package(".mipkg"));
    }

    #[test]
    fn legacy_note_only_for_old_names() {
        assert!(legacy_note("crs").is_some());
        assert!(legacy_note("loy").is_some());
        assert!(legacy_note("mika").is_none());
        assert!(legacy_note("ast").is_none());
        assert!(legacy_note("crm").unwrap().contains("retired"));
    }

    #[test]
    fn a_mika_with_a_view_is_mocida_ui() {
        assert!(source_is_mui("view Card() {\n  Text(\"hi\")\n}\n"));
        assert!(source_is_mui("// header\n\nApp() {\n  name: \"X\"\n}\n"));
        assert!(!source_is_mui("func main() {\n    println!(\"hi\")\n}\n"));
        // `view` mid-line is code, not a declaration.
        assert!(!source_is_mui("name = \"view \"\n"));
    }
}

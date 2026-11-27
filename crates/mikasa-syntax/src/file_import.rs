//! Textual expansion of `import("path.mika")` (dynamic file import).
//!
//! Replaces each `import("…")` / `import('…')` with the contents of the
//! referenced Mikasa source file (recursively). Used by the mikasa transpile
//! path (streaming parser) and by the `mika_import!` Rust macro before it
//! lowers Mikasa → Rust. The Astra interpreter expands the same form via the
//! AST ([`crate::program::ImportKind::File`]) instead.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Expand every `import("file.mika")` in `src`, resolving paths relative to
/// `base_dir`. Cycles error out. Nested imports inside the included files are
/// expanded too.
pub fn expand_file_imports(src: &str, base_dir: &Path) -> Result<String, String> {
    let mut visited = HashSet::new();
    // Seed with a synthetic key for the caller so a file can't import itself
    // through a relative loop without being detected once nested.
    expand_rec(src, base_dir, &mut visited)
}

fn expand_rec(
    src: &str,
    base_dir: &Path,
    visited: &mut HashSet<PathBuf>,
) -> Result<String, String> {
    let mut out = String::with_capacity(src.len());
    let mut rest = src;
    while let Some((before, raw_path, after)) = find_next_file_import(rest) {
        out.push_str(before);
        let resolved = resolve_import_path(base_dir, &raw_path)?;
        let canon = resolved
            .canonicalize()
            .unwrap_or_else(|_| resolved.clone());
        if !visited.insert(canon.clone()) {
            return Err(format!(
                "cycle in import({raw_path:?}): already importing {}",
                canon.display()
            ));
        }
        let nested = std::fs::read_to_string(&resolved).map_err(|e| {
            format!(
                "import({raw_path:?}): could not read {}: {e}",
                resolved.display()
            )
        })?;
        let nested_base = resolved.parent().unwrap_or(base_dir);
        let expanded = expand_rec(&nested, nested_base, visited)?;
        visited.remove(&canon);
        out.push_str("\n// --- begin file import: ");
        out.push_str(&raw_path);
        out.push_str(" ---\n");
        out.push_str(&expanded);
        if !expanded.ends_with('\n') {
            out.push('\n');
        }
        out.push_str("// --- end file import ---\n");
        rest = after;
    }
    out.push_str(rest);
    Ok(out)
}

/// Resolve `raw` relative to `base_dir`. Accepts `foo`, `foo.mika`, `./foo.mika`,
/// `subdir/foo.mika`. Prefers `.mika`, then legacy `.crs`.
pub fn resolve_import_path(base_dir: &Path, raw: &str) -> Result<PathBuf, String> {
    let raw = raw.trim().trim_matches('"').trim_matches('\'').trim();
    if raw.is_empty() {
        return Err("import(\"\"): empty path".into());
    }
    if raw.contains("::") {
        return Err(format!(
            "import({raw:?}): expected a file path, not a Rust path"
        ));
    }
    let p = base_dir.join(raw.trim_start_matches("./"));
    if p.is_file() {
        return Ok(p);
    }
    // Bare stem → try source extensions.
    if p.extension().is_none() {
        for ext in crate::ext::SOURCE_EXTS {
            let candidate = p.with_extension(ext);
            if candidate.is_file() {
                return Ok(candidate);
            }
        }
    }
    // Wrong/missing extension: still try swapping to a source ext.
    for ext in crate::ext::SOURCE_EXTS {
        let candidate = p.with_extension(ext);
        if candidate.is_file() {
            return Ok(candidate);
        }
    }
    Err(format!(
        "import({raw:?}): file not found next to {}",
        base_dir.display()
    ))
}

/// Find the next `import ( "path" )` / `import ( 'path' )` outside of comments
/// and string literals. Returns `(before, path_lit, after)`.
fn find_next_file_import(src: &str) -> Option<(&str, String, &str)> {
    let b = src.as_bytes();
    let mut i = 0;
    while i < b.len() {
        // Line comment
        if b[i] == b'/' && i + 1 < b.len() && b[i + 1] == b'/' {
            i += 2;
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        // Block comment
        if b[i] == b'/' && i + 1 < b.len() && b[i + 1] == b'*' {
            i += 2;
            while i + 1 < b.len() && !(b[i] == b'*' && b[i + 1] == b'/') {
                i += 1;
            }
            i = i.saturating_add(2);
            continue;
        }
        // String literal — skip
        if b[i] == b'"' || b[i] == b'\'' {
            let quote = b[i];
            i += 1;
            while i < b.len() {
                if b[i] == b'\\' && i + 1 < b.len() {
                    i += 2;
                    continue;
                }
                if b[i] == quote {
                    i += 1;
                    break;
                }
                i += 1;
            }
            continue;
        }
        if at_word(b, i, b"import") {
            let start = i;
            let mut j = skip_ws(b, i + 6);
            if j < b.len() && b[j] == b'(' {
                j = skip_ws(b, j + 1);
                if let Some((lit, after_lit)) = read_string_lit(b, j) {
                    let k = skip_ws(b, after_lit);
                    if k < b.len() && b[k] == b')' {
                        let path = lit;
                        let after = k + 1;
                        // Swallow a trailing semicolon if present.
                        let mut end = after;
                        let end_ws = skip_ws(b, end);
                        if end_ws < b.len() && b[end_ws] == b';' {
                            end = end_ws + 1;
                        } else {
                            end = after;
                        }
                        return Some((
                            &src[..start],
                            path,
                            &src[end..],
                        ));
                    }
                }
            }
        }
        i += 1;
    }
    None
}

fn at_word(bytes: &[u8], i: usize, word: &[u8]) -> bool {
    if i + word.len() > bytes.len() {
        return false;
    }
    if &bytes[i..i + word.len()] != word {
        return false;
    }
    let before_ok = i == 0 || !is_ident(bytes[i - 1]);
    let after = i + word.len();
    let after_ok = after >= bytes.len() || !is_ident(bytes[after]);
    before_ok && after_ok
}

fn is_ident(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

fn skip_ws(bytes: &[u8], mut i: usize) -> usize {
    while i < bytes.len() && bytes[i].is_ascii_whitespace() {
        i += 1;
    }
    i
}

fn read_string_lit(bytes: &[u8], i: usize) -> Option<(String, usize)> {
    if i >= bytes.len() || (bytes[i] != b'"' && bytes[i] != b'\'') {
        return None;
    }
    let quote = bytes[i];
    let mut j = i + 1;
    let mut out = String::new();
    while j < bytes.len() {
        if bytes[j] == b'\\' && j + 1 < bytes.len() {
            out.push(bytes[j + 1] as char);
            j += 2;
            continue;
        }
        if bytes[j] == quote {
            return Some((out, j + 1));
        }
        out.push(bytes[j] as char);
        j += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn finds_call_form_not_from_form() {
        let src = "import { a } from b\nimport(\"util.mika\")\nfunc main() {}\n";
        let (before, path, after) = find_next_file_import(src).expect("found");
        assert!(before.contains("from b"));
        assert_eq!(path, "util.mika");
        assert!(after.contains("func main"));
    }

    #[test]
    fn expands_nested() {
        let dir = std::env::temp_dir().join(format!(
            "mika-file-import-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("leaf.mika"), "func leaf() { println(\"L\") }\n").unwrap();
        std::fs::write(
            dir.join("mid.mika"),
            "import(\"leaf.mika\")\nfunc mid() { leaf() }\n",
        )
        .unwrap();
        let top = "import(\"mid.mika\")\nfunc main() { mid() }\n";
        let expanded = expand_file_imports(top, &dir).unwrap();
        assert!(expanded.contains("func leaf()"));
        assert!(expanded.contains("func mid()"));
        assert!(expanded.contains("func main()"));
        assert!(
            !expanded.contains("import(\"mid") && !expanded.contains("import(\"leaf"),
            "raw import() call should be expanded away: {expanded}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn cycle_is_rejected() {
        let dir = std::env::temp_dir().join(format!(
            "mika-file-import-cycle-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut a = std::fs::File::create(dir.join("a.mika")).unwrap();
        writeln!(a, "import(\"b.mika\")").unwrap();
        let mut b = std::fs::File::create(dir.join("b.mika")).unwrap();
        writeln!(b, "import(\"a.mika\")").unwrap();
        let err = expand_file_imports("import(\"a.mika\")\n", &dir).unwrap_err();
        assert!(err.contains("cycle"), "{err}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

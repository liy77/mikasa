//! `mikasa check` — fast static analysis (parse + lints), no cargo/rustc.
//!
//! Like `cargo clippy` for Mikasa: syntax errors, deprecations, unused
//! functions, unresolved imports, MUI parse errors. Exit 1 on errors (and on
//! warnings when `--deny-warnings`).
//!
//! Opt into the heavy Rust verifier with `--miri` / `--no-miri` (transpiles +
//! isolated nightly under `~/.astra/rustup`).

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use mikasa_syntax::program::{self, Item};
use mikasa_syntax::ast::Span;

use crate::internal::pretty;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Severity {
    Error,
    Warning,
}

struct Diag {
    severity: Severity,
    path: String,
    line: usize,
    col: usize,
    message: String,
}

/// Fast check: parse + lints, no compile. `deny_warnings` → warnings also fail.
pub fn run_fast_check(paths: &[String], deny_warnings: bool) -> i32 {
    let mut files: Vec<String> = Vec::new();
    for p in paths {
        collect(Path::new(p), &mut files);
    }
    if files.is_empty() {
        pretty::warn("no Mikasa / MocidaUI files found to check");
        return 0;
    }

    let mut diags: Vec<Diag> = Vec::new();
    for f in &files {
        check_file(f, &mut diags);
    }

    let mut errors = 0usize;
    let mut warnings = 0usize;
    for d in &diags {
        emit(d);
        match d.severity {
            Severity::Error => errors += 1,
            Severity::Warning => warnings += 1,
        }
    }

    if errors == 0 && warnings == 0 {
        pretty::ok(&format!(
            "check passed — {} file(s), no issues",
            files.len()
        ));
        return 0;
    }

    eprintln!();
    if errors > 0 {
        pretty::fail(&format!(
            "check: {errors} error(s), {warnings} warning(s) in {} file(s)",
            files.len()
        ));
    } else {
        pretty::warn(&format!(
            "check: {warnings} warning(s) in {} file(s)",
            files.len()
        ));
    }

    if errors > 0 || (deny_warnings && warnings > 0) {
        1
    } else {
        0
    }
}

fn check_file(path: &str, diags: &mut Vec<Diag>) {
    let src = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            diags.push(Diag {
                severity: Severity::Error,
                path: path.to_string(),
                line: 1,
                col: 1,
                message: format!("could not read file: {e}"),
            });
            return;
        }
    };

    let ext = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    if let Some(w) = mikasa_syntax::ext::deprecation_warning(&ext) {
        diags.push(Diag {
            severity: Severity::Warning,
            path: path.to_string(),
            line: 1,
            col: 1,
            message: w,
        });
    }

    let is_mui = crate::internal::mui::is_mui_file(path);
    if is_mui {
        check_mui(path, &src, diags);
        return;
    }

    check_mika(path, &src, diags);
}

fn check_mui(path: &str, src: &str, diags: &mut Vec<Diag>) {
    let doc = mui_syntax::parse(src);
    for e in &doc.errors {
        let (line, col) = offset_to_line_col(src, e.span.start as usize);
        diags.push(Diag {
            severity: Severity::Error,
            path: path.to_string(),
            line,
            col,
            message: e.message.clone(),
        });
    }
}

fn check_mika(path: &str, src: &str, diags: &mut Vec<Diag>) {
    if src.contains("from cstd") {
        if let Some(warning) = mikasa_syntax::ext::stdlib_deprecation("cstd") {
            let (line, col) = find_substr_line(src, "from cstd");
            diags.push(Diag {
                severity: Severity::Warning,
                path: path.to_string(),
                line,
                col,
                message: warning,
            });
        }
    }

    let prog = program::parse_program(src);
    for e in &prog.errors {
        let (line, col) = offset_to_line_col(src, e.span.start as usize);
        diags.push(Diag {
            severity: Severity::Error,
            path: path.to_string(),
            line,
            col,
            message: e.message.clone(),
        });
    }

    // Duplicate top-level function names.
    let mut seen: HashMap<String, Span> = HashMap::new();
    let mut fn_names: Vec<(String, Span)> = Vec::new();
    for item in &prog.items {
        if let Item::Function { name, span, .. } = item {
            fn_names.push((name.clone(), *span));
            if let Some(prev) = seen.insert(name.clone(), *span) {
                let (line, col) = offset_to_line_col(src, span.start as usize);
                let (pline, _) = offset_to_line_col(src, prev.start as usize);
                diags.push(Diag {
                    severity: Severity::Error,
                    path: path.to_string(),
                    line,
                    col,
                    message: format!(
                        "duplicate function `{name}` — also defined at line {pline}"
                    ),
                });
            }
        }
        if let Item::Import { path: mod_path, span, .. } = item {
            check_import(path, src, mod_path, *span, diags);
        }
    }

    // Unused functions (heuristic): defined once, identifier never appears elsewhere.
    for (name, span) in &fn_names {
        if name == "main" || name.starts_with('_') {
            continue;
        }
        if ident_count(src, name) <= 1 {
            let (line, col) = offset_to_line_col(src, span.start as usize);
            diags.push(Diag {
                severity: Severity::Warning,
                path: path.to_string(),
                line,
                col,
                message: format!(
                    "function `{name}` is never used — prefix with `_` if intentional"
                ),
            });
        }
    }

    // Brace balance (quick structural hint when the parser recovered).
    let (opens, closes) = brace_balance(src);
    if opens != closes {
        diags.push(Diag {
            severity: Severity::Error,
            path: path.to_string(),
            line: 1,
            col: 1,
            message: format!(
                "unbalanced braces: {opens} `{{` vs {closes} `}}`"
            ),
        });
    }
}

fn check_import(file: &str, src: &str, module: &str, span: Span, diags: &mut Vec<Diag>) {
    let module = module.trim().trim_matches('"');
    if module.is_empty() {
        return;
    }
    if let Some(warning) = mikasa_syntax::ext::stdlib_deprecation(module) {
        let (line, col) = offset_to_line_col(src, span.start as usize);
        diags.push(Diag {
            severity: Severity::Warning,
            path: file.to_string(),
            line,
            col,
            message: warning,
        });
    }
    if is_known_module(module, file) {
        return;
    }
    let (line, col) = offset_to_line_col(src, span.start as usize);
    diags.push(Diag {
        severity: Severity::Warning,
        path: file.to_string(),
        line,
        col,
        message: format!(
            "import `{module}` not found — not stdlib/cstd/mika, no sibling \
             `.mika`/`.crs`/`.rs`, and not in properties.kson. Try `mikasa install {module}`."
        ),
    });
}

fn is_known_module(module: &str, from_file: &str) -> bool {
    const STDLIB: &[&str] = &[
        "std", "core", "alloc", "crate", "super", "self", "mika", "cstd", "fs", "time",
        "url", "net", "ws", "json", "crypto", "http",
    ];
    if STDLIB.iter().any(|s| *s == module) {
        return true;
    }
    // Path-style / crate:: imports are left alone.
    if module.contains('/') || module.contains('\\') || module.contains("::") {
        return true;
    }
    let dir = Path::new(from_file).parent().unwrap_or_else(|| Path::new("."));
    for ext in ["mika", "crs", "rs", "mui"] {
        if dir.join(format!("{module}.{ext}")).is_file() {
            return true;
        }
    }
    // properties.kson deps (best-effort, no full parse).
    if let Some(root) = find_project_root(dir) {
        if let Ok(kson) = fs::read_to_string(root.join("properties.kson")) {
            // crude: look for the crate name as a key-ish token
            if kson.contains(module) {
                return true;
            }
        }
    }
    false
}

fn find_project_root(start: &Path) -> Option<PathBuf> {
    let mut cur = start.to_path_buf();
    loop {
        if cur.join("properties.kson").is_file() || cur.join("Cargo.toml").is_file() {
            return Some(cur);
        }
        if !cur.pop() {
            return None;
        }
    }
}

fn ident_count(src: &str, name: &str) -> usize {
    let bytes = src.as_bytes();
    let n = name.as_bytes();
    if n.is_empty() {
        return 0;
    }
    let mut count = 0usize;
    let mut i = 0usize;
    // Skip strings / comments roughly so "foo" in a string doesn't count.
    while i < bytes.len() {
        if bytes[i] == b'"' {
            i += 1;
            while i < bytes.len() {
                match bytes[i] {
                    b'\\' => i += 2,
                    b'"' => {
                        i += 1;
                        break;
                    }
                    _ => i += 1,
                }
            }
            continue;
        }
        if bytes[i] == b'/' && bytes.get(i + 1) == Some(&b'/') {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if bytes[i] == b'/' && bytes.get(i + 1) == Some(&b'*') {
            i += 2;
            while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                i += 1;
            }
            i = (i + 2).min(bytes.len());
            continue;
        }
        if i + n.len() <= bytes.len() && &bytes[i..i + n.len()] == n {
            let before_ok = i == 0 || !is_ident_byte(bytes[i - 1]);
            let after_ok = i + n.len() >= bytes.len() || !is_ident_byte(bytes[i + n.len()]);
            if before_ok && after_ok {
                count += 1;
            }
            i += n.len();
            continue;
        }
        i += 1;
    }
    count
}

fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

fn brace_balance(src: &str) -> (usize, usize) {
    let mut opens = 0usize;
    let mut closes = 0usize;
    let b = src.as_bytes();
    let mut i = 0usize;
    while i < b.len() {
        match b[i] {
            b'"' => {
                i += 1;
                while i < b.len() {
                    match b[i] {
                        b'\\' => i += 2,
                        b'"' => {
                            i += 1;
                            break;
                        }
                        _ => i += 1,
                    }
                }
            }
            b'/' if b.get(i + 1) == Some(&b'/') => {
                while i < b.len() && b[i] != b'\n' {
                    i += 1;
                }
            }
            b'/' if b.get(i + 1) == Some(&b'*') => {
                i += 2;
                while i + 1 < b.len() && !(b[i] == b'*' && b[i + 1] == b'/') {
                    i += 1;
                }
                i = (i + 2).min(b.len());
            }
            b'{' => {
                opens += 1;
                i += 1;
            }
            b'}' => {
                closes += 1;
                i += 1;
            }
            _ => i += 1,
        }
    }
    (opens, closes)
}

fn offset_to_line_col(src: &str, offset: usize) -> (usize, usize) {
    let offset = offset.min(src.len());
    let mut line = 1usize;
    let mut col = 1usize;
    for (i, ch) in src.char_indices() {
        if i >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    (line, col)
}

fn find_substr_line(src: &str, needle: &str) -> (usize, usize) {
    match src.find(needle) {
        Some(off) => offset_to_line_col(src, off),
        None => (1, 1),
    }
}

fn emit(d: &Diag) {
    let kind = match d.severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
    };
    eprintln!("{kind}: {}", d.message);
    eprintln!("  --> {}:{}:{}", d.path, d.line, d.col);
}

fn collect(path: &Path, out: &mut Vec<String>) {
    if path.is_file() {
        if let Some(s) = path.to_str() {
            if is_checkable(s) {
                out.push(s.to_string());
            }
        }
        return;
    }
    if path.is_dir() {
        for entry in walkdir::WalkDir::new(path)
            .follow_links(true)
            .into_iter()
            .filter_entry(|e| {
                !matches!(
                    e.file_name().to_str(),
                    Some("dist") | Some("target") | Some(".git") | Some("node_modules")
                )
            })
            .filter_map(Result::ok)
            .filter(|e| e.path().is_file())
        {
            if let Some(s) = entry.path().to_str() {
                if is_checkable(s) {
                    out.push(s.to_string());
                }
            }
        }
    }
}

fn is_checkable(path: &str) -> bool {
    let ext = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    mikasa_syntax::ext::is_source(&ext)
        || mikasa_syntax::ext::is_mui(&ext)
        || mikasa_syntax::ext::is_mui_legacy(&ext)
}

// ── Heavy path: transpile + rustc/Miri (`mikasa check --miri`) ───────────────

/// Directory of Astra's isolated rustup (does not touch the user's rustup).
fn astra_rustup_home() -> PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".astra").join("rustup")
}

fn rustup_available() -> bool {
    Command::new("rustup")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Ensures nightly + `miri` component in the isolated rustup. Idempotent: only
/// downloads on first run. Returns the toolchain name to use (`nightly`).
pub fn ensure_miri() -> Result<String, String> {
    if !rustup_available() {
        return Err(
            "the checker uses rustup (the standard Rust installer), which is not in PATH.\n\
             Install it once from https://rustup.rs and try again."
                .into(),
        );
    }
    let home = astra_rustup_home();
    let _ = std::fs::create_dir_all(&home);

    eprintln!("mikasa check --miri: ensuring verification toolchain (nightly + miri)…");
    let r = Command::new("rustup")
        .env("RUSTUP_HOME", &home)
        .args(["toolchain", "install", "nightly", "--profile", "minimal"])
        .status()
        .map_err(|e| format!("failed to run rustup: {e}"))?;
    if !r.success() {
        return Err("could not install the nightly toolchain".into());
    }
    let r = Command::new("rustup")
        .env("RUSTUP_HOME", &home)
        .args(["component", "add", "miri", "--toolchain", "nightly"])
        .status()
        .map_err(|e| format!("failed to add miri: {e}"))?;
    if !r.success() {
        return Err("could not add the miri component".into());
    }
    Ok("nightly".into())
}

/// Runs verification on the already-transpiled crate in `dist/rust/`.
/// `no_miri = true` runs only `cargo +nightly check` (type + borrow), without Miri.
pub fn run_miri_check(no_miri: bool) -> i32 {
    let toolchain = match ensure_miri() {
        Ok(t) => t,
        Err(e) => {
            eprintln!("mikasa check --miri: {e}");
            return 1;
        }
    };
    let home = astra_rustup_home();
    let mut cmd = Command::new("cargo");
    cmd.arg(format!("+{toolchain}"));
    if no_miri {
        cmd.arg("check");
    } else {
        cmd.arg("miri").arg("run");
    }
    cmd.current_dir("./dist/rust")
        .env("RUSTUP_HOME", &home)
        .stdin(std::process::Stdio::inherit())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit());
    match cmd.status() {
        Ok(s) if s.success() => {
            eprintln!("mikasa check --miri: OK — passed the Rust verifier");
            0
        }
        Ok(s) => s.code().unwrap_or(1),
        Err(e) => {
            eprintln!("mikasa check --miri: failed to run the verifier: {e}");
            1
        }
    }
}

/// Back-compat alias used by older call sites.
pub fn run_check(no_miri: bool) -> i32 {
    run_miri_check(no_miri)
}

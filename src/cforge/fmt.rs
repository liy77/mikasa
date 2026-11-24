//! `mikasa fmt` / `mikasa format` — pretty-print Mikasa + MocidaUI sources.
//!
//! * MUI (`.mui`, `.crm`, or a `.mika` with a `view`/`App()`) → [`mui_fmt`].
//! * Plain Mikasa (`.mika` / `.crs`) → brace-aware indent formatter that keeps
//!   strings and comments byte-for-byte.
//!
//! Idempotent: `fmt(fmt(x)) == fmt(x)`.

use std::fs;
use std::path::Path;

use crate::cforge::mui_fmt;
use crate::cforge::pretty;

const INDENT: &str = "    "; // 4 spaces — matches examples/mikasa

/// True for any path `mikasa fmt` knows how to rewrite.
pub fn is_formattable(path: &str) -> bool {
    if mui_fmt::is_formattable(path) {
        return true;
    }
    let ext = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    mikasa_syntax::ext::is_source(&ext)
}

/// Format a plain Mikasa source (not a MUI document).
pub fn format_mika(src: &str) -> String {
    let b = src.as_bytes();
    let mut out = String::with_capacity(src.len() + 16);
    let mut i = 0usize;
    let mut depth: i32 = 0;
    let mut at_line_start = true;
    let mut blank_run = 0usize;

    while i < b.len() {
        let c = b[i];

        // Line comments — copy through end of line.
        if c == b'/' && b.get(i + 1) == Some(&b'/') {
            if at_line_start {
                write_indent(&mut out, depth);
                at_line_start = false;
                blank_run = 0;
            }
            let start = i;
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
            out.push_str(&src[start..i]);
            continue;
        }

        // Block comments.
        if c == b'/' && b.get(i + 1) == Some(&b'*') {
            if at_line_start {
                write_indent(&mut out, depth);
                at_line_start = false;
                blank_run = 0;
            }
            let start = i;
            i += 2;
            while i + 1 < b.len() && !(b[i] == b'*' && b[i + 1] == b'/') {
                i += 1;
            }
            if i + 1 < b.len() {
                i += 2;
            }
            out.push_str(&src[start..i]);
            continue;
        }

        // Strings (keep escapes + `${…}` interpolation intact).
        if c == b'"' {
            if at_line_start {
                write_indent(&mut out, depth);
                at_line_start = false;
                blank_run = 0;
            }
            let start = i;
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
            out.push_str(&src[start..i.min(b.len())]);
            continue;
        }

        // Newlines — collapse 3+ blanks to 1 blank line; drop trailing spaces.
        if c == b'\n' || c == b'\r' {
            // Trim trailing whitespace on the current line.
            while out.ends_with(' ') || out.ends_with('\t') {
                out.pop();
            }
            if c == b'\r' {
                i += 1;
                if b.get(i) == Some(&b'\n') {
                    i += 1;
                }
            } else {
                i += 1;
            }
            if at_line_start {
                blank_run += 1;
                if blank_run <= 1 {
                    out.push('\n');
                }
            } else {
                out.push('\n');
                blank_run = 0;
            }
            at_line_start = true;
            continue;
        }

        // Skip leading whitespace on a line — we re-indent.
        if at_line_start && (c == b' ' || c == b'\t') {
            i += 1;
            continue;
        }

        // Closing brace decreases indent before emitting.
        if c == b'}' {
            depth = depth.saturating_sub(1);
            if at_line_start {
                write_indent(&mut out, depth);
                at_line_start = false;
                blank_run = 0;
            }
            out.push('}');
            i += 1;
            continue;
        }

        if at_line_start {
            write_indent(&mut out, depth);
            at_line_start = false;
            blank_run = 0;
        }

        out.push(c as char);
        i += 1;

        if c == b'{' {
            depth += 1;
        }
    }

    // Trim trailing blank lines; ensure exactly one trailing newline.
    while out.ends_with('\n') || out.ends_with(' ') || out.ends_with('\t') {
        out.pop();
    }
    if !out.is_empty() {
        out.push('\n');
    }
    out
}

fn write_indent(out: &mut String, depth: i32) {
    for _ in 0..depth.max(0) {
        out.push_str(INDENT);
    }
}

/// Format `src` with the right engine for `path`.
pub fn format_source(path: &str, src: &str) -> String {
    if crate::cforge::mui::is_mui_file(path) {
        mui_fmt::format_mui(src)
    } else {
        format_mika(src)
    }
}

/// `mikasa fmt <paths…>`: format each Mikasa / MocidaUI file (recurses dirs).
pub fn format_command(paths: &[String], check: bool, to_stdout: bool) -> i32 {
    let mut files: Vec<String> = Vec::new();
    for p in paths {
        collect(Path::new(p), &mut files);
    }
    if files.is_empty() {
        pretty::warn("no Mikasa / MocidaUI files found to format");
        return 0;
    }

    let mut changed = 0usize;
    let mut errors = 0usize;
    for f in &files {
        let src = match fs::read_to_string(f) {
            Ok(s) => s,
            Err(e) => {
                pretty::fail(&format!("{f}: {e}"));
                errors += 1;
                continue;
            }
        };
        let formatted = format_source(f, &src);

        if to_stdout {
            print!("{formatted}");
            continue;
        }
        if formatted == src {
            pretty::step(&format!("{f} — already formatted"));
            continue;
        }
        changed += 1;
        if check {
            pretty::warn(&format!("{f} — would reformat"));
        } else if let Err(e) = fs::write(f, &formatted) {
            pretty::fail(&format!("{f}: {e}"));
            errors += 1;
        } else {
            pretty::ok(&format!("formatted {f}"));
        }
    }

    if to_stdout {
        return 0;
    }
    if errors > 0 {
        return 1;
    }
    if check && changed > 0 {
        pretty::warn(&format!("{changed} file(s) need formatting"));
        return 1;
    }
    if !check {
        pretty::ok(&format!(
            "{} file(s) formatted, {} already clean",
            changed,
            files.len() - changed
        ));
    }
    0
}

fn collect(path: &Path, out: &mut Vec<String>) {
    if path.is_file() {
        if let Some(s) = path.to_str() {
            if is_formattable(s) {
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
                if is_formattable(s) {
                    out.push(s.to_string());
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::format_mika;

    #[test]
    fn indents_blocks() {
        let src = "func main() {\nprintln(\"hi\")\n}\n";
        let out = format_mika(src);
        assert_eq!(out, "func main() {\n    println(\"hi\")\n}\n");
    }

    #[test]
    fn idempotent() {
        let src = "func main() {\n    mut x = 1\n    if x > 0 {\n        println(\"${x}\")\n    }\n}\n";
        let once = format_mika(src);
        assert_eq!(once, format_mika(&once));
    }

    #[test]
    fn preserves_strings_and_comments() {
        let src = "// keep\nfunc main() {\n    println(\"a  b\") // trail\n}\n";
        let out = format_mika(src);
        assert!(out.contains("// keep"));
        assert!(out.contains("\"a  b\""));
        assert!(out.contains("// trail"));
    }
}

//! Copper parser: the transpiler's emit pass.
//!
//! Consumes the token stream produced by `mikasa-syntax`'s tokenizer and
//! walks it, emitting Rust source into a `result::Result` buffer. This is
//! the half of the pipeline `cforge` drives; the LSP uses `mikasa-syntax`'s
//! AST instead and never touches this crate.

// Pre-existing untouched code carries a backlog of stylistic clippy
// warnings. We silence the categories that aren't bugs so
// `cargo clippy -- -D warnings` stays green for CI. Tighten this list as
// the relevant code paths are rewritten.
#![allow(
    clippy::borrow_interior_mutable_const,
    clippy::collapsible_if,
    clippy::collapsible_match,
    clippy::declare_interior_mutable_const,
    clippy::doc_lazy_continuation,
    clippy::if_same_then_else,
    clippy::manual_strip,
    clippy::module_inception,
    clippy::needless_late_init,
    clippy::redundant_locals,
    clippy::to_string_trait_impl,
    clippy::unnecessary_unwrap,
    clippy::while_let_loop
)]

pub mod parser;

// Re-export at the crate root so `cforge` can write `mikasa_parser::Parser`
// and the parser's public surface stays flat.
pub use parser::Parser;

use mikasa_syntax::tokenizer::tokenizer::Tokenizer;
use std::path::{Path, PathBuf};

/// Tokenize + transpile Mikasa/Copper source to Rust.
pub fn transpile_source(src: &str) -> String {
    transpile_source_with_paths(src, None, None)
}

/// Like [`transpile_source`], with filesystem context for relative imports.
pub fn transpile_source_with_paths(
    src: &str,
    source_path: Option<PathBuf>,
    project_root: Option<PathBuf>,
) -> String {
    let mut tokenizer = Tokenizer::new(src.to_string());
    let tokens = tokenizer.tokenize();
    Parser::new(tokens)
        .with_paths(source_path, project_root)
        .parse()
}

/// Expand `import("…")`, transpile, and promote top-level items to `pub`
/// so the result can be pasted into a Rust module (used by `mika_import!`).
pub fn transpile_as_module(src: &str, source_path: &Path) -> Result<String, String> {
    let base = source_path.parent().unwrap_or_else(|| Path::new("."));
    let expanded = mikasa_syntax::file_import::expand_file_imports(src, base)?;
    let rust = transpile_source_with_paths(
        &expanded,
        Some(source_path.to_path_buf()),
        Some(base.to_path_buf()),
    );
    Ok(polish_for_include(&promote_top_level_pub(&rust)))
}

/// Prefix column-0 `fn`/`struct`/`enum`/`trait` with `pub` (skips `fn main`).
pub fn promote_top_level_pub(code: &str) -> String {
    let mut out = String::with_capacity(code.len() + 64);
    for line in code.lines() {
        let promote = (line.starts_with("fn ")
            || line.starts_with("struct ")
            || line.starts_with("enum ")
            || line.starts_with("trait "))
            && !line.starts_with("fn main(");
        if promote {
            out.push_str("pub ");
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

/// Make streaming-transpiler output safe to `include` into a Rust crate:
/// drop the synthetic empty `fn main`, and fix the newline-as-`,` quirk that
/// shows up on bare `return expr` / tail expressions.
fn polish_for_include(code: &str) -> String {
    let mut s = code.to_string();
    // `return 40+2,` / `40+2,` right before a closing brace → semicolon.
    s = s.replace(",\n}", ";\n}");
    s = s.replace(",\n }", ";\n}");
    s = s.replace(",\r\n}", ";\r\n}");
    strip_stub_main(&s)
}

fn strip_stub_main(code: &str) -> String {
    // Match a trailing (or lone) empty/stub `fn main() { … }` the streaming
    // emit always appends for script-style files — including the one-liner
    // `fn main() {}`.
    let mut lines: Vec<&str> = code.lines().collect();
    // Drop trailing blank lines first so the stub is last.
    while lines.last().is_some_and(|l| l.trim().is_empty()) {
        lines.pop();
    }
    if let Some(last) = lines.last() {
        let t = last.trim();
        if (t.starts_with("fn main(") || t.starts_with("pub fn main("))
            && t.ends_with('}')
            && t.chars().filter(|&c| c == '{').count() == 1
        {
            // Body between `{` and `}` is only whitespace / `;`.
            if let (Some(a), Some(b)) = (t.find('{'), t.rfind('}')) {
                let body = t[a + 1..b].trim();
                if body.is_empty() || body == ";" {
                    lines.pop();
                    while lines.last().is_some_and(|l| l.trim().is_empty()) {
                        lines.pop();
                    }
                    return lines.join("\n") + if code.ends_with('\n') { "\n" } else { "" };
                }
            }
        }
    }
    // Multi-line stub: walk from the end for `}` then matching `fn main()`.
    let mut i = lines.len();
    while i > 0 {
        i -= 1;
        let t = lines[i].trim();
        if t.is_empty() {
            continue;
        }
        if t == "}" {
            let mut j = i;
            let mut body_ok = true;
            while j > 0 {
                j -= 1;
                let u = lines[j].trim();
                if u.starts_with("fn main(") || u.starts_with("pub fn main(") {
                    if body_ok {
                        lines.drain(j..=i);
                    }
                    return lines.join("\n") + if code.ends_with('\n') { "\n" } else { "" };
                }
                if u.is_empty() || u == ";" || u == "{" {
                    continue;
                }
                body_ok = false;
            }
            break;
        }
        break;
    }
    code.to_string()
}

#[cfg(test)]
mod include_polish_tests {
    #[test]
    fn polishes_return_comma_and_strips_main() {
        let raw = "fn answer() -> i64{\n return 40+2,\n}\n\nfn main() {\n;\n}\n";
        let out = super::polish_for_include(&super::promote_top_level_pub(raw));
        assert!(out.contains("return 40+2;"), "{out}");
        assert!(!out.contains("fn main"), "{out}");
        assert!(out.contains("pub fn answer"), "{out}");
    }

    #[test]
    fn strips_oneline_empty_main() {
        let raw = "pub fn answer() -> i64 {\n    return 40 + 2;\n}\n\nfn main() {}\n";
        let out = super::polish_for_include(raw);
        assert!(!out.contains("fn main"), "{out}");
        assert!(out.contains("pub fn answer"), "{out}");
    }
}

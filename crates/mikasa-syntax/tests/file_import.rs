//! Expand / resolve tests for `import("file.mika")`.

use mikasa_syntax::file_import::{expand_file_imports, resolve_import_path};
use mikasa_syntax::program::{parse_program, ImportKind, Item};
use std::io::Write;

fn tmp_dir(label: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "mikasa-file-import-{}-{}",
        label,
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn parse_file_import_item() {
    let p = parse_program("import(\"util.mika\")\nfunc main() {}\n");
    assert!(p.errors.is_empty(), "{:?}", p.errors);
    assert!(matches!(
        &p.items[0],
        Item::Import {
            kind: ImportKind::File,
            path,
            ..
        } if path == "util.mika"
    ));
}

#[test]
fn resolve_prefers_mika_extension() {
    let dir = tmp_dir("resolve");
    std::fs::write(dir.join("util.mika"), "func f() {}\n").unwrap();
    let p = resolve_import_path(&dir, "util").unwrap();
    assert_eq!(p.extension().and_then(|e| e.to_str()), Some("mika"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn expand_nested_and_rejects_cycle() {
    let dir = tmp_dir("expand");
    std::fs::write(dir.join("leaf.mika"), "func leaf() {}\n").unwrap();
    std::fs::write(
        dir.join("mid.mika"),
        "import(\"leaf.mika\")\nfunc mid() {}\n",
    )
    .unwrap();
    let top = "import(\"mid.mika\")\nfunc main() {}\n";
    let expanded = expand_file_imports(top, &dir).unwrap();
    assert!(expanded.contains("func leaf()"));
    assert!(expanded.contains("func mid()"));
    assert!(!expanded.contains("import(\"mid.mika\")"));

    let mut a = std::fs::File::create(dir.join("a.mika")).unwrap();
    writeln!(a, "import(\"b.mika\")").unwrap();
    let mut b = std::fs::File::create(dir.join("b.mika")).unwrap();
    writeln!(b, "import(\"a.mika\")").unwrap();
    let err = expand_file_imports("import(\"a.mika\")\n", &dir).unwrap_err();
    assert!(err.contains("cycle"), "{err}");
    let _ = std::fs::remove_dir_all(&dir);
}

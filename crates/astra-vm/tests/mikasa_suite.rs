//! Runs every `tests/mikasa/*.mika` language test through `astra run`.

use std::path::PathBuf;
use std::process::Command;

fn suite_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/mikasa")
        .canonicalize()
        .expect("tests/mikasa")
}

fn astra_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_astra"))
}

#[test]
fn all_mikasa_language_tests() {
    let dir = suite_dir();
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .expect("read tests/mikasa")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.extension().and_then(|e| e.to_str()) == Some("mika")
                && p.file_name().and_then(|n| n.to_str()) != Some("harness.mika")
        })
        .collect();
    files.sort();
    assert!(
        !files.is_empty(),
        "expected language tests under {}",
        dir.display()
    );

    let mut failed = Vec::new();
    for file in &files {
        let out = Command::new(astra_bin())
            .arg("run")
            .arg(file)
            .output()
            .unwrap_or_else(|e| panic!("spawn astra: {e}"));
        if !out.status.success() {
            failed.push(format!(
                "{}:\nstdout:\n{}\nstderr:\n{}",
                file.display(),
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            ));
        }
    }
    assert!(
        failed.is_empty(),
        "{} language test(s) failed:\n\n{}",
        failed.len(),
        failed.join("\n\n")
    );
}

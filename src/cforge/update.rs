//! `mikasa update` / `mikasa upgrade` — replace this install with the latest
//! GitHub release. Uses curl only — no cargo/rustc required.

use std::path::{Path, PathBuf};
use std::process::Command;

const REPO: &str = "liy77/copper-lang";

pub fn run() -> i32 {
    let current = format!(
        "{} ({} {})",
        env!("CFORGE_VERSION"),
        env!("COPPER_BUILD_DATE"),
        env!("GIT_COMMIT_HASH")
    );
    println!("mikasa {current}");
    println!("checking https://github.com/{REPO}/releases …");

    let body = match curl_get(&format!(
        "https://api.github.com/repos/{REPO}/releases?per_page=20"
    )) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("could not reach GitHub: {e}");
            return 1;
        }
    };
    let Some((tag, url)) = pick_release(&body) else {
        eprintln!("no Mikasa release for this platform yet.");
        eprintln!("looked for an asset named with {}", platform_key());
        return 1;
    };
    println!("release {tag}");
    println!("downloading {url}");

    let exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("could not find this binary: {e}");
            return 1;
        }
    };
    let tmp = exe.with_extension("new");
    if let Err(e) = curl_download(&url, &tmp) {
        eprintln!("download failed: {e}");
        let _ = std::fs::remove_file(&tmp);
        return 1;
    }
    if let Err(e) = swap_in(&tmp, &exe) {
        eprintln!("could not replace {}: {e}", exe.display());
        return 1;
    }
    // Keep `mikasa` and legacy `cforge` names in sync in the same bin dir.
    sync_twin_names(&exe);
    println!("updated {}", exe.display());
    println!("open a new terminal and run `mikasa -v`");
    0
}

/// After replacing `mikasa` (or legacy `cforge`), mirror the other name so
/// both commands stay on PATH without a second download.
fn sync_twin_names(exe: &Path) {
    let stem = exe
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let twin_name = if stem == "mikasa" {
        if cfg!(windows) {
            "cforge.exe"
        } else {
            "cforge"
        }
    } else if stem == "cforge" {
        if cfg!(windows) {
            "mikasa.exe"
        } else {
            "mikasa"
        }
    } else {
        return;
    };
    let twin = exe.with_file_name(twin_name);
    if twin == exe {
        return;
    }
    match std::fs::copy(exe, &twin) {
        Ok(_) => {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(&twin, std::fs::Permissions::from_mode(0o755));
            }
            println!("also updated {}", twin.display());
        }
        Err(e) => eprintln!("note: could not sync {}: {e}", twin.display()),
    }
}

fn platform_key() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "apple"
    } else {
        "linux"
    }
}

/// Prefer a `nightly*` tag. Fall back to the first release that has an asset
/// for this OS.
fn pick_release(json: &str) -> Option<(String, String)> {
    let mut best_nightly: Option<(String, String)> = None;
    let mut best_any: Option<(String, String)> = None;
    for (tag, url) in release_assets(json) {
        if !url_matches_platform(&url) {
            continue;
        }
        if best_any.is_none() {
            best_any = Some((tag.clone(), url.clone()));
        }
        if tag.starts_with("nightly") && best_nightly.is_none() {
            best_nightly = Some((tag, url));
        }
    }
    best_nightly.or(best_any)
}

fn url_matches_platform(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    if cfg!(windows) {
        return lower.contains("windows") || lower.ends_with(".exe");
    }
    if cfg!(target_os = "macos") {
        let mac = lower.contains("macos") || lower.contains("apple") || lower.contains("darwin");
        if !mac {
            return false;
        }
        return if cfg!(target_arch = "aarch64") {
            lower.contains("arm64") || lower.contains("aarch64") || lower.contains("apple")
        } else {
            lower.contains("x86_64") || lower.contains("intel")
        };
    }
    lower.contains("linux")
}

fn release_assets(json: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut rest = json;
    while let Some(i) = rest.find("\"tag_name\"") {
        rest = &rest[i + 10..];
        let Some(tag) = next_string(rest) else { break };
        let window = rest.get(..8000).unwrap_or(rest);
        if let Some(url) = next_download(window) {
            out.push((tag, url));
        }
    }
    out
}

fn next_download(json: &str) -> Option<String> {
    let i = json.find("\"browser_download_url\"")?;
    next_string(&json[i + 22..])
}

fn next_string(after_key: &str) -> Option<String> {
    let colon = after_key.find(':')?;
    let after = after_key[colon + 1..].trim_start();
    let after = after.strip_prefix('"')?;
    let end = after.find('"')?;
    Some(after[..end].replace("\\u0026", "&"))
}

fn curl_get(url: &str) -> Result<String, String> {
    let out = Command::new("curl")
        .args([
            "-fsSL",
            "-H",
            "User-Agent: mikasa-update",
            "-H",
            "Accept: application/vnd.github+json",
            url,
        ])
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).into_owned());
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn curl_download(url: &str, dest: &Path) -> Result<(), String> {
    if let Some(parent) = dest.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let out = Command::new("curl")
        .args([
            "-fL",
            "--no-cache",
            "-H",
            "User-Agent: mikasa-update",
            "-H",
            "Accept: application/octet-stream",
            "-o",
        ])
        .arg(dest)
        .arg(url)
        .status()
        .map_err(|e| e.to_string())?;
    if !out.success() {
        return Err("curl exited with an error".into());
    }
    Ok(())
}

fn swap_in(downloaded: &Path, exe: &PathBuf) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(downloaded, std::fs::Permissions::from_mode(0o755))
            .map_err(|e| e.to_string())?;
        std::fs::rename(downloaded, exe).map_err(|e| e.to_string())?;
    }
    #[cfg(windows)]
    {
        let old = exe.with_extension("old");
        let _ = std::fs::remove_file(&old);
        std::fs::rename(exe, &old).map_err(|e| e.to_string())?;
        if let Err(e) = std::fs::rename(downloaded, exe) {
            let _ = std::fs::rename(&old, exe);
            return Err(e.to_string());
        }
    }
    Ok(())
}

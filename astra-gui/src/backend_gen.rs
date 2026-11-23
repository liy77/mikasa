// Generated from backend.mika — do not edit by hand.
// Change backend.mika and regenerate (see build.rs / README).

pub fn current_version() -> String {
    "0.1.0".to_string()
}

pub fn default_source() -> String {
    "func main() {\n    println(\"hello from Astra\")\n}\n".to_string()
}

pub fn ready_message() -> String {
    "ready — write Mikasa and hit Run".to_string()
}

pub fn running_status() -> String {
    "running…".to_string()
}

pub fn format_run_output(output: &str, error: &str) -> String {
    if error.is_empty() {
        if output.is_empty() {
            return "(no output)".to_string();
        }
        return output.to_string();
    }
    if output.is_empty() {
        return error.to_string();
    }
    format!("{output}{error}")
}

pub fn format_run_status(failed: bool) -> String {
    if failed {
        "failed".to_string()
    } else {
        "ran".to_string()
    }
}

pub fn is_newer(latest: &str, current: &str) -> bool {
    let a = version_nums(latest);
    let b = version_nums(current);
    let mut i = 0;
    while i < 3 {
        if a[i] != b[i] {
            return a[i] > b[i];
        }
        i += 1;
    }
    false
}

fn version_nums(v: &str) -> Vec<i64> {
    let clean = v.trim_start_matches('v');
    let mut out = vec![0, 0, 0];
    let mut i = 0;
    let mut part = String::new();
    for ch in clean.chars() {
        if ch.is_ascii_digit() {
            part.push(ch);
        } else {
            if i < 3 && !part.is_empty() {
                out[i] = part.parse::<i64>().unwrap_or(0);
                i += 1;
            }
            if ch != '.' {
                break;
            }
            part = String::new();
        }
    }
    if i < 3 && !part.is_empty() {
        out[i] = part.parse::<i64>().unwrap_or(0);
    }
    out
}

pub fn json_string(text: &str, field: &str) -> String {
    let needle = format!("\"{field}\"");
    let Some(at) = text.find(&needle) else {
        return String::new();
    };
    let rest = &text[at + needle.len()..];
    let Some(colon) = rest.find(':') else {
        return String::new();
    };
    let after = rest[colon + 1..].trim_start();
    if !after.starts_with('"') {
        return String::new();
    }
    let body = &after[1..];
    let Some(end) = body.find('"') else {
        return String::new();
    };
    body[..end].to_string()
}

pub fn pick_asset(text: &str, os_key: &str) -> String {
    let needle = "\"browser_download_url\"";
    let mut from = 0;
    while from < text.len() {
        let slice = &text[from..];
        let Some(rel) = slice.find(needle) else {
            break;
        };
        let url = json_string(&text[from + rel..], "browser_download_url");
        if url.contains(os_key) {
            return url;
        }
        if os_key == "apple" && url.contains("darwin") {
            return url;
        }
        from = from + rel + needle.len();
    }
    String::new()
}

pub fn describe_update(release_json: &str, current: &str, os_key: &str) -> String {
    let latest = json_string(release_json, "tag_name");
    let asset = pick_asset(release_json, os_key);
    let available = if is_newer(&latest, current) {
        "yes"
    } else {
        "no"
    };
    format!("{current}|{latest}|{available}|{asset}")
}

pub fn update_checking() -> String {
    "Checking GitHub for a newer build…".to_string()
}

pub fn update_applied(latest: &str) -> String {
    format!("Updated to {latest} — restart Astra.")
}

pub fn update_failed(err: &str) -> String {
    format!("Update failed: {err}")
}

pub fn update_current(latest: &str) -> String {
    format!("Already up to date ({latest}).")
}

pub fn update_blurb() -> String {
    "Astra checks GitHub releases for a newer nightly build.".to_string()
}

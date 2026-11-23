// Generated from backend.mika by `mikasa build`. Do not edit by hand;
// change backend.mika and regenerate.

pub fn current_version() -> String {
    return "0.1.0".to_string();
}
pub fn is_newer(latest: &str, current: &str) -> bool {
    let mut a = version_nums(&latest);
    let mut b = version_nums(&current);
    let mut i = 0;
    while i < 3 {
        if a[i] != b[i] {
            return a[i] > b[i];
        }
        i = i + 1;
    }
    return false;
}
fn version_nums(v: &str) -> Vec<i64> {
    let mut clean = v.trim_start_matches('v');
    let mut out = vec![0, 0, 0];
    let mut i = 0;
    let mut part = String::new();
    for ch in clean.chars() {
        if ch.is_ascii_digit() {
            part.push(ch);
        } else {
            if i < 3 && !part.is_empty() {
                out[i] = part.parse::<i64>().unwrap_or(0);
                i = i + 1;
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
    return out;
}
pub fn json_string(text: &str, field: &str) -> String {
    let mut needle = format!("\"{}\"", field);
    let mut at = text.find(&needle);
    if at.is_none() {
        return String::new();
    }
    let mut rest = &text[at.unwrap() + needle.len()..];
    let mut colon = rest.find(':');
    if colon.is_none() {
        return String::new();
    }
    let mut after = rest[colon.unwrap() + 1..].trim_start();
    if !after.starts_with('"') {
        return String::new();
    }
    let mut body = &after[1..];
    let mut end = body.find('"');
    if end.is_none() {
        return String::new();
    }
    return body[..end.unwrap()].to_string();
}
pub fn pick_asset(text: &str, os_key: &str) -> String {
    let mut needle = "\"browser_download_url\"";
    let mut from = 0;
    while from < text.len() {
        let mut slice = &text[from..];
        let mut rel = slice.find(needle);
        if rel.is_none() {
            break;
        }
        let mut url = json_string(&text[from + rel.unwrap()..], "browser_download_url");
        if url.contains(os_key) {
            return url;
        }
        if os_key == "apple" && url.contains("darwin") {
            return url;
        }
        from = from + rel.unwrap() + needle.len();
    }
    return String::new();
}
pub fn describe_update(release_json: &str, current: &str, os_key: &str) -> String {
    let mut latest = json_string(&release_json, "tag_name");
    let mut asset = pick_asset(&release_json, &os_key);
    let mut available = "no";
    if is_newer(&latest, &current) {
        available = "yes";
    }
    return format!("{}|{}|{}|{}", current, latest, available, asset);
}

//! PhantomData sugar and unused-generic hints.
//!
//! Inside a `struct` / `class` body:
//! ```text
//! phantom T
//! phantom T, U
//! ```
//! lowers to `_phantom_T: ::core::marker::PhantomData<T>` (etc.).
//!
//! [`collect_hints`] finds type parameters that never appear in fields and
//! aren't already marked — mikasa prints those when compiling / running.

use mikasa_syntax::tokenizer::{
    kind::TokenKind,
    tokens::{Data, Token},
};
use std::collections::HashSet;

fn synth(kind: TokenKind, value: &str) -> Token {
    Token::new(kind, value.to_string(), value.len(), Data::None, true)
}

fn is_trivia(t: &Token) -> bool {
    matches!(
        t.kind,
        TokenKind::Whitespace | TokenKind::Comment | TokenKind::DocComment
    )
}

fn prev_sig(tokens: &[Token], mut idx: usize) -> Option<usize> {
    while idx > 0 {
        idx -= 1;
        if !is_trivia(&tokens[idx]) && tokens[idx].kind != TokenKind::Newline {
            return Some(idx);
        }
    }
    None
}

fn next_sig(tokens: &[Token], mut idx: usize) -> Option<usize> {
    while idx < tokens.len() {
        if !is_trivia(&tokens[idx]) && tokens[idx].kind != TokenKind::Newline {
            return Some(idx);
        }
        idx += 1;
    }
    None
}

fn find_matching_brace(tokens: &[Token], open: usize) -> Option<usize> {
    let mut depth = 0i32;
    for (i, t) in tokens.iter().enumerate().skip(open) {
        match t.value.as_str() {
            "{" => depth += 1,
            "}" => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

fn find_matching_angle(tokens: &[Token], open: usize) -> Option<usize> {
    let mut depth = 0i32;
    for (i, t) in tokens.iter().enumerate().skip(open) {
        match t.value.as_str() {
            "<" => depth += 1,
            ">" => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            "{" | ";" if depth == 1 => return None,
            _ => {}
        }
    }
    None
}

fn at_field_start(tokens: &[Token], idx: usize) -> bool {
    match prev_sig(tokens, idx) {
        None => true,
        Some(p) => {
            matches!(tokens[p].value.as_str(), "{" | "," | ";")
                || tokens[p].kind == TokenKind::Newline
                || tokens[p].kind == TokenKind::Semicolon
        }
    }
}

/// Lower `phantom T` / `phantom T, U` field markers inside struct/class bodies,
/// and fill missing `_phantom_*` fields in `Type { … }` / `Type::<…> { … }`
/// literals so source never needs to mention `PhantomData`.
pub(super) fn rewrite(tokens: Vec<Token>) -> Vec<Token> {
    let mut phantoms: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    let mut out = Vec::with_capacity(tokens.len() + 16);
    let mut i = 0usize;
    while i < tokens.len() {
        let is_type_item = matches!(tokens[i].kind, TokenKind::Struct)
            || (tokens[i].kind == TokenKind::Keyword && tokens[i].value == "class");
        if !is_type_item {
            out.push(tokens[i].clone());
            i += 1;
            continue;
        }

        let type_name = next_sig(&tokens, i + 1)
            .filter(|&n| tokens[n].kind == TokenKind::Identifier)
            .map(|n| tokens[n].value.clone());

        let mut j = i;
        let mut open_brace = None;
        while j < tokens.len() {
            if tokens[j].value == "{" {
                open_brace = Some(j);
                break;
            }
            if tokens[j].kind == TokenKind::Semicolon {
                break;
            }
            j += 1;
        }
        let Some(ob) = open_brace else {
            out.push(tokens[i].clone());
            i += 1;
            continue;
        };
        let Some(cb) = find_matching_brace(&tokens, ob) else {
            out.push(tokens[i].clone());
            i += 1;
            continue;
        };

        out.extend_from_slice(&tokens[i..=ob]);
        let (body, marked) = rewrite_body_phantoms(&tokens[ob + 1..cb]);
        if let Some(name) = type_name {
            if !marked.is_empty() {
                phantoms.insert(name, marked);
            }
        }
        out.extend(body);
        out.push(tokens[cb].clone());
        i = cb + 1;
    }
    if phantoms.is_empty() {
        return out;
    }
    inject_phantom_in_literals(out, &phantoms)
}

fn rewrite_body_phantoms(body: &[Token]) -> (Vec<Token>, Vec<String>) {
    let mut out = Vec::with_capacity(body.len() + 8);
    let mut marked = Vec::new();
    let mut i = 0usize;
    while i < body.len() {
        if body[i].value == "phantom"
            && matches!(
                body[i].kind,
                TokenKind::Identifier | TokenKind::Keyword | TokenKind::ModuleVar
            )
            && at_field_start(body, i)
        {
            if let Some((params, end)) = parse_phantom_list(body, i + 1) {
                if !params.is_empty() {
                    for (pi, name) in params.iter().enumerate() {
                        if pi > 0 {
                            out.push(synth(TokenKind::Comma, ","));
                            out.push(synth(TokenKind::Newline, "\n"));
                        }
                        out.extend(phantom_field_tokens(name));
                        marked.push(name.clone());
                    }
                    let mut k = end;
                    while k < body.len()
                        && (is_trivia(&body[k])
                            || body[k].kind == TokenKind::Newline
                            || body[k].kind == TokenKind::Semicolon
                            || body[k].value == ";")
                    {
                        k += 1;
                    }
                    if end < body.len() && body[end].kind == TokenKind::Newline {
                        out.push(body[end].clone());
                    } else {
                        out.push(synth(TokenKind::Newline, "\n"));
                    }
                    i = k;
                    continue;
                }
            }
        }
        out.push(body[i].clone());
        i += 1;
    }
    (out, marked)
}

fn inject_phantom_in_literals(
    tokens: Vec<Token>,
    phantoms: &std::collections::HashMap<String, Vec<String>>,
) -> Vec<Token> {
    let mut out = Vec::with_capacity(tokens.len() + 8);
    let mut i = 0usize;
    while i < tokens.len() {
        if tokens[i].kind == TokenKind::Identifier {
            if let Some(params) = phantoms.get(&tokens[i].value) {
                if !is_type_definition_name(&tokens, i) {
                    if let Some((brace, close)) = struct_lit_span(&tokens, i) {
                        out.extend_from_slice(&tokens[i..=brace]);
                        let inner = &tokens[brace + 1..close];
                        let existing = field_names_in_lit(inner);
                        let mut has_fields = inner.iter().any(|t| {
                            !is_trivia(t) && t.kind != TokenKind::Newline && t.value != ","
                        });
                        out.extend_from_slice(inner);
                        for p in params {
                            let fname = format!("_phantom_{}", p.to_ascii_lowercase());
                            if existing.contains(&fname) {
                                continue;
                            }
                            if has_fields {
                                out.push(synth(TokenKind::Comma, ","));
                            }
                            has_fields = true;
                            out.push(synth(TokenKind::Identifier, &fname));
                            out.push(synth(TokenKind::Operator, ":"));
                            out.push(synth(TokenKind::Identifier, "PhantomData"));
                        }
                        out.push(tokens[close].clone());
                        i = close + 1;
                        continue;
                    }
                }
            }
        }
        out.push(tokens[i].clone());
        i += 1;
    }
    out
}

fn is_type_definition_name(tokens: &[Token], name_idx: usize) -> bool {
    match prev_sig(tokens, name_idx) {
        Some(p) => {
            matches!(tokens[p].kind, TokenKind::Struct)
                || (tokens[p].kind == TokenKind::Keyword
                    && matches!(
                        tokens[p].value.as_str(),
                        "class" | "codable" | "actor" | "pub" | "public"
                    ))
        }
        None => false,
    }
}

/// `Name {` or `Name::<…> {` / `Name<…> {` starting at the type identifier.
fn struct_lit_span(tokens: &[Token], name_idx: usize) -> Option<(usize, usize)> {
    let mut b = next_sig(tokens, name_idx + 1)?;
    if tokens[b].value == "::" {
        let n = next_sig(tokens, b + 1)?;
        if tokens[n].value != "<" {
            return None;
        }
        let gt = find_matching_angle(tokens, n)?;
        b = next_sig(tokens, gt + 1)?;
    } else if tokens[b].value == "<" {
        let gt = find_matching_angle(tokens, b)?;
        b = next_sig(tokens, gt + 1)?;
    }
    if tokens[b].value != "{" {
        return None;
    }
    let close = find_matching_brace(tokens, b)?;
    Some((b, close))
}

fn field_names_in_lit(inner: &[Token]) -> std::collections::HashSet<String> {
    let mut set = std::collections::HashSet::new();
    let mut i = 0usize;
    while i < inner.len() {
        if inner[i].kind == TokenKind::Identifier {
            if let Some(n) = next_sig(inner, i + 1) {
                if inner[n].value == ":" {
                    set.insert(inner[i].value.clone());
                }
            }
        }
        i += 1;
    }
    set
}

fn parse_phantom_list(tokens: &[Token], start: usize) -> Option<(Vec<String>, usize)> {
    let mut names = Vec::new();
    let mut i = start;
    loop {
        let Some(n) = next_sig(tokens, i) else {
            break;
        };
        if tokens[n].kind != TokenKind::Identifier {
            break;
        }
        // `phantom: Type` is a normal field name, not our sugar.
        if names.is_empty() {
            if let Some(after) = next_sig(tokens, n + 1) {
                if tokens[after].value == ":" {
                    return None;
                }
            }
        }
        names.push(tokens[n].value.clone());
        i = n + 1;
        let Some(sep) = next_sig(tokens, i) else {
            return Some((names, i));
        };
        if tokens[sep].value == "," {
            i = sep + 1;
            continue;
        }
        return Some((names, sep));
    }
    if names.is_empty() {
        None
    } else {
        Some((names, i))
    }
}

fn phantom_field_tokens(type_name: &str) -> Vec<Token> {
    // Bare `PhantomData<T>` — leading `::path::` breaks in struct fields
    // (newlines/`::` are split into fake fields). The parser prelude injects
    // `use std::marker::PhantomData`.
    vec![
        // Lowercase keeps rustc `non_snake_case` quiet while staying unique per param.
        synth(
            TokenKind::Identifier,
            &format!("_phantom_{}", type_name.to_ascii_lowercase()),
        ),
        synth(TokenKind::Operator, ":"),
        synth(TokenKind::Identifier, "PhantomData"),
        synth(TokenKind::Operator, "<"),
        synth(TokenKind::Identifier, type_name),
        synth(TokenKind::Operator, ">"),
    ]
}

/// Scan struct/class generics for unused type parameters that should be marked.
pub(super) fn collect_hints(tokens: &[Token]) -> Vec<String> {
    let mut hints = Vec::new();
    let mut i = 0usize;
    while i < tokens.len() {
        let is_type_item = matches!(tokens[i].kind, TokenKind::Struct)
            || (tokens[i].kind == TokenKind::Keyword && tokens[i].value == "class");
        if !is_type_item {
            i += 1;
            continue;
        }

        let Some(name_idx) = next_sig(tokens, i + 1) else {
            i += 1;
            continue;
        };
        if tokens[name_idx].kind != TokenKind::Identifier {
            i += 1;
            continue;
        }
        let type_name = tokens[name_idx].value.clone();

        let mut j = name_idx + 1;
        let mut angle_open = None;
        while j < tokens.len() {
            if tokens[j].value == "<" {
                angle_open = Some(j);
                break;
            }
            if tokens[j].value == "{" || tokens[j].kind == TokenKind::Semicolon {
                break;
            }
            if !is_trivia(&tokens[j]) && tokens[j].kind != TokenKind::Newline {
                break;
            }
            j += 1;
        }

        let Some(ao) = angle_open else {
            i += 1;
            continue;
        };
        let Some(ac) = find_matching_angle(tokens, ao) else {
            i += 1;
            continue;
        };
        let params = parse_type_params(&tokens[ao + 1..ac]);
        if params.is_empty() {
            i = ac + 1;
            continue;
        }

        let mut k = ac + 1;
        while k < tokens.len() && (is_trivia(&tokens[k]) || tokens[k].kind == TokenKind::Newline) {
            k += 1;
        }
        if k >= tokens.len() || tokens[k].value != "{" {
            i = k;
            continue;
        }
        let Some(cb) = find_matching_brace(tokens, k) else {
            i = k + 1;
            continue;
        };
        let body = &tokens[k + 1..cb];
        let marked = marked_phantom_params(body);
        let body_text = body_type_text(body);

        for p in &params {
            if marked.contains(p) {
                continue;
            }
            if type_param_used_in_body(p, &body_text) {
                continue;
            }
            hints.push(format!(
                "type `{type_name}<…>`: parameter `{p}` is unused — mark with `phantom {p}` inside the body (or use `{p}` in a field)"
            ));
        }
        i = cb + 1;
    }
    hints
}

fn parse_type_params(inner: &[Token]) -> Vec<String> {
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut i = 0usize;
    while i < inner.len() {
        let t = &inner[i];
        if is_trivia(t) || t.kind == TokenKind::Newline {
            i += 1;
            continue;
        }
        match t.value.as_str() {
            "<" | "(" | "[" => {
                depth += 1;
                i += 1;
            }
            ">" | ")" | "]" => {
                depth = (depth - 1).max(0);
                i += 1;
            }
            "," if depth == 0 => {
                i += 1;
            }
            _ if depth == 0 && t.value.starts_with('\'') => {
                i += 1;
                while i < inner.len() {
                    if inner[i].value == "," && depth == 0 {
                        break;
                    }
                    match inner[i].value.as_str() {
                        "<" | "(" => depth += 1,
                        ">" | ")" => depth = (depth - 1).max(0),
                        _ => {}
                    }
                    i += 1;
                }
            }
            _ if depth == 0 && t.kind == TokenKind::Identifier => {
                out.push(t.value.clone());
                i += 1;
                while i < inner.len() {
                    if inner[i].value == "," && depth == 0 {
                        break;
                    }
                    match inner[i].value.as_str() {
                        "<" | "(" | "[" => depth += 1,
                        ">" | ")" | "]" => depth = (depth - 1).max(0),
                        _ => {}
                    }
                    i += 1;
                }
            }
            _ => i += 1,
        }
    }
    out
}

fn marked_phantom_params(body: &[Token]) -> HashSet<String> {
    let mut set = HashSet::new();
    let mut i = 0usize;
    while i < body.len() {
        if body[i].value == "phantom"
            && at_field_start(body, i)
            && matches!(
                body[i].kind,
                TokenKind::Identifier | TokenKind::Keyword | TokenKind::ModuleVar
            )
        {
            if let Some((names, end)) = parse_phantom_list(body, i + 1) {
                for n in names {
                    set.insert(n);
                }
                i = end;
                continue;
            }
        }
        if body[i].value == "PhantomData" {
            if let Some(lt) = next_sig(body, i + 1) {
                if body[lt].value == "<" {
                    if let Some(gt) = find_matching_angle(body, lt) {
                        for t in &body[lt + 1..gt] {
                            if t.kind == TokenKind::Identifier {
                                set.insert(t.value.clone());
                            }
                        }
                        i = gt + 1;
                        continue;
                    }
                }
            }
        }
        i += 1;
    }
    set
}

fn body_type_text(body: &[Token]) -> String {
    let mut s = String::new();
    let mut i = 0usize;
    while i < body.len() {
        if is_trivia(&body[i]) {
            i += 1;
            continue;
        }
        // Skip `phantom T, U` sugar lines so marking alone suppresses the hint.
        if body[i].value == "phantom" && at_field_start(body, i) {
            if let Some((_names, end)) = parse_phantom_list(body, i + 1) {
                i = end;
                continue;
            }
        }
        s.push(' ');
        s.push_str(&body[i].value);
        i += 1;
    }
    s
}

fn type_param_used_in_body(param: &str, body_text: &str) -> bool {
    let bytes = body_text.as_bytes();
    let needle = param.as_bytes();
    let mut i = 0usize;
    while i + needle.len() <= bytes.len() {
        if &bytes[i..i + needle.len()] == needle {
            let before_ok = i == 0 || !is_ident_byte(bytes[i - 1]);
            let after = i + needle.len();
            let after_ok = after >= bytes.len() || !is_ident_byte(bytes[after]);
            if before_ok && after_ok {
                return true;
            }
        }
        i += 1;
    }
    false
}

fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

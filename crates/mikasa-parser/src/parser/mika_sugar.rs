//! Mikasa sugar: key paths, result builders, property-wrapper fields.
//!
//! Runs inside [`super::sugar::rewrite`] before `defer`, so deferred bodies
//! already see the lowered forms.

use mikasa_syntax::tokenizer::{
    kind::TokenKind,
    tokens::{Data, Token},
};

fn synth(kind: TokenKind, value: &str) -> Token {
    Token::new(kind, value.to_string(), value.len(), Data::None, true)
}

fn is_trivia(t: &Token) -> bool {
    matches!(
        t.kind,
        TokenKind::Whitespace | TokenKind::Comment | TokenKind::DocComment
    )
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
    let mut depth = 0isize;
    for i in open..tokens.len() {
        match tokens[i].kind {
            TokenKind::BraceStart => depth += 1,
            TokenKind::BraceEnd => {
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

/// `\Type.field` / `\Type.a.b` → `__MikaKeyPath { get: |__r| &__r.a.b, get_mut: … }`
pub(super) fn rewrite_keypaths(tokens: Vec<Token>) -> Vec<Token> {
    let mut out = Vec::with_capacity(tokens.len());
    let mut i = 0usize;
    while i < tokens.len() {
        if tokens[i].kind == TokenKind::KeyPathStart {
            if let Some(root_i) = next_sig(&tokens, i + 1) {
                if tokens[root_i].kind == TokenKind::Identifier
                    || tokens[root_i].kind == TokenKind::Type
                {
                    let root = tokens[root_i].value.clone();
                    let mut path = Vec::new();
                    let mut j = root_i;
                    loop {
                        let Some(dot_i) = next_sig(&tokens, j + 1) else {
                            break;
                        };
                        if tokens[dot_i].kind != TokenKind::Dot
                            && !(tokens[dot_i].kind == TokenKind::Operator
                                && tokens[dot_i].value == ".")
                        {
                            break;
                        }
                        let Some(field_i) = next_sig(&tokens, dot_i + 1) else {
                            break;
                        };
                        if tokens[field_i].kind != TokenKind::Identifier {
                            break;
                        }
                        path.push(tokens[field_i].value.clone());
                        j = field_i;
                    }
                    if !path.is_empty() {
                        let access = path.join(".");
                        // Opaque identifier so the streaming parser does not
                        // re-split the closures.
                        let expr = format!(
                            "__MikaKeyPath {{ get: |__r: &{root}| &__r.{access}, get_mut: |__r: &mut {root}| &mut __r.{access} }}"
                        );
                        out.push(synth(TokenKind::Identifier, &expr));
                        i = j + 1;
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

/// `@WrapperName field: Type` inside a struct/class body → `field: WrapperName`.
/// `@propertyWrapper` before a struct is consumed (marker only).
pub(super) fn rewrite_property_wrappers(tokens: Vec<Token>) -> Vec<Token> {
    let mut out = Vec::with_capacity(tokens.len());
    let mut i = 0usize;
    while i < tokens.len() {
        if tokens[i].kind == TokenKind::AtIdent {
            let name = tokens[i].value.trim_start_matches('@');
            if name == "propertyWrapper" || name == "resultBuilder" {
                // Marker attribute — drop it; downstream treats the next
                // struct as a plain struct (wrapper/builder methods are user-defined).
                i += 1;
                continue;
            }
            // Field annotation: `@Clamped age: int` → `age: Clamped`
            if let Some(field_i) = next_sig(&tokens, i + 1) {
                if tokens[field_i].kind == TokenKind::Identifier {
                    if let Some(colon_i) = next_sig(&tokens, field_i + 1) {
                        if tokens[colon_i].value == ":" {
                            // Skip the original type tokens until newline/comma/brace.
                            let mut j = colon_i + 1;
                            let mut depth = 0isize;
                            while j < tokens.len() {
                                let t = &tokens[j];
                                if depth == 0
                                    && (matches!(
                                        t.kind,
                                        TokenKind::Newline
                                            | TokenKind::Comma
                                            | TokenKind::BraceEnd
                                            | TokenKind::Semicolon
                                    ) || t.value == ","
                                        || t.value == ";")
                                {
                                    break;
                                }
                                match t.kind {
                                    TokenKind::BraceStart
                                    | TokenKind::ParenthesesStart
                                    | TokenKind::BracketStart
                                    | TokenKind::AngleStart => depth += 1,
                                    TokenKind::BraceEnd
                                    | TokenKind::ParenthesesEnd
                                    | TokenKind::BracketEnd
                                    | TokenKind::AngleEnd => {
                                        depth = depth.saturating_sub(1)
                                    }
                                    TokenKind::Operator if t.value == "<" => depth += 1,
                                    TokenKind::Operator if t.value == ">" => {
                                        depth = depth.saturating_sub(1)
                                    }
                                    _ => {}
                                }
                                j += 1;
                            }
                            out.push(tokens[field_i].clone());
                            out.push(synth(TokenKind::Colon, ":"));
                            out.push(synth(TokenKind::Identifier, name));
                            i = j;
                            continue;
                        }
                    }
                }
            }
            // Function annotation `@IntList` before `func` — keep for
            // `rewrite_result_builders` (leave AtIdent in place).
            out.push(tokens[i].clone());
            i += 1;
            continue;
        }
        out.push(tokens[i].clone());
        i += 1;
    }
    out
}

/// `@BuilderName func Ret name() { e1; e2; e3 }` → body becomes
/// `return BuilderName::buildBlock(vec![e1, e2, e3])`.
pub(super) fn rewrite_result_builders(tokens: Vec<Token>) -> Vec<Token> {
    let mut out = Vec::with_capacity(tokens.len());
    let mut i = 0usize;
    while i < tokens.len() {
        if tokens[i].kind == TokenKind::AtIdent {
            let builder = tokens[i].value.trim_start_matches('@').to_string();
            if builder == "propertyWrapper" || builder == "resultBuilder" {
                out.push(tokens[i].clone());
                i += 1;
                continue;
            }
            // Look ahead for `func` … `{` body `}`
            if let Some(func_i) = next_sig(&tokens, i + 1) {
                if tokens[func_i].value == "func" {
                    // Find the function body's opening brace.
                    let mut j = func_i;
                    let mut pdepth = 0isize;
                    let mut found_brace = None;
                    while j < tokens.len() {
                        match tokens[j].kind {
                            TokenKind::ParenthesesStart | TokenKind::ParametersStart => {
                                pdepth += 1
                            }
                            TokenKind::ParenthesesEnd | TokenKind::ParametersEnd => {
                                pdepth = pdepth.saturating_sub(1)
                            }
                            TokenKind::BraceStart if pdepth == 0 && j > func_i => {
                                found_brace = Some(j);
                                break;
                            }
                            _ => {}
                        }
                        j += 1;
                    }
                    if let Some(open) = found_brace {
                        if let Some(close) = find_matching_brace(&tokens, open) {
                            let body = &tokens[open + 1..close];
                            let exprs = collect_builder_exprs(body);
                            // Emit everything from func through `{`, then
                            // rewritten body, then `}`.
                            out.extend(tokens[func_i..=open].iter().cloned());
                            let joined = exprs.join(", ");
                            let ret = format!(
                                "return {builder}::buildBlock(vec![{joined}]);"
                            );
                            out.push(synth(TokenKind::Identifier, &ret));
                            out.push(tokens[close].clone());
                            i = close + 1;
                            continue;
                        }
                    }
                }
            }
        }
        out.push(tokens[i].clone());
        i += 1;
    }
    out
}

/// Split a builder body into expression strings (one per newline/semicolon,
/// skipping empty / trivia-only lines).
fn collect_builder_exprs(body: &[Token]) -> Vec<String> {
    let mut exprs = Vec::new();
    let mut cur = String::new();
    let mut depth = 0isize;
    for t in body {
        if is_trivia(t) {
            continue;
        }
        match t.kind {
            TokenKind::BraceStart
            | TokenKind::ParenthesesStart
            | TokenKind::BracketStart
            | TokenKind::ParametersStart => {
                depth += 1;
                cur.push_str(&t.value);
            }
            TokenKind::BraceEnd
            | TokenKind::ParenthesesEnd
            | TokenKind::BracketEnd
            | TokenKind::ParametersEnd => {
                depth = depth.saturating_sub(1);
                cur.push_str(&t.value);
            }
            TokenKind::Newline | TokenKind::Semicolon if depth == 0 => {
                let s = cur.trim().trim_end_matches(';').trim().to_string();
                if !s.is_empty() {
                    exprs.push(s);
                }
                cur.clear();
            }
            _ => {
                if !cur.is_empty()
                    && !cur.ends_with('(')
                    && !cur.ends_with('[')
                    && !cur.ends_with('{')
                    && !cur.ends_with('.')
                    && !cur.ends_with("::")
                    && t.value != ","
                    && t.value != ")"
                    && t.value != "]"
                    && t.value != "}"
                    && !t.value.starts_with('.')
                {
                    // Space between identifiers / keywords.
                    if cur
                        .chars()
                        .last()
                        .map(|c| c.is_ascii_alphanumeric() || c == '_')
                        .unwrap_or(false)
                        && t.value
                            .chars()
                            .next()
                            .map(|c| c.is_ascii_alphanumeric() || c == '_')
                            .unwrap_or(false)
                    {
                        cur.push(' ');
                    }
                }
                cur.push_str(&t.value);
            }
        }
    }
    let s = cur.trim().trim_end_matches(';').trim().to_string();
    if !s.is_empty() {
        exprs.push(s);
    }
    exprs
}

/// Observers (`willSet`/`didSet`) are lowered in `process_class_members`.
pub(super) fn rewrite_observers(tokens: Vec<Token>) -> Vec<Token> {
    tokens
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

fn find_matching_paren(tokens: &[Token], open: usize) -> Option<usize> {
    let mut depth = 0isize;
    for i in open..tokens.len() {
        match tokens[i].kind {
            TokenKind::ParenthesesStart | TokenKind::ParametersStart => depth += 1,
            TokenKind::ParenthesesEnd | TokenKind::ParametersEnd => {
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

/// `decode(Type, text)` → `JSONDecoder::new().decode::<Type>(&text)`.
pub(super) fn rewrite_decode_calls(tokens: Vec<Token>) -> Vec<Token> {
    let mut out = Vec::with_capacity(tokens.len());
    let mut i = 0usize;
    while i < tokens.len() {
        if tokens[i].value == "decode"
            && matches!(
                tokens[i].kind,
                TokenKind::Identifier | TokenKind::Keyword | TokenKind::ModuleVar
            )
        {
            let free = match prev_sig(&tokens, i) {
                Some(p) if tokens[p].kind == TokenKind::Dot || tokens[p].value == "." => false,
                Some(p) if tokens[p].value == "::" => false,
                _ => true,
            };
            if free {
                if let Some(open) = next_sig(&tokens, i + 1) {
                    if matches!(
                        tokens[open].kind,
                        TokenKind::ParenthesesStart | TokenKind::ParametersStart
                    ) {
                        if let Some(close) = find_matching_paren(&tokens, open) {
                            if let Some((ty, src)) = split_decode_args(&tokens[open + 1..close]) {
                                let expr = format!(
                                    "JSONDecoder::new().decode::<{ty}>({src})"
                                );
                                out.push(synth(TokenKind::Identifier, &expr));
                                i = close + 1;
                                continue;
                            }
                        }
                    }
                }
            }
        }
        out.push(tokens[i].clone());
        i += 1;
    }
    out
}

fn split_decode_args(args: &[Token]) -> Option<(String, String)> {
    let mut depth = 0isize;
    let mut comma = None;
    for (k, t) in args.iter().enumerate() {
        match t.kind {
            TokenKind::ParenthesesStart
            | TokenKind::ParametersStart
            | TokenKind::BracketStart
            | TokenKind::BraceStart
            | TokenKind::AngleStart => depth += 1,
            TokenKind::ParenthesesEnd
            | TokenKind::ParametersEnd
            | TokenKind::BracketEnd
            | TokenKind::BraceEnd
            | TokenKind::AngleEnd => depth -= 1,
            TokenKind::Operator if t.value == "<" => depth += 1,
            TokenKind::Operator if t.value == ">" => depth -= 1,
            _ if depth == 0 && t.value == "," => {
                comma = Some(k);
                break;
            }
            _ => {}
        }
    }
    let c = comma?;
    let ty_toks: Vec<&Token> = args[..c]
        .iter()
        .filter(|t| !is_trivia(t) && t.kind != TokenKind::Newline)
        .collect();
    if ty_toks.len() != 1 {
        return None;
    }
    let ty = &ty_toks[0].value;
    // Type name should look like a type (PascalCase / known alias).
    let first = ty.chars().next()?;
    if !first.is_uppercase() && *ty != "json" {
        return None;
    }
    let mut src = String::new();
    for t in args[c + 1..].iter().filter(|t| !is_trivia(t) && t.kind != TokenKind::Newline) {
        src.push_str(&t.value);
    }
    let src = src.trim();
    if src.is_empty() {
        return None;
    }
    // Always borrow: `decode(User, js)` → `&js`
    let src = if src.starts_with('&') {
        src.to_string()
    } else {
        format!("&{src}")
    };
    Some((ty.to_string(), src))
}

fn find_matching_bracket(tokens: &[Token], open: usize) -> Option<usize> {
    let mut depth = 0isize;
    for i in open..tokens.len() {
        match tokens[i].kind {
            TokenKind::BracketStart => depth += 1,
            TokenKind::BracketEnd => {
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

fn tokens_src(tokens: &[Token]) -> String {
    let mut s = String::new();
    for t in tokens {
        if is_trivia(t) {
            continue;
        }
        if t.kind == TokenKind::Newline {
            continue;
        }
        if !s.is_empty() {
            let last = s.chars().last().unwrap_or(' ');
            let first = t.value.chars().next().unwrap_or(' ');
            let need = (last.is_ascii_alphanumeric() || last == '_')
                && (first.is_ascii_alphanumeric() || first == '_' || first == '"');
            if need || last == ',' {
                s.push(' ');
            }
        }
        s.push_str(&t.value);
    }
    s
}

/// `json { … }` / `json [ … ]` → `json!({ … })` / `json!([ … ])`.
///
/// Skips `json!`, `from json`, `::json`, and `foo.json`.
pub(super) fn rewrite_json_literals(tokens: Vec<Token>) -> Vec<Token> {
    let mut out = Vec::with_capacity(tokens.len());
    let mut i = 0usize;
    while i < tokens.len() {
        let is_json_kw = tokens[i].value == "json"
            && matches!(
                tokens[i].kind,
                TokenKind::Keyword | TokenKind::Identifier | TokenKind::Json | TokenKind::Type
            );
        if is_json_kw {
            let prev_ok = match prev_sig(&tokens, i) {
                Some(p) if tokens[p].value == "!" || tokens[p].value == "::" || tokens[p].value == "." => {
                    false
                }
                Some(p) if tokens[p].value == "from" => false,
                _ => true,
            };
            if prev_ok {
                if let Some(open) = next_sig(&tokens, i + 1) {
                    if tokens[open].kind == TokenKind::BraceStart {
                        if let Some(close) = find_matching_brace(&tokens, open) {
                            let inner = tokens_src(&tokens[open + 1..close]);
                            out.push(synth(
                                TokenKind::Identifier,
                                &format!("json!({{ {inner} }})"),
                            ));
                            i = close + 1;
                            continue;
                        }
                    } else if tokens[open].kind == TokenKind::BracketStart {
                        if let Some(close) = find_matching_bracket(&tokens, open) {
                            let inner = tokens_src(&tokens[open + 1..close]);
                            out.push(synth(
                                TokenKind::Identifier,
                                &format!("json!([{inner}])"),
                            ));
                            i = close + 1;
                            continue;
                        }
                    }
                }
            }
        }
        out.push(tokens[i].clone());
        i += 1;
    }
    out
}

//! Implicit Tokio builtins — rewrite so Mikasa never needs `tokio::` in source.
//!
//! | Mikasa                         | Rust                                              |
//! |--------------------------------|---------------------------------------------------|
//! | `sleep(ms)`                    | `tokio::time::sleep(Duration::from_millis(ms)).await` |
//! | `spawn(async { … })` / `spawn({ … })` | `tokio::spawn(async move { … })`            |
//! | `timeout(ms, fut)`             | `tokio::time::timeout(Duration::from_millis(ms), fut).await` |
//! | `yield_now()`                  | `tokio::task::yield_now().await`                  |

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

fn tokens_to_src(tokens: &[Token]) -> String {
    let mut s = String::new();
    for t in tokens {
        if is_trivia(t) {
            continue;
        }
        // Newlines from the tokenizer often carry `;` — keep that so
        // statements inside `spawn({ … })` stay separated.
        if t.kind == TokenKind::Newline {
            if t.value.contains(';') {
                s.push(';');
            } else if !s.ends_with(';') && !s.is_empty() {
                s.push(';');
            }
            continue;
        }
        if !s.is_empty() {
            let last = s.chars().last().unwrap_or(' ');
            let first = t.value.chars().next().unwrap_or(' ');
            let need_space = ((last.is_ascii_alphanumeric() || last == '_')
                && (first.is_ascii_alphanumeric() || first == '_'))
                || last == ','
                || (last == '>' && first.is_ascii_alphanumeric());
            if need_space {
                s.push(' ');
            }
        }
        s.push_str(&t.value);
    }
    s
}

/// True when `sleep`/`spawn`/… is a free call, not `foo.sleep(` / `::sleep(`.
fn is_free_call(tokens: &[Token], name_i: usize) -> bool {
    match prev_sig(tokens, name_i) {
        Some(p) if tokens[p].kind == TokenKind::Dot || tokens[p].value == "." => false,
        Some(p) if tokens[p].value == "::" => false,
        _ => true,
    }
}

pub(super) fn rewrite_async_builtins(tokens: Vec<Token>) -> Vec<Token> {
    // Innermost first: a `sleep` inside `spawn({ sleep(10) })` must lower
    // before the outer `spawn` snapshots its body as source text.
    let mut tokens = tokens;
    for _ in 0..16 {
        let next = rewrite_async_builtins_once(&tokens);
        if next.len() == tokens.len()
            && next
                .iter()
                .zip(tokens.iter())
                .all(|(a, b)| a.value == b.value && a.kind == b.kind)
        {
            return tokens;
        }
        tokens = next;
    }
    tokens
}

fn rewrite_async_builtins_once(tokens: &[Token]) -> Vec<Token> {
    // Rewrite the *rightmost* call each pass so nested `sleep` inside
    // `spawn({…})` lowers before the outer `spawn` snapshots its body.
    let mut last: Option<(usize, usize, usize)> = None; // name_i, open, close
    let mut i = 0usize;
    while i < tokens.len() {
        let name = tokens[i].value.as_str();
        let is_ident = matches!(
            tokens[i].kind,
            TokenKind::Identifier | TokenKind::Keyword | TokenKind::ModuleVar
        );
        if is_ident
            && matches!(name, "sleep" | "spawn" | "timeout" | "yield_now")
            && is_free_call(tokens, i)
        {
            if let Some(open) = next_sig(tokens, i + 1) {
                if matches!(
                    tokens[open].kind,
                    TokenKind::ParenthesesStart | TokenKind::ParametersStart
                ) {
                    if let Some(close) = find_matching_paren(tokens, open) {
                        // Keep scanning inside the args so nested `sleep`
                        // inside `spawn({…})` becomes the rightmost (last).
                        last = Some((i, open, close));
                    }
                }
            }
        }
        i += 1;
    }
    let Some((name_i, open, close)) = last else {
        return tokens.to_vec();
    };
    let name = tokens[name_i].value.as_str();
    let args = &tokens[open + 1..close];
    let replacement = match name {
        "sleep" => rewrite_sleep(args),
        "spawn" => rewrite_spawn(args),
        "timeout" => rewrite_timeout(args),
        "yield_now" => "tokio::task::yield_now().await".to_string(),
        _ => return tokens.to_vec(),
    };
    let mut out = Vec::with_capacity(tokens.len());
    out.extend(tokens[..name_i].iter().cloned());
    out.push(synth(TokenKind::Identifier, &replacement));
    out.extend(tokens[close + 1..].iter().cloned());
    out
}

fn rewrite_sleep(args: &[Token]) -> String {
    let ms = tokens_to_src(args);
    format!(
        "tokio::time::sleep(std::time::Duration::from_millis(({ms}) as u64)).await"
    )
}

fn rewrite_spawn(args: &[Token]) -> String {
    // Strip a leading `async` / `async move` — we always emit `async move`.
    let mut i = 0usize;
    while i < args.len() && (is_trivia(&args[i]) || args[i].kind == TokenKind::Newline) {
        i += 1;
    }
    if i < args.len() && args[i].value == "async" {
        i += 1;
        while i < args.len() && (is_trivia(&args[i]) || args[i].kind == TokenKind::Newline) {
            i += 1;
        }
        if i < args.len() && args[i].value == "move" {
            i += 1;
        }
    }
    while i < args.len() && (is_trivia(&args[i]) || args[i].kind == TokenKind::Newline) {
        i += 1;
    }
    if i < args.len() && args[i].kind == TokenKind::BraceStart {
        if let Some(close) = find_matching_brace(args, i) {
            let body = tokens_to_src(&args[i + 1..close]);
            return format!("tokio::spawn(async move {{ {body} }})");
        }
    }
    // Fallback: treat the whole arg as a future expression.
    let fut = tokens_to_src(args);
    format!("tokio::spawn({fut})")
}

#[cfg(test)]
mod tests {
    use super::*;
    use mikasa_syntax::tokenizer::tokenizer::Tokenizer;

    #[test]
    fn sleep_inside_spawn_lowers_first() {
        let src = "spawn({\n  sleep(10)\n  println!(\"x\")\n})\n";
        let tokens = Tokenizer::new(src.to_string()).tokenize();
        let out = rewrite_async_builtins(tokens);
        let joined: String = out.iter().map(|t| t.value.clone()).collect();
        assert!(
            joined.contains("tokio::time::sleep"),
            "sleep not lowered: {joined}"
        );
        assert!(
            joined.contains("tokio::spawn"),
            "spawn not lowered: {joined}"
        );
        assert!(
            !joined.contains("sleep(10)"),
            "bare sleep remained: {joined}"
        );
    }
}

fn rewrite_timeout(args: &[Token]) -> String {
    // Split on top-level comma: `timeout(ms, fut)`
    let mut depth = 0isize;
    let mut comma = None;
    for (k, t) in args.iter().enumerate() {
        match t.kind {
            TokenKind::ParenthesesStart
            | TokenKind::ParametersStart
            | TokenKind::BracketStart
            | TokenKind::BraceStart => depth += 1,
            TokenKind::ParenthesesEnd
            | TokenKind::ParametersEnd
            | TokenKind::BracketEnd
            | TokenKind::BraceEnd => depth -= 1,
            _ if depth == 0 && t.value == "," => {
                comma = Some(k);
                break;
            }
            _ => {}
        }
    }
    let Some(c) = comma else {
        let src = tokens_to_src(args);
        return format!(
            "tokio::time::timeout(std::time::Duration::from_millis(0), {src}).await"
        );
    };
    let ms = tokens_to_src(&args[..c]);
    let fut = tokens_to_src(&args[c + 1..]);
    format!(
        "tokio::time::timeout(std::time::Duration::from_millis(({ms}) as u64), {fut}).await"
    )
}

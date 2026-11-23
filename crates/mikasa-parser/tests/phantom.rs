//! `phantom T` sugar + unused-generic hints.

use mikasa_syntax::tokenizer::tokenizer::Tokenizer;

fn transpile(src: &str) -> String {
    let tokens = Tokenizer::new(src.to_string()).tokenize();
    mikasa_parser::parser::parse(tokens)
}

fn hints(src: &str) -> Vec<String> {
    let tokens = Tokenizer::new(src.to_string()).tokenize();
    let p = mikasa_parser::Parser::new(tokens);
    p.phantom_hints().to_vec()
}

#[test]
fn phantom_marker_lowers_to_phantom_data() {
    let rust = transpile(
        "struct Holder<T> {\n\
             phantom T\n\
             value: i32\n\
         }\n",
    );
    assert!(
        rust.contains("PhantomData") && rust.contains("_phantom_t"),
        "missing PhantomData field:\n{rust}"
    );
    assert!(
        rust.contains("value"),
        "value field lost:\n{rust}"
    );
}

#[test]
fn phantom_multi_params() {
    let rust = transpile(
        "struct PairMark<T, U> {\n\
             phantom T, U\n\
         }\n",
    );
    assert!(
        rust.contains("_phantom_t") && rust.contains("_phantom_u"),
        "expected both phantom fields:\n{rust}"
    );
}

#[test]
fn unused_generic_emits_hint() {
    let h = hints(
        "struct Empty<T> {\n\
             value: i32\n\
         }\n",
    );
    assert!(
        h.iter().any(|m| m.contains("`T`") && m.contains("phantom T")),
        "expected unused-T hint, got: {h:?}"
    );
}

#[test]
fn marked_phantom_suppresses_hint() {
    let h = hints(
        "struct Empty<T> {\n\
             phantom T\n\
             value: i32\n\
         }\n",
    );
    assert!(
        h.is_empty(),
        "marked phantom should not warn: {h:?}"
    );
}

#[test]
fn used_generic_no_hint() {
    let h = hints(
        "struct Boxy<T> {\n\
             value: T\n\
         }\n",
    );
    assert!(h.is_empty(), "used T should not warn: {h:?}");
}

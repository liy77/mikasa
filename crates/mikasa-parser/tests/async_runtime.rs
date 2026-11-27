//! Implicit Tokio runtime: deps, `#[tokio::main]`, and builtins.

use mikasa_syntax::tokenizer::tokenizer::Tokenizer;

fn transpile(src: &str) -> String {
    let tokens = Tokenizer::new(src.to_string()).tokenize();
    mikasa_parser::parser::parse(tokens)
}

#[test]
fn async_main_gets_tokio_main_and_await() {
    let rust = transpile("async func main() {\n  println!(\"hi\")\n}\n");
    assert!(
        rust.contains("async fn __mikasa_main"),
        "async main not renamed: {rust}"
    );
    assert!(
        rust.contains("#[tokio::main]") && rust.contains("__mikasa_main().await"),
        "tokio entry missing: {rust}"
    );
}

#[test]
fn sleep_lowers_without_citing_tokio_in_source() {
    let rust = transpile("async func main() {\n  sleep(10)\n}\n");
    assert!(
        rust.contains("tokio::time::sleep"),
        "sleep not lowered: {rust}"
    );
    assert!(
        rust.contains(".await"),
        "sleep should auto-await: {rust}"
    );
}

#[test]
fn spawn_wraps_async_move() {
    let rust = transpile("async func main() {\n  spawn({ println!(\"x\") })\n}\n");
    assert!(
        rust.contains("tokio::spawn(async move"),
        "spawn not lowered: {rust}"
    );
}

#[test]
fn top_level_await_uses_tokio_main() {
    let rust = transpile("sleep(1)\nprintln!(\"ok\")\n");
    assert!(
        rust.contains("#[tokio::main]") && rust.contains("async fn main"),
        "top-level sleep should force async main: {rust}"
    );
}

#[test]
fn sync_main_unchanged_without_async() {
    let rust = transpile("func main() {\n  println!(\"hi\")\n}\n");
    assert!(
        !rust.contains("tokio"),
        "sync program must not pull tokio: {rust}"
    );
    assert!(
        rust.contains("fn main()") && rust.contains("__mikasa_main();"),
        "sync wrapper missing: {rust}"
    );
    assert!(
        !rust.contains("#[tokio::main]"),
        "sync main must not use tokio::main: {rust}"
    );
}

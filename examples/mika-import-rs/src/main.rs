//! Rust host that pulls in Mikasa via `mika_import!`.
//! The sibling `logic.mika` is Mikasa source only — never Rust.
mika_import::mika_import!("../logic.mika");

fn main() {
    println!("answer = {}", answer());
    greet("rust-host");
}

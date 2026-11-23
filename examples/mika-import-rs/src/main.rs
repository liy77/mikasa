//! Rust host that pulls in Mikasa via `mika_import!`.
mika_import::mika_import!("../logic.mika");

fn main() {
    println!("answer = {}", answer());
}

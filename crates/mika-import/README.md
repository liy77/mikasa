# mika-import

Compile-time bridge from Mikasa (`.mika`) into Rust.

```rust
mika_import::mika_import!("logic.mika");

fn main() {
    greeter(); // defined in logic.mika
}
```

Inside `.mika` the same idea is written without the macro name:

```mika
import("logic.mika")
```

Both forms expand nested file imports, then (for the Rust macro) transpile
Mikasa → Rust and paste the items at the call site.

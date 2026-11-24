# mika-import

Compile-time bridge from Mikasa (`.mika`) into Rust.

```rust
mika_import::mika_import!("logic.mika");

fn main() {
    println!("{}", answer()); // defined in logic.mika
}
```

The `.mika` side is plain Mikasa (never Rust):

```mika
func int answer() {
    return 40 + 2
}
```

Inside another `.mika` the include is spelled without the macro name:

```mika
import("logic.mika")
```

Both forms expand nested file imports, then (for the Rust macro) transpile
Mikasa → Rust and paste the items at the call site.

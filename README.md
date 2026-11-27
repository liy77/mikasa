<img src="./assets/mikasa.png" width="128"/>

> **Mikasa is now Mikasa.** The language was renamed: Mikasa → **Mikasa**, and
> its interpreter Alloy → **Astra**. The toolchain binary is `mikasa` (formerly
> `mikasa`); the interpreter binary is `astra` (the `astra` name still works
> where installed as an alias).
>
> File extensions were renamed too. The old ones still open, they are just no
> longer the recommended spelling:
>
> | | Recommended | Legacy (still works) |
> | --- | --- | --- |
> | Source | `.mika` | `.crs` |
> | Astra bytecode | `.ast` | `.loy` |
> | Library | `.mklib` | — |
> | Package | `.mipkg` | — |
>
> `.mika` is also the MocidaUI file: a `.mika` that declares a `view` or an
> `App()` is a UI document, so there is no separate extension for that.
> `.crm` is retired — it still opens, with a warning to rename it.

A high-level language that transpiles to Rust. Mikasa aims to keep Rust's
performance while giving you a syntax that reads like a modern scripting
language — string interpolation, optional chaining, ternaries, and JS-style
loop / match / closure forms — backed by the `mikasa` toolchain.

```mika
import { trim } from mika

func main() {
    mut name = trim("  Ada  ")
    println("Your name is $name")
}
```

<p align="center">
    <img src="./assets/mikasa-icon.png" width="220" />
</p>

## Two execution paths

| Path | Tool | How it works | When to use |
| --- | --- | --- | --- |
| **Transpile → Rust** | `mikasa` | `.mika` → Rust source → `cargo build` | Production, full Rust semantics |
| **Interpreter** | `astra` | tree-walks the AST directly, no compile step | Fast iteration, scripting, CI |

`astra run file.mika` is instant — like `python` or `node`. `mikasa run file.mika`
gives you native speed after the first compile (or `mikasa vm run` for the same
Astra engine without cargo). Both share the same `mikasa-syntax` AST.

## Install

There's also a [native installer](./installer-gui) — built with MUI
(mocida's declarative UI; no web stack, no Tauri), Windows-only for now —
that wraps the script below in a clickable form.

### Script install

See [`docs/INSTALL.md`](./docs/INSTALL.md) for the full guide. TL;DR — one
cross-platform Python installer (needs Python 3.7+):

```sh
python scripts/install.py
```

On Windows you can also double-click `scripts\install.bat`; on Unix run
`bash scripts/install.sh`. Both just forward to `install.py`. The installer
auto-detects admin / root and picks a global or per-user install accordingly.

Pass `--workspace` to also build and install `astra`, `lson`, and the LSP
servers in one shot.

## Compile and run (mikasa)

```sh
mikasa run ./main.mika         # transpile + cargo run
mikasa build ./src             # compile only, directory input
mikasa run examples/mikasa/loops.mika
```

`mikasa run` and `mikasa build` warn when the file uses a legacy extension
(`.crs`, `.loy`, `.crm`) — it still works, the spelling is deprecated.
`mikasa run` with no argument defaults to `./main.mika` in the current
directory.

### Flags

| Flag | Full name | Description | Example |
| --- | --- | --- | --- |
| `-i` | `--input`   | Input file or directory | `mikasa build -i main.mika` |
| `-o` | `--output`  | Output directory | `mikasa -o ./build` |
| `-t` | `--target`  | Cross-compile target | `mikasa -t windows` |
| `-c` | `--compile` | Compile (no run) | `mikasa build main.mika` |
|      | `--clean`   | Clean the output directory | `mikasa --clean` |
| `-V` | `--verbose` | Verbose output | `mikasa -V run main.mika` |
| `-v` | `--version` | Print version (with build date for pre-releases) | `mikasa -v` |

```sh
mikasa update                  # install the latest GitHub nightly
```

## Astra — the Mikasa interpreter

<p align="center">
    <img src="./assets/astra-logo.png" width="180" alt="Astra" />
</p>

Astra (binary `astra`) interprets `.mika` files directly with no compile step
and can also produce a portable `.ast` artifact that runs anywhere without a
Rust toolchain.

```sh
astra run file.mika            # instant — no compile
astra build file.mika          # compile to a portable .ast artifact
astra run app.ast              # run the artifact (no rustc needed)
astra check file.mika          # verify with real Rust + Miri
mikasa vm run file.mika        # same engine via the mikasa CLI
```

### Portable `.ast` artifacts

An `.ast` is a self-contained binary (magic header + bincoded AST). The older
`.loy` spelling still loads. If the
program imports sibling `.rs` files Astra compiles them to WebAssembly once
(cached by content hash in `~/.astra/cache`) and embeds the wasm into the
artifact — so the final `.ast` runs on any platform with no rustc and no
`.rs` files on disk.

```sh
astra build myapp.mika -o myapp.ast  # bundles sibling .rs as wasm
astra run myapp.ast                  # works anywhere, no rustc
```

### Importing Rust from Mikasa

Astra can call into sibling `.rs` files via WebAssembly — the Mikasa stays
interpreted (instant); only the Rust crosses into wasm. Plain `pub fn` is
auto-wrapped (no `extern "C"` required for scalar ABI):

```mika
// math.rs (sibling):  pub fn fib(n: i64) -> i64 { ... }

import { fib } from math

func main() {
    println("${fib(10)}")   // → 55, via embedded wasm
}
```

You can also splice another `.mika` file with `import("util.mika")`, or from
Rust hosts use `mika_import!("util.mika")` (`crates/mika-import`).

Current scalar ABI: `i64` / `i32` / `bool` / `f64`. Strings / `Vec` / structs
are the next phase.

### Astra GUI playground

`astra-gui/` is a dual-mode `astra` binary that adds a MUI playground — open
a `.mika` file, edit it, and see output live. Hot-reload re-evaluates on save;
resize splits output to the right on wide screens.

## Language features

### Classes

```mika
class Greeter {
    name: string

    Greeter(name: string) {
        self.name = name
    }

    void hello(self) {
        println("Hello, ${self.name}!")
    }
}

func main() {
    Greeter::new("Brian").hello()
}
```

The class lowers to a Rust `struct` + `impl`. The constructor (same name
as the class) becomes `pub fn new(...) -> Self`, and methods declared as
`void name(self)` become `pub fn name(&self)`. Use `mikasa run main.mika`
(or `astra run main.mika`) to see it print `Hello, Brian!`.

### Loops, match, optional chaining, ternary

```mika
mut count = 0
loop {
    count++
    if count == 3 { break }
}

mut user: Option<User> = Some(User { name: "Brian", age: 30 })
mut age = user?.age              // Some(30)

mut grade = score >= 9 ? "A" : "B"

mut kind = match n {
    0 => 0,
    1 | 2 | 3 => 1,
    n if n < 0 => -1,
    _ => 2
}
```

See [`examples/mikasa/`](./examples/mikasa) for runnable demos of each feature.
Run the automated suite with `python tests/run_all.py`.

### Built-in `mika` standard library

Rust's standard library is powerful but verbose for everyday scripting tasks
(reading a line of input, sleeping, getting the current time, running a shell
command). Mikasa ships an embedded `mika` module (`cstd` is the deprecated
alias) — import what you need:

```mika
import { trim, sleep_ms, now_ms, run, env, exists } from mika

func main() {
    mut name = trim("  Ada  ")
    println("Hello $name")

    mut started = now_ms()
    sleep_ms(100)
    println("slept ~${now_ms() - started}ms")

    println("HOME = ${env("HOME")}")
    println("exists Cargo.toml? ${exists("Cargo.toml")}")
    println("echo -> ${trim(run("echo hi"))}")
}
```

Available functions: `input`, `readln`, `read_int`, `read_float`, `to_int`,
`to_float`, `trim`, `split`, `join`, `exit`, `die`, `panic_if`, `sleep_ms`,
`now_ms`, `env`, `args`, `read_file`, `write_file`, `append_file`, `exists`,
`is_file`, `is_dir`, `list_dir`, `run`, `rand_int`.

The library lives in [`std/cstd.mika`](./std/cstd.mika) — written in Mikasa,
shared between the transpiler and Astra (single source of truth).

## Project layout

```
mikasa/
├── src/                # mikasa compiler entrypoint + legacy re-exports
├── crates/
│   ├── mikasa-syntax/  # Tokenizer + AST (shared by mikasa, astra, LSPs)
│   ├── mikasa-parser/  # Streaming transpiler parser
│   ├── astra-vm/       # Astra interpreter + `astra` CLI
│   ├── mikasa-lsp/     # Mikasa language server (hover, completion, goto-def)
│   ├── mui-syntax/     # MocidaUI parser (.mui, and .mika files with a view)
│   ├── mui-codegen/    # MUI → Rust codegen (M5 native path)
│   └── mui-lsp/        # MUI language server
├── astra-gui/          # Astra GUI playground (MUI host, standalone workspace)
├── installer-gui/      # Native MUI installer (Windows-first)
├── examples/
│   ├── mikasa/         # .mika demos (loops, interpolation, matching, cstd, ...)
│   └── mui/            # MocidaUI demos (hello, counter, keyboard, app, ...)
├── scripts/            # Python tooling: install / build / cleanup / diagnose / hooks
├── docs/               # INSTALL.md, TODO.md, design specs
├── std/                # Mikasa standard library (cstd.mika)
├── lson/               # LSON parser binaries (per-OS)
├── assets/             # Logos
├── main.mika            # Default file for `mikasa run` with no argument
├── properties.kson     # Project metadata + dependencies
├── Cargo.toml          # Workspace root
└── build.rs            # Stamps the build date into pre-release versions
```

## Examples

```sh
# mikasa (transpile → Rust)
mikasa run examples/mikasa/loops.mika           # loop / while / for / break / continue
mikasa run examples/mikasa/interpolation.mika   # "Hello $name", "${expr}"
mikasa run examples/mikasa/collections.mika     # vec literals, closures, ?
mikasa run examples/mikasa/matching.mika        # match arms, if let, while let
mikasa run examples/mikasa/optional.mika        # `?.` optional chaining
mikasa run examples/mikasa/ternary.mika         # `cond ? a : b`
mikasa run examples/mikasa/cstd.mika            # built-in stdlib (input, sleep, env, ...)
mikasa run examples/mikasa/unsafe.mika          # `unsafe func` and `unsafe { ... }` blocks

# Astra (interpreter — instant)
astra run examples/mikasa/loops.mika
astra run examples/mikasa/cstd.mika
astra build examples/mikasa/collections.mika -o collections.loy
astra run collections.loy

# MUI examples (need mui-dev on PATH):
mikasa run examples/mui/hello/hello.mui
mikasa run examples/mui/counter/counter.mui
mikasa run examples/mui/keyboard/keyboard.mui
mikasa run examples/mui/app/app.mui
```

## Roadmap / TODO

Full backlog with details: [`docs/TODO.md`](./docs/TODO.md).

**Language (mikasa)**
- [x] Core transpile pipeline (`.mika` → Rust), `mikasa run` / `-c`
- [x] Variables, functions, classes/structs/impl, imports
- [x] Loops, `match` (guards/`_`/`|`), `if let` / `while let`
- [x] Optional chaining `?.`, ternary `?:`, string interpolation `"${expr}"`
- [x] `unsafe` blocks + functions, raw pointers, generic return types
- [x] Optional return-type sugar + generic params on `impl` methods
- [x] `cstd` standard library, `.rs` interop (sibling Rust files)
- [x] CalVer versioning + build date in `--version`
- [ ] Generics in free-function parameters (`func<T> name(arg: T)`)
- [ ] `?:` ternary inside `${…}` interpolation
- [ ] Type aliases (`type Foo = Bar<T>`) inside `static`/`as` casts

**Astra interpreter**
- [x] Tree-walking interpreter for the full `mikasa-syntax` AST
- [x] `cstd` interpreted from `std/cstd.mika` (single source, no Rust reimplementation)
- [x] Portable `.loy` artifacts (`astra build` / `astra run`)
- [x] Imported `.rs` files run via embedded WebAssembly (wasm cache, embedded in `.loy`)
- [x] `astra check` — auto-provisioned Miri/rustc verification
- [x] Return-type checking at runtime
- [x] Astra GUI playground (MUI, hot-reload, native file dialog)
- [ ] Wasm ABI: strings, `Vec`, structs (currently `i64`/`bool` scalars only)
- [ ] Auto-generate export wrappers so plain `pub fn` works without `extern "C"`
- [ ] Short-circuit `&&` / `||` (both sides are currently pre-evaluated)

**MocidaUI (`.mui`, and `.mika` with a `view`)**
- [x] `mikasa run x.mui` dev render (via `mui-dev`)
- [x] `mikasa -c [-r] x.mui` codegen + native build (via `mui-codegen`)
- [x] Component import/reuse, `app { }` config + bundles
- [x] `onKeyInput` keyboard handlers, `Dialog` overlay, `Audio` component
- [x] Common `x:`/`y:` props on every widget
- [ ] Reactive `if`/`for`/`match` in node position (structural codegen — design ready)
- [ ] Codegen feature parity with the interpreter (string signals, conditional color)

**Installer**
- [x] MUI-based native installer (no Tauri/web)
- [x] `--workspace` flag builds and installs astra + LSPs in one shot
- [ ] Signed installer artifact + CI release packaging

## Contributing

Pull requests are welcome. Please open an issue first for non-trivial changes
so we can discuss the approach.

After cloning, activate the repo's git hooks once so commits and pushes
run the same lint gates CI does:

```sh
python scripts/hooks.py
```

This sets `core.hooksPath` to `.githooks/`. From then on:

- `pre-commit` runs `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings`
- `pre-push` runs `cargo test`

Bypass with `--no-verify` only when you have to (the same checks fail in CI
afterwards).

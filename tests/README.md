# Testes Mikasa / Astra

## Rodar tudo

```sh
python tests/run_all.py
# ou
python scripts/test.py
```

Por padrão o runner **recompila** `astra` em `CARGO_TARGET_DIR` (default
`/tmp/mikasa-target`) e usa esse binário — evita cair num `target/debug/astra`
antigo sem `import("…")` / wrap de `pub fn`.

Flags:

| Flag | Efeito |
| --- | --- |
| `--quick` | só `cargo test` das crates |
| `--lang-only` | só `tests/mikasa/*.mika` via `astra run` |
| `--examples` | também `examples/mikasa/**/*.mika` (pula stdin/rede) |
| `--no-build` | não recompila; escolhe o `astra` mais novo por mtime |

`ASTRA_BIN=/path/to/astra` força o binário. Falhas no stderr (`runtime error`,
`delegating to mikasa`, …) contam mesmo se o exit code vier 0.

## Suite de linguagem (`tests/mikasa/`)

Cada arquivo `NN_*.mika` importa `harness.mika` (`check` / `check_eq` / `summary`)
e termina com `exit(1)` se algum assert falhar.

| Arquivo | Cobertura |
| --- | --- |
| `01_basics` | aritmética, bool, string, assign |
| `02_operators` | comparação, unary, ternary, `++`/`--` |
| `03_control` | if/else, while, for, loop/break/continue |
| `04_functions` | call, multi-arg, recursão |
| `05_collections` | arrays, tuples |
| `06_strings` | len, upper, interpolação |
| `07_match_option` | match, Option, Result |
| `08_structs` | struct fields |
| `09_closures` | map/filter |
| `10_mika_stdlib` | **cada** fn headless de `mika`/`cstd` |
| `11_file_import` | `import("…")` + nested |
| `12_rust_interop` | `import { fib } from` `.rs` via wasm |

Fixtures: `tests/mikasa/fixtures/`.

Stdlib **não** coberta aqui (stdin interativo): `input`, `readln`, `read_int`,
`read_float`. `die` / `panic_if` / `exit` entram só via harness na falha.

## Crates (Rust)

- `mikasa-syntax` — parse + `file_import` expand
- `mikasa-parser` — codegen
- `astra-vm` — CLI + `tests/mikasa_suite.rs` (reexecuta a suite `.mika`)
- `mika-import` — testes da macro / polish

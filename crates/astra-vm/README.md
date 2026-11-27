<p align="center">
  <img src="../../assets/astra-logo.png" alt="Astra" width="180">
</p>

<h1 align="center">Astra</h1>

<p align="center">
  <em>VM de iteração rápida do Mikasa — interpreter tree-walking.</em>
</p>

> **Alloy → Astra** (branding). O binário da VM é **`astra`**. O nome de crate
> `astra-vm` e o magic `ASTRABC` do artefato ficam como estão.
>
> Backlog: [`docs/TODO-astra.md`](../../docs/TODO-astra.md)

---

## O que é

**Astra** executa `.mika` **na hora**, sem transpile e sem `cargo`. Usa a mesma
AST do `mikasa-syntax` que o transpiler (`mikasa` / `mikasa`) — o que roda aqui
é o mesmo Mikasa que vira Rust nativo.

| Ferramenta | Caminho | Uso |
| --- | --- | --- |
| `mikasa` / `mikasa` | `.mika` → Rust → `cargo build` | release nativo |
| **`astra`** | `.mika` → AST → interpret | script, playground, CI leve |
| `mikasa vm run` | mesmo interpreter | via CLI Mikasa |

## Comandos

```sh
astra run program.mika          # interpreta
astra build program.mika -o out.ast   # artefato portável (.ast / .loy)
astra run out.ast               # roda artefato (magic ASTRABC)
astra check program.mika        # rustc + Miri (via mikasa/mikasa)
mikasa vm run program.mika      # mesmo engine
```

Exemplo:

```mika
func sum_to(n: int) -> int {
    mut total = 0
    for i in 1..n {
        total += i
    }
    return total
}

func main() {
    println("Astra VM")
    println("sum 1..5 = ${sum_to(5)}")
}
```

```sh
$ astra run examples/mikasa/astra-hello.mika
Astra VM
sum 1..5 = 10
```

## Suporte hoje

- Literais, interpolação `"${…}"`, `println` / `print`
- Controle de fluxo, funções, structs/classes/`impl`, enums, `match`, closures, tuples
- Checagem de tipo de retorno no interpreter
- Stdlib (`mika`/`cstd`, `fs`, `time`, `json`, `http`, …)
- Imports locais `.mika` (merge) e **`import("arquivo.mika")`**
- **`.rs` via wasm** (`wasmi`, cache `~/.astra/cache`): `i64` / `i32` / `bool` / `f64`;
  `pub fn` idiomático é auto-wrapped (não precisa `extern "C"`)
- Artefato `.ast`/`.loy` com wasm embutido (`astra build`)

Aritmética **checked** (overflow / div-by-zero → `RuntimeError`). Sem borrow-check
estático — use `mikasa` ou `astra check`.

## Layout

```
crates/astra-vm/
├── src/
│   ├── lib.rs          re-exports
│   ├── interp.rs       Interpreter
│   ├── value.rs / env.rs / error.rs
│   ├── loader.rs       imports (.mika / .rs / import("…"))
│   ├── wasm.rs         Rust → wasm32 + wasmi
│   ├── bytecode.rs     .ast / .loy
│   ├── stdlib*.rs      nativos
│   └── bin/astra.rs    CLI
└── README.md           este arquivo
```

GUI nativa (playground): [`astra-gui/`](../../astra-gui/) — UI/backend em Mikasa,
host mocida fino em Rust.

## Roadmap (resumo)

1. ✅ Core tree-walk + tipos compostos + stdlib + return-check  
2. ✅ Artefato portável + `check` (Miri)  
3. ✅ Wasm escalar + wrap de `pub fn` + `import("…")`  
4. Wasm rica (strings / `Vec`) · short-circuit `&&`/`||` · `astra repl`  
5. (futuro) bytecode / Cranelift JIT  

Lista completa: [`docs/TODO-astra.md`](../../docs/TODO-astra.md).
Spec: [`docs/superpowers/specs/2026-06-22-astra-vm-design.md`](../../docs/superpowers/specs/2026-06-22-astra-vm-design.md).

## Build

```sh
cargo build -p astra-vm --bin astra
# → target/debug/astra
```

A crate é isolada: `mikasa` / `mui-*` não dependem dela no grafo principal de
forma que quebre CI cross-platform.

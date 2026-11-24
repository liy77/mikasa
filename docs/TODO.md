# Backlog / Roadmap

> Atualizado: 2026-11-23. Listas separadas por produto:

| Lista | Escopo |
| --- | --- |
| **[TODO-mikasa.md](./TODO-mikasa.md)** | Transpilador `mikasa`, sintaxe, LSP, MUI codegen, `mika_import!` |
| **[TODO-astra.md](./TODO-astra.md)** | VM `astra`, wasm interop, `astra-gui` |

README da VM: [`crates/astra-vm/README.md`](../crates/astra-vm/README.md).  
MUI / Astra GUI: [`docs/mui.md`](./mui.md) · [`astra-gui/README.md`](../astra-gui/README.md).

---

## Resumo — alta prioridade

### Mikasa
- Genéricos em parâmetros de função (`func<T> name(x: T)`).
- MUI codegen estrutural reativo (`if` / `for` / `match`).
- Handlers + reatividade live (M3/M4).

### Astra
- Wasm ABI rica (strings / `Vec` / slices + WASI).
- `&&` / `||` short-circuit.
- GUI: streaming de output, erros estruturados, packaging.

Detalhes e checkboxes → arquivos acima.

---

## ✅ Concluído (referência rápida)

| Item | Área |
| --- | --- |
| Extensões `.mika` / `.ast` + branding Mikasa/Astra | ambos |
| `mikasa init\|install\|upgrade` sem cargo | Mikasa |
| `import("arquivo.mika")` + `mika_import!` | Mikasa |
| Rust via wasm + wrap de `pub fn` + `f64` | Astra |
| `cstd`/`mika` interpretado de `std/cstd.mika` | Astra |
| Artefato portável `.ast`/`.loy` + `astra check` | Astra |
| Astra GUI (Mikasa UI + backend) | Astra |

# Astra — TODO

> Backlog da **VM** (`astra` / `crates/astra-vm`), wasm interop e GUI (`astra-gui`).
> Atualizado: 2026-11-23.
>
> Índice geral: [`TODO.md`](./TODO.md) · Mikasa: [`TODO-mikasa.md`](./TODO-mikasa.md)
> README da VM: [`crates/astra-vm/README.md`](../crates/astra-vm/README.md)

---

## Alta

- [ ] **Wasm ABI rica** — strings, `Vec<T>`, slices via guest memory + alloc/free
  (`wasm32-wasip1` + WASI no `wasmi::Linker`). Spec:
  `docs/superpowers/specs/2026-06-22-alloy-wasm-interop-design.md`.
- [ ] **Wasm Fase 3** — structs, `Option`/`Result`, traits (Component Model / `wit-bindgen`).
- [ ] **`&&` / `||` short-circuit** — hoje os dois lados pré-avaliam em `interp.rs`.

---

## Média

- [ ] **`astra repl`** — loop interativo (design da VM lista como próximo passo).
- [ ] **Generics / traits no interpreter** — hoje type-erased / lenient; alinhar com
  o transpiler quando genéricos de parâmetro existirem.
- [ ] **Astra GUI** (`astra-gui/`):
  - [ ] Output streaming (linha a linha).
  - [ ] Histórico de arquivos recentes.
  - [ ] Painel de erros estruturado.
  - [ ] Packaging `.app` / `.exe` com ícone (`packaging/`, `make-app.sh`).
- [ ] **`astra check` UX** — filtrar/formatar saída crua do Miri.
- [ ] **Cache wasm** — incluir versão do rustc / flags na chave (hoje só hash do `.rs`).

---

## Baixa / polish

- [ ] **Bytecode / Cranelift JIT** — backend além do tree-walk (opcional, pós-MVP).
- [ ] **Docs de usuário** — quick start Astra (`astra run|build|check`, `mikasa vm`).
- [ ] **Release** — `scripts/release-astra.py`: notarização macOS / assets de update
  alinhados ao updater da GUI.

---

## Feito recentemente (referência)

| Item | Notas |
| --- | --- |
| Rust `.rs` via wasm (`i64`/`i32`/`bool`/`f64`) | `crates/astra-vm/src/wasm.rs` |
| Auto-wrap de `pub fn` (sem `extern "C"`) | ergonomia do spec wasm |
| `import *` / `from "./foo.rs"` | loader |
| `import("file.mika")` no interpreter | merge AST |
| Artefato `.ast` / `.loy` + wasm embutido | `astra build` |
| GUI 100% Mikasa (`astra.mika` + `backend.mika`) | host Rust fino |
| Binário CLI `astra` | alias histórico `astra` onde ainda existir |
| Suite `tests/mikasa/` + `astra-vm` `mikasa_suite` | linguagem + stdlib + file import + rust wasm |
| Docs MUI + HIG | `docs/mui.md` |
| GUI headless + `--shot` | `MOCIDA_WINDOW_HIDDEN` / `MOCIDA_SHOT_*` |
| GUI HIG dark + ícones template | `astra.mika`, `assets/icons/` |
| Paths absolutos mocida no host | `register_assets` |

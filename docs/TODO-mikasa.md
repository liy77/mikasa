# Mikasa — TODO

> Backlog do **transpilador** (`mikasa` / `cforge`), sintaxe, LSP e bridge Rust.
> Atualizado: 2026-11-23.
>
> Índice geral: [`TODO.md`](./TODO.md) · Astra: [`TODO-astra.md`](./TODO-astra.md)

---

## Alta

- [ ] **Genéricos em parâmetros de função** — `func<T> name(x: T)` ainda não transpila
  (`crates/mikasa-parser` / `parse_function`).
- [ ] **MUI codegen — `if` / `for` / `match` estruturais reativos** — hoje o branch/`for`
  não reage de verdade; spec em `docs/superpowers/specs/2026-06-17-mui-structural-codegen-design.md`.
- [ ] **Handlers + reatividade live (M3/M4)** — emitidos como placeholders no codegen MUI.
- [ ] **`?:` ternary dentro de `${expr}`** — não passa pelo rewrite de ternário.

---

## Média

- [ ] **Aliases de tipo** — `type Foo = Bar<T>` não lowera bem em `static` / `as`.
- [ ] **`obj?.field`** — devolve `Option<&T>`; documentar / facilitar `.clone()` quando preciso.
- [ ] **mikasa-lsp** — `inlayHints`, `codeAction` (auto-import), `rename`, `formatting`,
  diagnósticos de tipo (hoje só parse).
- [ ] **mui-lsp** — `inlayHints` de `signal`, `codeAction` de import, `rename` de view,
  props inválidas.
- [ ] **`mika_import!` polish** — newline-as-`,` no transpile streaming ainda precisa de
  polish genérico (hoje há heurística em `transpile_as_module`); alinhar emit do parser.
- [ ] **Turbofish em expressões** — `foo::<T>()` (spec próprio).
- [ ] **Stdlib `mika` vs `cstd`** — completar migração de docs/exemplos para `from mika`.

---

## Baixa / polish

- [ ] **Docs de usuário** — `docs/INSTALL.md` / `README.md`: quick start Mikasa
  (`mikasa init|run|vm|install|upgrade`), `import("…")`, `mika_import!`.
- [ ] **installer-gui Windows** — PATH via `winreg` + `WM_SETTINGCHANGE` end-to-end.
- [ ] **scope / unused-var analysis** no transpiler (`scope.rs` quase vazio) —
  base para type-check leve.
- [ ] **CI / hooks** — manter fmt+clippy verdes; reduzir `#![allow(...)]` legado em `src/main.rs`.

---

## Feito recentemente (referência)

| Item | Notas |
| --- | --- |
| Extensões `.mika` / `.ast` / `.mklib` / `.mipkg` | legado `.crs`/`.loy` ainda carrega |
| `mikasa init` / `install` / `upgrade` sem cargo | CLI cargo-free |
| `import("arquivo.mika")` | include dinâmico de arquivo |
| `mika_import!("arquivo.mika")` | crate `crates/mika-import` para hosts Rust |
| Branding Mikasa (bin `mikasa`) | `cforge` permanece como alias/histórico |

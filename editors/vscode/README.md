# Mikasa for VSCode

Language support for [Mikasa](https://github.com/liy77/mikasa) (`.mika`, and the
legacy `.crs`). Mikasa was formerly called Copper.

## Features

- Syntax highlighting, including `guard`, `defer`, and MocidaUI markup inside a `.mika`.
- Snippets: `guard`, `guardlet`, `defer`, `func`, `struct`, `impl`, `match`, `import`.
- Diagnostics (syntax errors, unresolved imports).
- Document symbols, go to definition, hover, signature help.
- Completion: keywords, file-local symbols, `cstd::*` helpers.
- Semantic tokens, folding, and format document.
- Run and debug the current file (Astra interprets it; `mikasa` transpiles it).

## Requirements

You need the `copper-lsp` binary on `PATH` (or set `copper.serverPath`). Build it from the copper-lang repo:

```sh
cargo build --release -p copper-lsp
# Copy target/release/copper-lsp to a PATH directory, or:
# set copper.serverPath in VSCode settings to its absolute path.
```

## Settings

| Setting | Default | What it does |
|---------|---------|--------------|
| `copper.serverPath` | `copper-lsp` | Executable to launch as the LSP server. |
| `copper.trace.server` | `off` | LSP message tracing — `off` / `messages` / `verbose`. |
| `mikasa.runtime` | `astra` | `astra` interprets, `mikasa` transpiles to Rust. |
| `mikasa.astraPath` | `alloy` | Path to the Astra interpreter. |
| `mikasa.mikasaPath` | `mikasa` | Path to the transpiler. |

## Building the extension

```sh
npm install
npm run compile
```

For local testing: `F5` in VSCode opens an Extension Development Host with the extension loaded.

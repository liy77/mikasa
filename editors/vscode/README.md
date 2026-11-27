# mikasa-lsp

Language support for [Mikasa](https://github.com/liy77/mikasa) (`.mika`, and the
legacy `.crs`). Mikasa was formerly called Mikasa.

## Features

- Syntax highlighting, including `guard`, `defer`, and MocidaUI markup inside a `.mika`.
- Snippets: `guard`, `guardlet`, `defer`, `func`, `struct`, `impl`, `match`, `import`.
- Diagnostics (syntax errors, unresolved imports).
- Document symbols, go to definition, hover, signature help.
- Completion: keywords, file-local symbols, `cstd::*` helpers.
- Semantic tokens, folding, and format document.
- Run and debug the current file (Astra interprets it; `mikasa` transpiles it).

## Requirements

You need the `mikasa-lsp` binary on `PATH` (or set `mikasa.serverPath`). Build it from the mikasa repo:

```sh
cargo build --release -p mikasa-lsp
# Copy target/release/mikasa-lsp to a PATH directory, or:
# set mikasa.serverPath in VSCode settings to its absolute path.
```

## Settings

| Setting | Default | What it does |
|---------|---------|--------------|
| `mikasa.serverPath` | `mikasa-lsp` | Executable to launch as the LSP server. |
| `mikasa.trace.server` | `off` | LSP message tracing — `off` / `messages` / `verbose`. |
| `mikasa.runtime` | `astra` | `astra` interprets, `mikasa` transpiles to Rust. |
| `mikasa.astraPath` | `astra` | Path to the Astra interpreter. |
| `mikasa.mikasaPath` | `mikasa` | Path to the transpiler. |

## Building the extension

```sh
npm install
npm run compile
```

For local testing: `F5` in VSCode opens an Extension Development Host with the extension loaded.

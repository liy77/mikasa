# Astra GUI — native host + dual-mode binary

> **Alloy is now Astra.** Window title, icon, and copy say Astra; the binary
> stays `alloy`. App code is **100% Mikasa** (`astra.mika` + `backend.mika`).
> The only Rust left is the thin mocida host in `src/main.rs` (window loop,
> interpreter I/O, file dialog, curl) — it cannot be Mikasa yet because it
> links the native mocida runtime.

## Modes

```sh
alloy run file.mika    # interpret in the terminal
alloy file.mika        # same
alloy update           # headless self-update
alloy                  # open the GUI
alloy gui              # same
```

Tabs: **Playground** (editor + Run + output), **Runner** (open a `.mika`),
**Updates** (check/install). Shortcuts: F1 / F2 / F3.

## Layout

```
astra-gui/
├── astra.mika         UI (Mikasa / MocidaUI). Hot-reloads in dev.
├── backend.mika       App logic (version, update, run copy). std-only.
├── src/backend_gen.rs Transpiled from backend.mika (do not hand-edit).
├── src/main.rs        Thin mocida host + CLI glue.
├── app.bundle         Name/id + assets.
├── assets/            Icons + marks.
├── build.rs           Windows .ico embed.
└── packaging/         Astra.app on macOS.
```

## Build

Needs sibling `mocida/mocida-rs` and:

```sh
export MOCIDA_INCLUDE_DIR=/path/to/mocida/src/headers
export MOCIDA_LIB_DIR=/path/to/mocida/release/stage/lib
export MOCIDA_LIB_NAME=mocida
export MOCIDA_STATIC=0
export DYLD_LIBRARY_PATH=$MOCIDA_LIB_DIR   # macOS runtime

cargo build --manifest-path astra-gui/Cargo.toml
# → astra-gui/target/debug/alloy
```

## After editing backend.mika

Refresh the generated Rust module (types cleaned by hand if needed):

```sh
# source of truth stays backend.mika; sync src/backend_gen.rs to match
```

## Status

- **CLI**: reuses `astra-vm`.
- **GUI**: Mikasa UI + Mikasa backend; host only drives signals / I/O.
- **Updater**: GitHub latest release; needs published binary assets.

Backlog Astra (VM + GUI): [`docs/TODO-astra.md`](../docs/TODO-astra.md).
README da VM: [`crates/astra-vm/README.md`](../crates/astra-vm/README.md).

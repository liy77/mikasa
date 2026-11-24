# Astra GUI — native host + dual-mode binary

> **Astra.** Window title, icon, and copy say Astra; the binary
> stays `astra`. App code is **100% Mikasa** (`astra.mika` + `backend.mika`).
> The only Rust left is the thin mocida host in `src/main.rs`.
>
> Design / MUI reference: [`docs/mui.md`](../docs/mui.md).

## Modes

```sh
astra run file.mika              # interpret in the terminal
astra update                     # headless self-update
astra                            # open the GUI (foreground)
astra gui                        # same
astra gui --headless             # hidden window (no focus / Dock steal)
astra gui --shot /tmp/astra.png  # headless + PNG capture + quit
```

Env equivalents: `ASTRA_GUI_HEADLESS=1`, `ASTRA_GUI_SHOT=/path.png`,
`ASTRA_GUI_SHOT_FRAME=8` (also honors mocida’s `MOCIDA_WINDOW_HIDDEN` /
`MOCIDA_SHOT_*` directly).

Tabs: **Playground** · **Runner** · **Updates**. Shortcuts: F1 / F2 / F3.

Visual language: Apple HIG macOS dark (`#1C1C1E` / systemBlue `#0A84FF` /
SF Pro Text). Icons use SF Symbols **only on Apple**; Android / Windows /
Linux get `fallback:` / `mocida://icons/*.png` (see `Icon` in `docs/mui.md`).

## Layout

```
astra-gui/
├── astra.mika         UI (Mikasa / MocidaUI). Hot-reloads in dev.
├── backend.mika       App logic (version, update, run copy).
├── src/backend_gen.rs Transpiled from backend.mika (do not hand-edit).
├── src/main.rs        Thin mocida host + CLI glue + headless/shot.
├── app.bundle         Name/id + assets.
├── assets/            Icons (template) + marks.
├── build.rs           Windows .ico embed.
└── packaging/         Astra.app on macOS.
```

## Build

Needs sibling `mocida/mocida-rs` and:

```sh
export MOCIDA_INCLUDE_DIR=/path/to/mocida/src/headers
export MOCIDA_LIB_DIR=/path/to/mocida/release/stage/lib   # or build/darwin/release-shared
export MOCIDA_LIB_NAME=mocida
export MOCIDA_STATIC=0
export DYLD_FALLBACK_LIBRARY_PATH=$MOCIDA_LIB_DIR

cargo build --manifest-path astra-gui/Cargo.toml
# → astra-gui/target/debug/astra
```

## Agent / CI screenshot loop

```sh
SHOT=/tmp/astra-gui.png
DYLD_FALLBACK_LIBRARY_PATH=$MOCIDA_LIB_DIR \
  ./astra-gui/target/debug/astra gui --shot "$SHOT" --shot-frame 10
# inspect $SHOT, tweak astra.mika, rebuild/hot-reload, repeat
```

## Status

- **CLI**: reuses `astra-vm`.
- **GUI**: Mikasa UI + Mikasa backend; host drives signals / I/O.
- **Headless**: mocida `MOCIDA_WINDOW_HIDDEN` + `MOCIDA_SHOT_*`; host quits after shot.
- **Updater**: GitHub latest release; needs published binary assets.

Backlog: [`docs/TODO-astra.md`](../docs/TODO-astra.md).
VM README: [`crates/astra-vm/README.md`](../crates/astra-vm/README.md).

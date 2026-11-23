# Astra GUI Assets

Place icons here (the app references them via `app.bundle` and `build.rs`):

- **`astra-icon.png`** — the Astra star (source for the window icon,
  the Windows `.ico`, and the macOS `.icns`). Origin: `assets/astra-logo.png`
  in the repo.
- **`astra-icon.ico`** — generated from the PNG for `build.rs` to embed in the
  `.exe` (Windows). Generate with, e.g., ImageMagick:
  `magick astra-icon.png -define icon:auto-resize=256,128,64,48,32,16 astra-icon.ico`

On macOS the `.icns` is generated automatically by `packaging/make-app.sh` from
`astra-icon.png` (via `sips`/`iconutil`) — no need to commit it.

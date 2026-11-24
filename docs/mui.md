# MocidaUI (MUI) — guia atualizado

> Data: 2026-11-24  
> Escopo: documentos `.mui` / `.mika` com `view` ou `App()`, host Astra GUI,
> e o runtime mocida. Branding: **Mikasa** (linguagem) / **Astra** (VM+GUI) /
> **MocidaUI** (UI). Extensões legadas `.crs` / `.crm` ainda abrem.

## O que é

MocidaUI é a UI declarativa do mocida. Um arquivo é documento MUI quando:

- a extensão é `.mui`, **ou**
- a extensão é `.mika` e declara `view` / `App()` (`source_is_mui`).

Caso contrário, `.mika` transpila / interpreta como Mikasa puro.

Dois verbos no toolchain:

| Comando | Efeito |
| --- | --- |
| `mikasa run foo.mui` (ou `cforge run`) | Dev render via `mui-dev` (nunca linka mocida no crate portátil) |
| `mikasa -c [-r] [-b] foo.mui` | Codegen M5 → projeto cargo em `<out>/mui/` |

Astra GUI (`astra-gui/`) é um **host separado**: parseia `astra.mika`, linka
mocida + `mui-runtime`, e injeta signals do backend.

## Anatomia de um documento

```mika
App() {
  name: "Astra"
  width: 1120
  height: 740
  minWidth: 800
  minHeight: 560
  background: #1C1C1E
  mainContent: AstraApp
  host: "."
}

view AstraApp() {
  mut nav = signal(0)
  // …
  Stack(orientation: horizontal, align: stretch, …) { … }
}
```

- `App()` — config da janela (`window_config`).
- `view Name(params…)` — árvore reativa; `signal` + rebuild em `take_dirty`.
- Widgets: `Stack`, `Rectangle`, `Text`, `TextArea`, `Input`, `Image`,
  `Switch`, `Button`, …

## Layout (gotchas do runtime)

- `Stack.align` = eixo cruzado; `Stack.justify` = eixo principal.
- `align: stretch` **não** é válido — para pin à esquerda em coluna stretch:
  `width:` explícito + `justify: start`.
- Resize é **host-driven**: poll `UIApp_GetWidthG/HeightG` e rebuild.
- `Button` color é **literal-only** — `if/else` em `background:`/`textColor:`
  cai no azul default. Preferir dois botões com cores literais.
- `Input`/`TextField` = single-line; multi-line → `TextArea`.
- `renderer: vulkan` falha no macOS — omitir (Metal).

## Assets (`mocida://`) e `Icon`

`Image("mocida://…")` só resolve se o host registrou caminho **absoluto**:

```rust
mocida::bundle::set("mocida://icon", "/abs/path/to/app.png");
```

Para ícones de UI, preferir o componente **`Icon`** (mocida `UIIcon`):

```mika
// SF no Apple; PNG em Android/Windows/Linux (obrigatório fora da Apple)
Icon("play.fill", fallback: "mocida://icons/play.png", size: 18, tint: #FFFFFF)
Icon("sf://folder.fill", fallback: "mocida://icons/folder.png", size: 16, tint: #98989D)
Icon(source: "mocida://logo.png", size: 24)         // imagem (todas as plataformas)
Icon(glyph: "⌘", fontFamily: "SF Pro Text", size: 16, tint: #F5F5F7)  // glifo de fonte
```

**Apple only:** nomes bare (`play.fill`) / `sf://…` → AppKit
`NSImage systemSymbolName`. **Android / Windows / Linux / web:** nunca SF —
`fallback:` (PNG/`mocida://`) ou o mapa built-in em `UIIcon_ResolveSourceEx`
(`play.fill` → `mocida://icons/play.png`, etc.). Sem fallback e sem mapa →
tenta `mocida://icons/<name>.png`.

`app.bundle` aceita paths relativos ao CWD — no Astra GUI o host **sempre**
re-registra absolutos após `load_manifest`.

## Headless + screenshot (mocida)

Já existe no C toolkit — não reinventar:

| Env | Efeito |
| --- | --- |
| `MOCIDA_WINDOW_HIDDEN=1` | Janela criada hidden; `ShowWindow` no-op; macOS background app |
| `MOCIDA_SHOT_FRAME=N` + `MOCIDA_SHOT_PATH=file.png` | Salva o frame N como PNG |
| `MOCIDA_SCREENSHOT_DIR=…` | Destino do screenshot do device simulator |
| `MOCIDA_WINDOW_MINIMIZED=1` | Inicia minimizado |
| `MOCIDA_WINDOW_DISPLAY=N` | Centraliza no display N (1-based) |

API programática: `UIWindow_RequestShot(path)` (próximo frame).

Astra GUI expõe isso de forma amigável:

```sh
# Usuário normal — janela em primeiro plano
astra gui

# Agente / CI — sem foco, captura e sai
astra gui --headless --shot /tmp/astra.png --shot-frame 8
# ou:
ASTRA_GUI_HEADLESS=1 ASTRA_GUI_SHOT=/tmp/astra.png astra gui
```

`--shot` implica headless. O host chama `UIApp_CloseG` após o frame da captura.

## Text + signals

- Prefer `Text(name)` or `Text(format("v{}", name))` for live string signals.
  A literal `"v${name}"` can drop the interpolated part in some parse paths
  (only the static `"v"` remains).
- `SegmentedControl` exists in mocida, but destroying it on structural rebuild
  (`UIApp_SetChildren`) currently crashes — keep ModeChip-style stacks for now.

## Design language (Apple HIG)

O `astra.mika` de referência segue o HIG macOS (dark):

| Token | Valor | Uso |
| --- | --- | --- |
| Background | `#1C1C1E` | `systemBackground` |
| Sidebar / elevated | `#2C2C2E` | secondary grouped |
| Card / field | `#3A3A3C` | tertiary fill |
| Separator | `#38383A` | hairline |
| Label | `#F5F5F7` | primary |
| Secondary label | `#98989D` | captions |
| Accent | `#0A84FF` | systemBlue (dark) |
| Destructive / Run | `#0A84FF` (primário) | CTA |
| Radius | 8–12 | controles; 10 sidebar row |
| Font | `SF Pro Text` | corpo; Display só em títulos grandes |
| Spacing | 8 / 12 / 16 / 20 | ritmo HIG |

Evitar: vermelho neon como “selecionado”, pills excessivos, glow, cards
aninhados sem necessidade. Uma coluna de nav + um painel de conteúdo.

## Pipeline de hosts

```
astra.mika ──parse──► mui-syntax AST
                 │
                 ▼
           mui-runtime::build_view_seeded
                 │
                 ▼
           mocida App (SDL3 / Metal)
                 ▲
        on_tick: dirty / resize / hot-reload / backend I/O
```

`backend.mika` → `src/backend_gen.rs` (lógica sem mocida). O host Rust só
faz janela, interpreter I/O, file dialog e curl de update.

## Referências

- Spec GUI: [`docs/superpowers/specs/2026-06-22-astra-gui-design.md`](./superpowers/specs/2026-06-22-astra-gui-design.md)
- Codegen M5: [`docs/superpowers/specs/2026-06-17-mui-structural-codegen-design.md`](./superpowers/specs/2026-06-17-mui-structural-codegen-design.md)
- Host README: [`astra-gui/README.md`](../astra-gui/README.md)
- Backlog: [`docs/TODO-astra.md`](./TODO-astra.md)

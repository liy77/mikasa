//! Astra — dual-mode binary. (The interpreter was renamed from Alloy; the
//! binary is still `alloy`.)
//!
//! * `alloy run <file.crs>` / `alloy <file.crs>` → interpret in the terminal
//!   (reuses the portable `astra-vm` lib; output goes straight to stdout).
//! * `alloy update` → run the self-updater headlessly.
//! * `alloy` (no args) / `alloy gui` → open the MUI window (playground + file
//!   runner + updater).
//!
//! The GUI half links mocida + mui-runtime; the CLI half does not touch them,
//! so the same binary serves both. Build with:
//!   cargo build --release --manifest-path astra-gui/Cargo.toml
//! (needs MOCIDA_INCLUDE_DIR / MOCIDA_LIB_DIR — see mocida-sys).

// The GUI itself is Mikasa (`astra.mui`). This host is the one piece that has
// to be Rust: it links mocida and drives the window. The decisions it asks
// about — version comparison, reading the release JSON — live in
// `backend.mika` and are transpiled into the module below.
#[path = "backend_gen.rs"]
mod backend;

use std::path::Path;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();

    match args.first().map(String::as_str) {
        None | Some("gui") => run_gui(),
        Some("update") => match check_for_update() {
            info if info.available => match apply_release(&info.asset_url) {
                Ok(()) => {
                    println!("Astra updated to {}", info.latest);
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("alloy: update failed: {e}");
                    ExitCode::FAILURE
                }
            },
            info => {
                println!("Astra is already the latest version ({})", info.latest);
                ExitCode::SUCCESS
            }
        },
        Some("run") => match args.get(1) {
            Some(file) => cli_run(Path::new(file)),
            None => {
                eprintln!("usage: alloy run <file.crs>");
                ExitCode::FAILURE
            }
        },
        // Bare path: `alloy foo.crs`
        Some(path) if Path::new(path).exists() => cli_run(Path::new(path)),
        Some(other) => {
            eprintln!("alloy: unknown command `{other}` (use: run <file> | update | gui)");
            ExitCode::FAILURE
        }
    }
}

/// CLI interpretation — output to real stdout (not captured), like `astra-vm`'s
/// own binary. Reuses the portable interpreter.
fn cli_run(file: &Path) -> ExitCode {
    use astra_vm::interp::Interpreter;
    use mikasa_syntax::program::parse_program;

    let src = match std::fs::read_to_string(file) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("alloy: could not read {}: {e}", file.display());
            return ExitCode::FAILURE;
        }
    };
    let prog = parse_program(&src);
    if !prog.errors.is_empty() {
        for err in &prog.errors {
            eprintln!(
                "alloy: syntax error @ {}..{}: {}",
                err.span.start, err.span.end, err.message
            );
        }
        return ExitCode::FAILURE;
    }
    match Interpreter::new().run_program(&prog) {
        Ok(_) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!(
                "alloy: runtime error @ {}..{}: {}",
                e.span.start, e.span.end, e.message
            );
            ExitCode::FAILURE
        }
    }
}

// ===========================================================================
// GUI host (mocida + mui-runtime)
// ===========================================================================

/// The `.mui` baked into the binary (used in release / when the source file
/// isn't on disk). In dev, the on-disk `astra.mui` is preferred and watched for
/// changes so edits hot-reload live.
const MUI_BAKED: &str = include_str!("../astra.mui");

/// On-disk path of `astra.mui` (next to the crate), for dev hot-reload.
fn mui_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("astra.mui")
}

/// Register the marks and button glyphs under absolute paths, so they resolve
/// no matter which directory the binary was launched from.
fn register_assets(manifest_dir: &std::path::Path) {
    let _ = manifest_dir; // kept so the signature reads as the source of paths
    let star = manifest_dir
        .parent()
        .map(|p| p.join("assets").join("astra-logo.png"))
        .filter(|p| p.is_file());
    if let Some(star) = star {
        mocida::bundle::set("mocida://astra-logo.png", &star.to_string_lossy());
    }
    let mac_icon = manifest_dir.join("assets").join("astra-icon-mac.png");
    if mac_icon.is_file() {
        mocida::bundle::set("mocida://icon", &mac_icon.to_string_lossy());
    }
    let win_icon = manifest_dir.join("assets").join("astra-icon-win.png");
    if win_icon.is_file() {
        mocida::bundle::set("mocida://astra-icon-win.png", &win_icon.to_string_lossy());
    }
    for name in ["play", "code", "terminal", "update", "folder", "split"] {
        let path = manifest_dir
            .join("assets")
            .join("icons")
            .join(format!("{name}.png"));
        if path.is_file() {
            mocida::bundle::set(
                &format!("mocida://icons/{name}.png"),
                &path.to_string_lossy(),
            );
        }
    }
}

/// Run editor source through the interpreter. The backend decides nothing
/// here — running is the host's job because it owns the interpreter.
fn run_editor_source(src: &str) -> backend_run::RunResult {
    backend_run::run(src)
}

struct ReleaseInfo {
    latest: String,
    available: bool,
    asset_url: String,
}

/// Ask GitHub, then let the Mikasa backend decide whether the release is
/// newer and which asset matches this OS.
fn check_for_update() -> ReleaseInfo {
    let current = backend::current_version();
    let body = std::process::Command::new("curl")
        .args([
            "-sSL",
            "-H",
            "User-Agent: astra",
            "https://api.github.com/repos/liy77/mikasa/releases/latest",
        ])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default();
    let os_key = if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "apple"
    } else {
        "linux"
    };
    // "current|latest|yes|asset" — the format backend.mika's describe_update
    // returns.
    let described = backend::describe_update(&body, &current, os_key);
    let parts: Vec<&str> = described.splitn(4, '|').collect();
    ReleaseInfo {
        latest: parts.get(1).unwrap_or(&"").to_string(),
        available: parts.get(2).copied() == Some("yes"),
        asset_url: parts.get(3).unwrap_or(&"").to_string(),
    }
}

/// Download a release and swap it over the running executable.
fn apply_release(url: &str) -> Result<(), String> {
    if url.is_empty() {
        return Err("no release asset for this platform".into());
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let tmp = exe.with_extension("new");
    let status = std::process::Command::new("curl")
        .args(["-sSL", "-o"])
        .arg(&tmp)
        .arg(url)
        .status()
        .map_err(|e| e.to_string())?;
    if !status.success() {
        return Err("download failed".into());
    }
    std::fs::rename(&tmp, &exe).map_err(|e| e.to_string())
}

/// The interpreter, kept in its own module so the Mikasa backend stays free
/// of it. `backend.mika` answers questions; this runs programs.
mod backend_run {
    use std::cell::RefCell;
    use std::rc::Rc;

    use astra_vm::interp::Interpreter;
    use mikasa_syntax::program::parse_program;

    pub struct RunResult {
        pub output: String,
        pub error: Option<String>,
    }

    pub fn run(src: &str) -> RunResult {
        let prog = parse_program(src);
        if !prog.errors.is_empty() {
            let msg = prog
                .errors
                .iter()
                .map(|e| format!("syntax @ {}..{}: {}", e.span.start, e.span.end, e.message))
                .collect::<Vec<_>>()
                .join("\n");
            return RunResult {
                output: String::new(),
                error: Some(msg),
            };
        }
        let buf: Rc<RefCell<String>> = Rc::new(RefCell::new(String::new()));
        let result = Interpreter::with_output(Rc::clone(&buf)).run_program(&prog);
        let output = buf.borrow().clone();
        RunResult {
            output,
            error: result
                .err()
                .map(|e| format!("runtime @ {}..{}: {}", e.span.start, e.span.end, e.message)),
        }
    }
}

/// Current `.mui` source: the on-disk file if present, else the baked copy.
fn read_mui_source() -> String {
    std::fs::read_to_string(mui_path()).unwrap_or_else(|_| MUI_BAKED.to_string())
}

fn run_gui() -> ExitCode {
    match try_run_gui() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("alloy: failed to open the GUI: {e}");
            ExitCode::FAILURE
        }
    }
}

fn try_run_gui() -> Result<(), Box<dyn std::error::Error>> {
    // Before anything else. The first failed lookup of a `mocida://` key
    // caches that failure inside mocida, and App::new looks the icon up while
    // it loads app.bundle — so the registration has to win that race.
    register_assets(Path::new(env!("CARGO_MANIFEST_DIR")));
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::rc::Rc;

    use mocida::{App, Color};
    use mui_runtime::Reactive;

    // 1. Parse the .mui to extract the App{} config + entry view. Read from
    //    disk when available (dev hot-reload), else the baked copy.
    let mui_src = read_mui_source();
    let doc = mui_syntax::parse(&mui_src);
    let view = mui_runtime::entry_view(&doc).ok_or("no view found in astra.mui")?;
    let view_name = view.name.clone();
    let cfg = mui_runtime::window_config(&doc, &view_name);

    // 2. Pre-creation flags (must precede App::new).
    mui_runtime::prefer_custom_titlebar(&cfg);
    mui_runtime::prefer_renderer(&cfg);

    // Register every asset the window touches BEFORE it exists. The manifest
    // is loaded here too, then overridden with absolute paths — its own
    // entries are relative and only resolve from the crate directory.
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let bundle = manifest_dir.join("app.bundle");
    if bundle.is_file() {
        mocida::bundle::load_manifest(&bundle.to_string_lossy());
    }
    register_assets(manifest_dir);

    // Assets have to be registered before the window exists: the first frame
    // loads the logo, and a registration made afterwards never reaches it.
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let star = manifest_dir
        .parent()
        .map(|p| p.join("assets").join("astra-logo.png"))
        .filter(|p| p.is_file());
    if let Some(star) = &star {
        mocida::bundle::set("mocida://astra-logo.png", &star.to_string_lossy());
    }
    let win_icon = manifest_dir.join("assets").join("astra-icon-win.png");
    if win_icon.is_file() {
        mocida::bundle::set("mocida://astra-icon-win.png", &win_icon.to_string_lossy());
    }
    // The squircle mark, registered under the name the bundle manifest and the
    // header both use. Done here so it resolves before the window opens.
    let mac_icon = manifest_dir.join("assets").join("astra-icon-mac.png");
    if mac_icon.is_file() {
        mocida::bundle::set("mocida://icon", &mac_icon.to_string_lossy());
    }
    // Glyphs for the buttons. Material Symbols on every platform — SF Symbols
    // only load inside an Apple-signed bundle.
    for name in ["play", "code", "terminal", "update", "folder", "split"] {
        let path = manifest_dir.join("assets").join("icons").join(format!("{name}.png"));
        if path.is_file() {
            mocida::bundle::set(
                &format!("mocida://icons/{name}.png"),
                &path.to_string_lossy(),
            );
        }
    }

    // 3. Load app.bundle (name, id) and register the icon with an ABSOLUTE
    //    path, so `mocida://alloy-icon.png` resolves regardless of the CWD the
    //    binary is launched from (the manifest's relative path would not).
    // Register the assets BEFORE the manifest loads. `load_manifest` reads the
    // window icon immediately, and a relative entry in app.bundle only
    // resolves from the CWD — so an absolute registration has to win first.
    let icon = manifest_dir.join("assets").join("alloy-icon.png");
    if icon.is_file() {
        mocida::bundle::set("mocida://alloy-icon.png", &icon.to_string_lossy());
    }
    let logo = manifest_dir.join("assets").join("alloy-logo.png");
    if logo.is_file() {
        mocida::bundle::set("mocida://alloy-logo.png", &logo.to_string_lossy());
    }
    // The Astra mark (the star). The dove is Mikasa's; this window is Astra's.
    let star = manifest_dir
        .parent()
        .map(|p| p.join("assets").join("astra-logo.png"))
        .filter(|p| p.is_file());
    if let Some(star) = star {
        mocida::bundle::set("mocida://astra-logo.png", &star.to_string_lossy());
    }
    // The manifest was already loaded (and its relative entries overridden
    // with absolute paths) before the window flags, so nothing here reloads it.

    // 4. Create the window.
    let (br, bg, bb, _) = cfg.background;
    let mut app = App::new(&cfg.title, cfg.width, cfg.height)?;
    // App::new reloads app.bundle from the CWD and wipes every registration
    // made before it, so the absolute paths go in again now.
    register_assets(manifest_dir);
    app.set_background_color(Color::rgb(br as i32, bg as i32, bb as i32));
    if let Some(name) = &cfg.name {
        mocida::bundle::set_name(name);
        let _ = app.set_name(name);
    }
    if let Some(id) = &cfg.id {
        let _ = app.set_app_id(id);
    }
    if cfg.min_width > 0 || cfg.min_height > 0 {
        app.set_min_size(cfg.min_width, cfg.min_height);
    }

    // 5. Render tuning + backdrop + window icon.
    mui_runtime::apply_render_config(&mut app, &cfg);
    mui_runtime::apply_backdrop(&cfg);
    // The platform-shaped mark: a squircle on macOS, a rounded square on
    // Windows and Linux. The plain star is for inside the window.
    let app_icon = if cfg!(target_os = "macos") {
        "mocida://icon"
    } else {
        "mocida://astra-icon-win.png"
    };
    if app.set_window_icon(app_icon).unwrap_or(false) {
        // custom icon set
    } else {
        mui_runtime::set_default_window_icon(&mut app);
    }

    // 6. Fonts.
    mocida::text::search_fonts();
    let _ = mocida::text::get_font("Arial");

    // 7. Build the initial tree, seed signals. `source_cell` holds the live
    //    `.mui` text so the on-tick hot-reload can swap it without a restart.
    let source_cell = Rc::new(RefCell::new(mui_src.clone()));
    let loaded = mui_syntax::loader::load_from(&mui_src, Path::new("astra.mui"));
    let components = loaded.registry();
    let (children, reactive) =
        mui_runtime::build_view_seeded(view, &components, &HashMap::new())?;
    reactive.set_str("version", &backend::current_version());
    reactive.set_str("output", "ready — write some Mikasa and hit Run");
    // TextArea renders real newlines, so a multi-line default is fine.
    reactive.set_str(
        "source",
        "func main() {\n    println(\"hello from Astra\")\n}\n",
    );

    let slot: Rc<RefCell<Option<Reactive>>> = Rc::new(RefCell::new(Some(reactive)));
    app.set_children(children);

    // 8. On-tick: handle Run/Open/Update requests + rebuild on resize / structural change.
    // App::new reloads app.bundle a second time (from the exe dir) after every
    // registration made before it, so the assets go in once more on the first
    // tick — past both reloads — and the tree rebuilds so the images resolve.
    let assets_registered = Rc::new(std::cell::Cell::new(false));
    let app_ptr = app.as_ptr();
    let tick_slot = Rc::clone(&slot);
    // Remember the last-seen trigger counters so we only act on a change.
    let last_run = Rc::new(RefCell::new(0i32));
    let last_update = Rc::new(RefCell::new(0i32));
    let last_open = Rc::new(RefCell::new(0i32));
    // Last (win w, win h, screen w, screen h) we built against — seeded zeros so
    // the first tick rebuilds against the realized size, then again on any resize
    // so `Window.width`/`Window.height` (and `width: Window.width - N`) re-resolve.
    let last_size = std::rc::Rc::new(std::cell::Cell::new((0i32, 0i32, 0i32, 0i32)));
    // Hot-reload watch: the live `.mui` source cell + the last-seen mtime.
    let watch_source = Rc::clone(&source_cell);
    let last_mtime: Rc<RefCell<Option<std::time::SystemTime>>> = Rc::new(RefCell::new(None));

    // Shared rebuild closure — re-runs build against the current window size,
    // seeding from the live signal values so user state survives the swap.
    let rebuild = {
        let tick_slot = Rc::clone(&tick_slot);
        let source_cell = Rc::clone(&source_cell);
        move || {
            let seed = tick_slot
                .borrow()
                .as_ref()
                .map(|r| r.values())
                .unwrap_or_default();
            let src = source_cell.borrow().clone();
            let loaded = mui_syntax::loader::load_from(&src, Path::new("astra.mui"));
            let comps = loaded.registry();
            if let Some(v) = mui_runtime::entry_view(&loaded.entry) {
                if let Ok((c, r)) = mui_runtime::build_view_seeded(v, &comps, &seed) {
                    let raw = c.into_raw();
                    unsafe {
                        mocida::sys::UIApp_SetChildren(app_ptr, raw);
                    }
                    *tick_slot.borrow_mut() = Some(r);
                }
            }
        }
    };

    let assets_flag = Rc::clone(&assets_registered);
    app.on_tick(move || {
        if !assets_flag.get() {
            assets_flag.set(true);
            register_assets(Path::new(env!("CARGO_MANIFEST_DIR")));
            rebuild();
        }
        // (-1) Hot-reload: if astra.mui changed on disk, reload + rebuild live.
        let cur_m = std::fs::metadata(mui_path())
            .ok()
            .and_then(|md| md.modified().ok());
        if let Some(m) = cur_m {
            let prev = *last_mtime.borrow();
            if prev != Some(m) {
                *last_mtime.borrow_mut() = Some(m);
                if prev.is_some() {
                    *watch_source.borrow_mut() = read_mui_source();
                    rebuild();
                }
            }
        }

        // (0) Window resize / monitor change → rebuild so `Window.*` re-resolve.
        let cur_size = unsafe {
            (
                mocida::sys::UIApp_GetWidthG(),
                mocida::sys::UIApp_GetHeightG(),
                mocida::sys::UIScreen_GetWidth(),
                mocida::sys::UIScreen_GetHeight(),
            )
        };
        if cur_size.0 > 0 && cur_size.1 > 0 && cur_size != last_size.get() {
            last_size.set(cur_size);
            rebuild();
        }

        if let Some(r) = tick_slot.borrow().as_ref() {
            // --- Run trigger (incremented by the Run button in the .mui) ---
            // F1/F2/F3 switch views. Polled rather than handled in the .mui:
            // a key handler only fires on the widget that holds focus, and
            // the editor usually has it.
            if mocida::input::key_down("F1") {
                r.set_int("nav", 0);
            } else if mocida::input::key_down("F2") {
                r.set_int("nav", 1);
            } else if mocida::input::key_down("F3") {
                r.set_int("nav", 2);
            }

            let run_n = r.get("run_trigger").unwrap_or(0);
            if run_n != *last_run.borrow() {
                *last_run.borrow_mut() = run_n;
                // Prefer the path field (File runner) when it points at a real
                // file; otherwise run the editor `source` (Playground).
                let path = r.get_str("path_field").unwrap_or_default();
                let src = if !path.is_empty() && std::path::Path::new(&path).is_file() {
                    std::fs::read_to_string(&path).unwrap_or_default()
                } else {
                    r.get_str("source").unwrap_or_default()
                };
                // The decision to run is the host's (it owns the interpreter);
                // the backend only answers questions about the result.
                let res = run_editor_source(&src);
                let failed = res.error.is_some();
                let shown = match res.error {
                    Some(err) => format!("{}{}", res.output, err),
                    None => res.output,
                };
                let mark = if failed { "failed" } else { "ran" };
                r.set_str("output", &shown);
                r.set_str("status", mark);
            }

            // --- Open file trigger: native OS file picker (rfd) ---
            let open_n = r.get("open_trigger").unwrap_or(0);
            if open_n != *last_open.borrow() {
                *last_open.borrow_mut() = open_n;
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Mikasa", &["mika", "crs"])
                    .pick_file()
                {
                    let p = path.to_string_lossy().into_owned();
                    match std::fs::read_to_string(&p) {
                        Ok(src) => {
                            r.set_str("source", &src);
                            r.set_str("path_field", &p);
                            r.set_str("output", "");
                        }
                        Err(e) => r.set_str("output", &e.to_string()),
                    }
                }
            }

            // --- Update trigger ---
            let upd_n = r.get("update_trigger").unwrap_or(0);
            if upd_n != *last_update.borrow() {
                *last_update.borrow_mut() = upd_n;
                let info = check_for_update();
                let msg = if info.available {
                    match apply_release(&info.asset_url) {
                        Ok(()) => format!("Updated to {} — restart Astra.", info.latest),
                        Err(e) => format!("Update failed: {e}"),
                    }
                } else {
                    format!("Already up to date ({}).", info.latest)
                };
                r.set_str("update_status", &msg);
            }
        }

        // --- Structural rebuild when an if/for signal changed (e.g. tab) ---
        let dirty = tick_slot
            .borrow()
            .as_ref()
            .map(|r| r.take_dirty())
            .unwrap_or(false);
        if dirty {
            rebuild();
        }
    });

    // 9. Run (blocks until the window closes).
    app.show().run();
    drop(slot);
    Ok(())
}

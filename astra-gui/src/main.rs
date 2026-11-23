//! Astra — dual-mode binary (`alloy`).
//!
//! Authored app code is Mikasa: `astra.mika` (UI) + `backend.mika` (logic).
//! This file is the unavoidable mocida host + CLI glue — no app decisions here.

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
                eprintln!("usage: alloy run <file.mika>");
                ExitCode::FAILURE
            }
        },
        Some(path) if Path::new(path).exists() => cli_run(Path::new(path)),
        Some(other) => {
            eprintln!("alloy: unknown command `{other}` (use: run <file> | update | gui)");
            ExitCode::FAILURE
        }
    }
}

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

const MUI_BAKED: &str = include_str!("../astra.mika");

fn mui_path() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("astra.mika")
}

/// Register assets under absolute paths so `mocida://` resolves no matter
/// which directory the binary was launched from.
fn register_assets(manifest_dir: &Path) {
    let mac_icon = manifest_dir.join("assets").join("astra-icon-mac.png");
    if mac_icon.is_file() {
        mocida::bundle::set("mocida://icon", &mac_icon.to_string_lossy());
    }
    let win_icon = manifest_dir.join("assets").join("astra-icon-win.png");
    if win_icon.is_file() {
        mocida::bundle::set("mocida://astra-icon-win.png", &win_icon.to_string_lossy());
    }
    let logo = manifest_dir.join("assets").join("astra-logo.png");
    if logo.is_file() {
        mocida::bundle::set("mocida://astra-logo.png", &logo.to_string_lossy());
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

struct ReleaseInfo {
    latest: String,
    available: bool,
    asset_url: String,
}

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
    let described = backend::describe_update(&body, &current, os_key);
    let parts: Vec<&str> = described.splitn(4, '|').collect();
    ReleaseInfo {
        latest: parts.get(1).unwrap_or(&"").to_string(),
        available: parts.get(2).copied() == Some("yes"),
        asset_url: parts.get(3).unwrap_or(&"").to_string(),
    }
}

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
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    register_assets(manifest_dir);

    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::rc::Rc;

    use mocida::{App, Color};
    use mui_runtime::Reactive;

    let mui_src = read_mui_source();
    let doc = mui_syntax::parse(&mui_src);
    let view = mui_runtime::entry_view(&doc).ok_or("no view found in astra.mika")?;
    let view_name = view.name.clone();
    let cfg = mui_runtime::window_config(&doc, &view_name);

    mui_runtime::prefer_custom_titlebar(&cfg);
    mui_runtime::prefer_renderer(&cfg);

    let bundle = manifest_dir.join("app.bundle");
    if bundle.is_file() {
        mocida::bundle::load_manifest(&bundle.to_string_lossy());
    }
    register_assets(manifest_dir);

    let (br, bg, bb, _) = cfg.background;
    let mut app = App::new(&cfg.title, cfg.width, cfg.height)?;
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

    mui_runtime::apply_render_config(&mut app, &cfg);
    mui_runtime::apply_backdrop(&cfg);
    let app_icon = if cfg!(target_os = "macos") {
        "mocida://icon"
    } else {
        "mocida://astra-icon-win.png"
    };
    if !app.set_window_icon(app_icon).unwrap_or(false) {
        mui_runtime::set_default_window_icon(&mut app);
    }

    mocida::text::search_fonts();
    let _ = mocida::text::get_font("Arial");

    let source_cell = Rc::new(RefCell::new(mui_src.clone()));
    let loaded = mui_syntax::loader::load_from(&mui_src, Path::new("astra.mika"));
    let components = loaded.registry();
    let (children, reactive) =
        mui_runtime::build_view_seeded(view, &components, &HashMap::new())?;
    reactive.set_str("version", &backend::current_version());
    reactive.set_str("output", &backend::ready_message());
    reactive.set_str("source", &backend::default_source());

    let slot: Rc<RefCell<Option<Reactive>>> = Rc::new(RefCell::new(Some(reactive)));
    app.set_children(children);

    let assets_registered = Rc::new(std::cell::Cell::new(false));
    let app_ptr = app.as_ptr();
    let tick_slot = Rc::clone(&slot);
    let last_run = Rc::new(RefCell::new(0i32));
    let last_update = Rc::new(RefCell::new(0i32));
    let last_open = Rc::new(RefCell::new(0i32));
    let last_size = std::rc::Rc::new(std::cell::Cell::new((0i32, 0i32, 0i32, 0i32)));
    let watch_source = Rc::clone(&source_cell);
    let last_mtime: Rc<RefCell<Option<std::time::SystemTime>>> = Rc::new(RefCell::new(None));

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
            let loaded = mui_syntax::loader::load_from(&src, Path::new("astra.mika"));
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
                let path = r.get_str("path_field").unwrap_or_default();
                let src = if !path.is_empty() && Path::new(&path).is_file() {
                    std::fs::read_to_string(&path).unwrap_or_default()
                } else {
                    r.get_str("source").unwrap_or_default()
                };
                let res = backend_run::run(&src);
                let failed = res.error.is_some();
                let err = res.error.unwrap_or_default();
                r.set_str(
                    "output",
                    &backend::format_run_output(&res.output, &err),
                );
                r.set_str("status", &backend::format_run_status(failed));
            }

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

            let upd_n = r.get("update_trigger").unwrap_or(0);
            if upd_n != *last_update.borrow() {
                *last_update.borrow_mut() = upd_n;
                let info = check_for_update();
                let msg = if info.available {
                    match apply_release(&info.asset_url) {
                        Ok(()) => backend::update_applied(&info.latest),
                        Err(e) => backend::update_failed(&e),
                    }
                } else {
                    backend::update_current(&info.latest)
                };
                r.set_str("update_status", &msg);
            }
        }

        let dirty = tick_slot
            .borrow()
            .as_ref()
            .map(|r| r.take_dirty())
            .unwrap_or(false);
        if dirty {
            rebuild();
        }
    });

    app.show().run();
    drop(slot);
    Ok(())
}

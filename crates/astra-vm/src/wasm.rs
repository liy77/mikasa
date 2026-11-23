//! Running imported Rust (`.rs`) inside Astra via WebAssembly.
//!
//! The interpreter can't execute Rust directly. Instead of delegating the whole
//! program to `cforge` (the old `NeedsCforge` path), Astra compiles each
//! imported `.rs` to a **`wasm32-unknown-unknown`** module once (cached by
//! content hash under `~/.alloy/cache`), then runs it with an embedded
//! [`wasmi`] interpreter. Mikasa stays tree-walked (instant); only the Rust
//! leaves cross into wasm. This keeps `.ast` / `.loy` portable — wasm is itself
//! a platform-independent bytecode.
//!
//! **Supported ABI (scalars):** `i64` / `i32` / `bool` / `f64` (and narrower
//! integer widths widened at the boundary). The `.rs` may export
//! `#[no_mangle] pub extern "C"` directly, **or** declare ordinary
//! `pub fn` — Astra auto-generates an export-wrapper cdylib so idiomatic Rust
//! modules work from `.mika` without the `extern "C"` boilerplate.
//!
//! Richer types (strings/`Vec`/structs via WASI + alloc ABI) are the next phase
//! — see `docs/superpowers/specs/2026-06-22-alloy-wasm-interop-design.md`.

use std::path::{Path, PathBuf};
use wasmi::{Engine, Instance, Linker, Module, Store, Val, ValType, F32, F64};

/// A compiled+instantiated `.rs` module, ready to call.
pub struct WasmRuntime {
    store: Store<()>,
    instance: Instance,
}

impl WasmRuntime {
    /// Compiles `rs_path` to wasm (cached) and instantiates it. Prefer
    /// [`Self::from_rs_named`] when the Mikasa import list is known — that path
    /// can wrap plain `pub fn` into wasm exports.
    pub fn from_rs(rs_path: &Path) -> Result<Self, String> {
        Self::from_rs_named(rs_path, &[])
    }

    /// Like [`Self::from_rs`], but ensures every `name` is a callable export —
    /// generating an `extern "C"` wrapper around plain `pub fn` when needed.
    pub fn from_rs_named(rs_path: &Path, names: &[String]) -> Result<Self, String> {
        let bytes = compile_rs_to_wasm_named(rs_path, names)?;
        Self::from_wasm(&bytes)
    }

    /// Instantiates an in-memory wasm module.
    pub fn from_wasm(wasm: &[u8]) -> Result<Self, String> {
        let engine = Engine::default();
        let module = Module::new(&engine, wasm).map_err(|e| format!("invalid wasm: {e}"))?;
        let mut store = Store::new(&engine, ());
        let linker = <Linker<()>>::new(&engine);
        let instance = linker
            .instantiate_and_start(&mut store, &module)
            .map_err(|e| format!("wasm link/start failed: {e}"))?;
        Ok(Self { store, instance })
    }

    /// `true` if the module exports a callable function named `name`.
    pub fn exports(&self, name: &str) -> bool {
        self.instance.get_func(&self.store, name).is_some()
    }

    /// Calls an exported function with `i64` args, returning its `i64` result
    /// (or `0` for a `()`-returning export). Prefer [`Self::call`] when arg
    /// types may include floats.
    pub fn call_i64(&mut self, name: &str, args: &[i64]) -> Result<i64, String> {
        let vals: Vec<Val> = args.iter().map(|n| Val::I64(*n)).collect();
        match self.call_raw(name, &vals)? {
            Some(Val::I64(n)) => Ok(n),
            Some(Val::I32(n)) => Ok(n as i64),
            Some(Val::F64(n)) => Ok(n.to_float() as i64),
            Some(Val::F32(n)) => Ok(n.to_float() as i64),
            None => Ok(0),
            other => Err(format!(
                "wasm call `{name}`: unexpected result {other:?}"
            )),
        }
    }

    /// Calls an export, marshaling host scalars to the export's declared
    /// param types and lifting the result.
    pub fn call(&mut self, name: &str, args: &[WasmArg]) -> Result<WasmRet, String> {
        let func = self
            .instance
            .get_func(&self.store, name)
            .ok_or_else(|| format!("wasm export `{name}` not found"))?;
        let ty = func.ty(&self.store);
        let params = ty.params();
        if params.len() != args.len() {
            return Err(format!(
                "wasm call `{name}`: expects {} args, got {}",
                params.len(),
                args.len()
            ));
        }
        let mut vals = Vec::with_capacity(args.len());
        for (i, (want, got)) in params.iter().zip(args.iter()).enumerate() {
            vals.push(marshal_arg(*want, got).map_err(|e| {
                format!("wasm call `{name}` arg {i}: {e}")
            })?);
        }
        let raw = self.call_raw(name, &vals)?;
        Ok(match raw {
            None => WasmRet::Unit,
            Some(Val::I64(n)) => WasmRet::Int(n),
            Some(Val::I32(n)) => WasmRet::Int(n as i64),
            Some(Val::F64(n)) => WasmRet::Float(n.to_float()),
            Some(Val::F32(n)) => WasmRet::Float(n.to_float() as f64),
            Some(other) => {
                return Err(format!(
                    "wasm call `{name}`: unsupported result {other:?}"
                ))
            }
        })
    }

    fn call_raw(&mut self, name: &str, params: &[Val]) -> Result<Option<Val>, String> {
        let func = self
            .instance
            .get_func(&self.store, name)
            .ok_or_else(|| format!("wasm export `{name}` not found"))?;
        let ty = func.ty(&self.store);
        let mut results: Vec<Val> = ty
            .results()
            .iter()
            .map(|t| match t {
                ValType::I64 => Val::I64(0),
                ValType::I32 => Val::I32(0),
                ValType::F64 => Val::F64(F64::from_float(0.0)),
                ValType::F32 => Val::F32(F32::from_float(0.0)),
                _ => Val::I64(0),
            })
            .collect();
        func.call(&mut self.store, params, &mut results)
            .map_err(|e| format!("wasm call `{name}` failed: {e}"))?;
        Ok(results.into_iter().next())
    }
}

/// Host-side scalar passed into a wasm export.
#[derive(Debug, Clone, Copy)]
pub enum WasmArg {
    Int(i64),
    Float(f64),
    Bool(bool),
}

/// Host-side scalar returned from a wasm export.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WasmRet {
    Unit,
    Int(i64),
    Float(f64),
}

fn marshal_arg(want: ValType, got: &WasmArg) -> Result<Val, String> {
    match (want, got) {
        (ValType::I64, WasmArg::Int(n)) => Ok(Val::I64(*n)),
        (ValType::I64, WasmArg::Bool(b)) => Ok(Val::I64(*b as i64)),
        (ValType::I64, WasmArg::Float(f)) => Ok(Val::I64(*f as i64)),
        (ValType::I32, WasmArg::Int(n)) => Ok(Val::I32(*n as i32)),
        (ValType::I32, WasmArg::Bool(b)) => Ok(Val::I32(*b as i32)),
        (ValType::I32, WasmArg::Float(f)) => Ok(Val::I32(*f as i32)),
        (ValType::F64, WasmArg::Float(f)) => Ok(Val::F64(F64::from_float(*f))),
        (ValType::F64, WasmArg::Int(n)) => Ok(Val::F64(F64::from_float(*n as f64))),
        (ValType::F32, WasmArg::Float(f)) => Ok(Val::F32(F32::from_float(*f as f32))),
        (ValType::F32, WasmArg::Int(n)) => Ok(Val::F32(F32::from_float(*n as f32))),
        (want, got) => Err(format!("cannot pass {got:?} as {want:?}")),
    }
}

/// Compiles a single `.rs` file to a `wasm32-unknown-unknown` cdylib.
/// When `names` is non-empty, ensures those symbols are exported — wrapping
/// plain `pub fn` as needed.
pub fn compile_rs_to_wasm(rs_path: &Path) -> Result<Vec<u8>, String> {
    compile_rs_to_wasm_named(rs_path, &[])
}

/// Like [`compile_rs_to_wasm`], but wraps plain `pub fn` so every `name` is a
/// wasm export.
pub fn compile_rs_to_wasm_named(rs_path: &Path, names: &[String]) -> Result<Vec<u8>, String> {
    let src =
        std::fs::read(rs_path).map_err(|e| format!("could not read {}: {e}", rs_path.display()))?;
    let src_str = String::from_utf8_lossy(&src);

    // Fast path: compile the file as-is when it already exports everything
    // (or when the caller didn't ask for specific names).
    let direct = compile_bytes_cached(&src, rs_path, None)?;
    if names.is_empty() {
        return Ok(direct);
    }
    if let Ok(rt) = WasmRuntime::from_wasm(&direct) {
        if names.iter().all(|n| rt.exports(n)) {
            return Ok(direct);
        }
    }

    // Ergonomics path: generate an `extern "C"` wrapper around plain `pub fn`.
    let sigs = scan_pub_fns(&src_str);
    let mut missing = Vec::new();
    let mut wrap_fns = Vec::new();
    for name in names {
        match sigs.iter().find(|s| s.name == *name) {
            Some(sig) if sig.wasm_compatible() => wrap_fns.push(sig.clone()),
            Some(_) => {
                return Err(format!(
                    "`{name}` in {} uses types Astra's wasm bridge can't wrap yet \
                     (scalars only: i64/i32/bool/f64)",
                    rs_path.display()
                ));
            }
            None => missing.push(name.clone()),
        }
    }
    if !missing.is_empty() {
        return Err(format!(
            "could not find pub fn {} in {}",
            missing.join(", "),
            rs_path.display()
        ));
    }

    let wrapper = generate_wrapper(rs_path, &wrap_fns)?;
    compile_bytes_cached(wrapper.as_bytes(), rs_path, Some("wrap"))
}

/// Lists every `pub fn` name in `rs_path` that the wasm bridge can wrap.
/// Used for `import * from foo` when `foo.rs` is the sibling.
pub fn list_wrappable_fns(rs_path: &Path) -> Result<Vec<String>, String> {
    let src = std::fs::read_to_string(rs_path)
        .map_err(|e| format!("could not read {}: {e}", rs_path.display()))?;
    Ok(scan_pub_fns(&src)
        .into_iter()
        .filter(|s| s.wasm_compatible())
        .map(|s| s.name)
        .collect())
}

fn compile_bytes_cached(
    src: &[u8],
    rs_path: &Path,
    tag: Option<&str>,
) -> Result<Vec<u8>, String> {
    let mut key_bytes = src.to_vec();
    if let Some(t) = tag {
        key_bytes.extend_from_slice(b"\0tag:");
        key_bytes.extend_from_slice(t.as_bytes());
        // Absolute path feeds the `#[path = …]` wrapper — include it so a move
        // of the `.rs` doesn't reuse a stale wrapper pointing at the old place.
        key_bytes.extend_from_slice(b"\0path:");
        key_bytes.extend_from_slice(rs_path.to_string_lossy().as_bytes());
    }
    let hash = fnv1a(&key_bytes);
    let cache = cache_dir()?;
    let cached = cache.join(format!("{hash:016x}.wasm"));
    if let Ok(bytes) = std::fs::read(&cached) {
        return Ok(bytes);
    }

    let _ = std::fs::create_dir_all(&cache);
    // rustc derives the crate name from the source filename — use `_` not `.`
    // so names stay legal (`0391….wrap` was rejected).
    let src_file = if tag.is_some() {
        let p = cache.join(format!("{hash:016x}_wrap.rs"));
        std::fs::write(&p, src)
            .map_err(|e| format!("could not write wrapper {}: {e}", p.display()))?;
        p
    } else {
        rs_path.to_path_buf()
    };

    let tmp = cache.join(format!("{hash:016x}.wasm.tmp"));
    let mut cmd = std::process::Command::new("rustc");
    cmd.args([
        "--edition",
        "2021",
        "--target",
        "wasm32-unknown-unknown",
        "--crate-type",
        "cdylib",
        "-C",
        "opt-level=2",
        "-o",
    ])
    .arg(&tmp);
    if tag.is_some() {
        // Stable crate name independent of the hash filename.
        cmd.args(["--crate-name", "astra_rs_wrap"]);
    }
    let status = cmd
        .arg(&src_file)
        .status()
        .map_err(|e| format!("failed to launch rustc (is it installed?): {e}"))?;
    if !status.success() {
        return Err(format!(
            "rustc failed to compile {} to wasm",
            rs_path.display()
        ));
    }
    let bytes = std::fs::read(&tmp)
        .map_err(|e| format!("rustc produced no wasm for {}: {e}", rs_path.display()))?;
    let _ = std::fs::rename(&tmp, &cached);
    Ok(bytes)
}

/// Scalar kind for wrapper / ABI decisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Scalar {
    I64,
    I32,
    Bool,
    F64,
    F32,
    Unit,
}

impl Scalar {
    fn parse(ty: &str) -> Option<Self> {
        match ty.trim() {
            "" | "()" => Some(Self::Unit),
            // Exact ABI types only — wrappers pass these through without
            // guessing signedness/width (i16/u64/… stay unsupported for now).
            "i64" => Some(Self::I64),
            "i32" => Some(Self::I32),
            "bool" => Some(Self::Bool),
            "f64" => Some(Self::F64),
            "f32" => Some(Self::F32),
            _ => None,
        }
    }

    fn wasm_param(self) -> &'static str {
        match self {
            Self::I64 | Self::Bool => "i64",
            Self::I32 => "i32",
            Self::F64 => "f64",
            Self::F32 => "f32",
            Self::Unit => "i64", // shouldn't appear as a param
        }
    }

    fn wasm_ret(self) -> Option<&'static str> {
        match self {
            Self::Unit => None,
            Self::I64 | Self::Bool => Some("i64"),
            Self::I32 => Some("i32"),
            Self::F64 => Some("f64"),
            Self::F32 => Some("f32"),
        }
    }
}

#[derive(Debug, Clone)]
struct FnSig {
    name: String,
    params: Vec<Scalar>,
    ret: Scalar,
}

impl FnSig {
    fn wasm_compatible(&self) -> bool {
        let param_ok = |p: &Scalar| {
            matches!(
                p,
                Scalar::I64 | Scalar::I32 | Scalar::Bool | Scalar::F64 | Scalar::F32
            )
        };
        self.params.iter().all(param_ok)
            && matches!(
                self.ret,
                Scalar::Unit
                    | Scalar::I64
                    | Scalar::I32
                    | Scalar::Bool
                    | Scalar::F64
                    | Scalar::F32
            )
    }
}

/// Lightweight scan for top-level `pub fn` / `pub extern "C" fn` signatures.
/// Skips bodies; not a full Rust parser — good enough for the scalar interop
/// surface Astra supports today.
fn scan_pub_fns(src: &str) -> Vec<FnSig> {
    let stripped = strip_rust_comments(src);
    let bytes = stripped.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        // Match `pub` at a token boundary.
        if !at_ident(bytes, i, b"pub") {
            i += 1;
            continue;
        }
        let mut j = i + 3;
        j = skip_ws(bytes, j);
        // optional `unsafe`
        if at_ident(bytes, j, b"unsafe") {
            j = skip_ws(bytes, j + 6);
        }
        // optional `extern "C"` / `extern 'C'`
        if at_ident(bytes, j, b"extern") {
            j = skip_ws(bytes, j + 6);
            if j < bytes.len() && (bytes[j] == b'"' || bytes[j] == b'\'') {
                let quote = bytes[j];
                j += 1;
                while j < bytes.len() && bytes[j] != quote {
                    j += 1;
                }
                if j < bytes.len() {
                    j += 1;
                }
                j = skip_ws(bytes, j);
            }
        }
        if !at_ident(bytes, j, b"fn") {
            i += 1;
            continue;
        }
        j = skip_ws(bytes, j + 2);
        let Some((name, next)) = read_ident(bytes, j) else {
            i += 1;
            continue;
        };
        j = skip_ws(bytes, next);
        if j >= bytes.len() || bytes[j] != b'(' {
            i += 1;
            continue;
        }
        let Some((params_src, after_params)) = read_balanced(bytes, j, b'(', b')') else {
            i += 1;
            continue;
        };
        j = skip_ws(bytes, after_params);
        let mut ret = Scalar::Unit;
        if j < bytes.len() && bytes[j] == b'-' && j + 1 < bytes.len() && bytes[j + 1] == b'>' {
            j = skip_ws(bytes, j + 2);
            let ty_end = j;
            let mut k = j;
            // type token until `{` `;` or `where`
            while k < bytes.len() {
                if bytes[k] == b'{' || bytes[k] == b';' {
                    break;
                }
                if at_ident(bytes, k, b"where") {
                    break;
                }
                k += 1;
            }
            let ty = stripped[ty_end..k].trim();
            match Scalar::parse(ty) {
                Some(s) => ret = s,
                None => {
                    // Unsupported return type — record as incompatible.
                    out.push(FnSig {
                        name,
                        params: vec![Scalar::Unit],
                        ret: Scalar::Unit,
                    });
                    i = k.max(i + 1);
                    continue;
                }
            }
            j = k;
        }
        let mut params = Vec::new();
        let mut ok = true;
        for part in params_src.split(',') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            // `self` / `&self` — methods aren't wrappable at the cdylib root.
            if part == "self" || part.starts_with("&self") || part.starts_with("&mut self") {
                ok = false;
                break;
            }
            let ty = part.rsplit(':').next().unwrap_or("").trim();
            match Scalar::parse(ty) {
                Some(s) if s != Scalar::Unit => params.push(s),
                _ => {
                    ok = false;
                    break;
                }
            }
        }
        if ok {
            out.push(FnSig { name, params, ret });
        } else {
            out.push(FnSig {
                name,
                params: vec![Scalar::Unit],
                ret: Scalar::Unit,
            });
        }
        i = j.max(i + 1);
    }
    out
}

fn generate_wrapper(rs_path: &Path, fns: &[FnSig]) -> Result<String, String> {
    let abs = rs_path
        .canonicalize()
        .unwrap_or_else(|_| rs_path.to_path_buf());
    // Escape for a Rust string literal in `#[path = "…"]`.
    let path_lit = abs.to_string_lossy().replace('\\', "\\\\").replace('"', "\\\"");
    let mut out = String::new();
    out.push_str("// Generated by Astra — export wrapper for plain `pub fn` Rust modules.\n");
    out.push_str("#![allow(dead_code, unused_imports)]\n");
    out.push_str(&format!("#[path = \"{path_lit}\"]\nmod __astra_rs;\n\n"));
    for f in fns {
        out.push_str("#[no_mangle]\n");
        out.push_str("pub extern \"C\" fn ");
        out.push_str(&f.name);
        out.push('(');
        for (i, p) in f.params.iter().enumerate() {
            if i > 0 {
                out.push_str(", ");
            }
            out.push_str(&format!("a{i}: {}", p.wasm_param()));
        }
        out.push(')');
        if let Some(r) = f.ret.wasm_ret() {
            out.push_str(&format!(" -> {r}"));
        }
        out.push_str(" {\n    ");
        // Build the inner call with casts from wasm ABI → Rust types.
        let mut call = format!("__astra_rs::{}(", f.name);
        for (i, p) in f.params.iter().enumerate() {
            if i > 0 {
                call.push_str(", ");
            }
            match p {
                Scalar::Bool => call.push_str(&format!("a{i} != 0")),
                Scalar::I64 => call.push_str(&format!("a{i}")),
                Scalar::I32 => call.push_str(&format!("a{i} as i32")),
                Scalar::F64 => call.push_str(&format!("a{i}")),
                Scalar::F32 => call.push_str(&format!("a{i} as f32")),
                Scalar::Unit => {}
            }
        }
        call.push(')');
        match f.ret {
            Scalar::Unit => {
                out.push_str(&call);
                out.push_str(";\n");
            }
            Scalar::Bool => {
                out.push_str(&format!("if {call} {{ 1 }} else {{ 0 }}\n"));
            }
            Scalar::I64 => {
                out.push_str(&call);
                out.push('\n');
            }
            Scalar::I32 => {
                // Inner may return i32/i16/… — cast to i32 for the export.
                out.push_str(&format!("{call} as i32\n"));
            }
            Scalar::F64 => {
                out.push_str(&call);
                out.push('\n');
            }
            Scalar::F32 => {
                out.push_str(&format!("{call} as f32\n"));
            }
        }
        out.push_str("}\n\n");
    }
    Ok(out)
}

fn strip_rust_comments(src: &str) -> String {
    let b = src.as_bytes();
    let mut out = String::with_capacity(src.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'/' && i + 1 < b.len() && b[i + 1] == b'/' {
            out.push(' ');
            out.push(' ');
            i += 2;
            while i < b.len() && b[i] != b'\n' {
                out.push(' ');
                i += 1;
            }
            continue;
        }
        if b[i] == b'/' && i + 1 < b.len() && b[i + 1] == b'*' {
            out.push(' ');
            out.push(' ');
            i += 2;
            while i + 1 < b.len() && !(b[i] == b'*' && b[i + 1] == b'/') {
                out.push(if b[i] == b'\n' { '\n' } else { ' ' });
                i += 1;
            }
            if i + 1 < b.len() {
                out.push(' ');
                out.push(' ');
                i += 2;
            }
            continue;
        }
        if b[i] == b'"' {
            out.push('"');
            i += 1;
            while i < b.len() {
                out.push(b[i] as char);
                if b[i] == b'\\' && i + 1 < b.len() {
                    out.push(b[i + 1] as char);
                    i += 2;
                    continue;
                }
                if b[i] == b'"' {
                    i += 1;
                    break;
                }
                i += 1;
            }
            continue;
        }
        out.push(b[i] as char);
        i += 1;
    }
    out
}

fn skip_ws(bytes: &[u8], mut i: usize) -> usize {
    while i < bytes.len() && bytes[i].is_ascii_whitespace() {
        i += 1;
    }
    i
}

fn at_ident(bytes: &[u8], i: usize, word: &[u8]) -> bool {
    if i + word.len() > bytes.len() {
        return false;
    }
    if &bytes[i..i + word.len()] != word {
        return false;
    }
    let before_ok = i == 0 || !is_ident_byte(bytes[i - 1]);
    let after = i + word.len();
    let after_ok = after >= bytes.len() || !is_ident_byte(bytes[after]);
    before_ok && after_ok
}

fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

fn read_ident(bytes: &[u8], i: usize) -> Option<(String, usize)> {
    if i >= bytes.len() || !(bytes[i].is_ascii_alphabetic() || bytes[i] == b'_') {
        return None;
    }
    let mut j = i + 1;
    while j < bytes.len() && is_ident_byte(bytes[j]) {
        j += 1;
    }
    Some((String::from_utf8_lossy(&bytes[i..j]).into_owned(), j))
}

fn read_balanced(bytes: &[u8], start: usize, open: u8, close: u8) -> Option<(String, usize)> {
    if start >= bytes.len() || bytes[start] != open {
        return None;
    }
    let mut depth = 0i32;
    let mut i = start;
    while i < bytes.len() {
        if bytes[i] == open {
            depth += 1;
        } else if bytes[i] == close {
            depth -= 1;
            if depth == 0 {
                let inner = String::from_utf8_lossy(&bytes[start + 1..i]).into_owned();
                return Some((inner, i + 1));
            }
        }
        i += 1;
    }
    None
}

/// `~/.alloy/cache` (created lazily), overridable via `ALLOY_CACHE_DIR`.
fn cache_dir() -> Result<PathBuf, String> {
    if let Ok(dir) = std::env::var("ALLOY_CACHE_DIR") {
        return Ok(PathBuf::from(dir));
    }
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map_err(|_| "no HOME/USERPROFILE to locate the Alloy cache".to_string())?;
    Ok(PathBuf::from(home).join(".alloy").join("cache"))
}

/// FNV-1a 64-bit — a tiny, dependency-free content hash for cache keys.
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instantiate_and_call_i64() {
        let wasm = wat::parse_str(
            r#"(module
                 (func (export "add") (param i64 i64) (result i64)
                   local.get 0 local.get 1 i64.add)
                 (func (export "noop")))"#,
        )
        .expect("wat");
        let mut rt = WasmRuntime::from_wasm(&wasm).expect("instantiate");
        assert!(rt.exports("add"));
        assert!(!rt.exports("missing"));
        assert_eq!(rt.call_i64("add", &[20, 22]).unwrap(), 42);
        assert_eq!(rt.call_i64("noop", &[]).unwrap(), 0);
        assert!(rt.call_i64("missing", &[]).is_err());
    }

    #[test]
    fn call_marshals_f64() {
        let wasm = wat::parse_str(
            r#"(module
                 (func (export "twice") (param f64) (result f64)
                   local.get 0 local.get 0 f64.add))"#,
        )
        .expect("wat");
        let mut rt = WasmRuntime::from_wasm(&wasm).expect("instantiate");
        match rt.call("twice", &[WasmArg::Float(1.5)]).unwrap() {
            WasmRet::Float(v) => assert!((v - 3.0).abs() < 1e-9),
            other => panic!("expected float, got {other:?}"),
        }
    }

    #[test]
    fn cache_key_is_content_addressed() {
        assert_eq!(fnv1a(b"abc"), fnv1a(b"abc"));
        assert_ne!(fnv1a(b"abc"), fnv1a(b"abd"));
    }

    #[test]
    fn scans_plain_and_extern_fns() {
        let src = r#"
            // comment with pub fn fake() {}
            pub fn fib(n: i64) -> i64 { n }
            #[no_mangle]
            pub extern "C" fn add(a: i64, b: i64) -> i64 { a + b }
            pub fn flag(b: bool) -> bool { b }
            pub fn bad(s: String) -> String { s }
        "#;
        let sigs = scan_pub_fns(src);
        let names: Vec<_> = sigs.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"fib"));
        assert!(names.contains(&"add"));
        assert!(names.contains(&"flag"));
        assert!(names.contains(&"bad"));
        assert!(sigs.iter().find(|s| s.name == "fib").unwrap().wasm_compatible());
        assert!(sigs.iter().find(|s| s.name == "add").unwrap().wasm_compatible());
        assert!(sigs.iter().find(|s| s.name == "flag").unwrap().wasm_compatible());
        assert!(!sigs.iter().find(|s| s.name == "bad").unwrap().wasm_compatible());
        assert!(!names.contains(&"fake"));
    }

    #[test]
    fn wrapper_casts_bool() {
        let sig = FnSig {
            name: "flag".into(),
            params: vec![Scalar::Bool],
            ret: Scalar::Bool,
        };
        let w = generate_wrapper(Path::new("math.rs"), &[sig]).unwrap();
        assert!(w.contains("pub extern \"C\" fn flag(a0: i64) -> i64"));
        assert!(w.contains("a0 != 0"));
        assert!(w.contains("if __astra_rs::flag"));
    }
}

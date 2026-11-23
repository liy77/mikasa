//! Astra (formerly Alloy): tree-walking interpreter for Mikasa.

pub mod bytecode;
/// File extensions shared with the rest of the toolchain.
pub use mikasa_syntax::ext as source_ext;
pub mod env;
pub mod error;
pub mod interp;
pub mod intrinsics;
pub mod loader;
pub mod stdlib;
pub mod stdlib_ext;
pub mod value;
pub mod wasm;

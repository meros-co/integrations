//! Link flags for building the extension with plain cargo (tests, local
//! development). On macOS an extension module must leave Python's symbols to be
//! resolved by the interpreter at load time; maturin adds that itself, cargo
//! does not.
fn main() {
    pyo3_build_config::add_extension_module_link_args();
}

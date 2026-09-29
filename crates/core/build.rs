//! Embeds every spec in `specs/` into the library.
//!
//! A release is a fixed set of specs: nothing is loaded from disk at runtime, so
//! two consumers on the same version cannot be running different specs.

use std::{env, fs, path::PathBuf};

fn main() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let specs_dir = manifest.join("../../specs").canonicalize().expect("specs/ directory");
    println!("cargo:rerun-if-changed={}", specs_dir.display());

    let mut entries: Vec<PathBuf> = fs::read_dir(&specs_dir)
        .expect("read specs/")
        .map(|e| e.expect("spec entry").path())
        .filter(|p| p.extension().is_some_and(|e| e == "yaml"))
        .collect();
    entries.sort();

    let mut out = String::from("pub(crate) static EMBEDDED_SPECS: &[(&str, &str)] = &[\n");
    for path in &entries {
        println!("cargo:rerun-if-changed={}", path.display());
        let stem = path.file_stem().unwrap().to_string_lossy();
        out.push_str(&format!(
            "    ({stem:?}, include_str!({:?})),\n",
            path.display().to_string()
        ));
    }
    out.push_str("];\n");

    let dest = PathBuf::from(env::var("OUT_DIR").unwrap()).join("embedded_specs.rs");
    fs::write(dest, out).expect("write embedded_specs.rs");
}

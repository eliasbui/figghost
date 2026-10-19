//! Lists the notes in `docs/releases` (one file per version, named `<version>.md`) so the app can show what is new
//! in the version it is, with no network.

use std::{env, fs, path::Path};

fn main() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/releases");
    println!("cargo:rerun-if-changed={}", dir.display());
    let mut entries = Vec::new();
    if let Ok(read) = fs::read_dir(&dir) {
        for file in read.flatten() {
            let path = file.path();
            if path.extension().is_some_and(|e| e == "md")
                && let Some(version) = path.file_stem().and_then(|s| s.to_str())
            {
                entries.push((version.to_owned(), path.canonicalize().unwrap_or(path)));
            }
        }
    }
    entries.sort();
    let mut code = String::from("pub const NOTES: &[(&str, &str)] = &[\n");
    for (version, path) in entries {
        code.push_str(&format!(
            "    ({version:?}, include_str!({:?})),\n",
            path.display().to_string()
        ));
    }
    code.push_str("];\n");
    let out = Path::new(&env::var("OUT_DIR").expect("OUT_DIR")).join("release_notes.rs");
    fs::write(out, code).expect("write release_notes.rs");
}

fn main() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let output = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    for name in ["module-catalog.json", "app-update-source.json"] {
        let source = root.join(name);
        println!("cargo:rerun-if-changed={}", source.display());
        std::fs::copy(source, output.ancestors().nth(3).unwrap().join(name))
            .expect("copy trusted source beside host");
    }
    tauri_build::build()
}

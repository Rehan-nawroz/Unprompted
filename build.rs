fn main() {
    println!("cargo:rerun-if-changed=assets/icon.ico");
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winres::WindowsResource::new();
        res.set_icon("assets/icon.ico");
        res.set("ProductName", "Unprompted");
        res.set("FileDescription", "Unprompted - strip AI image metadata");
        res.set("CompanyName", "Unprompted");
        res.set("LegalCopyright", "MIT License");
        res.compile().expect("failed to embed Windows resources");
        // Newer cargo/rustc drops the `cargo:rustc-link-lib` that winres emits, so the
        // icon resource silently disappears from the exe. Link the compiled resource
        // object directly as a link argument instead.
        let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR");
        let obj = match std::env::var("CARGO_CFG_TARGET_ENV").as_deref() {
            Ok("msvc") => format!("{out_dir}\\resource.lib"),
            _ => format!("{out_dir}\\resource.o"),
        };
        println!("cargo:rustc-link-arg-bins={obj}");
    }
}

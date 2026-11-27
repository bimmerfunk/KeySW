// Embed the icons into keysw-win.exe (Windows builds only).
fn main() {
    println!("cargo:rerun-if-changed=keysw-win.rc");
    println!("cargo:rerun-if-changed=assets");
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    // KEYSW_SKIP_RC lets `cargo check --target ...-windows-gnu` run on Linux
    // without a resource compiler.
    if target_os == "windows" && std::env::var_os("KEYSW_SKIP_RC").is_none() {
        embed_resource::compile("keysw-win.rc", embed_resource::NONE)
            .manifest_optional()
            .unwrap();
    }
}

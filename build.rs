// Embeds the app icon and version info into the Windows .exe.
fn main() {
    println!("cargo:rerun-if-changed=assets/app-icon.ico");
    println!("cargo:rerun-if-changed=build.rs");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let mut res = winresource::WindowsResource::new();
    res.set_icon("assets/app-icon.ico");
    res.set("ProductName", "Tier List Maker");
    res.set("FileDescription", "Tier List Maker");
    res.set("CompanyName", "Trabi");
    res.set("LegalCopyright", "Made by Trabi");

    // Cross-compiling from macOS/Linux: use the MinGW tools from PATH.
    let host = std::env::var("HOST").unwrap_or_default();
    if !host.contains("windows") {
        res.set_windres_path("x86_64-w64-mingw32-windres");
        res.set_ar_path("x86_64-w64-mingw32-ar");
    }

    if let Err(err) = res.compile() {
        println!("cargo:warning=Could not embed the Windows icon: {err}");
    }
}

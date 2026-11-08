// Embeds the icon and version information in ProxyCraft.exe on Windows.
fn main() {
    println!("cargo:rerun-if-changed=assets/ProxyCraft.ico");
    #[cfg(windows)]
    {
        if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
            let version = env!("CARGO_PKG_VERSION");
            let mut res = winresource::WindowsResource::new();
            res.set_icon("assets/ProxyCraft.ico")
                .set("ProductName", "ProxyCraft")
                .set("FileDescription", "ProxyCraft")
                .set("CompanyName", "PitKub")
                .set(
                    "LegalCopyright",
                    "Copyright (c) 2026 Piotr Kubicki. MIT License.",
                )
                .set("ProductVersion", version)
                .set("FileVersion", version);
            if let Err(e) = res.compile() {
                println!("cargo:warning=could not embed the Windows icon and version: {e}");
            }
        }
    }
}

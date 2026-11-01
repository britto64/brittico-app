//! No Linux, o capturador de tela do libwebrtc (a "mostrar a tela" da Call) usa o portal do sistema pela GIO.
//! No Windows (build feito num Windows, como no GitHub Actions), o .exe ganha o ícone e a versão.
fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("linux") {
        for lib in ["gio-2.0", "gobject-2.0", "glib-2.0"] {
            println!("cargo:rustc-link-lib={lib}");
        }
    }
    #[cfg(windows)]
    {
        println!("cargo:rerun-if-changed=packaging/icons/brittico.ico");
        let mut res = winresource::WindowsResource::new();
        res.set_icon("packaging/icons/brittico.ico")
            .set("ProductName", "Brittico")
            .set("FileDescription", "Brittico")
            .set("CompanyName", "Brittico")
            .set("LegalCopyright", "Brittico")
            .set("OriginalFilename", "brittico.exe");
        if let Err(e) = res.compile() {
            println!("cargo:warning=sem ícone no .exe: {e}");
        }
    }
}

//! No Linux, o capturador de tela do libwebrtc (a "mostrar a tela" da Call) usa o portal do sistema pela GIO.
fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("linux") {
        for lib in ["gio-2.0", "gobject-2.0", "glib-2.0"] {
            println!("cargo:rustc-link-lib={lib}");
        }
    }
}

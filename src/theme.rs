//! Os tokens do site, tema Preto escuro (`public/assets/css/shell.css`, `:root[data-theme="dark"][data-preset="preto"]`),
//! e as fontes dele. O nome de cada constante é o da variável CSS, para dar para conferir uma com a outra.

use eframe::egui::{self, Color32, FontData, FontDefinitions, FontFamily, FontId, Stroke, Vec2};
use std::sync::Arc;

/// Os tokens de cor e de canto de um tema do site: cada campo é uma variável CSS de `shell.css`.
#[derive(Clone, Copy)]
pub struct Palette {
    pub cream_0: Color32,
    pub cream_1: Color32,
    pub cream_2: Color32,
    pub cream_3: Color32,
    pub cream_4: Color32,
    pub ink: Color32,
    pub soft: Color32,
    pub faint: Color32,
    pub secondary: Color32,
    pub on_secondary: Color32,
    pub accent: Color32,
    pub amber_soft: Color32,
    pub hairline_2: Color32,
    /// `--r-xs`, `--r-sm`, `--r-md`, `--r-lg`, `--r-xl`.
    pub r: [f32; 5],
    pub dark: bool,
}

const fn rgb(v: u32) -> Color32 {
    Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
}
/// `rgba(r,g,b,a)` já pré-multiplicado, como o egui guarda.
const fn rgba(v: u32, a: f32) -> Color32 {
    Color32::from_rgba_premultiplied(((v >> 16 & 0xff) as f32 * a + 0.5) as u8, ((v >> 8 & 0xff) as f32 * a + 0.5) as u8, ((v & 0xff) as f32 * a + 0.5) as u8, (a * 255.0 + 0.5) as u8)
}

/// As paletas do site (Preto, Agressivo, Arredondado), cada uma no escuro e no claro.
pub mod palettes {
    use super::*;
    pub const PRETO_ESCURO: Palette = Palette {
        cream_0: rgb(0x0D0E10), cream_1: rgb(0x000000), cream_2: rgb(0x16171A), cream_3: rgb(0x08090A), cream_4: rgb(0x25272B),
        ink: rgb(0xE7E9EA), soft: rgb(0xA8ADB3), faint: rgb(0x71767B),
        secondary: rgb(0xF49200), on_secondary: rgb(0x2A1C05), accent: rgb(0xFF7A1A),
        amber_soft: rgba(0xF49200, 0.16), hairline_2: rgba(0xE7E9EA, 0.08),
        r: [6.0, 10.0, 14.0, 16.0, 20.0], dark: true,
    };
    pub const PRETO_CLARO: Palette = Palette {
        cream_0: rgb(0xF7F9F9), cream_1: rgb(0xFFFFFF), cream_2: rgb(0xEFF3F4), cream_3: rgb(0xE4E8EA), cream_4: rgb(0xCFD9DE),
        ink: rgb(0x0F1419), soft: rgb(0x536471), faint: rgb(0x8B98A5),
        secondary: rgb(0xF49200), on_secondary: rgb(0x402C0C), accent: rgb(0xD95F00),
        amber_soft: rgba(0xF49200, 0.14), hairline_2: rgba(0x0F1419, 0.08),
        r: [6.0, 10.0, 14.0, 16.0, 20.0], dark: false,
    };
    pub const AGRESSIVO_ESCURO: Palette = Palette {
        cream_0: rgb(0x27231f), cream_1: rgb(0x151412), cream_2: rgb(0x1f1d1a), cream_3: rgb(0x11100e), cream_4: rgb(0x3d3731),
        ink: rgb(0xd4ccb4), soft: rgb(0xa39c89), faint: rgb(0x7a7463),
        secondary: rgb(0xf49200), on_secondary: rgb(0x2A1C05), accent: rgb(0x8c6257),
        amber_soft: rgba(0xf49200, 0.15), hairline_2: rgba(0xd4ccb4, 0.15),
        r: [0.0; 5], dark: true,
    };
    pub const AGRESSIVO_CLARO: Palette = Palette {
        cream_0: rgb(0xffffff), cream_1: rgb(0xf8f6f0), cream_2: rgb(0xeae5d9), cream_3: rgb(0xe0d8c8), cream_4: rgb(0xc2b8a3),
        ink: rgb(0x2b2824), soft: rgb(0x635b51), faint: rgb(0x8f8576),
        secondary: rgb(0xf49200), on_secondary: rgb(0x402C0C), accent: rgb(0xb84c35),
        amber_soft: rgba(0xf49200, 0.15), hairline_2: rgba(0x2b2824, 0.15),
        r: [0.0; 5], dark: false,
    };
    pub const ARREDONDADO_ESCURO: Palette = Palette {
        cream_0: rgb(0x272016), cream_1: rgb(0x15110B), cream_2: rgb(0x1F1912), cream_3: rgb(0x0D0B06), cream_4: rgb(0x42341E),
        ink: rgb(0xEBE6CF), soft: rgb(0xCBBD9C), faint: rgb(0x9C8B6C),
        secondary: rgb(0xF4B402), on_secondary: rgb(0x2A1C05), accent: rgb(0xE5352B),
        amber_soft: rgb(0x3B2C11), hairline_2: rgba(0xEBE6CF, 0.15),
        r: [6.0, 12.0, 14.0, 16.0, 28.0], dark: true,
    };
    pub const ARREDONDADO_CLARO: Palette = Palette {
        cream_0: rgb(0xFFFDF8), cream_1: rgb(0xFDF5E7), cream_2: rgb(0xF8EEDC), cream_3: rgb(0xF1E6CF), cream_4: rgb(0xE7D6B6),
        ink: rgb(0x2C2216), soft: rgb(0x6E5D46), faint: rgb(0x9A876C),
        secondary: rgb(0xF4A62C), on_secondary: rgb(0x402C0C), accent: rgb(0xD30000),
        amber_soft: rgb(0xFCE7C2), hairline_2: rgba(0x462D0A, 0.16),
        r: [6.0, 12.0, 14.0, 16.0, 28.0], dark: false,
    };
}

thread_local! {
    static CUR: std::cell::Cell<Palette> = const { std::cell::Cell::new(palettes::PRETO_ESCURO) };
}

/// A paleta em uso agora.
pub fn pal() -> Palette {
    CUR.with(|c| c.get())
}

/// Troca a paleta do app inteiro. `destaque` é a "Cor de Destaque" das preferências: ela troca o `--secondary`, a cor
/// da letra em cima dele (preta ou branca, pela luminância, como o `getContrastColor` do site) e o `--amber-soft`.
pub fn set(ctx: &egui::Context, mut p: Palette, destaque: Option<Color32>) {
    if let Some(c) = destaque {
        let yiq = (c.r() as u32 * 299 + c.g() as u32 * 587 + c.b() as u32 * 114) / 1000;
        p.secondary = c;
        p.on_secondary = if yiq >= 128 { rgb(0x1a1104) } else { Color32::WHITE };
        p.amber_soft = rgba(((c.r() as u32) << 16) | ((c.g() as u32) << 8) | c.b() as u32, 0.15);
    }
    CUR.with(|c| c.set(p));
    visuals(ctx);
    ctx.request_repaint();
}

macro_rules! token {
    ($($(#[$m:meta])* $name:ident: $t:ty = $e:expr;)*) => {$(
        $(#[$m])*
        #[allow(non_snake_case)]
        #[inline]
        pub fn $name() -> $t {
            let p = pal();
            $e(p)
        }
    )*};
}

token! {
    /// `--cream-1`: o fundo da página, da trilha e do conteúdo.
    CREAM_1: Color32 = |p: Palette| p.cream_1;
    /// `--cream-0`: colunas da Call, cabeçalhos, a barra "eu", o fundo dos ícones da trilha.
    CREAM_0: Color32 = |p: Palette| p.cream_0;
    /// `--cream-2`: a coluna dos grupos, cartões, a busca, os botões redondos.
    CREAM_2: Color32 = |p: Palette| p.cream_2;
    /// `--cream-3`: o começo do degradê das capas.
    CREAM_3: Color32 = |p: Palette| p.cream_3;
    /// `--cream-4`: as bordas e o divisor da trilha.
    CREAM_4: Color32 = |p: Palette| p.cream_4;
    INK: Color32 = |p: Palette| p.ink;
    /// `--text-soft`.
    SOFT: Color32 = |p: Palette| p.soft;
    /// `--text-faint-2`.
    FAINT: Color32 = |p: Palette| p.faint;
    /// `--secondary`: o âmbar de tudo que está ativo.
    SECONDARY: Color32 = |p: Palette| p.secondary;
    ON_SECONDARY: Color32 = |p: Palette| p.on_secondary;
    /// `--accent`: a barrinha da trilha, a etiqueta MOD.
    ACCENT: Color32 = |p: Palette| p.accent;
    /// `--amber-soft`.
    AMBER_SOFT: Color32 = |p: Palette| p.amber_soft;
    /// `--hairline-2`.
    HAIRLINE_2: Color32 = |p: Palette| p.hairline_2;
    R_XS: f32 = |p: Palette| p.r[0];
    R_SM: f32 = |p: Palette| p.r[1];
    R_MD: f32 = |p: Palette| p.r[2];
    R_LG: f32 = |p: Palette| p.r[3];
    /// Se o tema em uso é o escuro.
    DARK: bool = |p: Palette| p.dark;
}

/// `--dis-ok` e `--dis-off` da Call (iguais em todos os temas).
pub const DIS_OK: Color32 = Color32::from_rgb(0x3E, 0x9C, 0x5C);
pub const DIS_OFF: Color32 = Color32::from_rgb(0xE5, 0x35, 0x2B);

/// Open Sans 600, o corpo de todo o site.
pub fn f6(size: f32) -> FontId {
    FontId::new(size, FontFamily::Proportional)
}
pub fn f7(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("w700".into()))
}
pub fn f8(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("w800".into()))
}
/// Patua One (`--f-display`), dos títulos.
pub fn patua(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("patua".into()))
}
/// Avara, só do nome "brittico" no topo.
pub fn avara(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("avara".into()))
}

/// Balsamiq Sans 700: a letra do LivePixel (o `--lp-font`, Comic Sans de verdade só onde existir).
pub fn comic(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("comic".into()))
}
/// Balsamiq Sans 400 (o `placeholder` do chat do LivePixel).
pub fn comic4(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("comic4".into()))
}
/// Inter, o corpo do LivePixel (`--lp-body`), em 400, 600 e 700.
pub fn inter(size: f32, weight: u16) -> FontId {
    let fam = match weight {
        0..=450 => "inter4",
        451..=650 => "inter6",
        _ => "inter7",
    };
    FontId::new(size, FontFamily::Name(fam.into()))
}

pub fn install(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::empty();
    let files: [(&str, &'static [u8]); 10] = [
        ("bal7", include_bytes!("../assets/fonts/BalsamiqSans-Bold.ttf")),
        ("bal4", include_bytes!("../assets/fonts/BalsamiqSans-Regular.ttf")),
        ("inter4", include_bytes!("../assets/fonts/Inter-Regular.ttf")),
        ("inter6", include_bytes!("../assets/fonts/Inter-SemiBold.ttf")),
        ("inter7", include_bytes!("../assets/fonts/Inter-Bold.ttf")),
        ("os600", include_bytes!("../assets/fonts/OpenSans-SemiBold.ttf")),
        ("os700", include_bytes!("../assets/fonts/OpenSans-Bold.ttf")),
        ("os800", include_bytes!("../assets/fonts/OpenSans-ExtraBold.ttf")),
        ("patua", include_bytes!("../assets/fonts/PatuaOne-Regular.ttf")),
        ("avara", include_bytes!("../assets/fonts/Avara-Black.ttf")),
    ];
    for (name, bytes) in files {
        fonts.font_data.insert(name.into(), Arc::new(FontData::from_static(bytes)));
    }
    let fam = |list: &[&str]| list.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    fonts.families.insert(FontFamily::Proportional, fam(&["os600"]));
    fonts.families.insert(FontFamily::Monospace, fam(&["os600"]));
    fonts.families.insert(FontFamily::Name("w700".into()), fam(&["os700", "os600"]));
    fonts.families.insert(FontFamily::Name("w800".into()), fam(&["os800", "os600"]));
    fonts.families.insert(FontFamily::Name("patua".into()), fam(&["patua", "os600"]));
    fonts.families.insert(FontFamily::Name("avara".into()), fam(&["avara", "patua", "os600"]));
    fonts.families.insert(FontFamily::Name("comic".into()), fam(&["bal7", "os700"]));
    fonts.families.insert(FontFamily::Name("comic4".into()), fam(&["bal4", "os600"]));
    fonts.families.insert(FontFamily::Name("inter4".into()), fam(&["inter4", "os600"]));
    fonts.families.insert(FontFamily::Name("inter6".into()), fam(&["inter6", "os600"]));
    fonts.families.insert(FontFamily::Name("inter7".into()), fam(&["inter7", "os700"]));
    ctx.set_fonts(fonts);

    visuals(ctx);
    ctx.style_mut(|s| {
        s.spacing.item_spacing = Vec2::ZERO;
        s.spacing.scroll.floating = true;
        s.spacing.scroll.bar_width = 9.0;
        s.text_styles.insert(egui::TextStyle::Body, f6(13.5));
        s.text_styles.insert(egui::TextStyle::Button, f6(13.5));
    });
}

/// As cores que o próprio egui usa (o cursor do texto, a seleção, o fundo das janelas) na paleta em uso.
fn visuals(ctx: &egui::Context) {
    let mut v = if DARK() { egui::Visuals::dark() } else { egui::Visuals::light() };
    v.panel_fill = CREAM_1();
    v.window_fill = CREAM_0();
    v.window_stroke = Stroke::NONE;
    v.extreme_bg_color = CREAM_2();
    v.override_text_color = Some(INK());
    v.text_cursor.stroke = Stroke::new(2.0_f32, SECONDARY());
    v.selection.bg_fill = SECONDARY().linear_multiply(0.4);
    v.selection.stroke = Stroke::new(1.0_f32, SECONDARY());
    v.widgets.noninteractive.bg_stroke = Stroke::NONE;
    ctx.set_visuals(v);
}

//! As peças pequenas que todas as telas usam: texto com espaçamento de letra, ícone do site, avatar, etiqueta de cargo, degradê.

use crate::dados::Role;
use crate::theme::*;
use eframe::egui::{self, text::LayoutJob, Align2, Color32, FontId, ImageSource, Painter, Pos2, Rect, Stroke, TextFormat, Vec2};

/// Um ícone do site (`assets/icons/<nome>.svg`, tirado do shell.js e do call/js/icons.js por `tools/extrair-icones.py`).
#[macro_export]
macro_rules! ic {
    ($n:literal) => {
        eframe::egui::ImageSource::Bytes {
            uri: std::borrow::Cow::Borrowed(concat!("bytes://ic/", $n, ".svg")),
            bytes: eframe::egui::load::Bytes::Static(include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/icons/", $n, ".svg"))),
        }
    };
}

/// Uma imagem PNG de `assets/` (foto, emoji, ícone da marca).
#[macro_export]
macro_rules! png {
    ($n:literal) => {
        eframe::egui::ImageSource::Bytes {
            uri: std::borrow::Cow::Borrowed(concat!("bytes://png/", $n, ".png")),
            bytes: eframe::egui::load::Bytes::Static(include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/", $n, ".png"))),
        }
    };
}

/// `color-mix(in srgb, a t, b)`.
pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let m = |x: u8, y: u8| (x as f32 * t + y as f32 * (1.0 - t)).round() as u8;
    Color32::from_rgb(m(a.r(), b.r()), m(a.g(), b.g()), m(a.b(), b.b()))
}

pub fn hsl(h: f32, s: f32, l: f32) -> Color32 {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let hp = h / 60.0;
    let x = c * (1.0 - (hp % 2.0 - 1.0).abs());
    let (r, g, b) = match hp as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c / 2.0;
    let u = |v: f32| ((v + m) * 255.0).round() as u8;
    Color32::from_rgb(u(r), u(g), u(b))
}

/// `hueOf` de call/js/people.js: a cor de fundo do avatar sem foto vem do nome.
pub fn hue_of(name: &str) -> f32 {
    let mut h: u32 = 0;
    for ch in name.chars() {
        h = h.wrapping_mul(31).wrapping_add(ch as u32);
    }
    (h % 360) as f32
}

/// `initials` de call/js/people.js.
pub fn initials(name: &str) -> String {
    let parts: Vec<&str> = name.split_whitespace().collect();
    match parts.len() {
        0 => "?".into(),
        1 => parts[0].chars().take(2).collect::<String>().to_uppercase(),
        n => format!("{}{}", parts[0].chars().next().unwrap(), parts[n - 1].chars().next().unwrap()).to_uppercase(),
    }
}

/// Uma linha só, cortada com reticências se passar de `width` (`text-overflow:ellipsis`).
pub fn clip(p: &Painter, pos: Pos2, align: Align2, s: &str, font: FontId, color: Color32, width: f32) -> Rect {
    let mut job = LayoutJob::single_section(s.to_owned(), TextFormat { font_id: font, color, ..Default::default() });
    job.wrap = egui::text::TextWrapping { max_width: width, max_rows: 1, break_anywhere: true, overflow_character: Some('…') };
    let g = p.layout_job(job);
    let rect = align.anchor_size(pos, g.size());
    p.galley(rect.min, g, color);
    rect
}

/// O negrito da Patua One. Ela só tem um peso, e o navegador engrossa o desenho quando o CSS pede 700 ou 800
/// (o `font-synthesis`); comparando prints, o efeito equivale a uns 0,3 px na letra de 16. O egui encaixa o texto no
/// pixel inteiro, então meio pixel de deslocamento viraria um pixel inteiro e borraria a letra: a segunda cópia vai a
/// 1 px, com a transparência na proporção do deslocamento que o navegador daria.
pub fn bold(p: &Painter, pos: Pos2, align: Align2, s: &str, font: FontId, color: Color32, spacing: f32) -> Rect {
    let d = (font.size / 56.0).min(1.0);
    text(p, pos + Vec2::new(1.0, 0.0), align, s, font.clone(), color.gamma_multiply(d), spacing);
    let r = text(p, pos, align, s, font, color, spacing);
    r.expand2(Vec2::new(d / 2.0, 0.0))
}

/// Texto com `letter-spacing` (em px), ancorado em `pos` conforme `align`. Devolve onde ficou.
pub fn text(p: &Painter, pos: Pos2, align: Align2, s: &str, font: FontId, color: Color32, spacing: f32) -> Rect {
    let job = LayoutJob::single_section(s.to_owned(), TextFormat { font_id: font, color, extra_letter_spacing: spacing, ..Default::default() });
    let galley = p.layout_job(job);
    let rect = align.anchor_size(pos, galley.size());
    p.galley(rect.min, galley, color);
    rect
}

pub fn text_width(p: &Painter, s: &str, font: FontId, spacing: f32) -> f32 {
    let job = LayoutJob::single_section(s.to_owned(), TextFormat { font_id: font, extra_letter_spacing: spacing, ..Default::default() });
    p.layout_job(job).size().x
}

/// O cache de texturas do egui 0.31 guarda uma textura por endereço, sem olhar o tamanho: o mesmo SVG pedido
/// a 15 px e depois a 34 px sairia borrado. Pôr o tamanho no endereço dá uma textura nítida para cada tamanho.
fn sized(src: ImageSource<'static>, size: Vec2) -> ImageSource<'static> {
    match src {
        ImageSource::Bytes { uri, bytes } => {
            // o carregador de SVG reconhece o arquivo pela extensão: o tamanho entra antes dela
            let (stem, ext) = uri.rsplit_once('.').unwrap_or((&uri, ""));
            ImageSource::Bytes { uri: format!("{stem}@{}x{}.{ext}", size.x as u32, size.y as u32).into(), bytes }
        }
        other => other,
    }
}

/// Pinta um ícone tingido num quadrado de lado `size` centrado em `c`.
pub fn icon(ui: &egui::Ui, src: ImageSource<'static>, c: Pos2, size: f32, color: Color32) {
    let s = Vec2::splat(size);
    egui::Image::new(sized(src, s)).tint(color).fit_to_exact_size(s).paint_at(ui, Rect::from_center_size(c, s));
}

pub fn image(ui: &egui::Ui, src: ImageSource<'static>, rect: Rect, radius: f32) {
    egui::Image::new(sized(src, rect.size())).corner_radius(radius).fit_to_exact_size(rect.size()).paint_at(ui, rect);
}

/// O `.dz-av`: foto redonda ou as iniciais sobre `color-mix(hsl(h 55% 50%) 24%, --cream-2)`; o anel âmbar de quem fala; a bolinha de estado.
pub fn avatar(ui: &egui::Ui, name: &str, photo: Option<ImageSource<'static>>, c: Pos2, size: f32, speaking: bool, dot: Option<(Color32, Color32)>) {
    let p = ui.painter();
    let r = size / 2.0;
    if speaking {
        p.circle_stroke(c, r + 3.0, Stroke::new(2.0_f32, SECONDARY()));
    }
    match photo {
        Some(src) => image(ui, src, Rect::from_center_size(c, Vec2::splat(size)), r),
        None => {
            p.circle_filled(c, r, mix(hsl(hue_of(name), 0.55, 0.50), CREAM_2(), 0.24));
            text(p, c + Vec2::new(0.0, size * 0.02), Align2::CENTER_CENTER, &initials(name), f8(size * 0.38), INK(), 0.0);
        }
    }
    if let Some((fill, ring)) = dot {
        let d = c + Vec2::splat(r - 4.5);
        p.circle_filled(d, 5.5, ring);
        p.circle_filled(d, 3.5, fill);
    }
}

/// O `.rt--glyph`: o círculo na cor do cargo com o desenho dele em branco.
pub fn role_chip(ui: &egui::Ui, role: &Role, c: Pos2, size: f32) {
    let p = ui.painter();
    p.circle_filled(c, size / 2.0, role.color());
    p.circle_stroke(c, size / 2.0, Stroke::new(1.0_f32, Color32::from_white_alpha(61)));
    if let Some(g) = role.glyph() {
        icon(ui, g, c, size * 0.64, Color32::WHITE);
    }
}

/// Degradê vertical num retângulo (o `linear-gradient(0deg, …)` das barras de nome).
pub fn vgradient(p: &Painter, r: Rect, top: Color32, bottom: Color32) {
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(r.left_top(), top);
    mesh.colored_vertex(r.right_top(), top);
    mesh.colored_vertex(r.right_bottom(), bottom);
    mesh.colored_vertex(r.left_bottom(), bottom);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    p.add(mesh);
}

/// O aviso que sobe no pé da janela e some sozinho (`.kz-toast` do feed-core.js): 2,6 s na tela, entrando e saindo em 0,2 s.
#[derive(Default)]
pub struct Toast(Option<(String, f64)>);

impl Toast {
    pub fn say(&mut self, msg: &str, t: f64) {
        self.0 = Some((msg.to_owned(), t));
    }

    pub fn show(&mut self, ui: &egui::Ui, t: f64) {
        let Some((msg, since)) = self.0.clone() else { return };
        let age = t - since;
        if age > 2.6 + 0.2 {
            self.0 = None;
            return;
        }
        // entra e sai em 0,2 s, subindo 20 px
        let k = ((age / 0.2).min(1.0).min((2.8 - age) / 0.2)).clamp(0.0, 1.0) as f32;
        let screen = ui.ctx().screen_rect();
        let p = ui.ctx().layer_painter(egui::LayerId::new(egui::Order::Tooltip, egui::Id::new("toast")));
        let w = text_width(&p, &msg, f6(13.5), 0.0) + 40.0;
        let r = Rect::from_center_size(Pos2::new(screen.center().x, screen.bottom() - 24.0 - 21.0 + 20.0 * (1.0 - k)), Vec2::new(w, 42.0));
        p.rect_filled(r.translate(Vec2::new(0.0, 8.0)).expand(8.0), 29.0, Color32::from_black_alpha((40.0 * k) as u8));
        p.rect_filled(r, 21.0, INK().gamma_multiply(k));
        text(&p, r.center(), Align2::CENTER_CENTER, &msg, f6(13.5), CREAM_0().gamma_multiply(k), 0.0);
        ui.ctx().request_repaint();
    }
}

//! Nomes de sala, de canal e de categoria como o site os desenha (`call/js/role-name.js` e `dom.js`): o texto, com os
//! códigos de posição do LoL (":top:", ":jungle:", ":mid:", ":adc:", ":sup:" e os apelidos) trocados pelo ícone da
//! posição (os mesmos `/assets/roles/*.svg` do site), e os emojis desenhados como imagem.
//!
//! O egui não desenha emoji colorido (a fonte não tem, e ele não lê fontes coloridas): cada emoji vira a imagem do
//! Twemoji, buscada uma vez por execução pelo carregador HTTP (fica no cache de texturas). Sem internet, o emoji some
//! em vez de virar "?".

use crate::ui::{self, text, text_width};
use eframe::egui::{self, Align2, Color32, FontId, ImageSource, Pos2, Rect, Vec2};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Pos {
    Top,
    Jungle,
    Mid,
    Adc,
    Sup,
}

impl Pos {
    pub fn nome(self) -> &'static str {
        match self {
            Pos::Top => "Top",
            Pos::Jungle => "Jungle",
            Pos::Mid => "Mid",
            Pos::Adc => "ADC",
            Pos::Sup => "Suporte",
        }
    }
    fn icone(self) -> ImageSource<'static> {
        macro_rules! r {
            ($n:literal) => {
                ImageSource::Bytes {
                    uri: std::borrow::Cow::Borrowed(concat!("bytes://roles/", $n, ".svg")),
                    bytes: egui::load::Bytes::Static(include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/roles/", $n, ".svg"))),
                }
            };
        }
        match self {
            Pos::Top => r!("top"),
            Pos::Jungle => r!("jungle"),
            Pos::Mid => r!("mid"),
            Pos::Adc => r!("bot"),
            Pos::Sup => r!("support"),
        }
    }
}

/// O `roleOf` do site: maiúscula e acento não importam.
fn pos_de(palavra: &str) -> Option<Pos> {
    let p: String = palavra
        .chars()
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ã' | 'Á' | 'À' | 'Â' | 'Ã' => 'a',
            'é' | 'ê' | 'É' | 'Ê' => 'e',
            'í' | 'Í' => 'i',
            'ó' | 'ô' | 'õ' | 'Ó' | 'Ô' | 'Õ' => 'o',
            'ú' | 'Ú' => 'u',
            'ç' | 'Ç' => 'c',
            c => c.to_ascii_lowercase(),
        })
        .collect();
    Some(match p.as_str() {
        "top" => Pos::Top,
        "jungle" | "jg" | "selva" => Pos::Jungle,
        "mid" | "meio" => Pos::Mid,
        "adc" | "bot" | "atirador" => Pos::Adc,
        "sup" | "support" | "suporte" => Pos::Sup,
        _ => return None,
    })
}

#[derive(Clone, Debug, PartialEq)]
pub enum Parte {
    Texto(String),
    Pos(Pos),
    /// Os códigos do emoji no formato dos arquivos do Twemoji ("1f44b", "1f468-200d-1f4bb").
    Emoji(String),
}

/// Começa um emoji? Os blocos novos de pictogramas sempre; os símbolos antigos (☕, ✅, ⭐…) só os que já nascem emoji
/// ou vêm com o seletor U+FE0F. Um símbolo sem desenho no Twemoji (★, ✓) fica como letra, em vez de sumir.
fn pictograma(c: char, prox: Option<char>) -> bool {
    let u = c as u32;
    let fe0f = prox == Some('\u{FE0F}');
    const JA_EMOJI: [u32; 48] = [
        0x231A, 0x231B, 0x23E9, 0x23EA, 0x23EB, 0x23EC, 0x23F0, 0x23F3, 0x25FD, 0x25FE, 0x2614, 0x2615, 0x2648, 0x2649, 0x264A, 0x264B,
        0x264C, 0x264D, 0x264E, 0x264F, 0x2650, 0x2651, 0x2652, 0x2653, 0x267F, 0x2693, 0x26A1, 0x26AA, 0x26AB, 0x26BD, 0x26BE, 0x26C4,
        0x26C5, 0x26CE, 0x26D4, 0x26EA, 0x26F2, 0x26F3, 0x26F5, 0x26FA, 0x26FD, 0x2705, 0x270A, 0x270B, 0x2728, 0x274C, 0x2753, 0x2757,
    ];
    const JA_EMOJI2: [u32; 10] = [0x274E, 0x2754, 0x2755, 0x2795, 0x2796, 0x2797, 0x27B0, 0x27BF, 0x2B50, 0x2B55];
    (0x1F000..=0x1FAFF).contains(&u)
        || JA_EMOJI.contains(&u)
        || JA_EMOJI2.contains(&u)
        || u == 0x2B1B
        || u == 0x2B1C
        || (fe0f
            && (matches!(u, 0x2190..=0x21FF | 0x2300..=0x23FF | 0x2460..=0x27BF | 0x2900..=0x297F | 0x2B00..=0x2BFF | 0x3030 | 0x303D | 0x3297 | 0x3299 | 0xA9 | 0xAE | 0x203C | 0x2049 | 0x2122 | 0x2139)
                || c.is_ascii_digit()
                || c == '#'
                || c == '*'))
}

/// O nome em pedaços. Os códigos de posição seguem o `TOKEN` do site (2 a 10 letras entre dois pontos).
pub fn partes(s: &str) -> Vec<Parte> {
    let cs: Vec<char> = s.chars().collect();
    let mut out: Vec<Parte> = Vec::new();
    let mut txt = String::new();
    let flush = |txt: &mut String, out: &mut Vec<Parte>| {
        if !txt.is_empty() {
            out.push(Parte::Texto(std::mem::take(txt)));
        }
    };
    let mut i = 0;
    while i < cs.len() {
        let c = cs[i];
        if c == ':' {
            // procura o ":" que fecha, só com letras no meio
            let mut j = i + 1;
            while j < cs.len() && j - i <= 11 && cs[j].is_alphabetic() {
                j += 1;
            }
            if j < cs.len() && cs[j] == ':' && (2..=10).contains(&(j - i - 1)) {
                let palavra: String = cs[i + 1..j].iter().collect();
                if let Some(p) = pos_de(&palavra) {
                    flush(&mut txt, &mut out);
                    out.push(Parte::Pos(p));
                    i = j + 1;
                    continue;
                }
            }
        }
        if pictograma(c, cs.get(i + 1).copied()) {
            // a sequência inteira: seletores, tons de pele, keycap, ZWJ + o próximo, e as duas letras de uma bandeira
            let mut seq = vec![c];
            let mut j = i + 1;
            let bandeira = (0x1F1E6..=0x1F1FF).contains(&(c as u32));
            if bandeira && j < cs.len() && (0x1F1E6..=0x1F1FF).contains(&(cs[j] as u32)) {
                seq.push(cs[j]);
                j += 1;
            }
            while j < cs.len() {
                let u = cs[j] as u32;
                if u == 0xFE0F || u == 0x20E3 || (0x1F3FB..=0x1F3FF).contains(&u) || (0xE0020..=0xE007F).contains(&u) {
                    seq.push(cs[j]);
                    j += 1;
                } else if u == 0x200D && j + 1 < cs.len() {
                    seq.push(cs[j]);
                    seq.push(cs[j + 1]);
                    j += 2;
                } else {
                    break;
                }
            }
            // regra dos arquivos do Twemoji: sem ZWJ, o U+FE0F sai do nome
            let zwj = seq.contains(&'\u{200D}');
            let cod: Vec<String> = seq.iter().filter(|&&ch| zwj || ch != '\u{FE0F}').map(|&ch| format!("{:x}", ch as u32)).collect();
            flush(&mut txt, &mut out);
            out.push(Parte::Emoji(cod.join("-")));
            i = j;
            continue;
        }
        txt.push(c);
        i += 1;
    }
    flush(&mut txt, &mut out);
    out
}

/// O `plainName` do site: cada código vira o nome da posição; os emojis ficam (para dica de mouse e texto de campo).
pub fn simples(s: &str) -> String {
    let mut out = String::new();
    for p in partes(s) {
        match p {
            Parte::Texto(t) => out.push_str(&t),
            Parte::Pos(p) => {
                out.push(' ');
                out.push_str(p.nome());
                out.push(' ');
            }
            Parte::Emoji(_) => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// O nome sem os códigos nem emojis (para as iniciais de uma sala sem foto).
pub fn sem_icones(s: &str) -> String {
    partes(s)
        .into_iter()
        .filter_map(|p| if let Parte::Texto(t) = p { Some(t) } else { None })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// A primeira posição do nome, se houver (a sala sem foto e sem letras mostra o ícone dela).
pub fn primeira_pos(s: &str) -> Option<Pos> {
    partes(s).into_iter().find_map(|p| if let Parte::Pos(p) = p { Some(p) } else { None })
}

pub fn icone_pos(ui: &egui::Ui, p: Pos, c: Pos2, size: f32, cor: Color32) {
    ui::icon(ui, p.icone(), c, size, cor);
}

/// A imagem de um emoji (Twemoji 72x72).
pub fn emoji(cod: &str) -> ImageSource<'static> {
    ImageSource::Uri(format!("https://cdn.jsdelivr.net/gh/jdecked/twemoji@15.1.0/assets/72x72/{cod}.png").into())
}

/// Lado do ícone e do emoji ao lado de uma letra deste tamanho (o `.rolei` do site: 1.1em).
fn lado(font: &FontId) -> f32 {
    (font.size * 1.15).round()
}

/// O egui não conta o espaço no fim de um texto: com o espaço duro, o ícone que vem depois não gruda na palavra.
fn duro(t: &str) -> String {
    t.replace(' ', "\u{a0}")
}

/// Largura que o nome ocupa.
pub fn largura(p: &egui::Painter, s: &str, font: FontId, spacing: f32) -> f32 {
    let l = lado(&font);
    let ps = partes(s);
    let n = ps.len();
    ps.iter()
        .enumerate()
        .map(|(k, x)| match x {
            Parte::Texto(t) => text_width(p, &duro(t), font.clone(), spacing),
            _ => l + if k + 1 < n { 1.0 } else { 0.0 },
        })
        .sum()
}

/// Desenha o nome como o `text` (mesma âncora e mesma volta), com os ícones e emojis no lugar. `max` > 0 corta com
/// reticências o que passar.
pub fn desenhar(ui: &egui::Ui, pos: Pos2, align: Align2, s: &str, font: FontId, cor: Color32, spacing: f32, max: f32) -> Rect {
    let p = ui.painter();
    let ps = partes(s);
    // o caso de sempre: só texto, sem custo extra
    if ps.iter().all(|x| matches!(x, Parte::Texto(_))) {
        return if max > 0.0 { ui::clip(p, pos, align, s, font, cor, max) } else { text(p, pos, align, s, font, cor, spacing) };
    }
    let l = lado(&font);
    let total = largura(p, s, font.clone(), spacing);
    let w = if max > 0.0 { total.min(max) } else { total };
    let h = font.size * 1.35;
    let caixa = align.anchor_size(pos, Vec2::new(w, h));
    let cy = caixa.center().y;
    let fim = caixa.left() + w;
    let mut x = caixa.left();
    let n = ps.len();
    for (k, parte) in ps.iter().enumerate() {
        let resta = fim - x;
        if resta <= 0.5 {
            break;
        }
        match parte {
            Parte::Texto(t) => {
                let t = &duro(t);
                let tw = text_width(p, t, font.clone(), spacing);
                if tw > resta {
                    ui::clip(p, Pos2::new(x, cy), Align2::LEFT_CENTER, t, font.clone(), cor, resta);
                    break;
                }
                text(p, Pos2::new(x, cy), Align2::LEFT_CENTER, t, font.clone(), cor, spacing);
                x += tw;
            }
            Parte::Pos(ps_) => {
                if l > resta {
                    break;
                }
                icone_pos(ui, *ps_, Pos2::new(x + l / 2.0, cy), l, cor);
                x += l + if k + 1 < n { 1.0 } else { 0.0 };
            }
            Parte::Emoji(cod) => {
                if l > resta {
                    break;
                }
                egui::Image::new(emoji(cod)).fit_to_exact_size(Vec2::splat(l)).show_loading_spinner(false).paint_at(ui, Rect::from_center_size(Pos2::new(x + l / 2.0, cy), Vec2::splat(l)));
                x += l + if k + 1 < n { 1.0 } else { 0.0 };
            }
        }
    }
    caixa
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn codigos_e_emojis() {
        assert_eq!(partes("TOP treino :top:"), vec![Parte::Texto("TOP treino ".into()), Parte::Pos(Pos::Top)]);
        assert_eq!(partes(":JG: x"), vec![Parte::Pos(Pos::Jungle), Parte::Texto(" x".into())]);
        assert_eq!(partes("a :qualquer: b"), vec![Parte::Texto("a :qualquer: b".into())]);
        assert_eq!(partes("me puxa 👋"), vec![Parte::Texto("me puxa ".into()), Parte::Emoji("1f44b".into())]);
        assert_eq!(partes("🛡️"), vec![Parte::Emoji("1f6e1".into())]);
        assert_eq!(partes("👨‍💻"), vec![Parte::Emoji("1f468-200d-1f4bb".into())]);
        assert_eq!(partes("🇧🇷"), vec![Parte::Emoji("1f1e7-1f1f7".into())]);
        assert_eq!(partes("👋🏽"), vec![Parte::Emoji("1f44b-1f3fd".into())]);
        assert_eq!(partes("ok © 2026"), vec![Parte::Texto("ok © 2026".into())]);
        assert_eq!(partes("★ ✓"), vec![Parte::Texto("★ ✓".into())]);
        assert_eq!(partes("❤️ ✅"), vec![Parte::Emoji("2764".into()), Parte::Texto(" ".into()), Parte::Emoji("2705".into())]);
        assert_eq!(simples("SUP treino :sup:"), "SUP treino Suporte");
        assert_eq!(sem_icones(":top: Os Amigos 🌍"), "Os Amigos");
    }
}

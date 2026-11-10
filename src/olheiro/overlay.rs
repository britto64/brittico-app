//! O overlay: ícones redondos dos aliados perto de você, no canto de cima à esquerda do jogo, bem transparentes. Quem fala ganha
//! um anel verde e fica mais opaco; quem está longe (volume zero) some, como no LoL de proximidade. Aparecem também os aliados que
//! não estão no Brittico (os bots de um teste, um amigo sem o app): ficam só apagados, porque deles não vem voz.
//!
//! No Windows é uma janela própria do sistema, com transparência por pixel, sempre por cima, que deixa o mouse passar e não pega o
//! foco (`overlay_win.rs`); o desenho é feito aqui, por software, e testado em qualquer sistema. Nos outros sistemas, só para
//! desenvolver, é uma janela do egui. Funciona com o LoL em Janela sem borda. Não injeta nada no jogo e fica fora da área que o
//! Olheiro captura (o minimapa, embaixo).

use super::{Estado, Fase, Janela, JanelaRef};
use olheiro_motor::{Art, CHEIO_ATE, ZERO_EM};
use std::collections::HashMap;

pub const CIRCULO: f32 = 44.0;
pub const VAO: f32 = 8.0;
pub const MARGEM: f32 = 6.0;
pub const MAX: usize = 5;
/// Acima disto o microfone de alguém (a nota do `falando`) conta como falando.
const FALANDO: f32 = 0.04;
/// O volume (distância × o que você escolheu) abaixo do qual a pessoa está longe demais para aparecer.
const AUDIVEL: f32 = 0.02;
/// A transparência de quem fala e de quem está calado.
const OPACO_FALANDO: f32 = 0.92;
const OPACO_CALADO: f32 = 0.38;

/// Quem aparece: a pessoa, se fala, e quão visível (some aos poucos com a distância).
#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub alias: String,
    pub nome: String,
    pub falando: bool,
    pub alpha: f32,
}

/// Quanto se ouviria alguém a esta distância (a mesma curva do volume: cheio até 900, zero em 1350, queda quadrática).
fn proximidade(d: f64) -> f32 {
    if !d.is_finite() || d < 0.0 || d <= CHEIO_ATE {
        1.0
    } else if d >= ZERO_EM {
        0.0
    } else {
        (((ZERO_EM - d) / (ZERO_EM - CHEIO_ATE)).powi(2)) as f32
    }
}

/// Quem aparece agora. `nivel`: o quanto cada pessoa da sala fala; `mic`: o seu microfone; `t`: relógio em segundos (só o modo de demonstração).
pub fn quem(est: &Estado, nivel: &HashMap<String, f32>, mic: f32, t: f64) -> Vec<Item> {
    let Some(pt) = &est.partida else { return Vec::new() };
    // BRITTICO_OVERLAY=demo: o time inteiro aparece, cada um falando na sua vez e se afastando, para ver e capturar sem uma sala de voz
    if std::env::var("BRITTICO_OVERLAY").as_deref() == Ok("demo") {
        return pt
            .jogadores
            .iter()
            .filter(|g| g.time == pt.meu_time)
            .enumerate()
            .map(|(i, g)| {
                let vez = (t as usize / 2) % 5 == i;
                Item { alias: g.alias.clone(), nome: g.campeao.clone(), falando: vez, alpha: if vez { OPACO_FALANDO } else { OPACO_CALADO - 0.04 * i as f32 } }
            })
            .collect();
    }
    let mut out = Vec::new();
    for jg in pt.jogadores.iter().filter(|g| g.time == pt.meu_time || pt.personalizada) {
        if jg.chave == pt.voce {
            // você só aparece enquanto fala
            if est.microfone_aberto && mic > 0.03 {
                out.push(Item { alias: jg.alias.clone(), nome: jg.campeao.clone(), falando: true, alpha: OPACO_FALANDO });
            }
            continue;
        }
        if let Some(o) = est.ouvidos.iter().find(|o| o.chave == jg.chave) {
            let volume = o.proximidade * o.escolhido;
            if !o.na_partida || o.mutado || est.surdo || volume < AUDIVEL {
                continue;
            }
            let falando = nivel.get(&jg.chave).copied().unwrap_or(0.0) > FALANDO && !o.mudo;
            let base = if falando { OPACO_FALANDO } else { OPACO_CALADO };
            out.push(Item { alias: jg.alias.clone(), nome: jg.campeao.clone(), falando, alpha: base * (0.45 + 0.55 * o.proximidade.clamp(0.0, 1.0)) });
        } else if jg.time == pt.meu_time {
            // fora do Brittico (um bot, um amigo sem o app): aparece pela distância no mapa, sempre calado
            let prox = match (est.posicoes.get(&pt.voce), est.posicoes.get(&jg.chave)) {
                (Some(a), Some(b)) => proximidade(((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()),
                _ => 1.0,
            };
            if prox < AUDIVEL {
                continue;
            }
            out.push(Item { alias: jg.alias.clone(), nome: jg.campeao.clone(), falando: false, alpha: OPACO_CALADO * (0.45 + 0.55 * prox) });
        }
    }
    // quem fala fica no topo
    out.sort_by_key(|i| !i.falando);
    out.truncate(MAX);
    out
}

/// Quem aparece, lido do motor: usado pela janela do egui e pelo fio do Windows.
pub fn quem_agora(j: &Janela, t: f64) -> (Estado, Vec<Item>) {
    let est = j.motor.estado();
    let nivel: HashMap<String, f32> = j.motor.falando().into_iter().map(|(k, n, _)| (k, n)).collect();
    let itens = quem(&est, &nivel, j.motor.nivel_microfone(), t);
    (est, itens)
}

/// Uma imagem BGRA com o alfa já multiplicado (o que o Windows pede para uma janela com transparência por pixel).
#[derive(Clone, Debug, PartialEq)]
pub struct Imagem {
    pub w: usize,
    pub h: usize,
    pub bgra: Vec<u8>,
}

fn cor_do_nome(nome: &str) -> [f32; 3] {
    let h = nome.bytes().fold(7u32, |a, b| a.wrapping_mul(31).wrapping_add(b as u32));
    let t = (h % 360) as f32 / 60.0;
    let (c, x) = (0.55_f32, 0.55 * (1.0 - (t % 2.0 - 1.0).abs()));
    let (r, g, b) = match t as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    [r + 0.25, g + 0.25, b + 0.25]
}

/// A cor do ícone no ponto (`u`, `v`) de 0 a 1, em RGBA de 0 a 1 (média de quatro amostras).
fn amostra(art: &Art, u: f32, v: f32, passo: f32) -> [f32; 4] {
    let mut acc = [0.0f32; 4];
    for (du, dv) in [(-0.25, -0.25), (0.25, -0.25), (-0.25, 0.25), (0.25, 0.25)] {
        let x = (((u + du * passo) * art.w as f32) - 0.5).clamp(0.0, art.w as f32 - 1.0);
        let y = (((v + dv * passo) * art.h as f32) - 0.5).clamp(0.0, art.h as f32 - 1.0);
        let (x0, y0) = (x.floor() as usize, y.floor() as usize);
        let (x1, y1) = ((x0 + 1).min(art.w - 1), (y0 + 1).min(art.h - 1));
        let (fx, fy) = (x - x0 as f32, y - y0 as f32);
        let px = |xx: usize, yy: usize| {
            let i = (yy * art.w + xx) * 4;
            let a = art.rgba[i + 3] as f32 / 255.0;
            [art.rgba[i] as f32 / 255.0 * a, art.rgba[i + 1] as f32 / 255.0 * a, art.rgba[i + 2] as f32 / 255.0 * a, a]
        };
        let (p00, p10, p01, p11) = (px(x0, y0), px(x1, y0), px(x0, y1), px(x1, y1));
        for c in 0..4 {
            acc[c] += (p00[c] * (1.0 - fx) + p10[c] * fx) * (1.0 - fy) + (p01[c] * (1.0 - fx) + p11[c] * fx) * fy;
        }
    }
    acc.map(|v| v / 4.0)
}

/// Desenha os círculos, de cima para baixo, numa imagem transparente. `escala`: 1 para um jogo de 1080p.
pub fn rasterizar(itens: &[(Item, Option<Art>)], escala: f32) -> Imagem {
    let e = escala.max(0.5);
    let (circulo, vao, margem) = (CIRCULO * e, VAO * e, MARGEM * e);
    let w = (circulo + 2.0 * margem).ceil() as usize;
    let n = itens.len().max(1);
    let h = (n as f32 * (circulo + vao) - vao + 2.0 * margem).ceil() as usize;
    let mut px = vec![0.0f32; w * h * 4]; // RGBA premultiplicado, de 0 a 1
    let anel_verde = [0x3E as f32 / 255.0, 0x9C as f32 / 255.0, 0x5C as f32 / 255.0];
    for (i, (it, art)) in itens.iter().enumerate() {
        let cx = margem + circulo / 2.0;
        let cy = margem + circulo / 2.0 + i as f32 * (circulo + vao);
        let r = circulo / 2.0;
        let espessura = if it.falando { 3.0 * e } else { 1.5 * e };
        let alpha = it.alpha.clamp(0.0, 1.0);
        let cor_nome = cor_do_nome(&it.nome);
        let (y0, y1) = (((cy - r - 2.0).floor().max(0.0)) as usize, ((cy + r + 2.0).ceil() as usize).min(h));
        let (x0, x1) = (((cx - r - 2.0).floor().max(0.0)) as usize, ((cx + r + 2.0).ceil() as usize).min(w));
        for y in y0..y1 {
            for x in x0..x1 {
                let (dx, dy) = (x as f32 + 0.5 - cx, y as f32 + 0.5 - cy);
                let d = (dx * dx + dy * dy).sqrt();
                let disco = (r - d + 0.5).clamp(0.0, 1.0);
                if disco <= 0.0 {
                    continue;
                }
                // o ícone (ou, sem ele, uma cor do nome)
                let base = match art {
                    Some(a) => amostra(a, (dx + r) / (2.0 * r), (dy + r) / (2.0 * r), 1.0 / (2.0 * r)),
                    None => [cor_nome[0], cor_nome[1], cor_nome[2], 1.0],
                };
                let mut c = [base[0] * disco * alpha, base[1] * disco * alpha, base[2] * disco * alpha, base[3] * disco * alpha];
                // o anel, por cima, dentro da borda
                let faixa = ((d - (r - espessura)) + 0.5).clamp(0.0, 1.0) * disco;
                if faixa > 0.0 {
                    let (cor, a) = if it.falando { (anel_verde, alpha.max(0.9)) } else { ([0.0, 0.0, 0.0], 0.35 * alpha.max(0.4)) };
                    let sa = faixa * a;
                    for k in 0..3 {
                        c[k] = cor[k] * sa + c[k] * (1.0 - sa);
                    }
                    c[3] = sa + c[3] * (1.0 - sa);
                }
                let o = (y * w + x) * 4;
                px[o..o + 4].copy_from_slice(&c);
            }
        }
    }
    let mut bgra = vec![0u8; w * h * 4];
    for i in 0..w * h {
        let a = px[i * 4 + 3];
        let q = |v: f32| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
        bgra[i * 4] = q(px[i * 4 + 2].min(a));
        bgra[i * 4 + 1] = q(px[i * 4 + 1].min(a));
        bgra[i * 4 + 2] = q(px[i * 4].min(a));
        bgra[i * 4 + 3] = q(a);
    }
    Imagem { w, h, bgra }
}

/// Onde o overlay fica, em pixels da tela: perto do canto de cima à esquerda da janela do jogo, e a escala dele.
pub fn lugar(janela: (i32, i32, i32, i32)) -> (i32, i32, f32) {
    let (x, y, _w, h) = janela;
    let escala = (h as f32 / 1080.0).clamp(0.8, 2.0);
    (x + (12.0 * escala) as i32, y + (h as f32 * 0.12) as i32, escala)
}

/// Quando o overlay deve existir: a pessoa quer, o Olheiro está ligado e há partida com o jogo à vista.
pub fn quer(est: &Estado) -> Option<(i32, i32, i32, i32)> {
    est.leitura.janela.filter(|_| est.ligado && est.fase == Fase::EmPartida && est.ajustes.overlay)
}

#[cfg(windows)]
pub use super::overlay_win::iniciar;

/// Fora do Windows o overlay é uma janela do egui, só para desenvolver e capturar.
#[cfg(not(windows))]
pub fn iniciar(_j: JanelaRef) {}

#[cfg(not(windows))]
pub fn hospedar(ctx: &eframe::egui::Context, j: &JanelaRef) {
    use eframe::egui::{self, Pos2};
    let est = match j.lock() {
        Ok(g) => g.motor.estado(),
        Err(_) => return,
    };
    let Some(janela) = quer(&est) else {
        if let Ok(mut g) = j.lock() {
            g.overlay_pos = None;
        }
        return;
    };
    let ppp = ctx.pixels_per_point().max(0.5);
    let (px, py, _) = lugar(janela);
    let pos = Pos2::new(px as f32 / ppp, py as f32 / ppp);
    if let Ok(mut g) = j.lock() {
        if g.overlay_pos.is_some_and(|p| (p - pos).length() > 1.0) {
            ctx.send_viewport_cmd_to(vp(), egui::ViewportCommand::OuterPosition(pos));
        }
        g.overlay_pos = Some(pos);
    }
    let j2 = j.clone();
    ctx.show_viewport_deferred(vp(), builder(pos), move |ctx, _| mostrar(ctx, &j2));
}

#[cfg(windows)]
pub fn hospedar(_ctx: &eframe::egui::Context, _j: &JanelaRef) {}

#[cfg(not(windows))]
fn vp() -> eframe::egui::ViewportId {
    eframe::egui::ViewportId::from_hash_of("olheiro-overlay")
}

#[cfg(not(windows))]
fn builder(pos: eframe::egui::Pos2) -> eframe::egui::ViewportBuilder {
    eframe::egui::ViewportBuilder::default()
        .with_title("Olheiro overlay")
        .with_decorations(false)
        .with_transparent(true)
        .with_always_on_top()
        .with_mouse_passthrough(true)
        .with_taskbar(false)
        .with_resizable(false)
        .with_active(false)
        .with_position(pos)
        .with_inner_size([CIRCULO + 2.0 * MARGEM, (CIRCULO + VAO) * MAX as f32 + 2.0 * MARGEM])
}

#[cfg(not(windows))]
fn mostrar(ctx: &eframe::egui::Context, j: &JanelaRef) {
    use eframe::egui;
    let Ok(mut j) = j.lock() else { return };
    let t = ctx.input(|i| i.time);
    let (est, itens) = quem_agora(&j, t);
    let com_arte: Vec<(Item, Option<Art>)> = itens.into_iter().map(|i| { let a = j.motor.icone(&i.alias); (i, a) }).collect();
    let img = rasterizar(&com_arte, 1.0);
    let rgba: Vec<u8> = img.bgra.chunks(4).flat_map(|p| [p[2], p[1], p[0], p[3]]).collect();
    let tex = ctx.load_texture("olheiro-overlay", egui::ColorImage::from_rgba_premultiplied([img.w, img.h], &rgba), egui::TextureOptions::LINEAR);
    egui::CentralPanel::default().frame(egui::Frame::NONE).show(ctx, |ui| {
        egui::Image::new((tex.id(), egui::vec2(img.w as f32, img.h as f32))).paint_at(ui, egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(img.w as f32, img.h as f32)));
    });
    if est.fase == Fase::EmPartida {
        ctx.request_repaint_after(std::time::Duration::from_millis(66));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(falando: bool, alpha: f32) -> Item {
        Item { alias: "ahri".into(), nome: "Ahri".into(), falando, alpha }
    }

    fn alfa(img: &Imagem, x: usize, y: usize) -> u8 {
        img.bgra[(y * img.w + x) * 4 + 3]
    }

    #[test]
    fn o_fundo_e_totalmente_transparente_e_o_centro_do_circulo_nao() {
        let img = rasterizar(&[(item(false, 0.5), None)], 1.0);
        assert_eq!(alfa(&img, 0, 0), 0);
        assert_eq!(alfa(&img, img.w - 1, img.h - 1), 0);
        let (cx, cy) = (img.w / 2, (MARGEM + CIRCULO / 2.0) as usize);
        let a = alfa(&img, cx, cy);
        assert!((100..=140).contains(&a), "meio transparente: {a}");
    }

    #[test]
    fn quem_fala_tem_o_anel_verde() {
        let img = rasterizar(&[(item(true, 0.9), None)], 1.0);
        let (x, y) = ((MARGEM + 1.5) as usize, (MARGEM + CIRCULO / 2.0) as usize);
        let i = (y * img.w + x) * 4;
        let (b, g, r) = (img.bgra[i], img.bgra[i + 1], img.bgra[i + 2]);
        assert!(g > r && g > b, "anel verde: b{b} g{g} r{r}");
    }

    #[test]
    fn a_cor_vem_multiplicada_pelo_alfa() {
        let art = Art { w: 4, h: 4, rgba: [200u8, 100, 50, 255].repeat(16) };
        let img = rasterizar(&[(item(false, 0.4), Some(art))], 1.0);
        for p in img.bgra.chunks(4) {
            assert!(p[0] <= p[3] && p[1] <= p[3] && p[2] <= p[3], "premultiplicado");
        }
    }

    #[test]
    fn a_proximidade_segue_a_curva_do_volume() {
        assert_eq!(proximidade(0.0), 1.0);
        assert_eq!(proximidade(900.0), 1.0);
        assert!((proximidade(1125.0) - 0.25).abs() < 1e-6);
        assert_eq!(proximidade(1350.0), 0.0);
    }
}

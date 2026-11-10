//! O overlay: uma janela transparente, sempre por cima e que deixa o mouse passar, no canto de cima à esquerda do jogo. Mostra o
//! ícone redondo de cada aliado que você ouve; quem fala ganha um anel verde. Quem está longe (volume zero) some, como no LoL de
//! proximidade: se você não o ouve, não há por que vê-lo.
//!
//! Só aparece em partida, com o jogo em modo Janela sem borda (o mesmo que o Olheiro já pede para ler o minimapa). Não injeta nada no
//! jogo: é uma janela comum do Brittico por cima dele. Fica fora da área que o Olheiro captura, que é a do minimapa, embaixo.

use super::{Estado, Fase, Janela, JanelaRef};
use crate::theme::*;
use crate::ui::{self, text};
use eframe::egui::{self, Align2, Color32, Pos2, Rect, Stroke, Vec2, ViewportBuilder, ViewportCommand, ViewportId};
use std::time::Duration;

pub fn vp() -> ViewportId {
    ViewportId::from_hash_of("olheiro-overlay")
}

const CIRCULO: f32 = 44.0;
const VAO: f32 = 8.0;
const MARGEM: f32 = 6.0;
const MAX: usize = 5;
/// Acima disto o microfone de alguém (a nota do `falando`) conta como falando.
const FALANDO: f32 = 0.04;
/// O volume (distância × o que você escolheu) abaixo do qual a pessoa está longe demais para aparecer.
const AUDIVEL: f32 = 0.02;

pub fn builder(pos: Pos2) -> ViewportBuilder {
    ViewportBuilder::default()
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

/// Quem aparece: a pessoa, se fala, e quão visível (some aos poucos com a distância).
struct Item {
    alias: String,
    nome: String,
    falando: bool,
    alpha: f32,
}

fn quem(j: &Janela, est: &Estado, t: f64) -> Vec<Item> {
    let Some(pt) = &est.partida else { return Vec::new() };
    // BRITTICO_OVERLAY=demo: o time inteiro aparece, cada um falando na sua vez e se afastando, para ver e capturar sem uma sala de voz
    if std::env::var("BRITTICO_OVERLAY").as_deref() == Ok("demo") {
        return pt
            .jogadores
            .iter()
            .filter(|g| g.time == pt.meu_time)
            .enumerate()
            .map(|(i, g)| Item { alias: g.alias.clone(), nome: g.campeao.clone(), falando: (t as usize / 2) % 5 == i, alpha: if (t as usize / 2) % 5 == i { 1.0 } else { 0.5 - 0.06 * i as f32 } })
            .collect();
    }
    let nivel: std::collections::HashMap<String, f32> = j.motor.falando().into_iter().map(|(k, n, _)| (k, n)).collect();
    let mut out = Vec::new();
    for jg in pt.jogadores.iter().filter(|g| g.time == pt.meu_time || pt.personalizada) {
        if jg.chave == pt.voce {
            // você só aparece enquanto fala
            if est.microfone_aberto && j.motor.nivel_microfone() > 0.03 {
                out.push(Item { alias: jg.alias.clone(), nome: jg.campeao.clone(), falando: true, alpha: 1.0 });
            }
            continue;
        }
        let Some(o) = est.ouvidos.iter().find(|o| o.chave == jg.chave) else { continue };
        let volume = o.proximidade * o.escolhido;
        if !o.na_partida || o.mutado || est.surdo || volume < AUDIVEL {
            continue;
        }
        let falando = nivel.get(&jg.chave).copied().unwrap_or(0.0) > FALANDO && !o.mudo;
        let base = if falando { 1.0 } else { 0.5 };
        out.push(Item { alias: jg.alias.clone(), nome: jg.campeao.clone(), falando, alpha: base * (0.45 + 0.55 * o.proximidade.clamp(0.0, 1.0)) });
    }
    // quem fala por último fica no topo
    out.sort_by_key(|i| !i.falando);
    out.truncate(MAX);
    out
}

/// Chamada a cada quadro da janela grande: abre o overlay quando há partida e a pessoa quer, fecha (deixa de mostrá-lo) quando não.
pub fn hospedar(ctx: &egui::Context, j: &JanelaRef) {
    let est = match j.lock() {
        Ok(g) => g.motor.estado(),
        Err(_) => return,
    };
    let Some((x, y, _w, h)) = est.leitura.janela.filter(|_| est.ligado && est.fase == Fase::EmPartida && est.ajustes.overlay) else {
        if let Ok(mut g) = j.lock() {
            g.overlay_pos = None;
        }
        return;
    };
    let ppp = ctx.pixels_per_point().max(0.5);
    let pos = Pos2::new((x as f32 + 12.0) / ppp, (y as f32 + h as f32 * 0.12) / ppp);
    if let Ok(mut g) = j.lock() {
        if g.overlay_pos.is_some_and(|p| (p - pos).length() > 1.0) {
            ctx.send_viewport_cmd_to(vp(), ViewportCommand::OuterPosition(pos));
        }
        g.overlay_pos = Some(pos);
    }
    let j2 = j.clone();
    ctx.show_viewport_deferred(vp(), builder(pos), move |ctx, _| mostrar(ctx, &j2));
}

fn mostrar(ctx: &egui::Context, j: &JanelaRef) {
    let Ok(mut j) = j.lock() else { return };
    let j = &mut *j;
    let est = j.motor.estado();
    let t = ctx.input(|i| i.time);
    let itens = quem(j, &est, t);
    egui::CentralPanel::default().frame(egui::Frame::NONE).show(ctx, |ui| {
        let p = ui.painter().clone();
        for (i, it) in itens.iter().enumerate() {
            let r = Rect::from_min_size(Pos2::new(MARGEM, MARGEM + i as f32 * (CIRCULO + VAO)), Vec2::splat(CIRCULO));
            let a = (it.alpha.clamp(0.0, 1.0) * 255.0) as u8;
            match j.icone(ui.ctx(), &it.alias, t) {
                Some(tx) => {
                    egui::Image::new((tx.id(), r.size())).corner_radius(CIRCULO / 2.0).tint(Color32::from_white_alpha(a)).paint_at(ui, r);
                }
                None => {
                    p.circle_filled(r.center(), CIRCULO / 2.0, ui::mix(ui::hsl(ui::hue_of(&it.nome), 0.55, 0.50), Color32::BLACK, 0.3).gamma_multiply(it.alpha));
                    text(&p, r.center(), Align2::CENTER_CENTER, &ui::initials(&it.nome), f8(15.0), Color32::WHITE.gamma_multiply(it.alpha), 0.0);
                }
            }
            let anel = if it.falando { Stroke::new(3.0_f32, DIS_OK) } else { Stroke::new(1.5_f32, Color32::from_black_alpha((90.0 * it.alpha) as u8)) };
            p.circle_stroke(r.center(), CIRCULO / 2.0, anel);
        }
    });
    if est.fase == Fase::EmPartida {
        ctx.request_repaint_after(Duration::from_millis(66));
    }
}

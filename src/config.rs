//! As configurações do próprio app (a engrenagem no pé da trilha). O site não tem uma página assim: o que entra aqui
//! são os ajustes de voz e vídeo da Call do site (`call/js/ui-voice.js`: aparelhos, ruído, eco, volume automático), os
//! atalhos dela, a aparência (as mesmas preferências do perfil) e o que só um programa tem (abrir com o sistema, ficar na
//! bandeja). O visual é o das abas de configuração do perfil: o menu à esquerda e um cartão por aba.

use crate::perfil::ajustes::{self, Btn};
use crate::theme::*;
use crate::ui::{self, mix, text, text_width};
use crate::ic;
use eframe::egui::{self, Align2, Color32, CursorIcon, ImageSource, Order, Pos2, Rect, Sense, Stroke, StrokeKind, UiBuilder, Vec2};

const ABAS: [(&str, &str); 5] = [("voz", "Voz e vídeo"), ("atalhos", "Atalhos"), ("aparencia", "Aparência"), ("app", "O app"), ("sobre", "Sobre")];
const MICS: [&str; 3] = ["Padrão do sistema", "Microfone (HyperX Cloud II)", "Webcam C920 (microfone)"];
const SAIDAS: [&str; 3] = ["Padrão do sistema", "Fone (HyperX Cloud II)", "Alto-falantes (Realtek)"];
const CAMS: [&str; 2] = ["Webcam C920", "OBS Virtual Camera"];

pub struct ConfigState {
    aba: usize,
    mic: usize,
    saida: usize,
    cam: usize,
    /// A lista aberta de um dos seletores (0 microfone, 1 saída, 2 câmera).
    lista: Option<usize>,
    testando_mic: bool,
    vendo_cam: bool,
    ruido: bool,
    eco: bool,
    volume_auto: bool,
    apertar: bool,
    sensibilidade: f32,
    iniciar: bool,
    bandeja: bool,
    avisos: bool,
    retomar: bool,
    animacoes: bool,
    toast: ui::Toast,
}

impl Default for ConfigState {
    fn default() -> Self {
        Self {
            aba: 0,
            mic: 1,
            saida: 1,
            cam: 0,
            lista: None,
            testando_mic: false,
            vendo_cam: false,
            ruido: true,
            eco: true,
            volume_auto: true,
            apertar: false,
            sensibilidade: 0.35,
            iniciar: false,
            bandeja: true,
            avisos: true,
            retomar: true,
            animacoes: true,
            toast: ui::Toast::default(),
        }
    }
}

impl ConfigState {
    /// Se as animações de enfeite estão ligadas (as capas flutuando nos Jogos).
    pub fn animacoes(&self) -> bool {
        self.animacoes
    }
    pub fn abrir(&mut self, aba: usize) {
        self.aba = aba.min(ABAS.len() - 1);
    }
}

fn tab_icon(key: &str) -> ImageSource<'static> {
    match key {
        "voz" => ic!("mic"),
        "atalhos" => ic!("bolt"),
        "aparencia" => ic!("pf-tab-preferences"),
        "app" => ic!("gear"),
        _ => ic!("info"),
    }
}

pub fn show(ui: &mut egui::Ui, st: &mut ConfigState, perfil: &mut crate::perfil::PerfilState, body: Rect, t: f64) {
    let menu = Rect::from_min_size(body.min, Vec2::new(248.0, body.height()));
    let main = Rect::from_min_max(menu.right_top(), body.max);
    // o menu de abas, como o do perfil
    let p = ui.painter().clone();
    p.rect_filled(menu, 0.0, CREAM_2());
    let x = menu.left() + 12.0;
    let w = menu.width() - 24.0;
    text(&p, Pos2::new(x + 8.0, menu.top() + 12.0 + 21.5), Align2::LEFT_CENTER, "CONFIGURAÇÕES DO APP", f8(11.0), FAINT(), 0.66);
    let mut y = menu.top() + 12.0 + 35.0;
    for (i, (key, nome)) in ABAS.iter().enumerate() {
        let r = Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, 35.0));
        let resp = ui.interact(r, ui.id().with(("cfg-aba", i)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
        let on = st.aba == i;
        let (bg, fg, icf) = if on { (Some(SECONDARY()), ON_SECONDARY(), ON_SECONDARY()) } else if resp.hovered() { (Some(AMBER_SOFT()), INK(), FAINT()) } else { (None, SOFT(), FAINT()) };
        if let Some(bg) = bg {
            p.rect_filled(r, R_XS(), bg);
        }
        ui::icon(ui, tab_icon(key), Pos2::new(r.left() + 18.0, r.center().y), 16.0, icf);
        text(&p, Pos2::new(r.left() + 36.0, r.center().y), Align2::LEFT_CENTER, nome, f6(14.5), fg, 0.0);
        if resp.clicked() {
            st.aba = i;
            st.lista = None;
        }
        y += 36.0;
    }

    let mut child = ui.new_child(UiBuilder::new().max_rect(main));
    egui::ScrollArea::vertical().id_salt(("cfg", st.aba)).auto_shrink(false).show(&mut child, |ui| {
        let inner = 820.0_f32.min(main.width());
        let x = main.center().x - inner / 2.0 + 24.0;
        let w = inner - 48.0;
        let h = match st.aba {
            0 => 730.0,
            1 => 120.0 + 7.0 * 44.0 + 24.0,
            2 => 513.0,
            3 => 120.0 + 5.0 * 58.0 + 20.0,
            _ => 330.0,
        };
        let (r, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 24.0 + h + 90.0), Sense::hover());
        let top = r.top() + 24.0;
        match st.aba {
            0 => voz(ui, st, x, top, w, t),
            1 => atalhos(ui, x, top, w),
            2 => ajustes::preferencias(ui, perfil, x, top, w, t),
            3 => app(ui, st, x, top, w),
            _ => sobre(ui, st, x, top, w, t),
        }
    });
    st.toast.show(ui, t);
}

/// Um seletor (`<select>`): a caixa com o nome do escolhido e a seta; o clique abre a lista por cima.
fn seletor(ui: &egui::Ui, st: &mut ConfigState, k: usize, r: Rect, opcoes: &[&str], atual: usize) -> Option<usize> {
    let p = ui.painter();
    let resp = ui.interact(r, ui.id().with(("cfg-sel", k)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
    let aberto = st.lista == Some(k);
    p.rect_filled(r, R_MD(), CREAM_0());
    p.rect_stroke(r, R_MD(), Stroke::new(1.0_f32, if aberto || resp.hovered() { SECONDARY() } else { HAIRLINE_2() }), StrokeKind::Inside);
    ui::clip(p, Pos2::new(r.left() + 14.0, r.center().y), Align2::LEFT_CENTER, opcoes[atual], f6(14.0), INK(), r.width() - 44.0);
    ui::icon(ui, if aberto { ic!("chevron-up") } else { ic!("chevron-down") }, Pos2::new(r.right() - 18.0, r.center().y), 14.0, FAINT());
    if resp.clicked() {
        st.lista = if aberto { None } else { Some(k) };
    }
    if !aberto {
        return None;
    }
    let lr = Rect::from_min_size(r.left_bottom() + Vec2::new(0.0, 6.0), Vec2::new(r.width(), 8.0 + opcoes.len() as f32 * 36.0 + 8.0));
    let esc = egui::Area::new(ui.id().with(("cfg-lista", k))).order(Order::Foreground).fixed_pos(lr.min).constrain(false).show(ui.ctx(), |ui| {
        ui.allocate_exact_size(lr.size(), Sense::click());
        let p = ui.painter();
        p.rect_filled(lr.translate(Vec2::new(0.0, 6.0)).expand(3.0), R_MD() + 3.0, Color32::from_black_alpha(50));
        p.rect_filled(lr, R_MD(), CREAM_0());
        p.rect_stroke(lr, R_MD(), Stroke::new(1.0_f32, HAIRLINE_2()), StrokeKind::Inside);
        let mut esc = None;
        for (i, o) in opcoes.iter().enumerate() {
            let it = Rect::from_min_size(lr.min + Vec2::new(6.0, 8.0 + i as f32 * 36.0), Vec2::new(lr.width() - 12.0, 34.0));
            let ir = ui.interact(it, ui.id().with(("cfg-op", k, i)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
            if i == atual {
                p.rect_filled(it, R_SM(), AMBER_SOFT());
            } else if ir.hovered() {
                p.rect_filled(it, R_SM(), CREAM_2());
            }
            text(p, Pos2::new(it.left() + 10.0, it.center().y), Align2::LEFT_CENTER, o, f6(13.5), if i == atual { SECONDARY() } else { INK() }, 0.0);
            if i == atual {
                ui::icon(ui, ic!("check"), Pos2::new(it.right() - 16.0, it.center().y), 14.0, SECONDARY());
            }
            if ir.clicked() {
                esc = Some(i);
            }
        }
        esc
    });
    if esc.inner.is_some() || (ui.input(|i| i.pointer.any_pressed()) && !ui.input(|i| i.pointer.interact_pos()).is_some_and(|q| lr.contains(q) || r.contains(q))) {
        st.lista = None;
    }
    esc.inner
}

/// Uma linha com título, frase e o interruptor (a linha toda é clicável, como o `.toggle-row` do perfil).
fn chave(ui: &egui::Ui, x: f32, y: f32, w: f32, id: &str, titulo: &str, desc: &str, on: &mut bool) -> f32 {
    let p = ui.painter();
    let mut job = egui::text::LayoutJob::single_section(desc.to_owned(), egui::TextFormat { font_id: f6(12.5), color: SOFT(), line_height: Some(17.0), ..Default::default() });
    job.wrap.max_width = w - 60.0;
    let g = p.layout_job(job);
    let h = 19.0 + 2.0 + g.size().y;
    let row = Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, h));
    let resp = ui.interact(row, ui.id().with(("cfg-chave", id)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
    text(p, Pos2::new(x, y + 9.5), Align2::LEFT_CENTER, titulo, f7(14.0), INK(), 0.0);
    p.galley(Pos2::new(x, y + 21.0), g, SOFT());
    if resp.clicked() {
        *on = !*on;
    }
    ajustes::switch(ui, Rect::from_min_size(Pos2::new(row.right() - 44.0, row.center().y - 12.0), Vec2::new(44.0, 24.0)), id, *on);
    h
}

fn voz(ui: &mut egui::Ui, st: &mut ConfigState, x: f32, top: f32, w: f32, t: f64) {
    let p = ui.painter().clone();
    let cx = ajustes::card(&p, x, top, w, 730.0, "Voz e vídeo", "Os aparelhos e o som da Call. Valem para todas as salas, neste computador.");
    let cw = w - 50.0;
    let bw = 120.0;
    let mut y = top + 119.3;

    // microfone, com o medidor
    ajustes::label(&p, cx, y, "Microfone");
    let sel = Rect::from_min_size(Pos2::new(cx, y + 24.0), Vec2::new(cw - bw - 10.0, 42.0));
    let b = Rect::from_min_size(Pos2::new(sel.right() + 10.0, sel.top() + 2.0), Vec2::new(bw, 38.0));
    if ajustes::button(ui, b, "testar-mic", if st.testando_mic { "Parar" } else { "Testar" }, Btn::Contorno).clicked() {
        st.testando_mic = !st.testando_mic;
    }
    let medidor = Rect::from_min_size(Pos2::new(cx, sel.bottom() + 12.0), Vec2::new(cw, 8.0));
    p.rect_filled(medidor, 4.0, CREAM_2());
    if st.testando_mic {
        // a voz "pisca" no meio das palavras: um nível de mentira que sobe e desce
        let nivel = (((t * 7.3).sin() * 0.5 + 0.5) * ((t * 2.1).sin() * 0.35 + 0.65)) as f32;
        let corte = st.sensibilidade;
        let cor = if nivel > corte { DIS_OK } else { FAINT() };
        p.rect_filled(Rect::from_min_size(medidor.min, Vec2::new(medidor.width() * nivel, 8.0)), 4.0, cor);
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(33));
    }
    p.rect_filled(Rect::from_center_size(Pos2::new(medidor.left() + medidor.width() * st.sensibilidade, medidor.center().y), Vec2::new(2.0, 14.0)), 0.0, SECONDARY());
    ajustes::hint(&p, cx, medidor.bottom() + 6.0, cw, if st.testando_mic { "Fale alguma coisa: a barra fica verde quando passa da linha da sensibilidade." } else { "Aperte Testar para ver o nível do microfone." }, FAINT());
    if let Some(i) = seletor(ui, st, 0, sel, &MICS, st.mic) {
        st.mic = i;
    }
    y = medidor.bottom() + 6.0 + 17.0 + 18.0;

    // alto-falante
    ajustes::label(&p, cx, y, "Alto-falante");
    let sel = Rect::from_min_size(Pos2::new(cx, y + 24.0), Vec2::new(cw - bw - 10.0, 42.0));
    let b = Rect::from_min_size(Pos2::new(sel.right() + 10.0, sel.top() + 2.0), Vec2::new(bw, 38.0));
    if ajustes::button(ui, b, "testar-som", "Testar", Btn::Contorno).clicked() {
        st.toast.say(&format!("Tocando o som de teste em \"{}\".", SAIDAS[st.saida]), t);
    }
    if let Some(i) = seletor(ui, st, 1, sel, &SAIDAS, st.saida) {
        st.saida = i;
    }
    y += 24.0 + 42.0 + 20.0;

    // câmera, com a prévia
    ajustes::label(&p, cx, y, "Câmera");
    let sel = Rect::from_min_size(Pos2::new(cx, y + 24.0), Vec2::new(cw - bw - 10.0, 42.0));
    let b = Rect::from_min_size(Pos2::new(sel.right() + 10.0, sel.top() + 2.0), Vec2::new(bw, 38.0));
    if ajustes::button(ui, b, "ver-cam", if st.vendo_cam { "Fechar" } else { "Ver" }, Btn::Contorno).clicked() {
        st.vendo_cam = !st.vendo_cam;
    }
    if let Some(i) = seletor(ui, st, 2, sel, &CAMS, st.cam) {
        st.cam = i;
    }
    y += 24.0 + 42.0 + 20.0;
    if st.vendo_cam {
        let prev = Rect::from_min_size(Pos2::new(cx + cw - 220.0, top + 24.0), Vec2::new(196.0, 110.0));
        p.rect_filled(prev, R_SM(), Color32::from_rgb(11, 11, 12));
        let hue = ui::hue_of("Kennzy");
        p.rect_filled(prev.shrink(1.0), R_SM(), mix(ui::hsl(hue, 0.35, 0.30), Color32::BLACK, 0.7));
        p.circle_filled(prev.center() - Vec2::new(0.0, 12.0), 18.0, ui::hsl(hue, 0.45, 0.62));
        p.circle_filled(prev.center() + Vec2::new(0.0, 46.0), 38.0, ui::hsl(hue, 0.45, 0.62));
        text(&p, prev.left_bottom() + Vec2::new(8.0, -10.0), Align2::LEFT_CENTER, "prévia (simulada)", f7(10.5), Color32::WHITE, 0.0);
    }

    // o som
    p.rect_filled(Rect::from_min_size(Pos2::new(cx, y - 6.0), Vec2::new(cw, 1.0)), 0.0, HAIRLINE_2());
    y += 10.0;
    y += chave(ui, cx, y, cw, "ruido", "Supressão de ruído", "Tira o ventilador, o teclado e o barulho da rua da sua voz.", &mut st.ruido) + 16.0;
    y += chave(ui, cx, y, cw, "eco", "Cancelamento de eco", "Evita que os outros se ouçam de volta quando você usa caixa de som.", &mut st.eco) + 16.0;
    y += chave(ui, cx, y, cw, "agc", "Ajuste automático do volume", "Sobe a voz baixa e segura a alta, para todo mundo chegar parecido.", &mut st.volume_auto) + 16.0;
    y += chave(ui, cx, y, cw, "apertar", "Apertar para falar", "O microfone só abre enquanto você segura a tecla (Ctrl). Desligado, ele abre sozinho quando você fala.", &mut st.apertar) + 18.0;

    // a sensibilidade da detecção de voz
    ajustes::label(&p, cx, y, if st.apertar { "Sensibilidade (sem efeito com apertar para falar)" } else { "Sensibilidade da detecção de voz" });
    let trilho = Rect::from_min_size(Pos2::new(cx, y + 26.0), Vec2::new(cw, 18.0));
    let resp = ui.interact(trilho, ui.id().with("cfg-sens"), Sense::click_and_drag()).on_hover_cursor(CursorIcon::PointingHand);
    if let Some(q) = resp.interact_pointer_pos() {
        if resp.dragged() || resp.clicked() {
            st.sensibilidade = ((q.x - trilho.left()) / trilho.width()).clamp(0.05, 0.95);
        }
    }
    let bar = Rect::from_center_size(trilho.center(), Vec2::new(trilho.width(), 6.0));
    let a = if st.apertar { 0.4 } else { 1.0 };
    p.rect_filled(bar, 3.0, CREAM_2());
    p.rect_filled(Rect::from_min_size(bar.min, Vec2::new(bar.width() * st.sensibilidade, 6.0)), 3.0, SECONDARY().gamma_multiply(a));
    p.circle_filled(Pos2::new(bar.left() + bar.width() * st.sensibilidade, bar.center().y), 9.0, if resp.hovered() || resp.dragged() { INK() } else { SOFT() });
    text(&p, Pos2::new(cx, trilho.bottom() + 12.0), Align2::LEFT_CENTER, "mais sensível", f6(11.5), FAINT(), 0.0);
    text(&p, Pos2::new(cx + cw, trilho.bottom() + 12.0), Align2::RIGHT_CENTER, "só voz alta", f6(11.5), FAINT(), 0.0);
}

fn atalhos(ui: &mut egui::Ui, x: f32, top: f32, w: f32) {
    let p = ui.painter().clone();
    let cx = ajustes::card(&p, x, top, w, 120.0 + 7.0 * 44.0 + 24.0, "Atalhos", "Valem dentro da call, quando você não está escrevendo uma mensagem.");
    let cw = w - 50.0;
    let lista = [
        ("M", "Ligar e desligar o microfone"),
        ("D", "Parar de ouvir todo mundo (e voltar)"),
        ("V", "Ligar e desligar a câmera"),
        ("S", "Compartilhar a tela"),
        ("I", "Mostrar as estatísticas dos vídeos"),
        ("Esc", "Sair da tela cheia e voltar para a grade"),
        ("Ctrl", "Apertar para falar (quando ligado em Voz e vídeo)"),
    ];
    let mut y = top + 112.0;
    for (tecla, oque) in lista {
        let kw = text_width(&p, tecla, f8(12.0), 0.0) + 20.0;
        let k = Rect::from_min_size(Pos2::new(cx, y + 6.0), Vec2::new(kw.max(32.0), 28.0));
        p.rect_filled(k.translate(Vec2::new(0.0, 2.0)), R_XS(), CREAM_4());
        p.rect_filled(k, R_XS(), CREAM_2());
        text(&p, k.center(), Align2::CENTER_CENTER, tecla, f8(12.0), INK(), 0.0);
        text(&p, Pos2::new(cx + 70.0, k.center().y), Align2::LEFT_CENTER, oque, f6(14.0), SOFT(), 0.0);
        y += 44.0;
        if y < top + 112.0 + 6.0 * 44.0 {
            p.rect_filled(Rect::from_min_size(Pos2::new(cx, y), Vec2::new(cw, 1.0)), 0.0, HAIRLINE_2());
        }
    }
}

fn app(ui: &mut egui::Ui, st: &mut ConfigState, x: f32, top: f32, w: f32) {
    let p = ui.painter().clone();
    let cx = ajustes::card(&p, x, top, w, 120.0 + 5.0 * 58.0 + 20.0, "O app", "Como o Brittico se comporta neste computador.");
    let cw = w - 50.0;
    let mut y = top + 119.3;
    y += chave(ui, cx, y, cw, "iniciar", "Abrir junto com o computador", "O app já fica pronto na bandeja quando o computador liga, sem abrir a janela.", &mut st.iniciar) + 20.0;
    y += chave(ui, cx, y, cw, "bandeja", "Fechar para a bandeja", "O X da janela esconde o app em vez de sair: a call continua e você volta pelo ícone perto do relógio.", &mut st.bandeja) + 20.0;
    y += chave(ui, cx, y, cw, "avisos", "Avisar quando me chamarem", "Uma notificação do sistema quando alguém te marca ou te puxa para uma sala.", &mut st.avisos) + 20.0;
    y += chave(ui, cx, y, cw, "retomar", "Abrir onde parei", "Volta para a última tela que estava aberta, em vez do Início.", &mut st.retomar) + 20.0;
    chave(ui, cx, y, cw, "animacoes", "Animações", "As capas dos Jogos flutuando e os enfeites que se mexem. Desligar economiza processador em PC fraco.", &mut st.animacoes);
}

/// A memória própria do programa (o `RssAnon` do Linux: sem as bibliotecas e o driver de vídeo, que são divididos com
/// o resto do sistema), a mesma medida do README.
fn memoria() -> Option<f64> {
    let s = std::fs::read_to_string("/proc/self/status").ok()?;
    let kb: f64 = s.lines().find(|l| l.starts_with("RssAnon:"))?.split_whitespace().nth(1)?.parse().ok()?;
    Some(kb / 1024.0)
}

fn sobre(ui: &mut egui::Ui, st: &mut ConfigState, x: f32, top: f32, w: f32, t: f64) {
    let p = ui.painter().clone();
    let cx = ajustes::card(&p, x, top, w, 330.0, "Sobre", "O Brittico como programa: o site, tela por tela, leve para rodar junto com o jogo.");
    let mut y = top + 119.3;
    let linhas = [
        ("Versão", format!("{} (protótipo de interface, com dados de mentira)", env!("CARGO_PKG_VERSION"))),
        ("Memória própria", memoria().map(|m| format!("{m:.0} MB")).unwrap_or_else(|| "—".into())),
        ("Servidor", "desligado: nada sai deste computador".to_owned()),
    ];
    for (k, v) in linhas {
        text(&p, Pos2::new(cx, y + 10.0), Align2::LEFT_CENTER, k, f7(13.0), INK(), 0.0);
        text(&p, Pos2::new(cx + 140.0, y + 10.0), Align2::LEFT_CENTER, &v, f6(13.5), SOFT(), 0.0);
        y += 34.0;
    }
    ui.ctx().request_repaint_after(std::time::Duration::from_secs(2));
    let b1 = Rect::from_min_size(Pos2::new(cx, y + 14.0), Vec2::new(ajustes::btn_w(&p, "Abrir o site"), 36.0));
    if ajustes::button(ui, b1, "site", "Abrir o site", Btn::Cheio).clicked() {
        ui.ctx().open_url(egui::OpenUrl::new_tab("https://brittico.xyz"));
    }
    let b2 = Rect::from_min_size(Pos2::new(b1.right() + 10.0, y + 14.0), Vec2::new(ajustes::btn_w(&p, "Procurar atualização"), 36.0));
    if ajustes::button(ui, b2, "atualizar", "Procurar atualização", Btn::Contorno).clicked() {
        st.toast.say("Você já está na versão mais nova.", t);
    }
}

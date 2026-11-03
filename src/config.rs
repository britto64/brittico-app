//! As configurações do próprio app (a engrenagem no pé da trilha). O site não tem uma página assim: o que entra aqui
//! são os ajustes de voz e vídeo da Call do site (`call/js/ui-voice.js`: aparelhos, ruído, eco, volume automático), os
//! atalhos dela, a aparência (as mesmas preferências do perfil) e o que só um programa tem (abrir com o sistema, ficar na
//! bandeja). O visual é o das abas de configuração do perfil: o menu à esquerda e um cartão por aba.

use crate::perfil::ajustes::{self, Btn};
use crate::theme::*;
use crate::ui::{self, mix, text, text_width};
use crate::ic;
use olheiro_motor::som::{self, Controles, Mistura, MisturaRef, Som, QUADRO, TAXA};
use std::sync::Arc;
use eframe::egui::{self, Align2, Color32, CursorIcon, ImageSource, Order, Pos2, Rect, Sense, Stroke, StrokeKind, UiBuilder, Vec2};

const ABAS: [(&str, &str); 5] = [("voz", "Voz"), ("atalhos", "Atalhos"), ("aparencia", "Aparência"), ("app", "O app"), ("sobre", "Sobre")];

pub struct ConfigState {
    aba: usize,
    /// Os aparelhos que o sistema tem (o primeiro de cada lista é "Padrão do sistema", nome vazio) e o escolhido.
    mics: Vec<String>,
    saidas: Vec<String>,
    mic: usize,
    saida: usize,
    lidos: bool,
    /// A lista aberta de um dos seletores (0 microfone, 1 saída).
    lista: Option<usize>,
    /// O microfone aberto de verdade só para mostrar o nível (e o som de teste da saída).
    teste: Option<Teste>,
    iniciar: bool,
    bandeja: bool,
    avisos: bool,
    retomar: bool,
    animacoes: bool,
    /// A tela da Call em Full HD (o padrão é HD): o mesmo arquivo `<pasta>/tela` que a Call lê ao começar.
    tela_fullhd: bool,
    /// Codificar a tela na placa de vídeo (Windows): o arquivo `<pasta>/tela-placa`.
    tela_placa: bool,
    toast: ui::Toast,
}

impl Default for ConfigState {
    fn default() -> Self {
        Self {
            aba: 0,
            mics: vec![],
            saidas: vec![],
            mic: 0,
            saida: 0,
            lidos: false,
            lista: None,
            teste: None,
            iniciar: false,
            bandeja: true,
            avisos: true,
            retomar: true,
            animacoes: true,
            tela_fullhd: crate::call::tela_fullhd(),
            tela_placa: crate::call::tela_placa(),
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
            0 => 450.0,
            1 => 120.0 + 5.0 * 44.0 + 24.0,
            2 => 513.0,
            3 => 120.0 + 58.0 + 20.0,
            _ => 330.0,
        };
        let (r, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 24.0 + h + 90.0), Sense::hover());
        let top = r.top() + 24.0;
        match st.aba {
            0 => voz(ui, st, x, top, w, t),
            1 => atalhos(ui, x, top, w),
            2 => ajustes::preferencias(ui, perfil, x, top, w, t),
            3 => app(ui, st, x, top, w),
            _ => sobre(ui, x, top, w),
        }
    });
    st.toast.show(ui, t);
}

/// Um seletor (`<select>`): a caixa com o nome do escolhido e a seta; o clique abre a lista por cima.
fn seletor(ui: &egui::Ui, st: &mut ConfigState, k: usize, r: Rect, opcoes: &[String], atual: usize) -> Option<usize> {
    let p = ui.painter();
    let resp = ui.interact(r, ui.id().with(("cfg-sel", k)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
    let aberto = st.lista == Some(k);
    p.rect_filled(r, R_MD(), CREAM_0());
    p.rect_stroke(r, R_MD(), Stroke::new(1.0_f32, if aberto || resp.hovered() { SECONDARY() } else { HAIRLINE_2() }), StrokeKind::Inside);
    ui::clip(p, Pos2::new(r.left() + 14.0, r.center().y), Align2::LEFT_CENTER, &opcoes[atual], f6(14.0), INK(), r.width() - 44.0);
    ui::icon(ui, if aberto { ic!("chevron-up") } else { ic!("chevron-down") }, Pos2::new(r.right() - 18.0, r.center().y), 14.0, FAINT());
    if resp.clicked() {
        st.lista = if aberto { None } else { Some(k) };
    }
    if !aberto {
        return None;
    }
    let alto = (8.0 + opcoes.len() as f32 * 36.0 + 8.0).min(280.0);
    let lr = Rect::from_min_size(r.left_bottom() + Vec2::new(0.0, 6.0), Vec2::new(r.width(), alto));
    let esc = egui::Area::new(ui.id().with(("cfg-lista", k))).order(Order::Foreground).fixed_pos(lr.min).constrain(false).show(ui.ctx(), |ui| {
        ui.allocate_exact_size(lr.size(), Sense::click());
        let fundo = ui.painter().clone();
        fundo.rect_filled(lr.translate(Vec2::new(0.0, 6.0)).expand(3.0), R_MD() + 3.0, Color32::from_black_alpha(50));
        fundo.rect_filled(lr, R_MD(), CREAM_0());
        fundo.rect_stroke(lr, R_MD(), Stroke::new(1.0_f32, HAIRLINE_2()), StrokeKind::Inside);
        let mut esc = None;
        // a rolagem num filho com o tamanho da caixa; o pincel de dentro dela recorta, então nada vaza da lista
        let mut filho = ui.new_child(UiBuilder::new().max_rect(lr.shrink2(Vec2::new(0.0, 8.0))));
        egui::ScrollArea::vertical().id_salt(("cfg-rol", k)).max_height(alto - 16.0).show(&mut filho, |ui| {
            let p = ui.painter().clone();
            for (i, o) in opcoes.iter().enumerate() {
                let it = Rect::from_min_size(Pos2::new(lr.left() + 6.0, ui.cursor().top()), Vec2::new(lr.width() - 12.0, 34.0));
                ui.allocate_exact_size(Vec2::new(lr.width() - 12.0, 36.0), Sense::hover());
                let ir = ui.interact(it, ui.id().with(("cfg-op", k, i)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
                if i == atual {
                    p.rect_filled(it, R_SM(), AMBER_SOFT());
                } else if ir.hovered() {
                    p.rect_filled(it, R_SM(), CREAM_2());
                }
                ui::clip(&p, Pos2::new(it.left() + 10.0, it.center().y), Align2::LEFT_CENTER, o, f6(13.5), if i == atual { SECONDARY() } else { INK() }, it.width() - 40.0);
                if i == atual {
                    ui::icon(ui, ic!("check"), Pos2::new(it.right() - 16.0, it.center().y), 14.0, SECONDARY());
                }
                if ir.clicked() {
                    esc = Some(i);
                }
            }
        });
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

/// O teste da aba de voz: o som aberto com o aparelho escolhido. O microfone só anda o medidor; a saída toca um tom.
struct Teste {
    _som: Option<Som>,
    ctl: Arc<Controles>,
    mistura: MisturaRef,
    erro: Option<String>,
    tom_ate: f64,
    tom_n: usize,
}

fn arquivo() -> std::path::PathBuf {
    crate::net::dir().join("audio")
}

/// Os aparelhos que o usuário escolheu (microfone, saída); vazio = o padrão do sistema. Lido na hora de entrar numa call.
pub fn escolhidos() -> (String, String) {
    let s = std::fs::read_to_string(arquivo()).unwrap_or_default();
    let mut l = s.lines();
    (l.next().unwrap_or("").to_owned(), l.next().unwrap_or("").to_owned())
}

impl ConfigState {
    fn ler_aparelhos(&mut self) {
        let (m, s) = som::aparelhos();
        let (em, es) = escolhidos();
        self.mics = std::iter::once(String::new()).chain(m).collect();
        self.saidas = std::iter::once(String::new()).chain(s).collect();
        self.mic = self.mics.iter().position(|n| *n == em).unwrap_or(0);
        self.saida = self.saidas.iter().position(|n| *n == es).unwrap_or(0);
        self.lidos = true;
    }
    fn guardar(&self) {
        let _ = std::fs::create_dir_all(crate::net::dir());
        let _ = std::fs::write(arquivo(), format!("{}\n{}\n", self.mics.get(self.mic).cloned().unwrap_or_default(), self.saidas.get(self.saida).cloned().unwrap_or_default()));
    }
    fn abrir_teste(&mut self, saida: bool) {
        self.teste = None;
        let ctl = Controles::novos();
        let mistura = Mistura::nova();
        let (mic, sai) = (self.mics.get(self.mic).cloned().unwrap_or_default(), self.saidas.get(self.saida).cloned().unwrap_or_default());
        let (som, erro) = match Som::abrir(&mic, &sai, Some(mistura.clone()), ctl.clone()) {
            Ok(s) => (Some(s), None),
            Err(e) => (None, Some(e)),
        };
        self.teste = Some(Teste { _som: som, ctl, mistura, erro, tom_ate: 0.0, tom_n: 0 });
        let _ = saida;
    }
}

fn voz(ui: &mut egui::Ui, st: &mut ConfigState, x: f32, top: f32, w: f32, t: f64) {
    if !st.lidos {
        st.ler_aparelhos();
    }
    let p = ui.painter().clone();
    let cx = ajustes::card(&p, x, top, w, 450.0, "Voz", "Os aparelhos da Call. Valem para todas as salas, neste computador, na próxima vez que você entrar numa.");
    let cw = w - 50.0;
    let bw = 120.0;
    let mut y = top + 119.3;
    let nome = |lista: &[String], i: usize| -> String { if i == 0 || lista.get(i).is_none() { "Padrão do sistema".to_owned() } else { lista[i].clone() } };
    let mics: Vec<String> = (0..st.mics.len()).map(|i| nome(&st.mics, i)).collect();
    let saidas: Vec<String> = (0..st.saidas.len()).map(|i| nome(&st.saidas, i)).collect();
    let testando = st.teste.is_some();

    // microfone, com o medidor de verdade
    ajustes::label(&p, cx, y, "Microfone");
    let sel = Rect::from_min_size(Pos2::new(cx, y + 24.0), Vec2::new(cw - bw - 10.0, 42.0));
    let b = Rect::from_min_size(Pos2::new(sel.right() + 10.0, sel.top() + 2.0), Vec2::new(bw, 38.0));
    if ajustes::button(ui, b, "testar-mic", if testando { "Parar" } else { "Testar" }, Btn::Contorno).clicked() {
        if testando {
            st.teste = None;
        } else {
            st.abrir_teste(false);
        }
    }
    let medidor = Rect::from_min_size(Pos2::new(cx, sel.bottom() + 12.0), Vec2::new(cw, 8.0));
    p.rect_filled(medidor, 4.0, CREAM_2());
    let mut dica = "Aperte Testar para ver o nível do microfone.".to_owned();
    if let Some(te) = &mut st.teste {
        match &te.erro {
            Some(e) => dica = e.clone(),
            None => {
                let nivel = (te.ctl.nivel_mic.get() * 4.0).clamp(0.0, 1.0);
                te.ctl.nivel_mic.set(te.ctl.nivel_mic.get() * 0.9);
                p.rect_filled(Rect::from_min_size(medidor.min, Vec2::new(medidor.width() * nivel, 8.0)), 4.0, if nivel > 0.08 { DIS_OK } else { FAINT() });
                dica = "Fale alguma coisa: a barra sobe com a sua voz.".to_owned();
            }
        }
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(33));
    }
    ajustes::hint(&p, cx, medidor.bottom() + 6.0, cw, &dica, FAINT());
    if let Some(i) = seletor(ui, st, 0, sel, &mics, st.mic) {
        st.mic = i;
        st.guardar();
        if st.teste.is_some() {
            st.abrir_teste(false);
        }
    }
    y = medidor.bottom() + 6.0 + 17.0 + 18.0;

    // alto-falante, com um tom de verdade
    ajustes::label(&p, cx, y, "Alto-falante");
    let sel = Rect::from_min_size(Pos2::new(cx, y + 24.0), Vec2::new(cw - bw - 10.0, 42.0));
    let b = Rect::from_min_size(Pos2::new(sel.right() + 10.0, sel.top() + 2.0), Vec2::new(bw, 38.0));
    if ajustes::button(ui, b, "testar-som", "Testar", Btn::Contorno).clicked() {
        if st.teste.is_none() {
            st.abrir_teste(true);
        }
        if let Some(te) = &mut st.teste {
            te.tom_ate = t + 1.0;
            te.tom_n = 0;
        }
    }
    if let Some(te) = &mut st.teste {
        // um lá (440 Hz) baixinho, empurrado para a mistura enquanto o teste dura
        if t < te.tom_ate {
            if let Ok(mut m) = te.mistura.lock() {
                for _ in 0..4 {
                    let q: Vec<i16> = (0..QUADRO).map(|_| { te.tom_n += 1; ((te.tom_n as f32 * std::f32::consts::TAU * 440.0 / TAXA as f32).sin() * 5000.0) as i16 }).collect();
                    m.chegou("teste", &q);
                }
            }
            ui.ctx().request_repaint();
        }
    }
    if let Some(i) = seletor(ui, st, 1, sel, &saidas, st.saida) {
        st.saida = i;
        st.guardar();
        if st.teste.is_some() {
            st.abrir_teste(true);
        }
    }
    y += 24.0 + 42.0 + 20.0;

    p.rect_filled(Rect::from_min_size(Pos2::new(cx, y - 6.0), Vec2::new(cw, 1.0)), 0.0, HAIRLINE_2());
    ajustes::hint(&p, cx, y + 6.0, cw, "A voz sai já limpa: o app tira ruído, eco e acerta o volume sozinho, sem nada para ajustar.", FAINT());
    if ajustes::button(ui, Rect::from_min_size(Pos2::new(cx, y + 38.0), Vec2::new(ajustes::btn_w(&p, "Reler aparelhos"), 36.0)), "reler", "Reler aparelhos", Btn::Contorno).clicked() {
        st.teste = None;
        st.ler_aparelhos();
    }
}

fn atalhos(ui: &mut egui::Ui, x: f32, top: f32, w: f32) {
    let p = ui.painter().clone();
    let cx = ajustes::card(&p, x, top, w, 120.0 + 5.0 * 44.0 + 24.0, "Atalhos", "Valem dentro da call, quando você não está escrevendo uma mensagem.");
    let cw = w - 50.0;
    let lista = [
        ("M", "Ligar e desligar o microfone"),
        ("D", "Parar de ouvir todo mundo (e voltar)"),
                ("S", "Compartilhar a tela"),
        ("I", "Mostrar as estatísticas dos vídeos"),
        ("Esc", "Sair da tela cheia e voltar para a grade"),
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
        if y < top + 112.0 + 4.0 * 44.0 {
            p.rect_filled(Rect::from_min_size(Pos2::new(cx, y), Vec2::new(cw, 1.0)), 0.0, HAIRLINE_2());
        }
    }
}

fn app(ui: &mut egui::Ui, st: &mut ConfigState, x: f32, top: f32, w: f32) {
    let p = ui.painter().clone();
    let chaves = if cfg!(windows) { 3.0 } else { 2.0 };
    let cx = ajustes::card(&p, x, top, w, 120.0 + chaves * 58.0 + 20.0, "O app", "Como o Brittico se comporta neste computador.");
    let cw = w - 50.0;
    let y = top + 119.3;
    let y = y + chave(ui, cx, y, cw, "animacoes", "Animações", "As capas dos Jogos flutuando e os enfeites que se mexem. Desligar economiza processador em PC fraco.", &mut st.animacoes) + 12.0;
    p.rect_filled(Rect::from_min_size(Pos2::new(cx, y - 6.0), Vec2::new(cw, 1.0)), 0.0, HAIRLINE_2());
    let y = y + 6.0;
    let antes = st.tela_fullhd;
    let y = y + chave(ui, cx, y, cw, "tela-fullhd", "Tela em Full HD", "Mostrar a tela na Call em 1080p em vez de 720p. Pesa mais na conexão de quem assiste.", &mut st.tela_fullhd) + 12.0;
    if st.tela_fullhd != antes {
        let _ = std::fs::write(crate::net::dir().join("tela"), if st.tela_fullhd { "1080" } else { "720" });
    }
    if cfg!(windows) {
        p.rect_filled(Rect::from_min_size(Pos2::new(cx, y - 6.0), Vec2::new(cw, 1.0)), 0.0, HAIRLINE_2());
        let y = y + 6.0;
        let antes = st.tela_placa;
        chave(ui, cx, y, cw, "tela-placa", "Tela pela placa de vídeo", "Codificar a tela na placa de vídeo em vez do processador. Vale a partir da próxima vez que mostrar a tela.", &mut st.tela_placa);
        if st.tela_placa != antes {
            let _ = std::fs::write(crate::net::dir().join("tela-placa"), if st.tela_placa { "1" } else { "0" });
        }
    }
}

/// A memória própria do programa (o `RssAnon` do Linux: sem as bibliotecas e o driver de vídeo, que são divididos com
/// o resto do sistema), a mesma medida do README.
fn memoria() -> Option<f64> {
    let s = std::fs::read_to_string("/proc/self/status").ok()?;
    let kb: f64 = s.lines().find(|l| l.starts_with("RssAnon:"))?.split_whitespace().nth(1)?.parse().ok()?;
    Some(kb / 1024.0)
}

fn sobre(ui: &mut egui::Ui, x: f32, top: f32, w: f32) {
    let p = ui.painter().clone();
    let cx = ajustes::card(&p, x, top, w, 330.0, "Sobre", "O Brittico como programa, leve para rodar junto com o jogo.");
    let mut y = top + 119.3;
    let linhas = [
        ("Versão", env!("CARGO_PKG_VERSION").to_owned()),
        ("Memória própria", memoria().map(|m| format!("{m:.0} MB")).unwrap_or_else(|| "—".into())),
        ("Servidor", crate::net::api().to_owned()),
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
}

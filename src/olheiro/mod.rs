//! O LoL de proximidade (o Olheiro) numa janela própria, montada como a lista de amigos da Steam: você no topo, a barra de
//! voz, o seu time e, embaixo, o minimapa. Clicar em alguém abre o volume dele e o mutar (como no Discord).
//!
//! Abrir a janela já liga o Olheiro. Texto só para o que não está funcionando, dizendo o que fazer. Não tem ligação com a
//! Call nem com a conta: quem trabalha é o `olheiro-motor`.

use crate::perfil::ajustes::{self as aj, Btn};
use crate::theme::*;
use crate::ui::{self, mix, text};
use crate::{ic, png};
use eframe::egui::{self, Align2, Color32, CursorIcon, Order, Pos2, Rect, Sense, Shape, Stroke, StrokeKind, UiBuilder, Vec2, ViewportBuilder, ViewportCommand, ViewportId};
use olheiro_motor::{Ajustes, Comando, Conexao, Estado, Fase, Jogador, Lado, Motor, Ouvido, Partida, CHEIO_ATE, ZERO_EM};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub fn vp() -> ViewportId {
    ViewportId::from_hash_of("olheiro")
}

/// O tamanho da Summoner's Rift em unidades do jogo (x, y), o mesmo do núcleo do Olheiro.
const RIFT: (f64, f64) = (14870.0, 14980.0);
const PERFIL_H: f32 = 72.0;
const VOZ_H: f32 = 48.0;
const TITULO_H: f32 = 30.0;
const LINHA_H: f32 = 48.0;
const M: f32 = 12.0;

fn rift() -> egui::ImageSource<'static> {
    egui::ImageSource::Bytes { uri: "bytes://olheiro/rift.webp".into(), bytes: egui::load::Bytes::Static(include_bytes!("../../assets/olheiro/rift.webp")) }
}

pub struct Janela {
    pub motor: Motor,
    pub aberta: bool,
    aberta_flag: Arc<AtomicBool>,
    /// A engrenagem aberta (os ajustes tomam o lugar da lista).
    ajustes: bool,
    /// O que a pessoa está editando; vai para o motor a cada mudança (o campo de texto, ao sair dele).
    rascunho: Ajustes,
    visto: Option<Ajustes>,
    texturas: HashMap<String, egui::TextureHandle>,
    procurado: HashMap<String, f64>,
    /// O seletor aberto (0 microfone, 1 saída).
    lista: Option<usize>,
    /// A pessoa com o volume aberto.
    expandido: Option<String>,
    /// A janela grande do Brittico foi escondida pelo X (fica só esta).
    pub principal_oculta: bool,
}

pub type JanelaRef = Arc<Mutex<Janela>>;

impl Janela {
    pub fn nova(ctx: &egui::Context) -> JanelaRef {
        let aberta = Arc::new(AtomicBool::new(false));
        let (c, a) = (ctx.clone(), aberta.clone());
        let motor = Motor::iniciar(move || {
            if a.load(Ordering::Relaxed) {
                c.request_repaint_of(vp());
            }
        });
        let rascunho = motor.estado().ajustes;
        let mut j = Janela { motor, aberta: false, aberta_flag: aberta, ajustes: false, rascunho, visto: None, texturas: HashMap::new(), procurado: HashMap::new(), lista: None, expandido: None, principal_oculta: false };
        // BRITTICO_OLHEIRO=abrir | ajustes | simulada[N]: abre a janela já num estado, para testar e capturar
        // (N = qual aliado você é na partida simulada, 1 a 5; duas cópias com N diferentes caem na mesma sala de voz)
        if let Ok(v) = std::env::var("BRITTICO_OLHEIRO") {
            j.abrir();
            if let Some(n) = v.strip_prefix("simulada") {
                let vaga = n.parse::<usize>().unwrap_or(1).clamp(1, 5) - 1;
                j.motor.mandar(Comando::Simular(Some(vaga)));
            } else if v == "ajustes" {
                j.ajustes = true;
                j.motor.mandar(Comando::Aparelhos);
            }
        }
        // BRITTICO_OLHEIRO_ABERTO=chave: já com o volume dessa pessoa aberto (para capturar)
        j.expandido = std::env::var("BRITTICO_OLHEIRO_ABERTO").ok();
        Arc::new(Mutex::new(j))
    }

    /// Abrir já liga (e deixa ligado para as próximas vezes que o Brittico abrir).
    pub fn abrir(&mut self) {
        self.aberta = true;
        self.aberta_flag.store(true, Ordering::Relaxed);
        if !self.rascunho.apresentado {
            self.rascunho.apresentado = true;
            self.mandar_ajustes();
        }
        if !self.motor.estado().ligado {
            self.motor.mandar(Comando::Ligar);
        }
    }

    fn fechar(&mut self) {
        self.aberta = false;
        self.aberta_flag.store(false, Ordering::Relaxed);
        self.lista = None;
    }

    fn mandar_ajustes(&mut self) {
        self.motor.mandar(Comando::Ajustes(self.rascunho.clone()));
    }

    /// O ícone de um campeão, se o motor já o tem (procura de novo no máximo uma vez por segundo).
    fn icone(&mut self, ctx: &egui::Context, alias: &str, t: f64) -> Option<egui::TextureHandle> {
        if alias.is_empty() {
            return None;
        }
        if let Some(tx) = self.texturas.get(alias) {
            return Some(tx.clone());
        }
        if self.procurado.get(alias).is_some_and(|&quando| t - quando < 1.0) {
            return None;
        }
        self.procurado.insert(alias.to_owned(), t);
        let arte = self.motor.icone(alias)?;
        let img = egui::ColorImage::from_rgba_unmultiplied([arte.w, arte.h], &arte.rgba);
        let tx = ctx.load_texture(format!("olheiro-{alias}"), img, egui::TextureOptions::LINEAR);
        self.texturas.insert(alias.to_owned(), tx.clone());
        Some(tx)
    }
}

/// A frase curta do estado (a dica do "L" na trilha) e a cor da bolinha.
pub fn resumo(e: &Estado) -> (String, Color32) {
    if !e.ligado {
        return ("desligado".into(), FAINT());
    }
    match e.fase {
        Fase::LolFechado => ("LoL fechado".into(), FAINT()),
        Fase::Cliente => ("no cliente".into(), SECONDARY()),
        Fase::Carregando => ("carregando a partida".into(), SECONDARY()),
        Fase::EmPartida if e.na_sala() => (format!("na partida · {} na sala", e.sala.pessoas.len() + 1), DIS_OK),
        Fase::EmPartida => ("na partida".into(), SECONDARY()),
    }
}

pub fn builder() -> ViewportBuilder {
    let mut b = ViewportBuilder::default()
        .with_title("LoL de proximidade")
        .with_inner_size([340.0, 760.0])
        .with_min_inner_size([300.0, 520.0])
        // para as capturas: o Hyprland manda esta classe para a tela virtual
        .with_app_id(std::env::var("BRITTICO_OLHEIRO_ID").unwrap_or_else(|_| "brittico-olheiro".into()));
    if let Ok(img) = image::load_from_memory(include_bytes!("../../assets/brittico-icon.png")) {
        let img = img.to_rgba8();
        b = b.with_icon(Arc::new(egui::IconData { width: img.width(), height: img.height(), rgba: img.into_raw() }));
    }
    b
}

/// A janela inteira (chamada pelo egui na janela própria dela).
pub fn mostrar(ctx: &egui::Context, j: &JanelaRef) {
    let Ok(mut j) = j.lock() else { return };
    let j = &mut *j;
    let est = j.motor.estado();
    if j.visto.as_ref() != Some(&est.ajustes) {
        j.rascunho = est.ajustes.clone();
        j.visto = Some(est.ajustes.clone());
    }
    let t = ctx.input(|i| i.time);
    if ctx.input(|i| i.viewport().close_requested()) {
        j.fechar();
        if j.principal_oculta {
            ctx.send_viewport_cmd_to(ViewportId::ROOT, ViewportCommand::Close);
        }
        ctx.request_repaint_of(ViewportId::ROOT);
    }

    egui::CentralPanel::default().frame(egui::Frame::new().fill(CREAM_1())).show(ctx, |ui| {
        let full = ui.max_rect();
        let perfil = Rect::from_min_size(full.min, Vec2::new(full.width(), PERFIL_H));
        let voz = Rect::from_min_size(perfil.left_bottom(), Vec2::new(full.width(), VOZ_H));
        cabecalho(ui, j, &est, perfil, t);
        barra_de_voz(ui, j, &est, voz);
        let resto = Rect::from_min_max(voz.left_bottom(), full.max);
        if j.ajustes {
            ajustes(ui, j, &est, resto);
        } else {
            corpo(ui, j, &est, resto, t);
        }
    });
    // o "está falando" e o medidor mexem sem o motor avisar
    if est.fase == Fase::EmPartida || est.testando_microfone {
        ctx.request_repaint_after(Duration::from_millis(66));
    }
}

/// O rosto de alguém: o ícone do campeão se o Olheiro já o tem, as iniciais se não. Quadrado de cantos redondos, como a Steam.
fn rosto(ui: &egui::Ui, j: &mut Janela, alias: &str, nome: &str, r: Rect, t: f64) {
    match j.icone(ui.ctx(), alias, t) {
        Some(tx) => {
            egui::Image::new((tx.id(), r.size())).corner_radius(R_SM()).paint_at(ui, r);
        }
        None => {
            let p = ui.painter();
            p.rect_filled(r, R_SM(), mix(ui::hsl(ui::hue_of(nome), 0.55, 0.50), CREAM_2(), 0.24));
            text(p, r.center(), Align2::CENTER_CENTER, &ui::initials(nome), f8(r.height() * 0.36), INK(), 0.0);
        }
    }
}

/// "Nome#TAG" → ("Nome", "#TAG").
fn partes(riot: &str) -> (&str, &str) {
    match riot.find('#') {
        Some(i) => (&riot[..i], &riot[i..]),
        None => (riot, ""),
    }
}

/// Um botão de ícone sem fundo (só no mouse em cima).
fn icone_botao(ui: &egui::Ui, c: Pos2, id: &str, icone: egui::ImageSource<'static>, cor: Color32, dica: &str) -> bool {
    let r = Rect::from_center_size(c, Vec2::splat(32.0));
    let resp = ui.interact(r, ui.id().with(("ol-ib", id)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand).on_hover_text(dica);
    if resp.hovered() {
        ui.painter().rect_filled(r, R_SM(), CREAM_2());
    }
    ui::icon(ui, icone, c, 18.0, cor);
    resp.clicked()
}

/// Você: o rosto, o Riot ID e o que está acontecendo; a engrenagem à direita.
fn cabecalho(ui: &mut egui::Ui, j: &mut Janela, est: &Estado, r: Rect, t: f64) {
    let p = ui.painter().clone();
    p.rect_filled(r, 0.0, CREAM_0());
    let eu = est.partida.as_ref().and_then(|p| p.jogador(&p.voce)).cloned();
    // o Riot ID como o jogo escreve (o da partida); fora dela, o do cliente
    let riot = eu.as_ref().map(|e| e.nome.clone()).or_else(|| est.riot_id.clone());
    let foto = Rect::from_min_size(r.min + Vec2::new(M, 12.0), Vec2::splat(48.0));
    let nome_riot = riot.unwrap_or_else(|| "League of Legends".into());
    rosto(ui, j, eu.as_ref().map(|e| e.alias.as_str()).unwrap_or(""), &nome_riot, foto, t);
    let (nome, tag) = partes(&nome_riot);
    let x = foto.right() + 10.0;
    let nr = ui::clip(&p, Pos2::new(x, foto.top() + 13.0), Align2::LEFT_CENTER, nome, f7(15.5), SECONDARY(), r.right() - x - 80.0);
    if !tag.is_empty() {
        text(&p, Pos2::new(nr.right() + 2.0, foto.top() + 13.0), Align2::LEFT_CENTER, tag, f6(12.5), FAINT(), 0.0);
    }
    let (status, cor) = match (est.fase, &eu) {
        (Fase::EmPartida, Some(e)) => (e.campeao.clone(), DIS_OK),
        (Fase::EmPartida, None) => ("Em partida".into(), DIS_OK),
        (Fase::Carregando, _) => ("Carregando a partida".into(), SECONDARY()),
        (Fase::Cliente, _) => ("No cliente".into(), SOFT()),
        (Fase::LolFechado, _) => ("Fechado".into(), FAINT()),
    };
    text(&p, Pos2::new(x, foto.top() + 34.0), Align2::LEFT_CENTER, &status, f6(12.5), cor, 0.0);

    // à direita: a engrenagem e, com a janela grande escondida, a volta para o Brittico
    let cx = r.right() - M - 16.0;
    if icone_botao(ui, Pos2::new(cx, r.center().y), "ajustes", if j.ajustes { ic!("x") } else { ic!("gear") }, if j.ajustes { INK() } else { SOFT() }, if j.ajustes { "Fechar os ajustes" } else { "Ajustes" }) {
        j.ajustes = !j.ajustes;
        j.lista = None;
        if j.ajustes {
            j.motor.mandar(Comando::Aparelhos);
        }
    }
    if j.principal_oculta {
        let br = Rect::from_center_size(Pos2::new(cx - 36.0, r.center().y), Vec2::splat(32.0));
        let resp = ui.interact(br, ui.id().with("ol-brittico"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand).on_hover_text("Mostrar o Brittico");
        if resp.hovered() {
            p.rect_filled(br, R_SM(), CREAM_2());
        }
        ui::image(ui, png!("brittico-icon"), Rect::from_center_size(br.center(), Vec2::splat(18.0)), 4.0);
        if resp.clicked() {
            j.principal_oculta = false;
            let ctx = ui.ctx();
            ctx.send_viewport_cmd_to(ViewportId::ROOT, ViewportCommand::Visible(true));
            ctx.send_viewport_cmd_to(ViewportId::ROOT, ViewportCommand::Minimized(false));
            ctx.send_viewport_cmd_to(ViewportId::ROOT, ViewportCommand::Focus);
        }
    }
}

/// A barra de voz, como a "voice chat" da Steam: o estado da sala e o microfone e o fone.
fn barra_de_voz(ui: &mut egui::Ui, j: &mut Janela, est: &Estado, r: Rect) {
    let p = ui.painter().clone();
    let dentro = est.na_sala();
    p.rect_filled(r, 0.0, if dentro { mix(DIS_OK, CREAM_0(), 0.2) } else { CREAM_0() });
    p.rect_filled(Rect::from_min_size(r.min, Vec2::new(r.width(), 1.0)), 0.0, if dentro { DIS_OK.gamma_multiply(0.6) } else { HAIRLINE_2() });
    let (linha, cor) = match &est.sala.conexao {
        Conexao::Dentro => ("Sala da partida".to_owned(), INK()),
        Conexao::Entrando => ("Entrando…".to_owned(), SOFT()),
        Conexao::Falhou(_) => ("Sem conexão · tentando de novo".to_owned(), DIS_OFF),
        Conexao::Fora => ("Fora da partida".to_owned(), FAINT()),
    };
    text(&p, Pos2::new(r.left() + M, r.top() + 15.0), Align2::LEFT_CENTER, "VOZ", f8(10.5), if dentro { DIS_OK } else { FAINT() }, 0.6);
    ui::clip(&p, Pos2::new(r.left() + M, r.top() + 32.0), Align2::LEFT_CENTER, &linha, f6(13.0), cor, r.width() - 110.0);

    let cy = r.center().y;
    let fone = Pos2::new(r.right() - M - 16.0, cy);
    if icone_botao(ui, fone, "fone", if est.surdo { ic!("headset-off") } else { ic!("headset") }, if est.surdo { DIS_OFF } else { INK() }, if est.surdo { "Voltar a ouvir" } else { "Parar de ouvir" }) {
        j.motor.mandar(Comando::Surdo(!est.surdo));
    }
    let mic = fone - Vec2::new(38.0, 0.0);
    // o microfone acende com a sua voz
    let nivel = if est.microfone_aberto && dentro { (j.motor.nivel_microfone() * 4.0).min(1.0) } else { 0.0 };
    if nivel > 0.1 {
        p.rect_filled(Rect::from_center_size(mic, Vec2::splat(32.0)), R_SM(), DIS_OK.gamma_multiply(0.25 * nivel));
    }
    let cor_mic = if !est.microfone_aberto {
        DIS_OFF
    } else if nivel > 0.1 {
        DIS_OK
    } else {
        INK()
    };
    if icone_botao(ui, mic, "mic", if est.microfone_aberto { ic!("mic") } else { ic!("mic-off") }, cor_mic, if est.microfone_aberto { "Fechar o microfone" } else { "Abrir o microfone" }) {
        j.motor.mandar(Comando::Microfone(!est.microfone_aberto));
    }
}

/// O que não está funcionando, numa frase que diz o que fazer. `None`: está tudo detectado.
fn problema(est: &Estado) -> Option<(String, Color32)> {
    if let Some(e) = &est.som_erro {
        return Some((e.clone(), DIS_OFF));
    }
    if est.simulando.is_none() && est.fase == Fase::LolFechado {
        return Some(("Abra o League of Legends.".into(), SECONDARY()));
    }
    if est.fase == Fase::EmPartida && !est.leitura.problema.is_empty() {
        return Some((est.leitura.problema.clone(), if est.leitura.ok { SECONDARY() } else { DIS_OFF }));
    }
    if let Conexao::Falhou(e) = &est.sala.conexao {
        return Some((e.clone(), DIS_OFF));
    }
    None
}

/// O time (a lista, que rola) e o minimapa embaixo, no lugar dos "group chats" da Steam.
fn corpo(ui: &mut egui::Ui, j: &mut Janela, est: &Estado, r: Rect, t: f64) {
    let p = ui.painter().clone();
    let lado = (r.width() - 2.0 * M).min(r.height() * 0.55).max(120.0);
    let mapa = Rect::from_min_size(Pos2::new(r.center().x - lado / 2.0, r.bottom() - M - lado), Vec2::splat(lado));
    let mut fim_lista = mapa.top() - M;

    // o aviso do que falta, logo acima do mapa
    if let Some((frase, cor)) = problema(est) {
        let fundo = p.add(Shape::Noop);
        let w = r.width() - 2.0 * M;
        let mut job = egui::text::LayoutJob::single_section(frase, egui::TextFormat { font_id: f6(12.5), color: INK(), line_height: Some(17.0), ..Default::default() });
        job.wrap.max_width = w - 44.0;
        let g = p.layout_job(job);
        let h = g.size().y + 18.0;
        let ar = Rect::from_min_size(Pos2::new(r.left() + M, fim_lista - h), Vec2::new(w, h));
        p.set(fundo, Shape::Rect(egui::epaint::RectShape::new(ar, R_MD(), mix(cor, CREAM_0(), 0.14), Stroke::new(1.0_f32, cor.gamma_multiply(0.5)), StrokeKind::Inside)));
        ui::icon(ui, ic!("alert"), Pos2::new(ar.left() + 18.0, ar.top() + 17.5), 15.0, cor);
        p.galley(Pos2::new(ar.left() + 34.0, ar.top() + 9.0), g, INK());
        fim_lista = ar.top() - 8.0;
    }

    // o time
    let titulo = Rect::from_min_size(r.min, Vec2::new(r.width(), TITULO_H));
    p.rect_filled(titulo, 0.0, CREAM_2());
    let gente: Vec<Jogador> = match &est.partida {
        Some(pt) => {
            let mut v: Vec<Jogador> = pt.jogadores.iter().filter(|jg| jg.chave != pt.voce && (pt.personalizada || jg.time == pt.meu_time)).cloned().collect();
            // quem está na sala primeiro; o outro time (personalizada) por último
            v.sort_by_key(|jg| (jg.time != pt.meu_time, !est.sala.pessoas.contains_key(&jg.chave)));
            v
        }
        None => Vec::new(),
    };
    let na_sala = gente.iter().filter(|g| est.sala.pessoas.contains_key(&g.chave)).count();
    text(&p, Pos2::new(r.left() + M, titulo.center().y), Align2::LEFT_CENTER, "TIME", f8(10.5), SOFT(), 0.6);
    if !gente.is_empty() {
        text(&p, Pos2::new(r.right() - M, titulo.center().y), Align2::RIGHT_CENTER, &format!("{na_sala}/{} na sala", gente.len()), f6(11.5), FAINT(), 0.0);
    }
    let lista = Rect::from_min_max(titulo.left_bottom(), Pos2::new(r.right(), fim_lista.max(titulo.bottom())));
    let falando: HashMap<String, f32> = j.motor.falando().into_iter().map(|(k, n, _)| (k, n)).collect();
    let mut filho = ui.new_child(UiBuilder::new().max_rect(lista));
    filho.set_clip_rect(lista);
    egui::ScrollArea::vertical().id_salt("ol-time").auto_shrink(false).show(&mut filho, |ui| {
        let topo = ui.cursor().top();
        let mut y = topo + 4.0;
        if let Some(pt) = est.partida.clone() {
            for jg in &gente {
                let o = est.ouvidos.iter().find(|o| o.chave == jg.chave).cloned();
                y += linha(ui, j, est, jg, o.as_ref(), falando.get(&jg.chave).copied().unwrap_or(0.0), &pt, lista.left(), y, lista.width(), t);
            }
        }
        ui.allocate_exact_size(Vec2::new(ui.available_width(), (y - topo).max(0.0)), Sense::hover());
    });

    minimapa(ui, j, est, &falando, mapa, t);
}

/// Uma pessoa do time. Clicar abre o volume dela e o mutar.
#[allow(clippy::too_many_arguments)]
fn linha(ui: &mut egui::Ui, j: &mut Janela, est: &Estado, jg: &Jogador, o: Option<&Ouvido>, falando: f32, pt: &Partida, x: f32, y: f32, w: f32, t: f64) -> f32 {
    let p = ui.painter().clone();
    let aberta = o.is_some() && j.expandido.as_deref() == Some(jg.chave.as_str());
    let r = Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, LINHA_H));
    let resp = ui.interact(r, ui.id().with(("ol-linha", &jg.chave)), if o.is_some() { Sense::click() } else { Sense::hover() });
    if o.is_some() && resp.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    if aberta || (o.is_some() && resp.hovered()) {
        p.rect_filled(Rect::from_min_size(r.min, Vec2::new(w, if aberta { LINHA_H + 42.0 } else { LINHA_H })), 0.0, CREAM_2());
    }
    if resp.clicked() {
        j.expandido = if aberta { None } else { Some(jg.chave.clone()) };
    }
    let audivel = o.is_some_and(|o| o.na_partida && !o.mutado && o.proximidade * o.escolhido > 0.01) && !est.surdo;
    // a barrinha da Steam à esquerda: verde enquanto fala e você ouve
    if falando > 0.04 && o.is_some() {
        p.rect_filled(Rect::from_min_size(Pos2::new(x, y + 8.0), Vec2::new(3.0, LINHA_H - 16.0)), 1.5, if audivel { DIS_OK } else { FAINT() });
    }
    let foto = Rect::from_min_size(Pos2::new(x + M, y + 7.0), Vec2::splat(34.0));
    rosto(ui, j, &jg.alias, &jg.campeao, foto, t);
    if o.is_none() {
        p.rect_filled(foto, R_SM(), CREAM_1().gamma_multiply(0.55));
    }
    let tx = foto.right() + 10.0;
    let (nome, _) = partes(&jg.nome);
    let inimigo = jg.time != pt.meu_time;
    let cor_nome = if o.is_none() {
        FAINT()
    } else if inimigo {
        DIS_OFF
    } else {
        INK()
    };
    ui::clip(&p, Pos2::new(tx, y + 17.0), Align2::LEFT_CENTER, nome, f7(13.5), cor_nome, w - (tx - x) - M);
    let (estado, cor) = match o {
        None => ("sem Olheiro".to_owned(), FAINT()),
        Some(o) if o.mutado => ("mutado".to_owned(), DIS_OFF),
        Some(o) if o.mudo => ("microfone fechado".to_owned(), FAINT()),
        Some(o) if !o.na_partida => ("fora da partida".to_owned(), FAINT()),
        Some(_) if est.surdo => (String::new(), FAINT()),
        Some(o) => {
            let v = o.proximidade * o.escolhido;
            if v < 0.01 {
                ("longe".to_owned(), FAINT())
            } else if o.proximidade >= 0.999 && (o.escolhido - 1.0).abs() < 0.01 {
                ("perto".to_owned(), DIS_OK)
            } else {
                (format!("{:.0}%", v * 100.0), if o.proximidade >= 0.999 { DIS_OK } else { SECONDARY() })
            }
        }
    };
    let sub = if estado.is_empty() { jg.campeao.clone() } else { format!("{} · {estado}", jg.campeao) };
    ui::clip(&p, Pos2::new(tx, y + 33.0), Align2::LEFT_CENTER, &sub, f6(12.0), if estado.is_empty() { SOFT() } else { cor }, w - (tx - x) - M);

    let Some(o) = o.filter(|_| aberta) else { return LINHA_H };
    // o volume dele e o mutar, como no Discord
    let yy = y + LINHA_H;
    let mutar = Rect::from_center_size(Pos2::new(x + M + 17.0, yy + 18.0), Vec2::splat(30.0));
    let mr = ui.interact(mutar, ui.id().with(("ol-mutar", &jg.chave)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand).on_hover_text(if o.mutado { "Desmutar" } else { "Mutar" });
    p.rect_filled(mutar, R_SM(), if o.mutado { DIS_OFF } else if mr.hovered() { CREAM_4() } else { CREAM_1() });
    ui::icon(ui, if o.mutado { ic!("volume-x") } else { ic!("speaker") }, mutar.center(), 16.0, if o.mutado { Color32::WHITE } else { INK() });
    if mr.clicked() {
        if o.mutado {
            j.rascunho.mutados.remove(&jg.chave);
        } else {
            j.rascunho.mutados.insert(jg.chave.clone());
        }
        j.mandar_ajustes();
    }
    let dr = Rect::from_min_size(Pos2::new(tx, yy + 8.0), Vec2::new(w - (tx - x) - M - 44.0, 20.0));
    let mut v = o.escolhido;
    if deslizante(ui, dr, &format!("vol-{}", jg.chave), &mut v, 2.0) {
        if (v - 1.0).abs() < 0.001 {
            j.rascunho.volumes.remove(&jg.chave);
        } else {
            j.rascunho.volumes.insert(jg.chave.clone(), v);
        }
        j.mandar_ajustes();
    }
    text(&p, Pos2::new(x + w - M, yy + 18.0), Align2::RIGHT_CENTER, &format!("{:.0}%", o.escolhido * 100.0), f7(12.0), SOFT(), 0.0);
    LINHA_H + 42.0
}

/// O ponto do mapa (tela) de uma posição do jogo. `y` do jogo cresce para o norte; o da tela, para baixo.
fn no_mapa(r: Rect, x: f64, y: f64) -> Pos2 {
    Pos2::new(r.left() + (x / RIFT.0) as f32 * r.width(), r.bottom() - (y / RIFT.1) as f32 * r.height())
}

fn tracejado(p: &egui::Painter, c: Pos2, raio: f32, s: Stroke) {
    let n = 64;
    let pts: Vec<Pos2> = (0..=n).map(|i| c + Vec2::angled(i as f32 / n as f32 * std::f32::consts::TAU) * raio).collect();
    p.extend(Shape::dashed_line(&pts, s, 5.0, 5.0));
}

fn minimapa(ui: &mut egui::Ui, j: &mut Janela, est: &Estado, falando: &HashMap<String, f32>, mapa: Rect, t: f64) {
    let p = ui.painter().clone();
    ui::image(ui, rift(), mapa, R_MD());
    let em_partida = est.fase == Fase::EmPartida && est.partida.is_some();
    p.rect_filled(mapa, R_MD(), Color32::from_black_alpha(if em_partida { 50 } else { 150 }));
    let Some(pt) = est.partida.as_ref().filter(|_| em_partida) else { return };
    let s = mapa.width() / RIFT.0 as f32;
    let clip = p.with_clip_rect(mapa);
    if let Some(&(vx, vy)) = est.posicoes.get(&pt.voce) {
        // até onde você ouve cheio, e onde o som some
        let c = no_mapa(mapa, vx, vy);
        clip.circle_filled(c, CHEIO_ATE as f32 * s, SECONDARY().gamma_multiply(0.16));
        clip.circle_stroke(c, CHEIO_ATE as f32 * s, Stroke::new(1.5_f32, SECONDARY()));
        tracejado(&clip, c, ZERO_EM as f32 * s, Stroke::new(1.2_f32, Color32::from_white_alpha(140)));
    }
    let mut ordem: Vec<&Jogador> = pt.jogadores.iter().filter(|jg| est.posicoes.contains_key(&jg.chave)).collect();
    ordem.sort_by_key(|jg| jg.chave == pt.voce);
    let tam = (mapa.width() / 13.0).clamp(18.0, 28.0);
    for jg in ordem {
        let (gx, gy) = est.posicoes[&jg.chave];
        let c = no_mapa(mapa, gx, gy);
        let o = est.ouvidos.iter().find(|o| o.chave == jg.chave);
        let anel = if jg.chave == pt.voce {
            SECONDARY()
        } else if jg.time != pt.meu_time {
            DIS_OFF
        } else {
            match o {
                Some(o) => mix(DIS_OK, Color32::from_gray(110), 1.0 - o.proximidade),
                None => Color32::from_gray(110),
            }
        };
        if falando.get(&jg.chave).is_some_and(|&n| n > 0.04) {
            let ouve = o.is_some_and(|o| !o.mutado && o.proximidade > 0.01);
            clip.circle_stroke(c, tam / 2.0 + 5.0, Stroke::new(2.0_f32, if ouve { DIS_OK } else { FAINT() }));
        }
        clip.circle_filled(c, tam / 2.0 + 2.0, anel);
        match j.icone(ui.ctx(), &jg.alias, t) {
            Some(tx) => {
                egui::Image::new((tx.id(), Vec2::splat(tam))).corner_radius(tam / 2.0).paint_at(ui, Rect::from_center_size(c, Vec2::splat(tam)));
            }
            None => ui::avatar(ui, &jg.campeao, None, c, tam, false, None),
        }
        // clicar no mapa abre o volume da pessoa na lista
        if jg.chave != pt.voce && o.is_some() {
            let resp = ui.interact(Rect::from_center_size(c, Vec2::splat(tam + 4.0)), ui.id().with(("ol-mapa", &jg.chave)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand).on_hover_text(partes(&jg.nome).0);
            if resp.clicked() {
                j.expandido = Some(jg.chave.clone());
            }
        }
    }
}

/* ---------------- os ajustes (a engrenagem) ---------------- */

fn rotulo(p: &egui::Painter, x: f32, y: f32, s: &str) {
    text(p, Pos2::new(x, y + 8.0), Align2::LEFT_CENTER, s, f7(12.5), SOFT(), 0.0);
}

fn ajustes(ui: &mut egui::Ui, j: &mut Janela, est: &Estado, r: Rect) {
    let mut filho = ui.new_child(UiBuilder::new().max_rect(r));
    filho.set_clip_rect(r);
    egui::ScrollArea::vertical().id_salt("ol-ajustes").auto_shrink(false).show(&mut filho, |ui| {
        let p = ui.painter().clone();
        let x = r.left() + M;
        let w = r.width() - 2.0 * M;
        let topo = ui.cursor().top();
        let mut y = topo + 14.0;

        rotulo(&p, x, y, "Microfone");
        let sel_mic = Rect::from_min_size(Pos2::new(x, y + 22.0), Vec2::new(w, 36.0));
        y += 22.0 + 36.0 + 10.0;
        // o teste: a barra anda com a sua voz
        let medidor = Rect::from_min_size(Pos2::new(x, y + 11.0), Vec2::new(w - 86.0, 8.0));
        let b = Rect::from_min_size(Pos2::new(x + w - 76.0, y), Vec2::new(76.0, 30.0));
        if aj::button(ui, b, "ol-testar", if est.testando_microfone { "Parar" } else { "Testar" }, Btn::Contorno).clicked() {
            j.motor.mandar(Comando::TestarMicrofone(!est.testando_microfone));
        }
        p.rect_filled(medidor, 4.0, CREAM_2());
        if est.testando_microfone || est.na_sala() {
            let n = (j.motor.nivel_microfone() * 4.0).min(1.0);
            p.rect_filled(Rect::from_min_size(medidor.min, Vec2::new(medidor.width() * n, 8.0)), 4.0, if n > 0.1 { DIS_OK } else { FAINT() });
        }
        y += 30.0 + 16.0;
        rotulo(&p, x, y, "Saída");
        let sel_sai = Rect::from_min_size(Pos2::new(x, y + 22.0), Vec2::new(w, 36.0));
        y += 22.0 + 36.0 + 18.0;
        for (id, nome, qual) in [("ganho", "Volume do microfone", 0), ("geral", "Volume geral", 1)] {
            let v = if qual == 0 { j.rascunho.ganho_microfone } else { j.rascunho.volume_geral };
            rotulo(&p, x, y, nome);
            text(&p, Pos2::new(x + w, y + 8.0), Align2::RIGHT_CENTER, &format!("{:.0}%", v * 100.0), f7(12.0), SOFT(), 0.0);
            let mut v2 = v;
            if deslizante(ui, Rect::from_min_size(Pos2::new(x, y + 20.0), Vec2::new(w, 20.0)), id, &mut v2, 2.0) {
                if qual == 0 {
                    j.rascunho.ganho_microfone = v2;
                } else {
                    j.rascunho.volume_geral = v2;
                }
                j.mandar_ajustes();
            }
            y += 52.0;
        }

        rotulo(&p, x, y, "Minimapa");
        let seg = Rect::from_min_size(Pos2::new(x, y + 22.0), Vec2::new(w, 34.0));
        p.rect_filled(seg, 17.0, CREAM_2());
        for (i, (lado, nome)) in [(Lado::Esquerda, "Esquerda"), (Lado::Direita, "Direita")].into_iter().enumerate() {
            let b = Rect::from_min_size(Pos2::new(seg.left() + i as f32 * seg.width() / 2.0, seg.top()), Vec2::new(seg.width() / 2.0, seg.height())).shrink(3.0);
            let resp = ui.interact(b, ui.id().with(("ol-lado", i)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
            let on = j.rascunho.lado_minimapa == lado;
            if on {
                p.rect_filled(b, 14.0, SECONDARY());
            } else if resp.hovered() {
                p.rect_filled(b, 14.0, CREAM_4());
            }
            text(&p, b.center(), Align2::CENTER_CENTER, nome, f7(12.5), if on { ON_SECONDARY() } else { SOFT() }, 0.0);
            if resp.clicked() && !on {
                j.rascunho.lado_minimapa = lado;
                j.mandar_ajustes();
            }
        }
        y += 22.0 + 34.0 + 18.0;

        rotulo(&p, x, y, "Código do grupo");
        if campo(ui, Rect::from_min_size(Pos2::new(x, y + 22.0), Vec2::new(w, 36.0)), "grupo", &mut j.rascunho.codigo_grupo, "nenhum") {
            j.mandar_ajustes();
        }
        y += 22.0 + 36.0 + 18.0;

        let linha_r = Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, 32.0));
        if ui.interact(linha_r, ui.id().with("ol-pacote"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand).clicked() {
            j.rascunho.pacote_erro = !j.rascunho.pacote_erro;
            j.mandar_ajustes();
        }
        text(&p, Pos2::new(x, y + 16.0), Align2::LEFT_CENTER, "Gravar pacote de erro", f7(13.0), INK(), 0.0);
        aj::switch(ui, Rect::from_min_size(Pos2::new(x + w - 44.0, y + 4.0), Vec2::new(44.0, 24.0)), "ol-pacote", j.rascunho.pacote_erro);
        y += 32.0 + 12.0;

        let b1 = Rect::from_min_size(Pos2::new(x, y), Vec2::new(aj::btn_w(&p, "Abrir a pasta"), 32.0));
        if aj::button(ui, b1, "ol-pasta", "Abrir a pasta", Btn::Contorno).clicked() {
            abrir_pasta(ui.ctx(), &olheiro_motor::pasta());
        }
        let bw = aj::btn_w(&p, "Desligar");
        let b2 = Rect::from_min_size(Pos2::new(x + w - bw, y), Vec2::new(bw, 32.0));
        if aj::button(ui, b2, "ol-desligar", "Desligar", Btn::Contorno).clicked() {
            j.motor.mandar(Comando::Desligar);
            j.ajustes = false;
            j.fechar();
            ui.ctx().request_repaint_of(ViewportId::ROOT);
        }
        y += 32.0 + 16.0;
        text(&p, Pos2::new(x, y + 6.0), Align2::LEFT_CENTER, &format!("Olheiro {}", olheiro_motor::VERSAO), f6(11.0), FAINT(), 0.0);
        y += 20.0;

        // os seletores por último: a lista aberta fica por cima do resto
        let (mics, saidas) = est.aparelhos.clone();
        let atual = j.rascunho.microfone.clone();
        if let Some(m) = seletor(ui, j, 0, sel_mic, &mics, &atual) {
            j.rascunho.microfone = m;
            j.mandar_ajustes();
        }
        let atual = j.rascunho.saida.clone();
        if let Some(s) = seletor(ui, j, 1, sel_sai, &saidas, &atual) {
            j.rascunho.saida = s;
            j.mandar_ajustes();
        }
        ui.allocate_exact_size(Vec2::new(ui.available_width(), (y - topo + 12.0).max(0.0)), Sense::hover());
    });
}

/* ---------------- peças ---------------- */

/// Um controle deslizante de 0 a `max` (mostra em %). Devolve se mudou.
pub(crate) fn deslizante(ui: &egui::Ui, r: Rect, id: &str, v: &mut f32, max: f32) -> bool {
    let resp = ui.interact(r, ui.id().with(("ol-desl", id)), Sense::click_and_drag()).on_hover_cursor(CursorIcon::PointingHand);
    let mut mudou = false;
    if let Some(q) = resp.interact_pointer_pos() {
        if resp.dragged() || resp.clicked() {
            let novo = (((q.x - r.left()) / r.width()).clamp(0.0, 1.0) * max * 20.0).round() / 20.0;
            if (novo - *v).abs() > 1e-4 {
                *v = novo;
                mudou = true;
            }
        }
    }
    let p = ui.painter();
    let bar = Rect::from_center_size(r.center(), Vec2::new(r.width(), 6.0));
    let k = (*v / max).clamp(0.0, 1.0);
    p.rect_filled(bar, 3.0, CREAM_2());
    p.rect_filled(Rect::from_min_size(bar.min, Vec2::new(bar.width() * k, 6.0)), 3.0, SECONDARY());
    // a marca do 100%
    if max > 1.0 {
        p.rect_filled(Rect::from_center_size(Pos2::new(bar.left() + bar.width() / max, bar.center().y), Vec2::new(2.0, 12.0)), 1.0, FAINT());
    }
    p.circle_filled(Pos2::new(bar.left() + bar.width() * k, bar.center().y), 8.0, if resp.hovered() || resp.dragged() { INK() } else { SOFT() });
    mudou
}
/// Uma caixa de texto de uma linha. Devolve `true` quando a pessoa termina de editar (sai da caixa ou aperta Enter).
fn campo(ui: &mut egui::Ui, r: Rect, id: &str, valor: &mut String, dica: &str) -> bool {
    let p = ui.painter().clone();
    let fid = ui.id().with(("ol-campo", id));
    let foco = ui.memory(|m| m.has_focus(fid));
    p.rect_filled(r, R_MD(), CREAM_1());
    p.rect_stroke(r, R_MD(), Stroke::new(1.0_f32, if foco { SECONDARY() } else { HAIRLINE_2() }), StrokeKind::Inside);
    let mut f = ui.new_child(UiBuilder::new().max_rect(Rect::from_min_max(r.min + Vec2::new(12.0, 10.0), r.max - Vec2::new(12.0, 8.0))));
    let resp = f.add(
        egui::TextEdit::singleline(valor)
            .id(fid)
            .frame(false)
            .hint_text(egui::RichText::new(dica).color(FAINT()))
            .font(f6(13.5))
            .text_color(INK())
            .margin(egui::Margin::ZERO)
            .desired_width(r.width() - 24.0),
    );
    resp.lost_focus()
}
/// O seletor de aparelho: a caixa com o nome e a seta; a lista abre por cima.
fn seletor(ui: &egui::Ui, j: &mut Janela, k: usize, r: Rect, opcoes: &[String], atual: &str) -> Option<String> {
    let p = ui.painter();
    let resp = ui.interact(r, ui.id().with(("ol-sel", k)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
    let aberto = j.lista == Some(k);
    p.rect_filled(r, R_MD(), CREAM_1());
    p.rect_stroke(r, R_MD(), Stroke::new(1.0_f32, if aberto || resp.hovered() { SECONDARY() } else { HAIRLINE_2() }), StrokeKind::Inside);
    let nome = if atual.is_empty() { "Padrão do sistema" } else { atual };
    ui::clip(p, Pos2::new(r.left() + 12.0, r.center().y), Align2::LEFT_CENTER, nome, f6(13.5), INK(), r.width() - 40.0);
    ui::icon(ui, if aberto { ic!("chevron-up") } else { ic!("chevron-down") }, Pos2::new(r.right() - 16.0, r.center().y), 14.0, FAINT());
    if resp.clicked() {
        j.lista = if aberto { None } else { Some(k) };
    }
    if !aberto {
        return None;
    }
    let todas: Vec<String> = std::iter::once(String::new()).chain(opcoes.iter().cloned()).collect();
    let alto = (8.0 + todas.len() as f32 * 34.0 + 8.0).min(260.0);
    let lr = Rect::from_min_size(r.left_bottom() + Vec2::new(0.0, 6.0), Vec2::new(r.width(), alto));
    let esc = egui::Area::new(ui.id().with(("ol-lista", k))).order(Order::Foreground).fixed_pos(lr.min).constrain(true).show(ui.ctx(), |ui| {
        let p = ui.painter().clone();
        p.rect_filled(lr.translate(Vec2::new(0.0, 6.0)).expand(3.0), R_MD() + 3.0, Color32::from_black_alpha(50));
        p.rect_filled(lr, R_MD(), CREAM_0());
        p.rect_stroke(lr, R_MD(), Stroke::new(1.0_f32, HAIRLINE_2()), StrokeKind::Inside);
        let mut esc = None;
        egui::ScrollArea::vertical().max_height(alto - 16.0).show(ui, |ui| {
            // o pincel de dentro da rolagem recorta no que se vê: o de fora deixava os itens vazarem da lista
            let p = ui.painter().clone();
            ui.add_space(8.0);
            for (i, o) in todas.iter().enumerate() {
                let (it, ir) = ui.allocate_exact_size(Vec2::new(lr.width() - 12.0, 32.0), Sense::click());
                let it = it.translate(Vec2::new(6.0, 0.0));
                let marcado = o == atual;
                if marcado {
                    p.rect_filled(it, R_SM(), AMBER_SOFT());
                } else if ir.hovered() {
                    p.rect_filled(it, R_SM(), CREAM_2());
                }
                let rot = if o.is_empty() { "Padrão do sistema" } else { o.as_str() };
                ui::clip(&p, Pos2::new(it.left() + 10.0, it.center().y), Align2::LEFT_CENTER, rot, f6(13.0), if marcado { SECONDARY() } else { INK() }, it.width() - 36.0);
                if ir.clicked() {
                    esc = Some(o.clone());
                }
                let _ = i;
            }
        });
        esc
    });
    if esc.inner.is_some() || (ui.input(|i| i.pointer.any_pressed()) && !ui.input(|i| i.pointer.interact_pos()).is_some_and(|q| lr.contains(q) || r.contains(q))) {
        j.lista = None;
    }
    esc.inner
}
/// Abre a pasta no gerenciador de arquivos do sistema (sem abrir terminal nem `cmd`).
fn abrir_pasta(ctx: &egui::Context, dir: &std::path::Path) {
    let _ = std::fs::create_dir_all(dir);
    let caminho = dir.to_string_lossy().replace('\\', "/");
    let url = if caminho.starts_with('/') { format!("file://{caminho}") } else { format!("file:///{caminho}") };
    ctx.open_url(egui::OpenUrl::new_tab(url));
}

//! O que abre por cima do canvas do LivePixel: o cartão de um pixel (clique direito), o menu do perfil, as barras do
//! overlay de imagem e da moderação, e as janelas (escolher o streamer, configurações, notificações, a roleta diária, o
//! envio da imagem e a denúncia). Medidas e cores do `livepixel.css`.

use super::*;
use eframe::egui::{Order, TextureHandle, TextureOptions};

/// A janela aberta (só uma de cada vez, como no site).
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Janela {
    Streamer,
    Config(bool),
    Notificacoes,
    Roleta,
    Imagem,
    Denuncia((i32, i32)),
}

/// A roleta: quanto a roda já girou (graus), o giro em andamento (de, até, quando começou, o prêmio) e o prêmio na tela.
pub(super) struct Roleta {
    rot: f32,
    giro: Option<(f32, f32, f64, usize)>,
    ganhou: Option<(usize, f64)>,
    pub(super) pode: bool,
    pub(super) proximo: f64,
}
impl Default for Roleta {
    fn default() -> Self {
        Self { rot: 0.0, giro: None, ganhou: None, pode: true, proximo: 0.0 }
    }
}
pub(super) fn roleta_pode(r: &Roleta) -> bool {
    r.pode
}

/// O servidor sorteou `premio`: a roda gira até a fatia dele.
pub(super) fn girar(st: &mut LpState, premio: usize, t: f64) {
    let fatia = 360.0 / 7.0;
    let alvo = premio as f32 * fatia + fatia / 2.0;
    let base = (st.roleta.rot / 360.0).ceil() * 360.0;
    st.roleta.giro = Some((st.roleta.rot, base + 360.0 * 6.0 + (360.0 - alvo), t, premio));
}

/// Os prêmios das sete fatias, na ordem do desenho (a 0 começa no topo e anda no sentido do relógio).
const PREMIOS: [u32; 7] = [5, 10, 50, 100, 250, 500, 1000];
/// Onde cada número fica na roda de 300×300 e o giro dele (do `<text class="lp-rl-label">`).
const ROTULOS: [(f32, f32, f32, f32); 7] = [
    (183.4, 80.6, 25.0, -64.29),
    (225.1, 132.9, 25.0, -12.86),
    (210.2, 198.0, 25.0, 38.57),
    (150.0, 227.0, 25.0, 90.0),
    (89.8, 198.0, 25.0, 141.43),
    (74.9, 132.9, 25.0, 192.86),
    (116.6, 80.6, 21.0, 244.29),
];

/// A imagem de guia (o overlay): a foto como chegou, onde está no canvas (células), o tamanho em células, a
/// transparência, o "pixelar", se está travada, e o desenho já reduzido às células.
pub(super) struct Selo {
    img: image::RgbaImage,
    pub(super) x: f32,
    pub(super) y: f32,
    pub(super) gw: u32,
    pub(super) gh: u32,
    pub(super) opacidade: f32,
    pub(super) pixelar: u32,
    pub(super) travado: bool,
    tex: Option<(TextureHandle, (u32, u32, u32))>,
}

/// O envio da imagem: o arquivo escolhido e ainda não aplicado (nome, foto) e o diálogo do sistema aberto num fio à parte.
#[derive(Default)]
pub(super) struct Envio {
    escolhido: Option<(String, image::RgbaImage)>,
    dialogo: Option<std::thread::JoinHandle<Option<std::path::PathBuf>>>,
    erro: Option<String>,
}

/// A moderação: a barra aberta, a ferramenta ligada (apagar ou selecionar) e os pixels selecionados.
#[derive(Default)]
pub(super) struct Moderacao {
    pub(super) aberta: bool,
    pub(super) apagar: bool,
    pub(super) selecionar: bool,
    pub(super) sel: std::collections::HashSet<(i32, i32)>,
}

/// O estado das configurações do jogo (a aba Visual): fundo, cor e grossura da grade.
pub(super) struct Visual {
    pub(super) fundo: Color32,
    pub(super) grade_cor: Option<Color32>,
    pub(super) grade_w: f32,
}
impl Default for Visual {
    fn default() -> Self {
        Self { fundo: rgb(0xe2e8f0), grade_cor: None, grade_w: 1.0 }
    }
}

/// A busca da janela "quem você representa?": o texto e o streamer escolhido (ainda não confirmado).
#[derive(Default)]
pub(super) struct Busca {
    pub(super) q: String,
    pub(super) escolhido: Option<String>,
}

/// A denúncia: o motivo marcado e o texto do "outro motivo".
#[derive(Default)]
pub(super) struct Denuncia {
    motivo: usize,
    texto: String,
}

/* ================= peças das janelas ================= */

/// O ✕ de fechar, desenhado (as letras do jogo não têm o glifo).
fn cruz(p: &Painter, c: Pos2, lado: f32, cor: Color32, w: f32) {
    let h = lado / 2.0;
    p.line_segment([c + Vec2::new(-h, -h), c + Vec2::new(h, h)], Stroke::new(w, cor));
    p.line_segment([c + Vec2::new(h, -h), c + Vec2::new(-h, h)], Stroke::new(w, cor));
}

/// A estrela de cinco pontas do miolo da roleta.
fn estrela(p: &Painter, c: Pos2, r: f32, cor: Color32) {
    let pts: Vec<Pos2> = (0..10)
        .map(|i| {
            let a = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::PI / 5.0;
            let rr = if i % 2 == 0 { r } else { r * 0.45 };
            c + Vec2::angled(a) * rr
        })
        .collect();
    // o polígono não é convexo: um leque de triângulos a partir do centro
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(c, cor);
    for q in &pts {
        mesh.colored_vertex(*q, cor);
    }
    for i in 0..10u32 {
        mesh.add_triangle(0, 1 + i, 1 + (i + 1) % 10);
    }
    p.add(mesh);
}

/// O fundo das janelas: o canvas escurecido e sem cor (o site pixela e dessatura a cena; aqui ela fica cinza e escura).
fn fundo(p: &Painter, body: Rect) {
    p.rect_filled(body, 0.0, Color32::from_rgba_unmultiplied(118, 120, 128, 140));
    p.rect_filled(body, 0.0, rgba(0x0e0f16, 0.5));
}

/// Um `.lp-btn` (o botão cheio de ação): azul, letra do jogo, borda escura. Devolve se foi clicado.
fn botao(ui: &egui::Ui, r: Rect, id: &str, label: &str, ativo: bool) -> bool {
    let resp = ui.interact(r, ui.id().with(("lp-btn", id)), if ativo { Sense::click() } else { Sense::hover() });
    let resp = if ativo { resp.on_hover_cursor(CursorIcon::PointingHand) } else { resp };
    let p = ui.painter();
    let a = if ativo { 1.0 } else { 0.45 };
    let bg = if ativo && resp.hovered() { ui::mix(Color32::WHITE, ACCENT, 0.08) } else { ACCENT };
    boxed(p, r, bg.gamma_multiply(a), BW, LINE.gamma_multiply(a));
    text(p, r.center(), Align2::CENTER_CENTER, label, comic(17.0), BG.gamma_multiply(a), 0.0);
    ativo && resp.clicked()
}

/// O `.lp-modal__skip`: o texto que fecha ("agora não", "cancelar").
fn pular(ui: &egui::Ui, c: Pos2, id: &str, label: &str) -> bool {
    let p = ui.painter();
    let w = text_width(p, label, inter(13.0, 600), 0.0) + 20.0;
    let r = Rect::from_center_size(c, Vec2::new(w, 28.0));
    let resp = ui.interact(r, ui.id().with(("lp-skip", id)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
    let col = if resp.hovered() { INK } else { DIM };
    let tr = text(p, c, Align2::CENTER_CENTER, label, inter(13.0, 600), col, 0.0);
    if resp.hovered() {
        p.line_segment([Pos2::new(tr.left(), tr.bottom() - 1.0), Pos2::new(tr.right(), tr.bottom() - 1.0)], Stroke::new(1.0_f32, col));
    }
    resp.clicked()
}

/// Texto quebrado e centrado (o `.lp-modal__text`): devolve a altura.
fn paragrafo(p: &Painter, top_c: Pos2, s: &str, font: FontId, color: Color32, w: f32, lh: f32) -> f32 {
    let mut job = LayoutJob::single_section(s.to_owned(), TextFormat { font_id: font, color, line_height: Some(lh), ..Default::default() });
    job.wrap.max_width = w;
    job.halign = egui::Align::Center;
    let g = p.layout_job(job);
    let h = g.size().y;
    p.galley(Pos2::new(top_c.x, top_c.y), g, color);
    h
}

/// O cabeçalho das janelas de lista (`.lp-settings__header`): ícone, título e o ✕. Devolve se fechou.
fn cabecalho(ui: &egui::Ui, card: Rect, icon: ImageSource<'static>, titulo: &str) -> bool {
    let p = ui.painter();
    let y = card.top() + 20.0 + 13.0;
    ui::icon(ui, icon, Pos2::new(card.left() + 20.0 + 9.0, y), 18.0, INK);
    text(p, Pos2::new(card.left() + 20.0 + 18.0 + 8.0, y), Align2::LEFT_CENTER, titulo, inter(18.0, 700), INK, 0.0);
    let x = Rect::from_center_size(Pos2::new(card.right() - 20.0 - 14.0, y), Vec2::new(28.0, 28.0));
    let resp = ui.interact(x, ui.id().with(("lp-x", titulo)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand).on_hover_text("fechar");
    cruz(p, x.center(), 11.0, if resp.hovered() { BAD } else { DIM }, 1.6);
    p.rect_filled(Rect::from_min_size(Pos2::new(card.left() + 20.0, y + 13.0 + 12.0), Vec2::new(card.width() - 40.0, 2.0)), 0.0, LINE);
    resp.clicked()
}

/// O interruptor das configurações (`.lp-switch-slider`): 40×22, redondo, azul ligado.
fn chave(ui: &egui::Ui, r: Rect, id: &str, on: bool) {
    let k = ui.ctx().animate_bool_with_time(ui.id().with(("lp-sw", id)), on, 0.2);
    let p = ui.painter();
    p.rect_filled(r, 12.0, ui::mix(ACCENT, LINE, k));
    p.rect_stroke(r, 12.0, Stroke::new(1.0_f32, ui::mix(ACCENT, DIM, k)), StrokeKind::Inside);
    p.circle_filled(Pos2::new(r.left() + 1.0 + 2.0 + 8.0 + 18.0 * k, r.center().y), 8.0, Color32::WHITE);
}

/// Um `.lp-range`: o nome e o valor em cima, o trilho chapado com o puxador quadrado. Devolve o valor novo.
fn faixa(ui: &egui::Ui, r: Rect, id: &str, nome: &str, valor: f32, min: f32, max: f32, passo: f32, fmt: &dyn Fn(f32) -> String) -> f32 {
    let p = ui.painter();
    text(p, Pos2::new(r.left(), r.top() + 7.0), Align2::LEFT_CENTER, nome, inter(11.0, 700), DIM, 0.3);
    text(p, Pos2::new(r.right(), r.top() + 7.0), Align2::RIGHT_CENTER, &fmt(valor), comic(12.0), ACCENT, 0.0);
    let trilho = Rect::from_min_size(Pos2::new(r.left(), r.top() + 20.0), Vec2::new(r.width(), 16.0));
    let resp = ui.interact(trilho, ui.id().with(("lp-faixa", id)), Sense::click_and_drag()).on_hover_cursor(CursorIcon::PointingHand);
    let mut v = valor;
    if let Some(q) = resp.interact_pointer_pos() {
        if resp.dragged() || resp.clicked() {
            let k = ((q.x - trilho.left()) / trilho.width()).clamp(0.0, 1.0);
            v = ((min + k * (max - min)) / passo).round() * passo;
        }
    }
    let k = (v - min) / (max - min);
    let bar = Rect::from_center_size(trilho.center(), Vec2::new(trilho.width(), 10.0));
    p.rect_filled(bar, 0.0, LINE);
    let inner = bar.shrink(2.0);
    p.rect_filled(inner, 0.0, rgb(0x333c57));
    p.rect_filled(Rect::from_min_size(inner.min, Vec2::new(inner.width() * k, inner.height())), 0.0, ACCENT);
    let th = Rect::from_center_size(Pos2::new(trilho.left() + trilho.width() * k, trilho.center().y), Vec2::new(12.0, 16.0));
    p.rect_filled(th.expand(2.0), 0.0, if resp.hovered() || resp.dragged() { ACCENT } else { LINE_HI });
    p.rect_filled(th, 0.0, BG);
    p.rect_filled(th.shrink(2.0), 0.0, INK);
    v
}

/// Uma camada por cima de tudo, do tamanho do corpo, que segura os cliques (o que está atrás não recebe nada).
fn camada<R>(ui: &egui::Ui, body: Rect, id: &str, f: impl FnOnce(&mut egui::Ui) -> R) -> R {
    egui::Area::new(ui.id().with(("lp-camada", id)))
        .order(Order::Foreground)
        .fixed_pos(body.min)
        .constrain(false)
        .show(ui.ctx(), |ui| {
            ui.set_clip_rect(body);
            let (_, _) = ui.allocate_exact_size(body.size(), Sense::click());
            f(ui)
        })
        .inner
}

/* ================= o cartão de um pixel ================= */

pub(super) fn info(ui: &mut egui::Ui, st: &mut LpState, body: Rect, t: f64) {
    let Some((at, scr)) = st.info else { return };
    let Some(px) = st.pixels.get(&at).cloned() else {
        st.info = None;
        return;
    };
    let card = Rect::from_min_size(
        Pos2::new((scr.x + 14.0).clamp(body.left() + 8.0, body.right() - 240.0 - 8.0), (scr.y + 14.0).clamp(body.top() + 8.0, body.bottom() - 160.0 - 8.0)),
        Vec2::new(240.0, 145.0),
    );
    let fechar = egui::Area::new(ui.id().with("lp-info"))
        .order(Order::Foreground)
        .fixed_pos(card.min)
        .constrain(false)
        .show(ui.ctx(), |ui| {
            let (r, _) = ui.allocate_exact_size(card.size(), Sense::click());
            let p = ui.painter();
            p.rect_filled(r.translate(Vec2::new(0.0, 6.0)).expand(6.0), 8.0, Color32::from_black_alpha(40));
            boxed(p, r, BG, BW, LINE);
            let inner = Rect::from_min_max(r.min + Vec2::new(BW + 14.0, BW + 12.0), r.max - Vec2::new(BW + 14.0, BW + 12.0));
            // quem pintou e por quem
            let av = Rect::from_min_size(inner.min, Vec2::splat(36.0));
            // sem foto, o lugar dela fica vazio (o site esconde a imagem que não carregou, com a borda junto)
            if let Some(src) = net::imagem(&px.foto) {
                p.rect_filled(av, 0.0, Color32::WHITE);
                ui::image(ui, src, av.shrink(2.0), 0.0);
                p.rect_stroke(av, 0.0, Stroke::new(2.0_f32, LINE), StrokeKind::Inside);
            }
            ui::clip(p, Pos2::new(av.right() + 10.0, av.top() + 9.0), Align2::LEFT_CENTER, &px.nome, inter(13.0, 700), INK, inner.width() - 46.0 - 24.0);
            ui::clip(p, Pos2::new(av.right() + 10.0, av.top() + 26.0), Align2::LEFT_CENTER, &format!("representa {}", st.name_of(&px.streamer)), inter(11.0, 400), DIM, inner.width() - 46.0 - 24.0);
            let x = Rect::from_center_size(Pos2::new(inner.right() - 6.0, inner.top() + 8.0), Vec2::splat(22.0));
            let xr = ui.interact(x, ui.id().with("lp-info-x"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand).on_hover_text("fechar");
            text(p, x.center(), Align2::CENTER_CENTER, "×", inter(20.0, 400), if xr.hovered() { INK } else { DIM }, 0.0);
            // a cor, quando, denunciar
            let my = av.bottom() + 10.0;
            p.rect_filled(Rect::from_min_size(Pos2::new(inner.left(), my), Vec2::new(inner.width(), 2.0)), 0.0, rgba(0x333c57, 0.5));
            let row = my + 12.0 + 8.0;
            let sw = Rect::from_center_size(Pos2::new(inner.left() + 8.0, row), Vec2::splat(16.0));
            p.rect_filled(sw, 0.0, px.c);
            p.rect_stroke(sw, 0.0, Stroke::new(2.0_f32, LINE), StrokeKind::Inside);
            let quando = chrono::Local::now() - chrono::Duration::seconds((t - px.t) as i64);
            text(p, Pos2::new(sw.right() + 10.0, row), Align2::LEFT_CENTER, &quando.format("%d/%m/%Y, %H:%M:%S").to_string(), inter(11.0, 400), DIM, 0.0);
            let rep = Rect::from_center_size(Pos2::new(inner.right() - 8.0, row), Vec2::splat(24.0));
            let rr = ui.interact(rep, ui.id().with("lp-info-den"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand).on_hover_text("denunciar este pixel");
            ui::icon(ui, ic!("lp-infoReport"), rep.center(), if rr.hovered() { 18.4 } else { 16.0 }, BAD.gamma_multiply(if rr.hovered() { 1.0 } else { 0.75 }));
            // compartilhar
            let ay = row + 8.0 + 10.0;
            p.rect_filled(Rect::from_min_size(Pos2::new(inner.left(), ay), Vec2::new(inner.width(), 2.0)), 0.0, rgba(0x333c57, 0.5));
            let sh = Rect::from_min_size(Pos2::new(inner.left(), ay + 12.0), Vec2::new(inner.width(), 30.0));
            let sr = ui.interact(sh, ui.id().with("lp-info-share"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand).on_hover_text("compartilhar essa localização");
            let (bg, line, fg) = if sr.hovered() { (rgba(0x41a6f6, 0.15), ACCENT, ACCENT) } else { (rgba(0x333c57, 0.35), LINE, INK) };
            boxed(p, sh, bg, 2.0, line);
            let tw = text_width(p, "Compartilhar essa localização", inter(12.0, 600), 0.0);
            ui::icon(ui, ic!("lp-infoShare"), Pos2::new(sh.center().x - tw / 2.0 - 3.0 - 7.0, sh.center().y), 14.0, fg);
            text(p, Pos2::new(sh.center().x + 10.0, sh.center().y), Align2::CENTER_CENTER, "Compartilhar essa localização", inter(12.0, 600), fg, 0.0);
            if sr.clicked() {
                ui.ctx().copy_text(format!("https://brittico.xyz/livepixel/?x={}&y={}&z={:.2}", at.0, at.1, st.cam.2.max(2.5)));
                return Some(Some("Link da localização copiado!"));
            }
            if rr.clicked() {
                st.janela = Some(Janela::Denuncia(at));
                st.denuncia = Denuncia::default();
                return Some(None);
            }
            xr.clicked().then_some(None)
        })
        .inner;
    if let Some(msg) = fechar {
        st.info = None;
        if let Some(m) = msg {
            st.say(m, false, t);
        }
    }
    // clicar fora fecha
    if ui.input(|i| i.pointer.any_pressed()) && !ui.input(|i| i.pointer.interact_pos()).is_some_and(|q| card.contains(q)) {
        st.info = None;
    }
}

/* ================= o menu do perfil ================= */

pub(super) fn menu(ui: &mut egui::Ui, st: &mut LpState, body: Rect, prof: Rect, t: f64) {
    if !st.pmenu {
        return;
    }
    let itens: [(&str, ImageSource<'static>, bool); 4] = [
        ("notificações", ic!("lp-pmenuNotif"), false),
        ("configurações", ic!("lp-pmenuSettings"), false),
        ("trocar streamer", ic!("lp-pmenuChange"), false),
        ("sair da conta", ic!("lp-pmenuLogout"), true),
    ];
    let h = BW * 2.0 + 12.0 + 4.0 * 37.0 + 3.0 * 4.0;
    let r = Rect::from_min_size(Pos2::new(body.right() - GAP - 190.0, prof.top() - STACK - h), Vec2::new(190.0, h));
    let escolha = egui::Area::new(ui.id().with("lp-pmenu"))
        .order(Order::Foreground)
        .fixed_pos(r.min)
        .constrain(false)
        .show(ui.ctx(), |ui| {
            ui.allocate_exact_size(r.size(), Sense::click());
            let p = ui.painter();
            boxed(p, r, BG, BW, LINE);
            let mut esc = None;
            for (i, (nome, icon, perigo)) in itens.into_iter().enumerate() {
                let it = Rect::from_min_size(r.min + Vec2::new(BW + 6.0, BW + 6.0 + i as f32 * 41.0), Vec2::new(r.width() - 2.0 * (BW + 6.0), 37.0));
                let resp = ui.interact(it, ui.id().with(("lp-pm", i)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
                if resp.hovered() {
                    p.rect_filled(it, 0.0, if perigo { rgba(0xb13e53, 0.15) } else { rgba(0x333c57, 0.4) });
                }
                ui::icon(ui, icon, Pos2::new(it.left() + 12.0 + 8.0, it.center().y), 16.0, if perigo { BAD } else { ACCENT });
                text(p, Pos2::new(it.left() + 12.0 + 16.0 + 10.0, it.center().y), Align2::LEFT_CENTER, nome, inter(13.0, 600), INK, 0.0);
                if resp.clicked() {
                    esc = Some(i);
                }
            }
            esc
        })
        .inner;
    match escolha {
        Some(0) => st.janela = Some(Janela::Notificacoes),
        Some(1) => st.janela = Some(Janela::Config(false)),
        Some(2) => {
            st.janela = Some(Janela::Streamer);
            st.busca = Busca { q: String::new(), escolhido: st.streamer.clone() };
        }
        Some(_) => st.say("o login ainda não chegou ao app: aqui você é sempre o Kennzy de teste", false, t),
        None => {}
    }
    if escolha.is_some() || (ui.input(|i| i.pointer.any_pressed()) && !ui.input(|i| i.pointer.interact_pos()).is_some_and(|q| r.contains(q) || prof.contains(q))) {
        st.pmenu = false;
    }
}

/* ================= as barras do overlay e da moderação ================= */

/// A barra do overlay de imagem, acima do canto de baixo à direita: opacidade, pixelar, trocar, travar, co-op e remover.
pub(super) fn barra_selo(ui: &mut egui::Ui, st: &mut LpState, body: Rect, t: f64) {
    let Some(selo) = st.selo.as_mut() else { return };
    let w = BW * 2.0 + 24.0 + 32.0 + 14.0 + 112.0 + 14.0 + 112.0 + 4.0 * (14.0 + 32.0);
    let r = Rect::from_min_size(Pos2::new(body.right() - GAP - w, body.bottom() - GAP - HUD_H - STACK - 58.0), Vec2::new(w, 58.0));
    let p = ui.painter().clone();
    ui.interact(r, ui.id().with("lp-selo-barra"), Sense::click_and_drag());
    boxed(&p, r, BG, BW, LINE);
    let mut x = r.left() + BW + 12.0;
    let cy = r.center().y;
    let tit = Rect::from_min_size(Pos2::new(x, cy - 16.0), Vec2::splat(32.0));
    boxed(&p, tit, rgba(0x41a6f6, 0.12), 2.0, ACCENT);
    ui::icon(ui, ic!("lp-stampBtn"), tit.center(), 16.0, ACCENT);
    x = tit.right() + 14.0;
    selo.opacidade = faixa(ui, Rect::from_min_size(Pos2::new(x, cy - 18.0), Vec2::new(112.0, 36.0)), "opacidade", "opacidade", selo.opacidade, 0.1, 1.0, 0.05, &|v| format!("{}%", (v * 100.0).round()));
    x += 112.0 + 14.0;
    selo.pixelar = faixa(ui, Rect::from_min_size(Pos2::new(x, cy - 18.0), Vec2::new(112.0, 36.0)), "pixelar", "pixelar", selo.pixelar as f32, 1.0, 32.0, 1.0, &|v| format!("{}px", v as u32)) as u32;
    x += 112.0 + 14.0;
    let botoes: [(&str, &str, ImageSource<'static>, bool, bool); 4] = [
        ("trocar", "trocar imagem", ic!("lp-stampReplaceBtn"), false, false),
        ("travar", "travar/destravar posição", ic!("lp-stampLockBtn"), selo.travado, false),
        ("coop", "convidar alguém para o co-op (ver esta imagem)", ic!("lp-stampCoopBtn"), false, false),
        ("remover", "remover imagem", ic!("lp-stampDeleteBtn"), false, true),
    ];
    let mut acao = None;
    for (key, tip, icon, ativo, perigo) in botoes {
        let b = Rect::from_min_size(Pos2::new(x, cy - 16.0), Vec2::splat(32.0));
        let resp = hit(ui, b, ("selo", key), tip);
        let (bg, line, fg) = if ativo {
            (ACCENT, ACCENT, BG)
        } else if resp.hovered() && perigo {
            (BAD, LINE_HI, Color32::WHITE)
        } else if resp.hovered() {
            (LINE, LINE_HI, INK)
        } else {
            (BG, LINE, INK)
        };
        boxed(&p, b, bg, 2.0, line);
        ui::icon(ui, icon, b.center(), 16.0, fg);
        if resp.clicked() {
            acao = Some(key);
        }
        x = b.right() + 14.0;
    }
    match acao {
        Some("trocar") => {
            st.janela = Some(Janela::Imagem);
            st.envio = Envio::default();
        }
        Some("travar") => selo.travado = !selo.travado,
        Some("coop") => st.say("o co-op pede outra conta Premium: ainda não chegou ao app", false, t),
        Some("remover") => st.selo = None,
        _ => {}
    }
}

/// A barra da moderação: apagar de qualquer pessoa, selecionar e mover, a contagem e a dica.
pub(super) fn barra_mod(ui: &mut egui::Ui, st: &mut LpState, body: Rect) {
    if !st.moderacao.aberta {
        return;
    }
    let p = ui.painter().clone();
    let n = st.moderacao.sel.len();
    let dica = {
        let pan = "Espaço + arrastar (ou botão do meio) move a câmera.";
        if st.moderacao.apagar {
            format!("Arraste sobre os pixels para apagar, de qualquer pessoa. {pan}")
        } else if st.moderacao.selecionar && n > 0 {
            format!("Arraste dentro da seleção para mover · Delete apaga · Esc limpa · Shift soma à seleção. {pan}")
        } else if st.moderacao.selecionar {
            format!("Arraste para selecionar pixels. {pan}")
        } else {
            String::new()
        }
    };
    let g = {
        let mut job = LayoutJob::single_section(dica.clone(), TextFormat { font_id: inter(11.0, 400), color: LP_E, line_height: Some(15.4), ..Default::default() });
        job.wrap.max_width = 330.0;
        p.layout_job(job)
    };
    let gh = if dica.is_empty() { 0.0 } else { 8.0 + g.size().y };
    let largura = 2.0 * (BW + 12.0) + 32.0 + 10.0 + 86.0 + 10.0 + 108.0 + if n > 0 { 10.0 + 70.0 + 10.0 + 32.0 + 10.0 + 32.0 } else { 0.0 };
    let w = largura.max(if dica.is_empty() { 0.0 } else { 330.0 + 2.0 * (BW + 12.0) });
    let h = 2.0 * (BW + 10.0) + 32.0 + gh;
    let r = Rect::from_min_size(Pos2::new(body.right() - GAP - w, body.bottom() - GAP - HUD_H - STACK - h), Vec2::new(w, h));
    ui.interact(r, ui.id().with("lp-mod-barra"), Sense::click_and_drag());
    boxed(&p, r, BG, BW, LINE);
    let mut x = r.left() + BW + 12.0;
    let cy = r.top() + BW + 10.0 + 16.0;
    let tit = Rect::from_min_size(Pos2::new(x, cy - 16.0), Vec2::splat(32.0));
    boxed(&p, tit, PALETTE[6], 2.0, rgb(0x2b8a4e));
    ui::icon(ui, ic!("lp-modBtn"), tit.center(), 16.0, Color32::WHITE);
    x = tit.right() + 10.0;
    for (i, (nome, tip, icon)) in [("apagar", "apagar pixels de qualquer pessoa (arraste sobre eles)", ic!("lp-modToolErase")), ("selecionar", "selecionar vários pixels (arraste); arraste dentro da seleção para mover", ic!("lp-modToolSelect"))].into_iter().enumerate() {
        let bw = 10.0 + 16.0 + 6.0 + text_width(&p, nome, comic(12.0), 0.0) + 10.0 + 4.0;
        let b = Rect::from_min_size(Pos2::new(x, cy - 16.0), Vec2::new(bw, 32.0));
        let resp = hit(ui, b, ("modtool", i), tip);
        let on = if i == 0 { st.moderacao.apagar } else { st.moderacao.selecionar };
        let (bg, line, fg) = if on { (rgba(0x38b764, 0.22), PALETTE[6], Color32::WHITE) } else if resp.hovered() { (BG, LINE_HI, INK) } else { (BG, LINE, DIM) };
        boxed(&p, b, bg, 2.0, line);
        ui::icon(ui, icon, Pos2::new(b.left() + 10.0 + 8.0, cy), 16.0, fg);
        text(&p, Pos2::new(b.left() + 10.0 + 16.0 + 6.0, cy), Align2::LEFT_CENTER, nome, comic(12.0), fg, 0.0);
        if resp.clicked() {
            if i == 0 {
                st.moderacao.apagar = !on;
                st.moderacao.selecionar = false;
                st.moderacao.sel.clear();
            } else {
                st.moderacao.selecionar = !on;
                st.moderacao.apagar = false;
            }
        }
        x = b.right() + 10.0;
    }
    if n > 0 {
        let c = text(&p, Pos2::new(x, cy), Align2::LEFT_CENTER, &format!("{n} {}", if n == 1 { "pixel" } else { "pixels" }), comic(12.0), PALETTE[6], 0.0);
        x = c.right() + 10.0;
        for (key, tip, icon, perigo) in [("apagar-sel", "apagar seleção (Delete)", ic!("lp-modDeleteBtn"), true), ("limpar-sel", "limpar seleção (Esc)", ic!("lp-modClearBtn"), false)] {
            let b = Rect::from_min_size(Pos2::new(x, cy - 16.0), Vec2::splat(32.0));
            let resp = hit(ui, b, key, tip);
            let (bg, fg) = if resp.hovered() && perigo { (BAD, Color32::WHITE) } else if resp.hovered() { (LINE, INK) } else { (BG, INK) };
            boxed(&p, b, bg, 2.0, if resp.hovered() { LINE_HI } else { LINE });
            ui::icon(ui, icon, b.center(), 16.0, fg);
            if resp.clicked() {
                if perigo {
                    st.apagar_selecao();
                } else {
                    st.moderacao.sel.clear();
                }
            }
            x = b.right() + 10.0;
        }
    }
    if !dica.is_empty() {
        p.galley(Pos2::new(r.left() + BW + 12.0, cy + 16.0 + 8.0), g, LP_E);
    }
}

/* ================= as janelas ================= */

pub(super) fn janelas(ui: &mut egui::Ui, st: &mut LpState, body: Rect, t: f64) {
    poll_dialogo(ui, st);
    let Some(j) = st.janela else { return };
    let fechar = camada(ui, body, "janela", |ui| {
        fundo(ui.painter(), body);
        match j {
            Janela::Streamer => streamer(ui, st, body, t),
            Janela::Config(premium) => config(ui, st, body, premium),
            Janela::Notificacoes => notificacoes(ui, body),
            Janela::Roleta => roleta(ui, st, body, t),
            Janela::Imagem => imagem(ui, st, body, t),
            Janela::Denuncia(at) => denuncia(ui, st, body, at, t),
        }
    });
    let esc = ui.input(|i| i.key_pressed(Key::Escape));
    if fechar || esc {
        st.janela = None;
    }
}

/// Um cartão centrado no corpo, com a altura que o conteúdo pedir.
fn cartao(body: Rect, w: f32, h: f32) -> Rect {
    Rect::from_center_size(body.center(), Vec2::new(w, h))
}

/// "quem você representa?": a busca nos streamers, o escolhido e o "entrar no canvas".
fn streamer(ui: &mut egui::Ui, st: &mut LpState, body: Rect, t: f64) -> bool {
    let tem = st.busca.escolhido.is_some();
    let h = 36.0 + 30.0 + 18.0 + 44.0 + 18.0 + 52.0 + if tem { 18.0 + 62.0 } else { 0.0 } + 18.0 + 54.0 + 18.0 + 28.0 + 30.0;
    let card = cartao(body, 400.0, h);
    let p = ui.painter().clone();
    boxed(&p, card, BG, BW, LINE);
    let cx = card.center().x;
    let mut y = card.top() + 36.0;
    text(&p, Pos2::new(cx, y + 15.0), Align2::CENTER_CENTER, "quem você representa?", comic(25.0), INK, 0.0);
    y += 30.0 + 18.0;
    y += paragrafo(&p, Pos2::new(cx, y), "cada pixel seu pinta o território do seu streamer.", inter(14.0, 400), DIM, 300.0, 21.7) + 18.0;
    // a busca
    let campo = Rect::from_min_size(Pos2::new(card.left() + 35.0, y), Vec2::new(card.width() - 70.0, 52.0));
    let fid = ui.id().with("lp-busca");
    let foco = ui.memory(|m| m.has_focus(fid));
    boxed(&p, campo, rgba(0x333c57, 0.35), BW, if foco { ACCENT } else { LINE });
    ui::icon(ui, ic!("search"), Pos2::new(campo.left() + 15.0 + 8.0, campo.center().y), 16.0, if foco { ACCENT } else { LP_E });
    let mut fui = ui.new_child(UiBuilder::new().max_rect(Rect::from_min_max(campo.min + Vec2::new(44.0, 13.0), campo.max - Vec2::new(16.0, 13.0))));
    fui.add(
        egui::TextEdit::singleline(&mut st.busca.q)
            .id(fid)
            .frame(false)
            .hint_text(egui::RichText::new("buscar streamer…").font(inter(15.0, 600)).color(LP_E))
            .font(inter(15.0, 600))
            .text_color(INK)
            .vertical_align(egui::Align::Center)
            .margin(egui::Margin::ZERO)
            .desired_width(campo.width() - 60.0),
    );
    y = campo.bottom();
    // o escolhido
    if let Some(sid) = st.busca.escolhido.clone() {
        y += 18.0;
        let ch = Rect::from_min_size(Pos2::new(campo.left(), y), Vec2::new(campo.width(), 62.0));
        boxed(&p, ch, rgba(0x41a6f6, 0.08), BW, ACCENT);
        let av = Rect::from_min_size(Pos2::new(ch.left() + BW + 14.0, ch.center().y - 18.0), Vec2::splat(36.0));
        if let Some(src) = st.avatar_of(&sid) {
            p.rect_filled(av, 0.0, Color32::WHITE);
            ui::image(ui, src, av.shrink(2.0), 0.0);
            p.rect_stroke(av, 0.0, Stroke::new(2.0_f32, LINE), StrokeKind::Inside);
        }
        text(&p, Pos2::new(av.right() + 12.0, ch.center().y), Align2::LEFT_CENTER, &st.name_of(&sid), inter(15.0, 700), INK, 0.0);
        let x = Rect::from_center_size(Pos2::new(ch.right() - BW - 14.0 - 8.0, ch.center().y), Vec2::splat(28.0));
        let xr = ui.interact(x, ui.id().with("lp-esc-x"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand).on_hover_text("trocar");
        text(&p, x.center(), Align2::CENTER_CENTER, "×", inter(24.0, 400), if xr.hovered() { BAD } else { DIM }, 0.0);
        if xr.clicked() {
            st.busca.escolhido = None;
        }
        y = ch.bottom();
    }
    y += 18.0;
    let go = Rect::from_min_size(Pos2::new(campo.left(), y), Vec2::new(campo.width(), 54.0));
    if botao(ui, go, "entrar", "entrar no canvas", tem) {
        if st.m.me.id.is_empty() {
            st.say("entre na sua conta para pintar", true, t);
            return true;
        }
        if let Some(sid) = st.busca.escolhido.clone() {
            st.escolher_streamer(ui.ctx(), sid);
        }
        return true;
    }
    let fechar = pular(ui, Pos2::new(cx, go.bottom() + 18.0 + 14.0), "explorar", "prefiro só explorar");
    if fechar {
        st.streamer = None;
    }
    // os resultados, por cima do resto, embaixo do campo
    let q = st.busca.q.trim().to_lowercase();
    if !q.is_empty() {
        let achados: Vec<MockStreamer> = st.busca_res.clone();
        let n = achados.len().max(1);
        let lr = Rect::from_min_size(campo.left_bottom(), Vec2::new(campo.width(), (n as f32 * 56.0 + BW).min(210.0)));
        let lp = ui.ctx().layer_painter(egui::LayerId::new(Order::Tooltip, ui.id().with("lp-res")));
        lp.rect_filled(lr, 0.0, BG);
        lp.rect_stroke(lr, 0.0, Stroke::new(BW, LINE), StrokeKind::Inside);
        lp.rect_filled(Rect::from_min_size(lr.min + Vec2::new(BW, 0.0), Vec2::new(lr.width() - 2.0 * BW, BW)), 0.0, BG);
        if achados.is_empty() {
            text(&lp, Pos2::new(lr.left() + 16.0, lr.top() + 28.0), Align2::LEFT_CENTER, if st.busca_feita != q.as_str() && st.busca.q.trim().chars().count() >= 2 { "buscando…" } else if q.chars().count() < 2 { "digite pelo menos 2 letras" } else { "nenhum streamer encontrado" }, inter(13.0, 400), DIM, 0.0);
        }
        for (i, s) in achados.iter().enumerate() {
            let it = Rect::from_min_size(Pos2::new(lr.left() + BW, lr.top() + i as f32 * 56.0), Vec2::new(lr.width() - 2.0 * BW, 56.0));
            if it.bottom() > lr.bottom() {
                break;
            }
            let resp = ui.interact(it, ui.id().with(("lp-res", i)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
            if resp.hovered() {
                lp.rect_filled(it, 0.0, rgba(0x333c57, 0.4));
            }
            if i > 0 {
                lp.rect_filled(Rect::from_min_size(it.min, Vec2::new(it.width(), 2.0)), 0.0, rgba(0x333c57, 0.5));
            }
            let av = Rect::from_min_size(Pos2::new(it.left() + 14.0, it.center().y - 16.0), Vec2::splat(32.0));
            if let Some(src) = s.avatar.as_deref().and_then(net::imagem) {
                lp.rect_filled(av, 0.0, Color32::WHITE);
                egui::Image::new(src).paint_at(ui, av.shrink(2.0));
                lp.rect_stroke(av, 0.0, Stroke::new(2.0_f32, LINE), StrokeKind::Inside);
            }
            text(&lp, Pos2::new(av.right() + 12.0, it.center().y - 8.0), Align2::LEFT_CENTER, &s.name, inter(14.0, 700), INK, 0.0);
            text(&lp, Pos2::new(av.right() + 12.0, it.center().y + 9.0), Align2::LEFT_CENTER, &format!("@{}", if s.username.is_empty() { &s.name } else { &s.username }), inter(11.0, 400), DIM, 0.0);
            if resp.clicked() {
                st.busca.escolhido = Some(s.id.clone());
                st.busca.q.clear();
            }
        }
    }
    fechar
}

/// As configurações do jogo: a aba Visual (fundo, grade, territórios, cor e grossura da grade) e a Premium.
fn config(ui: &mut egui::Ui, st: &mut LpState, body: Rect, premium: bool) -> bool {
    let h = if premium { 470.0 } else { 20.0 + 26.0 + 14.0 + 16.0 + 37.0 + 16.0 + 92.0 + 14.0 + 82.0 + 14.0 + 120.0 + 20.0 };
    let card = cartao(body, 440.0, h);
    let p = ui.painter().clone();
    boxed(&p, card, BG, BW, INK);
    let fechou = cabecalho(ui, card, ic!("lp-pmenuSettings"), "Configurações");
    // as abas
    let ty = card.top() + 20.0 + 26.0 + 14.0 + 16.0;
    let mut x = card.left() + 20.0;
    for (i, nome) in ["Visual & Tema", "Premium"].iter().enumerate() {
        let w = text_width(&p, nome, inter(14.0, 700), 0.0) + 24.0 + if i == 1 { 17.0 } else { 0.0 };
        let r = Rect::from_min_size(Pos2::new(x, ty), Vec2::new(w, 37.0));
        let resp = ui.interact(r, ui.id().with(("lp-cfg-aba", i)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
        let on = (i == 1) == premium;
        let c = if on { ACCENT } else { DIM };
        let mut tx = r.left() + 12.0;
        if i == 1 {
            ui::icon(ui, ic!("lp-profPremBadge"), Pos2::new(tx + 6.0, r.center().y), 12.0, GOLD);
            tx += 17.0;
        }
        text(&p, Pos2::new(tx, r.center().y), Align2::LEFT_CENTER, nome, inter(14.0, 700), c, 0.0);
        if on {
            p.rect_filled(Rect::from_min_size(Pos2::new(r.left(), r.bottom() - 3.0), Vec2::new(w, 3.0)), 0.0, ACCENT);
        }
        if resp.clicked() {
            st.janela = Some(Janela::Config(i == 1));
        }
        x = r.right() + 8.0;
    }
    p.rect_filled(Rect::from_min_size(Pos2::new(card.left() + 20.0, ty + 37.0), Vec2::new(card.width() - 40.0, 2.0)), 0.0, LINE);
    let gx = card.left() + 20.0;
    let gw = card.width() - 40.0;
    let mut y = ty + 37.0 + 2.0 + 16.0;
    let grupo = |p: &Painter, y: f32, h: f32| {
        let r = Rect::from_min_size(Pos2::new(gx, y), Vec2::new(gw, h));
        p.rect_filled(r, 4.0, rgba(0x1a1c2c, 0.2));
        p.rect_stroke(r, 4.0, Stroke::new(1.0_f32, LINE), StrokeKind::Inside);
        r
    };
    if premium {
        let g = grupo(&p, y, 86.0);
        let badge = Rect::from_min_size(g.min + Vec2::new(15.0, 15.0), Vec2::new(text_width(&p, "USUÁRIO GRATUITO", inter(12.0, 700), 0.5) + 20.0, 24.0));
        p.rect_filled(badge, 4.0, ACCENT);
        text(&p, badge.center(), Align2::CENTER_CENTER, "USUÁRIO GRATUITO", inter(12.0, 700), Color32::WHITE, 0.5);
        let mut job = LayoutJob::single_section("Assine para desbloquear o overlay de imagem e +120 pixels a cada 10 min.".into(), TextFormat { font_id: inter(13.0, 400), color: INK, line_height: Some(18.2), ..Default::default() });
        job.wrap.max_width = gw - 30.0;
        p.galley(Pos2::new(g.left() + 15.0, badge.bottom() + 8.0), p.layout_job(job), INK);
        y = g.bottom() + 14.0;
        let g = grupo(&p, y, 150.0);
        ui::icon(ui, ic!("users"), Pos2::new(g.left() + 15.0 + 8.0, g.top() + 15.0 + 8.0), 16.0, INK);
        text(&p, Pos2::new(g.left() + 15.0 + 22.0, g.top() + 15.0 + 8.0), Align2::LEFT_CENTER, "Modo família", inter(13.0, 700), INK, 0.0);
        let mut job = LayoutJob::single_section("Quem está na sua família vê o seu overlay de imagem no canvas e ajuda a pintar. Você também vê o overlay de quem liberou o dele.".into(), TextFormat { font_id: inter(12.0, 400), color: DIM, line_height: Some(16.8), ..Default::default() });
        job.wrap.max_width = gw - 30.0;
        let g2 = p.layout_job(job);
        let gh = g2.size().y;
        p.galley(Pos2::new(g.left() + 15.0, g.top() + 15.0 + 24.0), g2, DIM);
        let mut job = LayoutJob::default();
        let f = |c: Color32, w| TextFormat { font_id: inter(12.0, w), color: c, line_height: Some(16.8), ..Default::default() };
        job.append("Só quem é ", 0.0, f(INK, 400));
        job.append("Premium", 0.0, f(INK, 700));
        job.append(" pode abrir uma família. Mas você pode ", 0.0, f(INK, 400));
        job.append("entrar", 0.0, f(INK, 700));
        job.append(" na família de alguém: quando te convidarem, o convite aparece no chat e nas notificações.", 0.0, f(INK, 400));
        job.wrap.max_width = gw - 30.0;
        p.galley(Pos2::new(g.left() + 15.0, g.top() + 15.0 + 24.0 + gh + 10.0), p.layout_job(job), INK);
        y = g.bottom() + 14.0;
        let g = grupo(&p, y, 104.0);
        text(&p, Pos2::new(g.left() + 15.0, g.top() + 15.0 + 8.0), Align2::LEFT_CENTER, "Presentear Premium", inter(13.0, 700), INK, 0.0);
        let b = Rect::from_min_size(Pos2::new(g.left() + 15.0, g.top() + 15.0 + 24.0 + 6.0), Vec2::new(gw - 30.0, 40.0));
        let resp = ui.interact(b, ui.id().with("lp-gift"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
        p.rect_filled(b, 6.0, LINE);
        p.extend(Shape::dashed_line(&[b.left_top(), b.right_top(), b.right_bottom(), b.left_bottom(), b.left_top()], Stroke::new(2.0_f32, ACCENT), 5.0, 3.0));
        text(&p, b.center(), Align2::CENTER_CENTER, "Escolher quem vai ganhar", inter(13.0, 700), INK, 0.0);
        if resp.clicked() {
            st.say("presentear o Premium pede o pagamento (Pix ou cartão), que ainda não chegou ao app", false, ui.input(|i| i.time));
        }
        return fechou;
    }
    // o fundo do canvas
    let g = grupo(&p, y, 92.0);
    text(&p, Pos2::new(g.center().x, g.top() + 14.0 + 8.0), Align2::CENTER_CENTER, "Fundo do Canvas", inter(13.0, 700), INK, 0.0);
    let fundos: [(u32, &str); 5] = [(0xffffff, "Branco"), (0xe2e8f0, "Cinza Claro"), (0x566c86, "Cinza Misto"), (0x1a1c2c, "Azul Escuro"), (0x0f172a, "Escuro Total")];
    let total = 6.0 * 30.0 + 5.0 * 8.0;
    let mut sx = g.center().x - total / 2.0;
    for (c, nome) in fundos {
        let r = Rect::from_min_size(Pos2::new(sx, g.top() + 14.0 + 24.0), Vec2::splat(30.0));
        let resp = ui.interact(r, ui.id().with(("lp-fundo", c)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand).on_hover_text(nome);
        let on = st.visual.fundo == rgb(c);
        let rr = if resp.hovered() { r.expand(1.5) } else { r };
        p.rect_filled(rr, 4.0, rgb(c));
        p.rect_stroke(rr, 4.0, Stroke::new(2.0_f32, if on { ACCENT } else if resp.hovered() { LINE_HI } else { LINE }), StrokeKind::Inside);
        if on {
            p.rect_stroke(rr.expand(2.0), 6.0, Stroke::new(2.0_f32, ACCENT), StrokeKind::Outside);
        }
        if resp.clicked() {
            st.visual.fundo = rgb(c);
        }
        sx = r.right() + 8.0;
    }
    let pk = Rect::from_min_size(Pos2::new(sx, g.top() + 14.0 + 24.0), Vec2::splat(30.0));
    let pr = ui.interact(pk, ui.id().with("lp-fundo-cor"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand).on_hover_text("Cor personalizada");
    p.extend(Shape::dashed_line(&[pk.left_top(), pk.right_top(), pk.right_bottom(), pk.left_bottom(), pk.left_top()], Stroke::new(2.0_f32, DIM), 4.0, 3.0));
    if pr.clicked() {
        st.fundo_picker = !st.fundo_picker;
    }
    if st.fundo_picker {
        let mut c = st.visual.fundo;
        let area = egui::Area::new(ui.id().with("lp-fundo-picker")).order(Order::Tooltip).fixed_pos(pk.left_bottom() + Vec2::new(0.0, 6.0)).show(ui.ctx(), |ui| {
            egui::Frame::new().fill(BG).stroke(Stroke::new(BW, LINE)).inner_margin(10.0).show(ui, |ui| {
                ui.spacing_mut().item_spacing = Vec2::splat(6.0);
                egui::color_picker::color_picker_color32(ui, &mut c, egui::color_picker::Alpha::Opaque)
            })
        });
        if area.inner.inner {
            st.visual.fundo = c;
        }
        if ui.input(|i| i.pointer.any_pressed()) && !area.response.hovered() && !pr.hovered() {
            st.fundo_picker = false;
        }
    }
    y = g.bottom() + 14.0;
    // grade e territórios
    let g = grupo(&p, y, 82.0);
    for (i, nome) in ["Mostrar Grade no Canvas", "Área & Nomes dos Streamers"].iter().enumerate() {
        let row = Rect::from_min_size(Pos2::new(g.left() + 15.0, g.top() + 14.0 + i as f32 * 32.0), Vec2::new(gw - 30.0, 22.0));
        let resp = ui.interact(row, ui.id().with(("lp-cfg-sw", i)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
        text(&p, row.left_center(), Align2::LEFT_CENTER, nome, inter(14.0, 600), INK, 0.0);
        let on = if i == 0 { st.grid } else { st.outlines };
        chave(ui, Rect::from_min_size(Pos2::new(row.right() - 40.0, row.top()), Vec2::new(40.0, 22.0)), nome, on);
        if resp.clicked() {
            if i == 0 {
                st.grid = !st.grid;
            } else {
                st.outlines = !st.outlines;
            }
        }
    }
    y = g.bottom() + 14.0;
    // a cor e a grossura da grade
    let g = grupo(&p, y, 120.0);
    text(&p, Pos2::new(g.center().x, g.top() + 14.0 + 8.0), Align2::CENTER_CENTER, "Cor da Linha da Grade", inter(13.0, 700), INK, 0.0);
    let sw = Rect::from_min_size(Pos2::new(g.left() + 15.0, g.top() + 14.0 + 22.0), Vec2::splat(30.0));
    let cor = st.visual.grade_cor;
    let sr = ui.interact(sw, ui.id().with("lp-grade-cor"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
    p.rect_filled(sw, 2.0, cor.unwrap_or(BG));
    p.rect_stroke(sw, 2.0, Stroke::new(1.0_f32, LINE_HI), StrokeKind::Inside);
    let val = match cor {
        Some(c) => format!("#{:02x}{:02x}{:02x}", c.r(), c.g(), c.b()),
        None => "rgba(26,28,44,.08)".into(),
    };
    text(&p, Pos2::new(sw.right() + 10.0, sw.center().y), Align2::LEFT_CENTER, &val, FontId::monospace(13.0), DIM, 0.0);
    if sr.clicked() {
        st.grade_picker = !st.grade_picker;
    }
    if st.grade_picker {
        let mut c = cor.unwrap_or(BG);
        let area = egui::Area::new(ui.id().with("lp-grade-picker")).order(Order::Tooltip).fixed_pos(sw.left_bottom() + Vec2::new(0.0, 6.0)).show(ui.ctx(), |ui| {
            egui::Frame::new().fill(BG).stroke(Stroke::new(BW, LINE)).inner_margin(10.0).show(ui, |ui| {
                ui.spacing_mut().item_spacing = Vec2::splat(6.0);
                egui::color_picker::color_picker_color32(ui, &mut c, egui::color_picker::Alpha::Opaque)
            })
        });
        if area.inner.inner {
            st.visual.grade_cor = Some(c);
        }
        if ui.input(|i| i.pointer.any_pressed()) && !area.response.hovered() && !sr.hovered() {
            st.grade_picker = false;
        }
    }
    let ly = sw.bottom() + 12.0;
    let mut job = LayoutJob::default();
    job.append("Grossura da Linha: ", 0.0, TextFormat { font_id: inter(13.0, 700), color: INK, ..Default::default() });
    job.append(&format!("{}px", st.visual.grade_w as u32), 0.0, TextFormat { font_id: inter(13.0, 700), color: INK, ..Default::default() });
    let lg = p.layout_job(job);
    p.galley(Pos2::new(g.center().x - lg.size().x / 2.0, ly), lg, INK);
    let fx = Rect::from_min_size(Pos2::new(g.left() + 15.0, ly + 14.0), Vec2::new(gw - 30.0, 36.0));
    let novo = faixa(ui, Rect::from_min_size(fx.min - Vec2::new(0.0, 14.0), fx.size()), "grade-w", "", st.visual.grade_w, 1.0, 4.0, 1.0, &|_| String::new());
    st.visual.grade_w = novo;
    fechou
}

fn notificacoes(ui: &mut egui::Ui, body: Rect) -> bool {
    let card = cartao(body, 460.0, 20.0 + 26.0 + 12.0 + 2.0 + 16.0 + 70.0 + 20.0 + 6.0);
    let p = ui.painter().clone();
    boxed(&p, card, BG, BW, INK);
    let fechou = cabecalho(ui, card, ic!("lp-pmenuNotif"), "Notificações");
    let lista = Rect::from_min_size(Pos2::new(card.left() + 20.0, card.top() + 20.0 + 26.0 + 12.0 + 2.0 + 16.0), Vec2::new(card.width() - 40.0, 70.0));
    boxed(&p, lista, BG, 2.0, LINE);
    text(&p, lista.center(), Align2::CENTER_CENTER, "Nenhuma notificação por enquanto.", inter(13.0, 400), DIM, 0.0);
    fechou
}

/// O Roletasso: a roda que gira 4 s e para no prêmio, um giro grátis por dia, e o "VOCÊ GANHOU!" por cima.
fn roleta(ui: &mut egui::Ui, st: &mut LpState, body: Rect, t: f64) -> bool {
    let card = cartao(body, 440.0, 640.0f32.min(body.height() - 20.0));
    let p = ui.painter().clone();
    // a sombra dura e as duas bordas (branca por dentro, escura por fora)
    p.rect_filled(card.translate(Vec2::new(10.0, 10.0)).expand(4.0), 0.0, Color32::from_black_alpha(115));
    p.rect_filled(card.expand(3.0), 0.0, BG);
    p.rect_filled(card, 0.0, BG);
    // o quadriculado que desliza
    let off = ((t / 8.0).fract() * 20.0) as f32;
    let pc = p.with_clip_rect(card.shrink(4.0));
    let mut yy = card.top() - 20.0;
    let mut row = 0;
    while yy < card.bottom() {
        let mut xx = card.left() - 20.0 + off + if row % 2 == 1 { 10.0 } else { 0.0 };
        while xx < card.right() {
            pc.rect_filled(Rect::from_min_size(Pos2::new(xx, yy), Vec2::splat(10.0)), 0.0, rgba(0x29366f, 0.5));
            xx += 20.0;
        }
        yy += 10.0;
        row += 1;
    }
    p.rect_stroke(card, 0.0, Stroke::new(4.0_f32, Color32::WHITE), StrokeKind::Inside);
    ui.ctx().request_repaint_after(std::time::Duration::from_millis(50));

    // fechar
    let x = Rect::from_min_size(Pos2::new(card.right() - 10.0 - 34.0, card.top() + 10.0), Vec2::splat(34.0));
    let xr = ui.interact(x, ui.id().with("lp-rl-x"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand).on_hover_text("fechar");
    p.rect_filled(x.expand(3.0), 0.0, BG);
    boxed(&p, x, if xr.hovered() { ui::mix(Color32::WHITE, rgb(0x3b5dc9), 0.2) } else { rgb(0x3b5dc9) }, 3.0, Color32::WHITE);
    cruz(&p, x.center(), 10.0, Color32::WHITE, 2.4);

    // o título em letras coladas, só com o contorno escuro, e a fita
    let cores = [rgb(0x41a6f6), rgb(0x73eff7), rgb(0x3b5dc9), rgb(0x94b0c2)];
    titulo(&p, Pos2::new(card.center().x, card.top() + 24.0 + 26.0), "ROLETASSO", 48.0, &cores);
    let fita = Rect::from_center_size(Pos2::new(card.center().x, card.top() + 24.0 + 52.0 + 12.0 + 14.0), Vec2::new(text_width(&p, "1 GIRO GRÁTIS POR DIA", comic(12.0), 1.2) + 32.0 + 6.0, 27.0));
    let rot = egui::emath::Rot2::from_angle(-2f32.to_radians());
    let quad = |r: Rect, c: Color32| {
        let pts: Vec<Pos2> = [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom()].iter().map(|q| fita.center() + rot * (*q - fita.center())).collect();
        p.add(Shape::convex_polygon(pts, c, Stroke::NONE));
    };
    quad(fita.expand(3.0), BG);
    quad(fita, Color32::WHITE);
    quad(fita.shrink(3.0), rgb(0x3b5dc9));
    let fg = p.layout_no_wrap("1 GIRO GRÁTIS POR DIA".into(), comic(12.0), Color32::WHITE);
    let half = fg.size() / 2.0;
    p.add(egui::epaint::TextShape::new(fita.center() - rot * half, fg, Color32::WHITE).with_angle(-2f32.to_radians()));

    // a roda
    let stage = Rect::from_center_size(Pos2::new(card.center().x, fita.bottom() + 30.0 + 150.0), Vec2::splat(300.0));
    let girando = st.roleta.giro.is_some();
    let pisca = ((t * if girando { 1.0 / 0.26 } else { 1.0 }) * 2.0) as i64 % 2 == 0;
    let moldura = if pisca { ImageSource::Bytes { uri: "bytes://lp/roleta-moldura-a.svg".into(), bytes: egui::load::Bytes::Static(include_bytes!("../../assets/livepixel/roleta-moldura-a.svg")) } } else { ImageSource::Bytes { uri: "bytes://lp/roleta-moldura-b.svg".into(), bytes: egui::load::Bytes::Static(include_bytes!("../../assets/livepixel/roleta-moldura-b.svg")) } };
    egui::Image::new(moldura).fit_to_exact_size(stage.size()).paint_at(ui, stage);
    // o giro: a curva cubic-bezier(.5,1,.89,1) de 4 s
    if let Some((de, ate, t0, premio)) = st.roleta.giro {
        let k = ((t - t0) / 4.0).clamp(0.0, 1.0) as f32;
        st.roleta.rot = de + (ate - de) * bezier(k);
        if t - t0 >= 4.1 {
            st.roleta.giro = None;
            st.roleta.rot = ate;
            st.roleta.pode = false;
            st.roleta.proximo = t + 86400.0;
            st.bonus += PREMIOS[premio];
            st.roleta.ganhou = Some((premio, t));
            st.chat.push(Msg { id: i64::MAX, global: true, user: String::new(), streamer: String::new(), text: format!("Parabéns, {} pixels extras foram depositados na sua conta!", PREMIOS[premio]) });
        }
    }
    let ang = st.roleta.rot.to_radians();
    egui::Image::new(ImageSource::Bytes { uri: "bytes://lp/roleta-roda.svg".into(), bytes: egui::load::Bytes::Static(include_bytes!("../../assets/livepixel/roleta-roda.svg")) })
        .rotate(ang, Vec2::splat(0.5))
        .fit_to_exact_size(stage.size())
        .paint_at(ui, stage);
    let rr = egui::emath::Rot2::from_angle(ang);
    for (i, (lx, ly, fs, a)) in ROTULOS.iter().enumerate() {
        let c = stage.center() + rr * (Vec2::new(*lx, *ly) - Vec2::splat(150.0));
        let label = format!("+{}", PREMIOS[i]);
        let la = a.to_radians() + ang;
        let lr = egui::emath::Rot2::from_angle(la);
        for k in 0..12 {
            let o = Vec2::angled(k as f32 / 12.0 * std::f32::consts::TAU) * 2.5;
            let g = p.layout_no_wrap(label.clone(), comic(*fs), BG);
            let half = g.size() / 2.0;
            p.add(egui::epaint::TextShape::new(c + o - lr * half, g, BG).with_angle(la));
        }
        let g = p.layout_no_wrap(label, comic(*fs), Color32::WHITE);
        let half = g.size() / 2.0;
        p.add(egui::epaint::TextShape::new(c - lr * half, g, Color32::WHITE).with_angle(la));
    }
    // o ponteiro (balança enquanto gira) e o miolo
    let pw = 300.0 * 0.14;
    let tick = if girando { ((t / if st.roleta.giro.is_some_and(|g| t - g.2 > 2.2) { 0.42 } else { 0.11 }) as i64 % 2) as f32 * -16.0 } else { 0.0 };
    let pr = Rect::from_min_size(Pos2::new(stage.center().x - pw / 2.0, stage.top() - 21.0), Vec2::new(pw, pw * 52.0 / 40.0));
    egui::Image::new(ImageSource::Bytes { uri: "bytes://lp/roleta-ponteiro.svg".into(), bytes: egui::load::Bytes::Static(include_bytes!("../../assets/livepixel/roleta-ponteiro.svg")) })
        .rotate(tick.to_radians(), Vec2::new(0.5, 0.14))
        .fit_to_exact_size(pr.size())
        .paint_at(ui, pr);
    let hub = 300.0 * 0.19 / 2.0;
    p.circle_filled(stage.center() + Vec2::new(0.0, 4.0), hub + 4.0 + 3.0, BG);
    p.circle_filled(stage.center(), hub + 4.0 + 3.0, BG);
    p.circle_filled(stage.center(), hub + 4.0, Color32::WHITE);
    p.circle_filled(stage.center(), hub, LP_4);
    estrela(&p, stage.center() + Vec2::new(0.0, 2.0), 12.0, Color32::WHITE);
    estrela(&p, stage.center(), 12.0, BAD);

    // os prêmios
    let mut chips = vec![];
    let mut total = 0.0;
    for v in PREMIOS {
        let s = format!("+{v}");
        let w = text_width(&p, &s, comic(12.0), 0.0) + 18.0 + 4.0;
        total += w;
        chips.push((s, w));
    }
    total += 7.0 * 6.0;
    let mut cx = card.center().x - total / 2.0;
    let cy = stage.bottom() + 14.0 + 8.0 + 11.0;
    for (s, w) in chips {
        let r = Rect::from_min_size(Pos2::new(cx, cy - 11.0), Vec2::new(w, 22.0));
        p.rect_filled(r.expand(2.0), 0.0, BG);
        boxed(&p, r, rgb(0x29366f), 2.0, Color32::WHITE);
        text(&p, r.center(), Align2::CENTER_CENTER, &s, comic(12.0), rgb(0x73eff7), 0.0);
        cx = r.right() + 7.0;
    }

    // o botão de arcade
    let by = cy + 11.0 + 18.0;
    let mut y = by;
    if !st.roleta.pode {
        let falta = (st.roleta.proximo - t).max(0.0) as i64;
        let s = format!("PRÓXIMO GIRO EM {:02}:{:02}:{:02}", falta / 3600, falta % 3600 / 60, falta % 60);
        let w = text_width(&p, &s, comic(12.0), 0.8) + 28.0 + 6.0;
        let r = Rect::from_center_size(Pos2::new(card.center().x, y + 14.0), Vec2::new(w, 28.0));
        boxed(&p, r, rgb(0x29366f), 3.0, rgb(0x73eff7));
        text(&p, r.center(), Align2::CENTER_CENTER, &s, comic(12.0), Color32::WHITE, 0.8);
        y = r.bottom() + 14.0;
    }
    let b = Rect::from_min_size(Pos2::new(card.left() + 22.0, y), Vec2::new(card.width() - 44.0, 62.0));
    let pode = st.roleta.pode && !girando;
    let resp = ui.interact(b, ui.id().with("lp-girar"), if pode { Sense::click() } else { Sense::hover() });
    let resp = if pode { resp.on_hover_cursor(CursorIcon::PointingHand) } else { resp };
    let fundo_ = resp.is_pointer_button_down_on() && pode;
    let bb = if fundo_ { b.translate(Vec2::new(0.0, 6.0)) } else { b };
    p.rect_filled(bb.expand(3.0).translate(Vec2::new(0.0, if fundo_ { 2.0 } else { 8.0 })), 0.0, BG);
    p.rect_filled(bb.expand(3.0), 0.0, BG);
    let face = if !pode { LINE } else if resp.hovered() { ui::mix(Color32::WHITE, rgb(0x3b5dc9), 0.12) } else { rgb(0x3b5dc9) };
    boxed(&p, bb, face, 4.0, Color32::WHITE);
    let label = if girando { "GIRANDO..." } else { "GIRAR!" };
    let fg = if pode || girando { Color32::WHITE } else { DIM };
    for (dx, dy) in [(-2.0, -2.0), (2.0, -2.0), (-2.0, 2.0), (2.0, 2.0), (0.0, 3.0)] {
        text(&p, bb.center() + Vec2::new(dx, dy), Align2::CENTER_CENTER, label, comic(26.0), BG, 2.0);
    }
    text(&p, bb.center(), Align2::CENTER_CENTER, label, comic(26.0), fg, 2.0);
    if resp.clicked() && pode {
        // o servidor sorteia; a roda gira quando a resposta chega (`girar`)
        st.roleta.pode = false;
        st.pedir_giro(ui.ctx());
    }
    let mut job = LayoutJob::default();
    let f = |c: Color32, w| TextFormat { font_id: inter(12.0, w), color: c, line_height: Some(18.0), ..Default::default() };
    job.append("o prêmio vira ", 0.0, f(DIM, 400));
    job.append("pixels bônus", 0.0, f(rgb(0x73eff7), 700));
    job.append(" e entra direto no seu saldo de pintura", 0.0, f(DIM, 400));
    job.wrap.max_width = 330.0;
    job.halign = egui::Align::Center;
    p.galley(Pos2::new(card.center().x, bb.bottom() + 8.0 + 8.0 + 14.0), p.layout_job(job), DIM);

    // o resultado, por cima
    if let Some((premio, t0)) = st.roleta.ganhou {
        let wp = ui.ctx().layer_painter(egui::LayerId::new(Order::Tooltip, ui.id().with("lp-rl-win")));
        let k = ((t - t0) / 0.24).min(1.0) as f32;
        wp.rect_filled(card, 0.0, rgba(0x0e0f16, 0.9 * k));
        // a chuva de pixels
        let cores = [rgb(0x29366f), rgb(0x3b5dc9), rgb(0x41a6f6), rgb(0x73eff7), rgb(0x94b0c2), rgb(0xf4f4f4)];
        for i in 0..36 {
            let d = 1.6 + (i % 5) as f64 * 0.35;
            let fase = ((t - t0) / d + i as f64 * 0.137).fract();
            let x = card.left() + ((i * 97) % 431) as f32;
            let y = card.top() - 14.0 + (card.height() + 28.0) * ((fase * 24.0).floor() / 24.0) as f32;
            let s = 6.0 + (i % 3) as f32 * 3.0;
            wp.rect_filled(Rect::from_min_size(Pos2::new(x, y), Vec2::splat(s)), 0.0, cores[i % 6].gamma_multiply(k));
        }
        let pop = {
            let a = ((t - t0) / 0.42).min(1.0) as f32;
            let a = (a * 6.0).floor() / 6.0;
            if a < 0.6 { 0.4 + (1.12 - 0.4) * a / 0.6 } else { 1.12 - 0.12 * (a - 0.6) / 0.4 }
        };
        let c = card.center() - Vec2::new(0.0, 40.0);
        let jackpot = premio == 6;
        let rotulo = if jackpot { "JACKPOT!!!" } else if PREMIOS[premio] >= 100 { "QUE SORTE!" } else { "VOCÊ GANHOU!" };
        let cor_rot = if jackpot { [rgb(0x3b5dc9), rgb(0x41a6f6), rgb(0x73eff7), rgb(0xf4f4f4)][((t / 0.15) as usize) % 4] } else { rgb(0x73eff7) };
        let fr = comic(26.0 * pop);
        for k2 in 0..16 {
            let o = Vec2::angled(k2 as f32 / 16.0 * std::f32::consts::TAU) * 3.0;
            text(&wp, c + Vec2::new(0.0, -70.0) * pop + o, Align2::CENTER_CENTER, rotulo, fr.clone(), BG, 2.0);
        }
        text(&wp, c + Vec2::new(0.0, -70.0) * pop, Align2::CENTER_CENTER, rotulo, fr, cor_rot, 2.0);
        let pulo = if ((t * 2.0 / 0.6) as i64) % 2 == 0 { 0.0 } else { -5.0 };
        let fa = comic(96.0 * pop);
        let amount = format!("+{}", PREMIOS[premio]);
        let ac = c + Vec2::new(0.0, pulo);
        for k2 in 0..16 {
            let o = Vec2::angled(k2 as f32 / 16.0 * std::f32::consts::TAU) * 10.0;
            text(&wp, ac + o, Align2::CENTER_CENTER, &amount, fa.clone(), Color32::WHITE, 0.0);
        }
        for k2 in 0..16 {
            let o = Vec2::angled(k2 as f32 / 16.0 * std::f32::consts::TAU) * 7.0;
            text(&wp, ac + o, Align2::CENTER_CENTER, &amount, fa.clone(), BG, 0.0);
        }
        text(&wp, ac, Align2::CENTER_CENTER, &amount, fa, rgb(0x73eff7), 0.0);
        text(&wp, c + Vec2::new(0.0, 62.0) * pop, Align2::CENTER_CENTER, "pixels bônus na sua conta", inter(13.0, 400), rgb(0xcbd5e1), 0.0);
        let pb = Rect::from_center_size(c + Vec2::new(0.0, 112.0) * pop, Vec2::new(200.0, 54.0) * pop);
        let presp = ui.interact(pb, ui.id().with("lp-pegar"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
        wp.rect_filled(pb.expand(3.0).translate(Vec2::new(0.0, 8.0)), 0.0, BG);
        wp.rect_filled(pb.expand(3.0), 0.0, BG);
        boxed(&wp, pb, if presp.hovered() { ui::mix(Color32::WHITE, ACCENT, 0.12) } else { ACCENT }, 4.0, Color32::WHITE);
        for (dx, dy) in [(-2.0, -2.0), (2.0, -2.0), (-2.0, 2.0), (2.0, 2.0)] {
            text(&wp, pb.center() + Vec2::new(dx, dy), Align2::CENTER_CENTER, "PEGAR!", comic(22.0 * pop), BG, 2.0);
        }
        text(&wp, pb.center(), Align2::CENTER_CENTER, "PEGAR!", comic(22.0 * pop), Color32::WHITE, 2.0);
        if presp.clicked() {
            st.roleta.ganhou = None;
        }
    }
    xr.clicked()
}

/// `cubic-bezier(.5,1,.89,1)`: acha o `s` da curva cujo x é `k` e devolve o y dele.
fn bezier(k: f32) -> f32 {
    let (x1, y1, x2, y2) = (0.5f32, 1.0f32, 0.89f32, 1.0f32);
    let bx = |s: f32| 3.0 * (1.0 - s).powi(2) * s * x1 + 3.0 * (1.0 - s) * s * s * x2 + s * s * s;
    let by = |s: f32| 3.0 * (1.0 - s).powi(2) * s * y1 + 3.0 * (1.0 - s) * s * s * y2 + s * s * s;
    let (mut lo, mut hi) = (0.0f32, 1.0f32);
    for _ in 0..30 {
        let mid = (lo + hi) / 2.0;
        if bx(mid) < k {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    by((lo + hi) / 2.0)
}

/// O título do Roletasso: letras coladas e inclinadas, com o contorno escuro.
fn titulo(p: &Painter, c: Pos2, s: &str, size: f32, cores: &[Color32]) {
    let f = comic(size);
    let ws: Vec<f32> = s.chars().map(|ch| text_width(p, &ch.to_string(), f.clone(), 0.0)).collect();
    let total: f32 = ws.iter().sum::<f32>() - 4.0 * (ws.len() as f32 - 1.0);
    let mut x = c.x - total / 2.0;
    for (i, ch) in s.chars().enumerate() {
        let w = ws[i];
        let ang = (((i * 37) % 9) as f32 - 4.0).to_radians();
        let cc = Pos2::new(x + w / 2.0, c.y);
        let paint = |off: Vec2, color: Color32| {
            let g = p.layout_no_wrap(ch.to_string(), f.clone(), color);
            let half = g.size() / 2.0;
            let rot = egui::emath::Rot2::from_angle(ang);
            p.add(egui::epaint::TextShape::new(cc + off - rot * half, g, color).with_angle(ang));
        };
        for k in 0..16 {
            let a = k as f32 / 16.0 * std::f32::consts::TAU;
            paint(Vec2::new(a.cos(), a.sin()) * 5.5 + Vec2::new(0.0, 1.0), LINE);
        }
        paint(Vec2::ZERO, cores[i % cores.len()]);
        x += w - 4.0;
    }
}

/// A janela do overlay: soltar, colar ou escolher a imagem, ver a prévia e usar.
fn imagem(ui: &mut egui::Ui, st: &mut LpState, body: Rect, t: f64) -> bool {
    // a imagem solta em cima da janela
    let soltos = ui.input(|i| i.raw.dropped_files.clone());
    for f in soltos {
        if let Some(path) = f.path {
            carregar(st, &path);
        } else if let Some(bytes) = f.bytes {
            match image::load_from_memory(&bytes) {
                Ok(img) => st.envio.escolhido = Some((f.name.clone(), img.to_rgba8())),
                Err(_) => st.envio.erro = Some("esse arquivo não é uma imagem que dê para abrir".into()),
            }
        }
    }
    let tem = st.envio.escolhido.is_some();
    let card = cartao(body, 460.0, 36.0 + 30.0 + 16.0 + 44.0 + 16.0 + if tem { 300.0 } else { 190.0 } + 16.0 + 54.0 + 8.0 + 28.0 + 30.0);
    let p = ui.painter().clone();
    boxed(&p, card, BG, BW, LINE);
    let cx = card.center().x;
    let mut y = card.top() + 36.0;
    text(&p, Pos2::new(cx, y + 15.0), Align2::CENTER_CENTER, "overlay de imagem", comic(25.0), INK, 0.0);
    y += 30.0 + 16.0;
    y += paragrafo(&p, Pos2::new(cx, y), "use uma imagem como guia para pintar. ela fica por cima do canvas só para você.", inter(14.0, 400), DIM, 300.0, 21.7) + 16.0;
    let drop = Rect::from_min_size(Pos2::new(card.left() + 35.0, y), Vec2::new(card.width() - 70.0, if tem { 300.0 } else { 190.0 }));
    let dr = ui.interact(drop, ui.id().with("lp-drop"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
    let sobre = ui.input(|i| !i.raw.hovered_files.is_empty());
    let (line, bg, fg) = if sobre { (ACCENT, rgba(0x41a6f6, 0.12), ACCENT) } else if dr.hovered() { (ACCENT, rgba(0x333c57, 0.22), INK) } else { (if tem { LINE } else { LINE_HI }, rgba(0x333c57, 0.22), DIM) };
    p.rect_filled(drop, 0.0, bg);
    if tem || sobre {
        p.rect_stroke(drop, 0.0, Stroke::new(BW, line), StrokeKind::Inside);
    } else {
        let r = drop.shrink(BW / 2.0);
        p.extend(Shape::dashed_line(&[r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom(), r.left_top()], Stroke::new(BW, line), 9.0, 6.0));
    }
    if let Some((nome, img)) = &st.envio.escolhido {
        let tex = st.envio_tex.get_or_insert_with(|| ui.ctx().load_texture("lp-envio", egui::ColorImage::from_rgba_unmultiplied([img.width() as usize, img.height() as usize], img.as_raw()), TextureOptions::LINEAR)).clone();
        let (iw, ih) = (img.width() as f32, img.height() as f32);
        let k = ((drop.width() - 40.0) / iw).min(200.0 / ih).min(1.0);
        let ir = Rect::from_center_size(Pos2::new(cx, drop.top() + 22.0 + 100.0), Vec2::new(iw * k, ih * k));
        // o quadriculado do transparente
        let pc = p.with_clip_rect(ir);
        let mut yy = ir.top();
        let mut r = 0;
        while yy < ir.bottom() {
            let mut xx = ir.left() + if r % 2 == 1 { 8.0 } else { 0.0 };
            p.rect_filled(Rect::from_min_max(Pos2::new(ir.left(), yy), Pos2::new(ir.right(), yy + 8.0)).intersect(ir), 0.0, Color32::WHITE);
            while xx < ir.right() {
                pc.rect_filled(Rect::from_min_size(Pos2::new(xx, yy), Vec2::splat(8.0)), 0.0, rgb(0xe2e8f0));
                xx += 16.0;
            }
            yy += 8.0;
            r += 1;
        }
        egui::Image::new((tex.id(), ir.size())).paint_at(ui, ir);
        p.rect_stroke(ir.expand(2.0), 0.0, Stroke::new(2.0_f32, LINE), StrokeKind::Inside);
        ui::clip(&p, Pos2::new(cx, ir.bottom() + 6.0 + 9.0), Align2::CENTER_TOP, nome, inter(13.0, 700), INK, 320.0);
        text(&p, Pos2::new(cx, ir.bottom() + 6.0 + 28.0 + 7.0), Align2::CENTER_CENTER, &format!("{}×{} px", img.width(), img.height()), inter(11.0, 400), LP_E, 0.0);
        text(&p, Pos2::new(cx, drop.bottom() - 16.0), Align2::CENTER_CENTER, "clique, arraste ou cole para trocar", inter(11.0, 400), LP_E, 0.0);
    } else {
        let c = drop.center() - Vec2::new(0.0, 26.0);
        ui::icon(ui, ic!("lp-stampReplaceBtn"), c, 24.0, ACCENT);
        text(&p, c + Vec2::new(0.0, 30.0), Align2::CENTER_CENTER, if sobre { "solte a imagem" } else { "arraste uma imagem aqui" }, comic(17.0), if sobre { ACCENT } else { INK }, 0.0);
        text(&p, c + Vec2::new(0.0, 54.0), Align2::CENTER_CENTER, "ou clique para escolher · ou cole com Ctrl+V", inter(13.0, 400), fg, 0.0);
        text(&p, c + Vec2::new(0.0, 74.0), Align2::CENTER_CENTER, "PNG, JPG ou WebP · até 5 MB", inter(11.0, 400), LP_E, 0.0);
    }
    if let Some(e) = &st.envio.erro {
        text(&p, Pos2::new(cx, drop.bottom() + 8.0), Align2::CENTER_CENTER, e, inter(12.0, 600), BAD, 0.0);
    }
    if dr.clicked() && st.envio.dialogo.is_none() {
        st.envio.dialogo = Some(std::thread::spawn(|| rfd::FileDialog::new().set_title("escolher imagem").add_filter("imagem", &["png", "jpg", "jpeg", "webp"]).pick_file()));
    }
    // colar com Ctrl+V: o egui entrega o que veio da área de transferência como texto; um caminho de arquivo serve
    let colado = ui.input(|i| i.events.iter().find_map(|e| if let egui::Event::Paste(s) = e { Some(s.clone()) } else { None }));
    if let Some(s) = colado {
        let path = std::path::PathBuf::from(s.trim().trim_start_matches("file://"));
        if path.is_file() {
            carregar(st, &path);
        }
    }
    y = drop.bottom() + 16.0;
    let go = Rect::from_min_size(Pos2::new(drop.left(), y), Vec2::new(drop.width(), 54.0));
    if botao(ui, go, "usar", "usar imagem", tem) {
        if let Some((_, img)) = st.envio.escolhido.take() {
            // 32 células de largura e a altura na proporção, começando onde a câmera está
            let gw = 32;
            let gh = ((img.height() as f32 / img.width() as f32) * gw as f32).round().max(1.0) as u32;
            st.selo = Some(Selo { img, x: (st.cam.0 - gw as f32 / 2.0).round(), y: (st.cam.1 - gh as f32 / 2.0).round(), gw, gh, opacidade: 0.5, pixelar: 4, travado: false, tex: None });
            st.envio_tex = None;
            st.say("imagem no canvas: arraste para posicionar e trave quando estiver no lugar", false, t);
            return true;
        }
    }
    let fechou = pular(ui, Pos2::new(cx, go.bottom() + 8.0 + 14.0), "cancelar", "cancelar");
    if fechou {
        st.envio = Envio::default();
        st.envio_tex = None;
    }
    fechou
}

fn carregar(st: &mut LpState, path: &std::path::Path) {
    let nome = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    match std::fs::metadata(path) {
        Ok(m) if m.len() > 5 * 1024 * 1024 => {
            st.envio.erro = Some("a imagem passa de 5 MB".into());
            return;
        }
        _ => {}
    }
    match image::open(path) {
        Ok(img) => {
            st.envio.escolhido = Some((nome, img.to_rgba8()));
            st.envio.erro = None;
            st.envio_tex = None;
        }
        Err(_) => st.envio.erro = Some("esse arquivo não é uma imagem que dê para abrir".into()),
    }
}

/// O diálogo de arquivo roda num fio à parte; quando ele volta, a imagem entra na janela.
fn poll_dialogo(ui: &egui::Ui, st: &mut LpState) {
    if st.envio.dialogo.as_ref().is_some_and(|h| h.is_finished()) {
        if let Some(Ok(Some(path))) = st.envio.dialogo.take().map(|h| h.join()) {
            carregar(st, &path);
        }
    }
    if st.envio.dialogo.is_some() {
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(200));
    }
}

/// O overlay no canvas: a imagem reduzida às células (com o "pixelar"), transparente, e a borda com as alças quando
/// destravada. Devolve o retângulo na tela.
pub(super) fn desenhar_selo(ui: &egui::Ui, st: &mut LpState, area: Rect) -> Option<Rect> {
    let ps = PX * st.cam.2;
    let a = st.w2s(area, st.selo.as_ref()?.x, st.selo.as_ref()?.y);
    let selo = st.selo.as_mut()?;
    let chave = (selo.gw, selo.gh, selo.pixelar);
    if selo.tex.as_ref().is_none_or(|(_, k)| *k != chave) {
        let ras = rasterizar(&selo.img, selo.gw, selo.gh, selo.pixelar);
        let tex = ui.ctx().load_texture("lp-selo", ras, TextureOptions::NEAREST);
        selo.tex = Some((tex, chave));
    }
    let r = Rect::from_min_size(a, Vec2::new(selo.gw as f32 * ps, selo.gh as f32 * ps));
    let (tex, _) = selo.tex.as_ref()?;
    egui::Image::new((tex.id(), r.size())).tint(Color32::from_white_alpha((selo.opacidade * 255.0) as u8)).paint_at(ui, r);
    if !selo.travado {
        let p = ui.painter_at(area);
        let pts = [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom(), r.left_top()];
        p.extend(Shape::dashed_line(&pts, Stroke::new(2.0_f32, ACCENT.gamma_multiply(0.9)), 4.0, 4.0));
        for c in [r.left_top(), r.right_top(), r.left_bottom(), r.right_bottom()] {
            let h = Rect::from_center_size(c, Vec2::splat(10.0));
            p.rect_filled(h, 0.0, Color32::WHITE);
            p.rect_stroke(h, 0.0, Stroke::new(2.0_f32, ACCENT), StrokeKind::Inside);
        }
    }
    Some(r)
}

/// A imagem como o jogo pinta: 1 pixel por célula, e o "pixelar" junta p×p células em blocos com a média das cores
/// (pesada pela opacidade, para o transparente não escurecer o bloco).
fn rasterizar(img: &image::RgbaImage, gw: u32, gh: u32, p: u32) -> egui::ColorImage {
    let small = image::imageops::resize(img, gw, gh, image::imageops::FilterType::Triangle);
    let mut d = small.into_raw();
    let p = p.max(1);
    if p > 1 {
        let mut by = 0;
        while by < gh {
            let mut bx = 0;
            while bx < gw {
                let (x1, y1) = ((bx + p).min(gw), (by + p).min(gh));
                let (mut r, mut g, mut b, mut a, mut n) = (0f32, 0f32, 0f32, 0f32, 0f32);
                for y in by..y1 {
                    for x in bx..x1 {
                        let i = ((y * gw + x) * 4) as usize;
                        let al = d[i + 3] as f32;
                        r += d[i] as f32 * al;
                        g += d[i + 1] as f32 * al;
                        b += d[i + 2] as f32 * al;
                        a += al;
                        n += 1.0;
                    }
                }
                for y in by..y1 {
                    for x in bx..x1 {
                        let i = ((y * gw + x) * 4) as usize;
                        if a > 0.0 {
                            d[i] = (r / a) as u8;
                            d[i + 1] = (g / a) as u8;
                            d[i + 2] = (b / a) as u8;
                        }
                        d[i + 3] = (a / n) as u8;
                    }
                }
                bx += p;
            }
            by += p;
        }
    }
    egui::ColorImage::from_rgba_unmultiplied([gw as usize, gh as usize], &d)
}

/// A denúncia de um pixel: o motivo, o texto do "outro" e enviar.
fn denuncia(ui: &mut egui::Ui, st: &mut LpState, body: Rect, at: (i32, i32), t: f64) -> bool {
    let outro = st.denuncia.motivo == 3;
    let card = cartao(body, 440.0, 36.0 + 30.0 + 8.0 + 22.0 + 16.0 + 4.0 * 44.0 + 12.0 + if outro { 78.0 } else { 0.0 } + 16.0 + 54.0 + 8.0 + 28.0 + 30.0);
    let p = ui.painter().clone();
    boxed(&p, card, BG, BW, LINE);
    let x = card.left() + 32.0;
    let w = card.width() - 64.0;
    let mut y = card.top() + 36.0;
    text(&p, Pos2::new(x, y + 15.0), Align2::LEFT_CENTER, "denunciar pixel", comic(25.0), INK, 0.0);
    y += 30.0 + 8.0;
    let quem = st.pixels.get(&at).map(|px| px.nome.clone()).unwrap_or_default();
    let onde = if quem.is_empty() { format!("pixel em x {} · y {}", at.0, at.1) } else { format!("pixel de {quem} em x {} · y {}", at.0, at.1) };
    text(&p, Pos2::new(x, y + 11.0), Align2::LEFT_CENTER, &format!("{onde}. informe o motivo para revisão pela moderação."), inter(14.0, 400), DIM, 0.0);
    y += 22.0 + 16.0;
    for (i, motivo) in ["desenho obsceno / impróprio", "conteúdo ofensivo / discurso de ódio", "vandalismo / spam", "outro motivo"].iter().enumerate() {
        let r = Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, 36.0));
        let resp = ui.interact(r, ui.id().with(("lp-motivo", i)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
        boxed(&p, r, rgba(0x1a1c2c, 0.4), 2.0, if resp.hovered() { ACCENT } else { LINE });
        let c = Pos2::new(r.left() + 12.0 + 8.0, r.center().y);
        p.circle_stroke(c, 7.0, Stroke::new(1.5_f32, if st.denuncia.motivo == i { ACCENT } else { DIM }));
        if st.denuncia.motivo == i {
            p.circle_filled(c, 7.5, ACCENT);
            p.circle_filled(c, 3.0, Color32::WHITE);
        }
        text(&p, Pos2::new(r.left() + 12.0 + 16.0 + 10.0, r.center().y), Align2::LEFT_CENTER, motivo, inter(13.0, 400), INK, 0.0);
        if resp.clicked() {
            st.denuncia.motivo = i;
        }
        y = r.bottom() + 8.0;
    }
    y += 4.0;
    if outro {
        let ta = Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, 64.0));
        let fid = ui.id().with("lp-den-texto");
        let foco = ui.memory(|m| m.has_focus(fid));
        boxed(&p, ta, BG, 2.0, if foco { ACCENT } else { LINE });
        let mut fui = ui.new_child(UiBuilder::new().max_rect(ta.shrink(10.0)));
        fui.add(egui::TextEdit::multiline(&mut st.denuncia.texto).id(fid).frame(false).hint_text(egui::RichText::new("descreva os detalhes da denúncia…").color(LP_E)).font(inter(13.0, 400)).text_color(INK).char_limit(300).margin(egui::Margin::ZERO).desired_width(ta.width() - 20.0));
        y = ta.bottom() + 14.0;
    }
    y += 16.0;
    let go = Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, 54.0));
    if botao(ui, go, "denunciar", "enviar denúncia", !outro || !st.denuncia.texto.trim().is_empty()) {
        // a denúncia pede a verificação anti-robô do site (Turnstile), que só existe no navegador: abre lá, no pixel
        net::abrir_no_navegador(&format!("{}/livepixel/?x={}&y={}&z=8", net::SITE, at.0, at.1));
        st.say("a denúncia abre no navegador, no pixel (o site pede a verificação anti-robô)", false, t);
        return true;
    }
    pular(ui, Pos2::new(card.center().x, go.bottom() + 8.0 + 14.0), "cancelar-den", "cancelar")
}

//! As abas de configuração do perfil (`#p-settings-panels` do /u/, só para o dono): Editar Conta, Preferências,
//! Segurança, Privacidade e Conexões. Cada uma é um `.settings-card` no meio da página, com as medidas do site
//! (janela de 1600×900). Salvar vai às mesmas rotas do site (`/api/me/profile`, `/api/me/preferences`, `/api/me/privacy`,
//! `/api/me/primary`, `/api/me/identities/:id`); ligar outra plataforma abre o login dela no navegador.

use super::{banner, hex, para, plat, PerfilState};
use crate::theme::*;
use crate::ui::{self, text, text_width};
use crate::ic;
#[allow(unused_imports)]
use serde_json;
use eframe::egui::{self, Align2, Color32, CursorIcon, Painter, Pos2, Rect, Sense, Stroke, UiBuilder, Vec2};

/// `--lol-win` e o fundo do selo "Ativa" (`--lol-win-soft`).
const WIN: Color32 = Color32::from_rgb(0x1a, 0x7f, 0x4b);
const WIN_SOFT: Color32 = Color32::from_rgba_premultiplied(3, 13, 8, 26);
const BIO_MAX: usize = 300;
/// As cinco cores prontas do "Cor de Destaque".
const SWATCHES: [Color32; 5] = [
    Color32::from_rgb(0xF4, 0xA6, 0x2C),
    Color32::from_rgb(0x4a, 0xde, 0x80),
    Color32::from_rgb(0x38, 0xbd, 0xf8),
    Color32::from_rgb(0xa7, 0x8b, 0xfa),
    Color32::from_rgb(0xf4, 0x72, 0xb6),
];

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tema {
    Escuro,
    Claro,
    Sistema,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Paleta {
    Preto,
    Agressivo,
    Arredondado,
}

/// O que as abas guardam enquanto a pessoa mexe (o formulário ainda não salvo, os interruptores, as contas).
pub struct Ajustes {
    bio: String,
    banner: Option<Color32>,
    picker: bool,
    /// O aviso ao lado do "Salvar" (`#set-status`) e de quando ele apareceu.
    pub(crate) status: Option<(&'static str, f64)>,
    pub tema: Tema,
    pub paleta: Paleta,
    pub destaque: Option<Color32>,
    custom: bool,
    publico: bool,
    mensagens: bool,
    historico: bool,
    pub(crate) priv_status: Option<(&'static str, f64)>,
    /// A plataforma que empresta nome e foto ao perfil, quando há mais de uma verificada.
    principal: Option<String>,
}

impl Tema {
    pub fn chave(self) -> &'static str {
        match self {
            Tema::Escuro => "dark",
            Tema::Claro => "light",
            Tema::Sistema => "system",
        }
    }
    pub fn de(s: &str) -> Option<Tema> {
        match s {
            "dark" => Some(Tema::Escuro),
            "light" => Some(Tema::Claro),
            "system" => Some(Tema::Sistema),
            _ => None,
        }
    }
}

impl Paleta {
    /// O `preset` do site: o Preto é o padrão e vai como `null`.
    pub fn chave(self) -> Option<&'static str> {
        match self {
            Paleta::Preto => None,
            Paleta::Agressivo => Some("agressivo"),
            Paleta::Arredondado => Some("arredondado"),
        }
    }
    pub fn de(s: Option<&str>) -> Paleta {
        match s {
            Some("agressivo") => Paleta::Agressivo,
            Some("arredondado") => Paleta::Arredondado,
            _ => Paleta::Preto,
        }
    }
}

pub fn cor_hex(c: Color32) -> String {
    format!("#{:02x}{:02x}{:02x}", c.r(), c.g(), c.b())
}

/// O que volta de cada coisa salva no servidor.
pub enum Envio {
    Perfil(Result<serde_json::Value, crate::net::Erro>),
    Privacidade(Result<serde_json::Value, crate::net::Erro>),
    Principal(String, Result<serde_json::Value, crate::net::Erro>),
    Desvincular(String, Result<serde_json::Value, crate::net::Erro>),
    Prefs(Result<serde_json::Value, crate::net::Erro>),
}

impl Ajustes {
    /// O formulário passa a mostrar o que está salvo na conta.
    pub(crate) fn carregar_do_servidor(&mut self, m: &super::MockFile) {
        self.bio = m.profile.bio.clone();
        self.banner = m.profile.banner_color.as_deref().map(hex);
        self.publico = m.profile.is_public;
        self.mensagens = m.profile.show_messages;
        self.historico = m.profile.show_messages_limit == 0;
        self.principal = Some(m.profile.primary.clone());
    }

    /// Aplica as preferências de aparência salvas na conta (`/api/me/preferences`).
    pub fn aplicar_prefs(&mut self, v: &serde_json::Value) {
        let p = &v["preferences"];
        if let Some(t) = p.get("theme").and_then(|t| t.as_str()).and_then(Tema::de) {
            self.tema = t;
        }
        if p.get("preset").is_some() {
            self.paleta = Paleta::de(p["preset"].as_str());
        }
        if let Some(a) = p.get("accentColor") {
            self.destaque = a.as_str().filter(|s| s.len() == 7 && s.starts_with('#')).map(hex);
        }
    }

    pub fn new(bio: &str, banner: Option<Color32>) -> Self {
        Self {
            bio: bio.to_owned(),
            banner,
            picker: false,
            status: None,
            tema: Tema::Sistema,
            paleta: Paleta::Preto,
            destaque: None,
            custom: false,
            publico: true,
            mensagens: true,
            historico: false,
            priv_status: None,
            principal: None,
        }
    }
}

/// A coluna do meio com a aba aberta (`aba` = 1..=5).
pub fn show(ui: &mut egui::Ui, st: &mut PerfilState, main: Rect, t: f64) {
    let mut child = ui.new_child(UiBuilder::new().max_rect(main));
    egui::ScrollArea::vertical().id_salt(("ajustes", st.aba)).auto_shrink(false).show(&mut child, |ui| {
        let inner = 820.0_f32.min(main.width());
        let x = main.center().x - inner / 2.0 + 24.0;
        let w = inner - 48.0;
        let h = match st.aba {
            1 => 453.3,
            2 => 513.0,
            3 => 313.0,
            4 => 334.0,
            _ => 384.0,
        };
        let (r, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 24.0 + h + 90.0), Sense::hover());
        let top = r.top() + 24.0;
        match st.aba {
            1 => account(ui, st, x, top, w, t),
            2 => preferences(ui, st, x, top, w, t),
            3 => security(ui, st, x, top, w, t),
            4 => privacy(ui, st, x, top, w, t),
            _ => connections(ui, st, x, top, w, t),
        }
    });
}

/* ---------------- peças ---------------- */

/// O `.settings-card`: o fundo, o título em Patua e a frase de baixo. Devolve onde começa o conteúdo (x).
pub(crate) fn card(p: &Painter, x: f32, top: f32, w: f32, h: f32, title: &str, desc: &str) -> f32 {
    p.rect_filled(Rect::from_min_size(Pos2::new(x, top), Vec2::new(w, h)), 20.0, CREAM_0());
    let cx = x + 25.0;
    ui::bold(p, Pos2::new(cx, top + 53.6), Align2::LEFT_CENTER, title, patua(20.0), INK(), 0.0);
    text(p, Pos2::new(cx, top + 79.1 + 10.15), Align2::LEFT_CENTER, desc, f6(13.5), SOFT(), 0.0);
    cx
}

pub(crate) fn label(p: &Painter, x: f32, y: f32, s: &str) {
    text(p, Pos2::new(x, y + 9.0), Align2::LEFT_CENTER, s, f7(13.0), INK(), 0.0);
}

/// O `.form-hint`, que quebra a linha na largura do cartão (17 px por linha).
pub(crate) fn hint(p: &Painter, x: f32, y: f32, w: f32, s: &str, color: Color32) {
    let g = para(p, s, f6(12.0), color, w, 17.0);
    p.galley(Pos2::new(x, y), g, color);
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Btn {
    /// `.btn`: o laranja do `--accent` com a letra branca.
    Cheio,
    /// `.btn--outline`.
    Contorno,
}

pub(crate) fn button(ui: &egui::Ui, r: Rect, id: impl std::hash::Hash, s: &str, kind: Btn) -> egui::Response {
    let resp = ui.interact(r, ui.id().with(("aj-btn", id)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
    let p = ui.painter();
    match kind {
        Btn::Cheio => {
            // `filter:brightness(1.06)` no mouse em cima
            let bg = if resp.hovered() { ui::mix(Color32::WHITE, ACCENT(), 0.06) } else { ACCENT() };
            p.rect_filled(r, r.height() / 2.0, bg);
            text(p, r.center(), Align2::CENTER_CENTER, s, f7(13.5), Color32::WHITE, 0.0);
        }
        Btn::Contorno => {
            p.rect_filled(r, r.height() / 2.0, if resp.hovered() { CREAM_2() } else { CREAM_0() });
            p.rect_stroke(r, r.height() / 2.0, Stroke::new(1.0_f32, HAIRLINE_2()), egui::StrokeKind::Inside);
            text(p, r.center(), Align2::CENTER_CENTER, s, f7(13.5), INK(), 0.0);
        }
    }
    resp
}

/// A largura de um `.btn` com o texto `s` (16 px de cada lado).
pub(crate) fn btn_w(p: &Painter, s: &str) -> f32 {
    text_width(p, s, f7(13.5), 0.0) + 32.0
}

/// O interruptor (`.switch` com `.slider`): 44×24, a bolinha anda 20 px em 0,2 s.
pub(crate) fn switch(ui: &egui::Ui, r: Rect, id: &str, on: bool) {
    let k = ui.ctx().animate_bool_with_time(ui.id().with(("sw", id)), on, 0.2);
    let p = ui.painter();
    let bg = ui::mix(SECONDARY(), CREAM_2(), k);
    p.rect_filled(r, 12.0, bg);
    p.rect_stroke(r, 12.0, Stroke::new(1.0_f32, if on { SECONDARY() } else { HAIRLINE_2() }), egui::StrokeKind::Inside);
    let knob = ui::mix(ON_SECONDARY(), SOFT(), k);
    p.circle_filled(Pos2::new(r.left() + 1.0 + 2.0 + 9.0 + 20.0 * k, r.center().y), 9.0, knob);
}

/// O aviso que some sozinho depois de 4 s (`say` do /u/).
pub(crate) fn fresh(s: &Option<(&'static str, f64)>, t: f64, ctx: &egui::Context) -> Option<&'static str> {
    let (msg, at) = (*s)?;
    if t - at > 4.0 {
        return None;
    }
    ctx.request_repaint_after(std::time::Duration::from_secs_f64(4.0 - (t - at) + 0.05));
    Some(msg)
}

/* ---------------- Editar Conta ---------------- */

fn account(ui: &mut egui::Ui, st: &mut PerfilState, x: f32, top: f32, w: f32, t: f64) {
    let fw = w - 280.0 - 20.0;
    let p = ui.painter().clone();
    let cx = card(&p, x, top, fw, 453.3, "Informações do Perfil", "Altere como seu perfil é exibido publicamente na comunidade.");
    let cw = fw - 50.0;
    let aj = &mut st.aj;

    // a cor do banner: a caixa do `<input type=color>` e o "Usar o padrão"
    label(&p, cx, top + 119.3, "Cor do Banner");
    let boxr = Rect::from_min_size(Pos2::new(cx, top + 143.3), Vec2::new(84.0, 42.0));
    let resp = ui.interact(boxr, ui.id().with("aj-banner"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
    p.rect_filled(boxr, R_MD(), CREAM_0());
    p.rect_stroke(boxr, R_MD(), Stroke::new(1.0_f32, if aj.picker { SECONDARY() } else { HAIRLINE_2() }), egui::StrokeKind::Inside);
    let sw = Rect::from_min_max(boxr.min + Vec2::new(9.0, 8.0), boxr.max - Vec2::new(9.0, 8.0));
    p.rect_filled(sw, 0.0, aj.banner.unwrap_or(Color32::BLACK));
    p.rect_stroke(sw, 0.0, Stroke::new(1.0_f32, Color32::from_gray(0xE3)), egui::StrokeKind::Inside);
    if resp.clicked() {
        aj.picker = !aj.picker;
    }
    let reset = Rect::from_min_size(Pos2::new(cx + 100.0, top + 148.3), Vec2::new(114.6, 32.0));
    let rr = ui.interact(reset, ui.id().with("aj-reset"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
    p.rect_filled(reset, 16.0, if rr.hovered() { CREAM_2() } else { CREAM_0() });
    text(&p, reset.center(), Align2::CENTER_CENTER, "Usar o padrão", f6(13.0), INK(), 0.0);
    if rr.clicked() {
        aj.banner = None;
        aj.picker = false;
    }
    hint(&p, cx, top + 189.3, cw, "Uma cor sólida no lugar do degradê. \"Usar o padrão\" volta ao degradê do tema.", FAINT());

    // a biografia: a caixa, o texto e o contador
    label(&p, cx, top + 243.3, "Biografia");
    let field = Rect::from_min_size(Pos2::new(cx, top + 267.3), Vec2::new(cw, 79.0));
    let fid = ui.id().with("aj-bio");
    let focused = ui.memory(|m| m.has_focus(fid));
    p.rect_filled(field, R_MD(), CREAM_0());
    p.rect_stroke(field, R_MD(), Stroke::new(1.0_f32, if focused { SECONDARY() } else { HAIRLINE_2() }), egui::StrokeKind::Inside);
    let mut fui = ui.new_child(UiBuilder::new().max_rect(Rect::from_min_max(field.min + Vec2::new(15.0, 11.0), field.max - Vec2::new(15.0, 11.0))));
    fui.add(
        egui::TextEdit::multiline(&mut aj.bio)
            .id(fid)
            .frame(false)
            .hint_text(egui::RichText::new("Escreva um pouco sobre você...").color(FAINT()))
            .font(f6(14.0))
            .text_color(INK())
            .char_limit(BIO_MAX)
            .margin(egui::Margin::ZERO)
            .desired_rows(3)
            .desired_width(field.width() - 30.0),
    );
    text(&p, Pos2::new(cx, top + 355.3 + 8.5), Align2::LEFT_CENTER, &format!("{}/{BIO_MAX} caracteres.", aj.bio.chars().count()), f6(12.0), FAINT(), 0.0);

    // Salvar: o perfil (o cartão do "Meu Perfil") passa a mostrar a bio e a cor novas
    let save = Rect::from_min_size(Pos2::new(cx, top + 392.3), Vec2::new(btn_w(&p, "Salvar Alterações"), 36.0));
    if button(ui, save, "salvar", "Salvar Alterações", Btn::Cheio).clicked() {
        let bio = st.aj.bio.trim().to_owned();
        let cor = st.aj.banner.map(cor_hex);
        st.m.profile.bio = bio.clone();
        st.m.profile.banner_color = cor.clone();
        let corpo = serde_json::json!({ "bio": bio, "bannerColor": cor.unwrap_or_default() });
        st.enviar(ui.ctx(), move || Envio::Perfil(crate::net::pedir_ja("PATCH", "/api/me/profile", Some(&corpo))));
    }
    if let Some(msg) = fresh(&st.aj.status, t, ui.ctx()) {
        text(&p, Pos2::new(save.right() + 12.0, save.center().y), Align2::LEFT_CENTER, msg, f7(12.0), WIN, 0.0);
    }

    preview(ui, st, x + fw + 20.0, top);

    // o seletor de cor, flutuando embaixo da caixa (o que o navegador abre para o `<input type=color>`)
    if st.aj.picker {
        let mut c = st.aj.banner.unwrap_or(Color32::from_rgb(0xe3, 0xe5, 0xe8));
        let area = egui::Area::new(ui.id().with("aj-picker")).order(egui::Order::Foreground).fixed_pos(boxr.left_bottom() + Vec2::new(0.0, 6.0)).show(ui.ctx(), |ui| {
            egui::Frame::new().fill(CREAM_2()).corner_radius(R_MD()).inner_margin(12.0).show(ui, |ui| {
                ui.spacing_mut().item_spacing = Vec2::splat(6.0);
                egui::color_picker::color_picker_color32(ui, &mut c, egui::color_picker::Alpha::Opaque)
            })
        });
        if area.inner.inner {
            st.aj.banner = Some(c);
        }
        // clicar fora fecha
        if ui.input(|i| i.pointer.any_pressed()) && !area.response.hovered() && !resp.hovered() && !resp.clicked() {
            st.aj.picker = false;
        }
    }
}

/// A prévia em tempo real (`.edit-preview-card`): o cartão pequeno com o que está no formulário, salvo ou não.
fn preview(ui: &egui::Ui, st: &PerfilState, x: f32, top: f32) {
    let p = ui.painter();
    let bio = st.aj.bio.trim();
    let bio = if bio.is_empty() { "Sua biografia aparecerá aqui..." } else { bio };
    let g = para(p, bio, f6(12.5), SOFT(), 188.0, 17.5);
    let ph = 1.0 + 64.0 + 36.0 + 8.0 + 21.0 + 6.0 + g.size().y + 24.0 + 1.0;
    let outer = Rect::from_min_size(Pos2::new(x, top), Vec2::new(280.0, 21.0 + 15.0 + 12.0 + ph + 21.0));
    p.rect_filled(outer, 20.0, CREAM_0());
    ui::icon(ui, ic!("pf-eye"), Pos2::new(x + 21.0 + 7.0, top + 21.0 + 7.5), 14.0, SECONDARY());
    text(p, Pos2::new(x + 21.0 + 20.0, top + 21.0 + 7.5), Align2::LEFT_CENTER, "PRÉVIA EM TEMPO REAL", f8(11.0), FAINT(), 0.55);

    let pc = Rect::from_min_size(Pos2::new(x + 21.0, top + 48.0), Vec2::new(238.0, ph));
    p.rect_filled(pc, 20.0, CREAM_0());
    let cover = Rect::from_min_size(pc.min + Vec2::splat(1.0), Vec2::new(236.0, 64.0));
    banner(p, cover, pc, 20.0, CREAM_0(), st.aj.banner);
    p.rect_stroke(pc, 20.0, Stroke::new(1.0_f32, HAIRLINE_2()), egui::StrokeKind::Inside);
    let av = Rect::from_min_size(Pos2::new(pc.left() + 25.0, cover.bottom() - 36.0), Vec2::splat(72.0));
    p.circle_filled(av.center(), 36.0, CREAM_0());
    match crate::net::imagem(&st.m.me.avatar) {
        Some(src) => ui::image(ui, src, av.shrink(3.0), 33.0),
        None => ui::avatar(ui, &st.m.me.display_name, None, av.center(), av.width() - 6.0, false, None),
    }
    let ny = av.bottom() + 8.0 + 10.5;
    let name = ui::bold(p, Pos2::new(av.left(), ny), Align2::LEFT_CENTER, &st.m.me.display_name, patua(18.0), INK(), 0.0);
    let mut nx = name.right() + 8.0;
    for role in &st.m.profile.roles {
        let tw = text_width(p, &role.name, patua(11.0), 0.0) + 18.0;
        let tag = Rect::from_min_size(Pos2::new(nx, ny - 10.0), Vec2::new(tw, 20.0));
        p.rect_filled(tag.expand(1.0), 11.0, Color32::from_white_alpha(61));
        p.rect_filled(tag, 10.0, hex(&role.color));
        text(p, tag.center() + Vec2::new(0.0, 0.5), Align2::CENTER_CENTER, &role.name, patua(11.0), Color32::WHITE, 0.0);
        nx = tag.right() + 6.0;
    }
    p.galley(Pos2::new(av.left(), ny + 10.5 + 6.0), g, SOFT());
}

/* ---------------- Preferências ---------------- */

/// As preferências (tema, paleta, cor de destaque) num cartão em `x`, `top`: o perfil e as configurações do app usam.
pub(crate) fn preferencias(ui: &mut egui::Ui, st: &mut PerfilState, x: f32, top: f32, w: f32, t: f64) {
    preferences(ui, st, x, top, w, t);
}

fn preferences(ui: &mut egui::Ui, st: &mut PerfilState, x: f32, top: f32, w: f32, t: f64) {
    let p = ui.painter().clone();
    let cx = card(&p, x, top, w, 513.0, "Preferências do Site", "Personalize sua experiência no Brittico.");
    let aj = &mut st.aj;

    label(&p, cx, top + 119.3, "Tema da Interface");
    let mut bx = cx;
    for (tema, nome, icon) in [(Tema::Escuro, "Escuro", ic!("pf-theme-dark")), (Tema::Claro, "Claro", ic!("pf-theme-light")), (Tema::Sistema, "Sistema", ic!("pf-theme-system"))] {
        let bw = 16.0 + 15.0 + 8.0 + text_width(&p, nome, f6(13.0), 0.0) + 16.0 + 2.0;
        let r = Rect::from_min_size(Pos2::new(bx, top + 143.3), Vec2::new(bw, 36.0));
        let resp = ui.interact(r, ui.id().with(("tema", nome)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
        let hot = aj.tema == tema || resp.hovered();
        let (bg, fg) = if hot { (SECONDARY(), ON_SECONDARY()) } else { (CREAM_2(), SOFT()) };
        p.rect_filled(r, 18.0, bg);
        if !hot {
            p.rect_stroke(r, 18.0, Stroke::new(1.0_f32, HAIRLINE_2()), egui::StrokeKind::Inside);
        }
        ui::icon(ui, icon, Pos2::new(r.left() + 17.0 + 7.5, r.center().y), 15.0, fg);
        text(&p, Pos2::new(r.left() + 17.0 + 15.0 + 8.0, r.center().y), Align2::LEFT_CENTER, nome, f6(13.0), fg, 0.0);
        if resp.clicked() {
            aj.tema = tema;
        }
        bx = r.right() + 8.0;
    }

    // as três paletas, cada uma com a miniatura do site nas cores dela
    label(&p, cx, top + 199.3, "Paleta Global do Site");
    struct Mini {
        paleta: Paleta,
        nome: &'static str,
        bg: Color32,
        bar: Color32,
        dot: Color32,
        l1: Color32,
        l2: Color32,
        round: bool,
    }
    let rgb = Color32::from_rgb;
    let minis = [
        Mini { paleta: Paleta::Preto, nome: "Preto", bg: rgb(0, 0, 0), bar: rgb(0x0D, 0x0E, 0x10), dot: rgb(0x71, 0x76, 0x7B), l1: rgb(0xF4, 0x92, 0x00), l2: rgb(0xFF, 0x7A, 0x1A), round: true },
        Mini { paleta: Paleta::Agressivo, nome: "Agressivo", bg: rgb(0x17, 0x17, 0x16), bar: rgb(0x27, 0x23, 0x1f), dot: rgb(0xa3, 0x9c, 0x89), l1: rgb(0xf4, 0x92, 0x00), l2: rgb(0x8c, 0x62, 0x57), round: false },
        Mini { paleta: Paleta::Arredondado, nome: "Arredondado", bg: rgb(0xFD, 0xF5, 0xE7), bar: rgb(0xF8, 0xEE, 0xDC), dot: rgb(0x6E, 0x5D, 0x46), l1: rgb(0xF4, 0xA6, 0x2C), l2: rgb(0xD3, 0x00, 0x00), round: true },
    ];
    for (i, m) in minis.iter().enumerate() {
        let r = Rect::from_min_size(Pos2::new(cx + i as f32 * (131.6 + 16.0), top + 229.3), Vec2::new(131.6, 128.0));
        let resp = ui.interact(r, ui.id().with(("paleta", i)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
        let on = aj.paleta == m.paleta;
        // o `translateY(-2px)` do mouse em cima
        let r = if resp.hovered() && !on { r.translate(Vec2::new(0.0, -2.0)) } else { r };
        if on {
            p.rect_filled(r, R_MD(), AMBER_SOFT());
            p.rect_stroke(r.expand(1.0), R_MD() + 1.0, Stroke::new(3.0_f32, SECONDARY()), egui::StrokeKind::Inside);
        } else {
            p.rect_filled(r, R_MD(), CREAM_0());
            if resp.hovered() {
                p.rect_stroke(r, R_MD(), Stroke::new(2.0_f32, FAINT()), egui::StrokeKind::Inside);
            }
        }
        let pv = Rect::from_min_size(r.min + Vec2::splat(10.0), Vec2::new(111.6, 74.0));
        let rad = if m.round { 12.0 } else { 0.0 };
        p.rect_filled(pv, rad, m.bg);
        let bar = Rect::from_min_size(pv.min, Vec2::new(pv.width(), 16.0));
        p.rect_filled(bar, egui::CornerRadius { nw: rad as u8, ne: rad as u8, sw: 0, se: 0 }, m.bar);
        for k in 0..2 {
            let c = Pos2::new(pv.left() + 1.0 + 6.0 + 3.0 + k as f32 * 9.0, bar.center().y);
            if m.round {
                p.circle_filled(c, 3.0, m.dot.gamma_multiply(0.4));
            } else {
                p.rect_filled(Rect::from_center_size(c, Vec2::splat(6.0)), 0.0, m.dot.gamma_multiply(0.4));
            }
        }
        let lr = if m.round { 3.0 } else { 0.0 };
        p.rect_filled(Rect::from_min_size(pv.min + Vec2::new(9.0, 23.0), Vec2::new((pv.width() - 18.0) * 0.6, 6.0)), lr, m.l1.gamma_multiply(0.8));
        p.rect_filled(Rect::from_min_size(pv.min + Vec2::new(9.0, 35.0), Vec2::new((pv.width() - 18.0) * 0.4, 6.0)), lr, m.l2.gamma_multiply(0.8));
        p.rect_stroke(pv, rad, Stroke::new(1.0_f32, HAIRLINE_2()), egui::StrokeKind::Inside);
        text(&p, Pos2::new(r.center().x, r.top() + 96.0 + 9.0), Align2::CENTER_CENTER, m.nome, f7(13.0), INK(), 0.0);
        if resp.clicked() {
            aj.paleta = m.paleta;
        }
    }

    // a cor de destaque: as cinco bolinhas, o "Personalizada" e o "Restaurar padrão"
    label(&p, cx, top + 377.3, "Cor de Destaque");
    for (i, c) in SWATCHES.iter().enumerate() {
        let ctr = Pos2::new(cx + 16.0 + i as f32 * 40.0, top + 407.3 + 16.0);
        let hit = Rect::from_center_size(ctr, Vec2::splat(32.0));
        let resp = ui.interact(hit, ui.id().with(("swatch", i)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
        let on = aj.destaque == Some(*c) && !aj.custom;
        let rad = if on || resp.hovered() { 17.6 } else { 16.0 };
        p.circle_filled(ctr, rad, *c);
        if on {
            p.circle_stroke(ctr, rad - 1.0, Stroke::new(2.0_f32, INK()));
        }
        if resp.clicked() {
            aj.destaque = Some(*c);
            aj.custom = false;
        }
    }
    let cb = Rect::from_min_size(Pos2::new(cx + 208.0, top + 407.3), Vec2::new(135.8, 32.0));
    let resp = ui.interact(cb, ui.id().with("aj-custom"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
    p.rect_filled(cb, 16.0, if resp.hovered() || aj.custom { CREAM_2() } else { CREAM_0() });
    if aj.custom {
        p.rect_stroke(cb, 16.0, Stroke::new(2.0_f32, aj.destaque.unwrap_or(SECONDARY())), egui::StrokeKind::Inside);
    }
    ui::icon(ui, ic!("pf-custom"), Pos2::new(cb.left() + 13.0 + 8.0, cb.center().y), 16.0, INK());
    text(&p, Pos2::new(cb.left() + 13.0 + 16.0 + 6.0, cb.center().y), Align2::LEFT_CENTER, "Personalizada", f6(13.0), INK(), 0.0);
    if resp.clicked() {
        aj.custom = !aj.custom;
        if aj.destaque.is_none() {
            aj.destaque = Some(SECONDARY());
        }
    }
    if aj.destaque.is_some() {
        let rr = Rect::from_min_size(Pos2::new(cb.right() + 8.0, cb.top()), Vec2::new(text_width(&p, "Restaurar padrão", f6(13.0), 0.0), 32.0));
        let r2 = ui.interact(rr, ui.id().with("aj-clear"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
        text(&p, rr.left_center(), Align2::LEFT_CENTER, "Restaurar padrão", f6(13.0), if r2.hovered() { INK() } else { FAINT() }, 0.0);
        if r2.clicked() {
            aj.destaque = None;
            aj.custom = false;
        }
    }
    hint(&p, cx, top + 459.3, w - 50.0, "Tema, paleta e cor de destaque ficam salvos na sua conta e seguem você em qualquer navegador.", FAINT());

    if aj.custom {
        let mut c = aj.destaque.unwrap_or(SECONDARY());
        let area = egui::Area::new(ui.id().with("aj-custom-picker")).order(egui::Order::Foreground).fixed_pos(cb.left_bottom() + Vec2::new(0.0, 6.0)).show(ui.ctx(), |ui| {
            egui::Frame::new().fill(CREAM_2()).corner_radius(R_MD()).inner_margin(12.0).show(ui, |ui| {
                ui.spacing_mut().item_spacing = Vec2::splat(6.0);
                egui::color_picker::color_picker_color32(ui, &mut c, egui::color_picker::Alpha::Opaque)
            })
        });
        if area.inner.inner {
            aj.destaque = Some(c);
        }
        if ui.input(|i| i.pointer.any_pressed()) && !area.response.hovered() && !resp.hovered() {
            aj.custom = false;
        }
    }
    let _ = t;
}

/* ---------------- Segurança ---------------- */

fn security(ui: &mut egui::Ui, st: &mut PerfilState, x: f32, top: f32, w: f32, t: f64) {
    let p = ui.painter().clone();
    let cx = card(&p, x, top, w, 313.0, "Segurança da Conta", "Com que conta você entrou, e como sair.");
    let cw = w - 50.0;
    label(&p, cx, top + 119.3, "Sessão atual");
    let it = Rect::from_min_size(Pos2::new(cx, top + 143.3), Vec2::new(cw, 50.0));
    p.rect_filled(it, R_MD(), CREAM_2());
    let plat_name = plat(&st.m.me.platform).2;
    text(&p, Pos2::new(it.left() + 15.0, it.center().y), Align2::LEFT_CENTER, &format!("{} · {plat_name}", st.m.me.display_name), f8(15.5), INK(), 0.0);
    let bw = text_width(&p, "Ativa", f7(11.0), 0.0) + 16.0;
    let badge = Rect::from_min_size(Pos2::new(it.right() - 15.0 - bw, it.center().y - 10.5), Vec2::new(bw, 21.0));
    p.rect_filled(badge, 10.5, WIN_SOFT);
    text(&p, badge.center(), Align2::CENTER_CENTER, "Ativa", f7(11.0), WIN, 0.0);
    hint(
        &p,
        cx,
        top + 197.3,
        cw,
        "A sessão é assinada pelo servidor e mora só neste computador — não existe uma lista de dispositivos para revogar de longe. Sair apaga a sessão daqui.",
        FAINT(),
    );
    let out = Rect::from_min_size(Pos2::new(cx, top + 251.3), Vec2::new(btn_w(&p, "Sair desta conta") - 2.0, 38.0));
    if button(ui, out, "sair", "Sair desta conta", Btn::Contorno).clicked() {
        st.pedir_sair = true;
    }
    let _ = t;
}

/* ---------------- Privacidade ---------------- */

fn privacy(ui: &mut egui::Ui, st: &mut PerfilState, x: f32, top: f32, w: f32, t: f64) {
    let p = ui.painter().clone();
    let cx = card(&p, x, top, w, 334.0, "Privacidade do Perfil", "Controle quem pode ver suas estatísticas e mensagens.");
    let cw = w - 50.0;
    let rows: [(&str, &str, f32); 3] = [
        ("Perfil Público", "Permitir que outros membros da comunidade vejam sua atividade e suas publicações.", 119.3),
        ("Exibir Histórico de Mensagens", "Exibir suas mensagens enviadas na Twitch/Kick no seu perfil.", 177.3),
        (
            "Mostrar o Histórico Completo",
            "Por padrão, quem visita seu perfil vê só as 10 mensagens mais recentes. Ligue para deixar o histórico inteiro à vista de qualquer um — seu perfil aparece na busca do site.",
            235.3,
        ),
    ];
    for (i, (title, desc, dy)) in rows.iter().enumerate() {
        let g = para(&p, desc, f6(12.5), SOFT(), cw - 44.0 - 16.0, 17.0);
        let h = 19.0 + 2.0 + g.size().y;
        let row = Rect::from_min_size(Pos2::new(cx, top + dy), Vec2::new(cw, h));
        // a linha toda é o `<label>`: clicar em qualquer ponto dela vira o interruptor
        let resp = ui.interact(row, ui.id().with(("priv", i)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
        text(&p, Pos2::new(cx, row.top() + 9.5), Align2::LEFT_CENTER, title, f7(14.0), INK(), 0.0);
        p.galley(Pos2::new(cx, row.top() + 21.0), g, SOFT());
        let on = match i {
            0 => &mut st.aj.publico,
            1 => &mut st.aj.mensagens,
            _ => &mut st.aj.historico,
        };
        if resp.clicked() {
            *on = !*on;
        }
        let on = *on;
        if resp.clicked() {
            let corpo = match i {
                0 => serde_json::json!({ "isPublic": on }),
                1 => serde_json::json!({ "showMessages": on }),
                _ => serde_json::json!({ "showMessagesLimit": if on { 0 } else { 10 } }),
            };
            st.enviar(ui.ctx(), move || Envio::Privacidade(crate::net::pedir_ja("PUT", "/api/me/privacy", Some(&corpo))));
        }
        switch(ui, Rect::from_min_size(Pos2::new(row.right() - 44.0, row.center().y - 12.0), Vec2::new(44.0, 24.0)), title, on);
    }
    if let Some(msg) = fresh(&st.aj.priv_status, t, ui.ctx()) {
        text(&p, Pos2::new(cx, top + 312.0), Align2::LEFT_CENTER, msg, f7(12.0), WIN, 0.0);
    }
}

/* ---------------- Conexões ---------------- */

fn connections(ui: &mut egui::Ui, st: &mut PerfilState, x: f32, top: f32, w: f32, t: f64) {
    let p = ui.painter().clone();
    let cx = card(&p, x, top, w, 384.0, "Contas Conectadas", "Vincule suas plataformas de streaming para unificar seus pontos e estatísticas.");
    let cw = w - 50.0;
    let verified = st.m.profile.identities.iter().filter(|i| i.verified).count();
    let mut acao: Option<(&'static str, String)> = None;
    for (k, plat_key) in ["twitch", "youtube", "kick"].iter().enumerate() {
        let it = Rect::from_min_size(Pos2::new(cx, top + 119.3 + k as f32 * 83.0), Vec2::new(cw, 71.0));
        p.rect_filled(it, R_LG(), CREAM_2());
        let (icon, color, nome) = plat(plat_key);
        let ib = Rect::from_min_size(Pos2::new(it.left() + 17.0, it.center().y - 18.0), Vec2::splat(36.0));
        p.rect_filled(ib, R_MD(), CREAM_0());
        ui::icon(ui, icon, ib.center(), 22.0, color);
        let id = st.m.profile.identities.iter().find(|i| i.platform == *plat_key);
        let is_primary = id.is_some_and(|i| i.verified) && verified > 1 && st.aj.principal.as_deref() == Some(plat_key);
        let n = text(&p, Pos2::new(it.left() + 67.0, it.top() + 15.0 + 9.5), Align2::LEFT_CENTER, nome, f7(14.0), INK(), 0.0);
        if is_primary {
            let tw = text_width(&p, "PRINCIPAL", f8(10.0), 0.4) + 14.0;
            let tag = Rect::from_min_size(Pos2::new(n.right() + 6.0, n.center().y - 8.0), Vec2::new(tw, 16.0));
            p.rect_filled(tag, 8.0, SECONDARY());
            text(&p, tag.center(), Align2::CENTER_CENTER, "PRINCIPAL", f8(10.0), ON_SECONDARY(), 0.4);
        }
        let sy = it.top() + 38.0 + 8.5;
        let mut actions: Vec<(&'static str, Btn)> = vec![];
        match id {
            Some(i) if i.verified => {
                text(&p, Pos2::new(it.left() + 67.0, sy), Align2::LEFT_CENTER, &format!("Verificada como {}", i.display_name), f6(12.0), WIN, 0.0);
                if verified > 1 && !is_primary {
                    actions.push(("Tornar principal", Btn::Contorno));
                }
                if verified > 1 {
                    actions.push(("Desvincular", Btn::Contorno));
                }
            }
            Some(i) => {
                text(&p, Pos2::new(it.left() + 67.0, sy), Align2::LEFT_CENTER, &format!("Vista no chat como {} — não verificada", i.display_name.to_lowercase()), f6(12.0), FAINT(), 0.0);
                if *plat_key != "kick" {
                    actions.push(("Verificar", Btn::Cheio));
                }
            }
            None => {
                text(&p, Pos2::new(it.left() + 67.0, sy), Align2::LEFT_CENTER, "Não conectada", f6(12.0), FAINT(), 0.0);
                if *plat_key != "kick" {
                    actions.push(("Conectar", Btn::Cheio));
                }
            }
        }
        if *plat_key == "kick" && !id.is_some_and(|i| i.verified) {
            text(&p, Pos2::new(it.right() - 17.0, it.center().y), Align2::RIGHT_CENTER, "em breve", f7(11.0), FAINT(), 0.0);
        }
        let mut bx = it.right() - 17.0;
        for (label, kind) in actions.iter().rev() {
            let bw = btn_w(&p, label) + 2.0;
            let r = Rect::from_min_size(Pos2::new(bx - bw, it.center().y - 19.0), Vec2::new(bw, 38.0));
            if button(ui, r, (plat_key, label), label, *kind).clicked() {
                acao = Some((label, plat_key.to_string()));
            }
            bx = r.left() - 8.0;
        }
    }
    match acao {
        Some(("Tornar principal", k)) => {
            let corpo = serde_json::json!({ "platform": k });
            st.enviar(ui.ctx(), move || Envio::Principal(k.clone(), crate::net::pedir_ja("PUT", "/api/me/primary", Some(&corpo))));
        }
        Some(("Desvincular", k)) => match st.m.profile.identities.iter().find(|i| i.platform == k).map(|i| i.id) {
            Some(id) if id > 0 => st.enviar(ui.ctx(), move || Envio::Desvincular(k.clone(), crate::net::pedir_ja("DELETE", &format!("/api/me/identities/{id}"), None))),
            _ => st.toast.say("Recarregue o perfil e tente de novo.", t),
        },
        Some((_, k)) => {
            // ligar outra plataforma: o login dela no navegador; ao voltar, a página do perfil mostra a conta nova
            let volta = format!("{}/u/", crate::net::SITE);
            crate::net::abrir_no_navegador(&format!("{}/api/link/{k}/login?return={}", crate::net::api(), crate::net::enc(&volta)));
            st.toast.say(&format!("Termine de ligar a {} no navegador; o perfil atualiza quando você voltar.", plat(&k).2), t);
            st.religar_em = Some(t);
        }
        None => {}
    }
}

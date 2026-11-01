//! O perfil (`public/u/index.html`), do jeito que o dono vê o próprio: três colunas.
//!
//! O menu de abas à esquerda (`.chlist`: Meu Perfil e as configurações da conta) · o perfil no meio (o cartão com capa,
//! foto, nome, cargo e plataformas; os números; a grade de atividade de um ano; as publicações no feed) · as últimas
//! mensagens do chat à direita (`.members`). As abas de configuração moram em `perfil/ajustes.rs`.
//!
//! Os dados são o `profile` de `assets/mock/home.json`, o mesmo que `tools/site-mock.cjs` entrega ao /u/ do site.

pub mod ajustes;

use crate::theme::*;
use crate::ui::{self, mix, text, text_width};
use crate::ic;
use chrono::{Datelike, Local, NaiveDate, NaiveDateTime, TimeZone};
use eframe::egui::{self, text::LayoutJob, Align, Align2, Color32, CursorIcon, ImageSource, Painter, Pos2, Rect, Sense, Stroke, TextFormat, UiBuilder, Vec2};
use serde::Deserialize;
use std::sync::Arc;

const CHLIST_W: f32 = 248.0;
const MEMBERS_W: f32 = 270.0;
/// `.main__inner` do /u/: até 820 px, 24 de folga em volta.
const INNER_MAX: f32 = 820.0;
const PAD: f32 = 24.0;
/// As 53 semanas da grade (um ano móvel).
const WEEKS: i64 = 53;
/// O verde dos pontos positivos (`--lol-win`) e do selo "reivindicado" no escuro.
const WIN: Color32 = Color32::from_rgb(0x1a, 0x7f, 0x4b);
const CLAIM_BG: Color32 = Color32::from_rgb(0x16, 0x30, 0x1c);
const CLAIM_FG: Color32 = Color32::from_rgb(0x7c, 0xe0, 0xa0);
const CHECK: Color32 = Color32::from_rgb(0x54, 0xc1, 0x5f);
/// Os verdes da grade no tema escuro: o /u/ usa os do GitHub mesmo no Preto (a grade da home troca pelo âmbar).
const GREENS: [Color32; 4] = [Color32::from_rgb(0x0e, 0x44, 0x29), Color32::from_rgb(0x00, 0x6d, 0x32), Color32::from_rgb(0x26, 0xa6, 0x41), Color32::from_rgb(0x39, 0xd3, 0x53)];
const MESES: [&str; 12] = ["Jan", "Fev", "Mar", "Abr", "Mai", "Jun", "Jul", "Ago", "Set", "Out", "Nov", "Dez"];
const MESES_CURTOS: [&str; 12] = ["jan.", "fev.", "mar.", "abr.", "mai.", "jun.", "jul.", "ago.", "set.", "out.", "nov.", "dez."];
const MESES_LONGOS: [&str; 12] = ["janeiro", "fevereiro", "março", "abril", "maio", "junho", "julho", "agosto", "setembro", "outubro", "novembro", "dezembro"];

/// O que a tela mostra, montado das respostas de `/api/profile/:id` (+ `/activity` e `/posts`).
#[derive(Default)]
pub(crate) struct MockFile {
    pub(crate) me: MockMe,
    pub(crate) profile: MockProfile,
    my_posts: Vec<MockPost>,
    my_posts_total: u32,
}
#[derive(Default)]
pub(crate) struct MockMe {
    pub(crate) login: String,
    pub(crate) display_name: String,
    pub(crate) platform: String,
    pub(crate) avatar: String,
    pub(crate) id: i64,
}
#[derive(Default)]
pub(crate) struct MockProfile {
    since: String,
    points_total: i64,
    points_batidas: i64,
    messages: i64,
    activity: Vec<u32>,
    activity_hidden: bool,
    pub(crate) bio: String,
    /// A cor sólida da capa (`#rrggbb`); sem ela, o degradê do tema.
    pub(crate) banner_color: Option<String>,
    roles: Vec<MockRole>,
    claimed: bool,
    pub(crate) identities: Vec<Identity>,
    stats: Vec<MockStat>,
    chat: Vec<MockChat>,
    pub(crate) is_public: bool,
    pub(crate) show_messages: bool,
    pub(crate) show_messages_limit: i64,
    pub(crate) primary: String,
    messages_hidden: bool,
}
#[derive(Deserialize, Default, Clone)]
#[serde(default)]
struct MockRole {
    name: String,
    color: String,
}
#[derive(Deserialize, Default, Clone)]
#[serde(default)]
pub(crate) struct Identity {
    pub(crate) id: i64,
    pub(crate) platform: String,
    pub(crate) username: String,
    #[serde(rename = "displayName")]
    pub(crate) display_name: String,
    pub(crate) verified: bool,
}
#[derive(Deserialize, Default)]
#[serde(default)]
struct MockStat {
    platform: String,
    count: i64,
    #[serde(rename = "firstAt")]
    first_at: Option<String>,
}
#[derive(Default)]
struct MockChat {
    platform: String,
    username: String,
    message: String,
    ago_min: i64,
}
#[derive(Default)]
struct MockPost {
    id: u32,
    title: String,
    message_text: String,
    score: i64,
    comment_count: i64,
    ago_min: i64,
    image: String,
}

/* as respostas da API */
#[derive(Deserialize, Default)]
#[serde(default)]
struct ApiPerfil {
    profile: ApiPerfilProfile,
    identities: Vec<Identity>,
    stats: Vec<MockStat>,
    #[serde(rename = "totalMessages")]
    total_messages: i64,
    messages: Vec<ApiMsg>,
    #[serde(rename = "messagesHidden")]
    messages_hidden: bool,
}
#[derive(Deserialize, Default)]
#[serde(default)]
struct ApiPerfilProfile {
    id: i64,
    #[serde(rename = "displayName")]
    display_name: String,
    avatar: Option<String>,
    #[serde(rename = "createdAt")]
    created_at: String,
    bio: Option<String>,
    #[serde(rename = "bannerColor")]
    banner_color: Option<String>,
    #[serde(rename = "primaryPlatform")]
    primary_platform: Option<String>,
    claimed: bool,
    #[serde(rename = "isPublic")]
    is_public: bool,
    #[serde(rename = "showMessages")]
    show_messages: bool,
    #[serde(rename = "showMessagesLimit")]
    show_messages_limit: Option<i64>,
    points: ApiPontos,
    roles: Vec<MockRole>,
}
#[derive(Deserialize, Default)]
#[serde(default)]
struct ApiPontos {
    total: i64,
    batidas: i64,
}
#[derive(Deserialize, Default)]
#[serde(default)]
struct ApiMsg {
    platform: String,
    username: String,
    message: String,
    timestamp: String,
}

fn post_mini(p: crate::home::ApiPost, now: i64) -> MockPost {
    MockPost {
        id: p.id,
        ago_min: (now - crate::home::parse_ts(&p.timestamp)) / 60,
        title: p.title.unwrap_or_default(),
        message_text: p.message_text.unwrap_or_default(),
        score: p.score as i64,
        comment_count: p.comment_count as i64,
        image: p.image_filename.filter(|f| !f.is_empty()).map(|f| format!("/api/feed/images/{f}")).unwrap_or_default(),
    }
}

fn arquivo_aparencia() -> std::path::PathBuf {
    crate::net::dir().join("aparencia.json")
}

fn guardar_aparencia(a: (ajustes::Tema, ajustes::Paleta, Option<Color32>)) {
    thread_local!(static ULTIMA: std::cell::Cell<Option<(ajustes::Tema, ajustes::Paleta, Option<Color32>)>> = const { std::cell::Cell::new(None) });
    if ULTIMA.with(|u| u.get()) == Some(a) {
        return;
    }
    ULTIMA.with(|u| u.set(Some(a)));
    let v = serde_json::json!({ "theme": a.0.chave(), "preset": a.1.chave(), "accentColor": a.2.map(ajustes::cor_hex) });
    let _ = std::fs::create_dir_all(crate::net::dir());
    let _ = std::fs::write(arquivo_aparencia(), v.to_string());
}

/// A aparência guardada neste computador (o app abre nela antes de falar com o servidor).
pub fn aparencia_local(aj: &mut ajustes::Ajustes) {
    if let Some(v) = std::fs::read_to_string(arquivo_aparencia()).ok().and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok()) {
        aj.aplicar_prefs(&serde_json::json!({ "preferences": v }));
    }
}

/// Busca tudo o que a página de um perfil mostra (na thread de quem chamou).
fn buscar(id: i64) -> Result<MockFile, crate::net::Erro> {
    use crate::net::pedir_ja;
    let now = Local::now().timestamp();
    let d: ApiPerfil = serde_json::from_value(pedir_ja("GET", &format!("/api/profile/{id}"), None)?).map_err(|e| crate::net::Erro { status: 0, msg: e.to_string() })?;
    let a: crate::home::ActivityResp = pedir_ja("GET", &format!("/api/profile/{id}/activity"), None).ok().and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default();
    let m: crate::home::PostsResp = pedir_ja("GET", &format!("/api/profile/{id}/posts?limit=10&offset=0"), None).ok().and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default();
    let pr = d.profile;
    let since = d.stats.iter().filter_map(|s| s.first_at.clone()).min().unwrap_or(pr.created_at.clone());
    let prim = pr.primary_platform.clone().or_else(|| d.identities.first().map(|i| i.platform.clone())).unwrap_or_else(|| "twitch".into());
    let login = d.identities.iter().find(|i| i.platform == prim).or(d.identities.first()).map(|i| i.username.clone()).unwrap_or_default();
    Ok(MockFile {
        me: MockMe { login, display_name: pr.display_name, platform: prim.clone(), avatar: pr.avatar.unwrap_or_default(), id: pr.id },
        profile: MockProfile {
            since: chrono::DateTime::from_timestamp(crate::home::parse_ts(&since), 0).map(|d| d.format("%Y-%m-%d %H:%M:%S").to_string()).unwrap_or_default(),
            points_total: pr.points.total,
            points_batidas: pr.points.batidas,
            messages: d.total_messages,
            activity: crate::home::activity_vec(&a),
            activity_hidden: a.hidden,
            bio: pr.bio.unwrap_or_default(),
            banner_color: pr.banner_color,
            roles: pr.roles,
            claimed: pr.claimed,
            identities: d.identities,
            stats: d.stats,
            chat: d.messages.into_iter().map(|m| MockChat { ago_min: (now - crate::home::parse_ts(&m.timestamp)) / 60, platform: m.platform, username: m.username, message: m.message }).collect(),
            is_public: pr.is_public,
            show_messages: pr.show_messages,
            show_messages_limit: pr.show_messages_limit.unwrap_or(10),
            primary: prim,
            messages_hidden: d.messages_hidden,
        },
        my_posts: m.posts.into_iter().map(|p| post_mini(p, now)).collect(),
        my_posts_total: m.total,
    })
}

/// As abas do menu da esquerda, na ordem do site, com a categoria de cada uma.
const ABAS: [(&str, &str, &str); 6] = [
    ("profile", "Meu Perfil", "Navegação"),
    ("account", "Editar Conta", "Configurações"),
    ("preferences", "Preferências", "Configurações"),
    ("security", "Segurança", "Segurança & Privacidade"),
    ("privacy", "Privacidade", "Segurança & Privacidade"),
    ("connections", "Conexões", "Integrações"),
];

pub struct PerfilState {
    pub(crate) m: MockFile,
    /// O perfil que está na tela: o de quem usa (`None`) ou o de outra pessoa.
    pub alvo: Option<i64>,
    /// Para qual (geração da conta, alvo) os dados foram buscados.
    carregado: Option<(u64, Option<i64>)>,
    carga: Option<crate::net::Tarefa<MockFile>>,
    mais: Option<crate::net::Tarefa<crate::home::PostsResp>>,
    erro: Option<String>,
    envios: Vec<crate::net::Tarefa<ajustes::Envio>>,
    /// As preferências de aparência da conta: o pedido, para qual conta, e o que foi salvo por último.
    prefs: Option<crate::net::Tarefa<serde_json::Value>>,
    prefs_de: u64,
    prefs_salvas: Option<(ajustes::Tema, ajustes::Paleta, Option<Color32>)>,
    prefs_mudou: f64,
    /// O "Sair desta conta": o `main.rs` sai.
    pub pedir_sair: bool,
    /// Quando a pessoa foi ligar outra plataforma no navegador (o perfil é buscado de novo ao voltar o foco).
    pub(crate) religar_em: Option<f64>,
    now: i64,
    since: i64,
    pub aba: usize,
    toast: ui::Toast,
    /// As abas de configuração (o formulário, as preferências, os interruptores).
    pub aj: ajustes::Ajustes,
    /// Para as capturas: abre já rolado até aqui (`BRITTICO_TELA=perfil-baixo`).
    pub rolar: Option<f32>,
}

impl Default for PerfilState {
    fn default() -> Self {
        let now = Local::now().timestamp();
        let aj = ajustes::Ajustes::new("", None);
        Self { m: MockFile::default(), alvo: None, carregado: None, carga: None, mais: None, erro: None, envios: Vec::new(), prefs: None, prefs_de: u64::MAX, prefs_salvas: None, prefs_mudou: 0.0, pedir_sair: false, religar_em: None, now, since: now, aba: 0, toast: ui::Toast::default(), aj, rolar: None }
    }
}

impl PerfilState {
    /// Abre o perfil de outra pessoa (ou o próprio, se o id for o de quem usa).
    pub fn abrir_outro(&mut self, id: i64) {
        self.alvo = if id == crate::conta::perfil_id() { None } else { Some(id) };
        self.aba = 0;
    }

    /// O perfil na tela é o de quem está usando (mostra as abas de configuração).
    pub fn dono(&self) -> bool {
        self.alvo.is_none()
    }

    /// Manda algo ao servidor em segundo plano; a volta é tratada em `passo`.
    pub(crate) fn enviar(&mut self, ctx: &egui::Context, f: impl FnOnce() -> ajustes::Envio + Send + 'static) {
        self.envios.push(crate::net::rodar(ctx, move || Ok(f())));
    }

    /// As preferências de aparência: buscadas uma vez por conta, salvas quando mudam (meio segundo depois do último
    /// toque, para o seletor de cor não mandar um pedido por quadro). Vale também para o botão de sol do topo.
    pub fn passo_prefs(&mut self, ctx: &egui::Context, geracao: u64, t: f64) {
        let logado = crate::conta::perfil_id() > 0;
        if self.prefs_de != geracao {
            self.prefs_de = geracao;
            self.prefs_salvas = None;
            if logado {
                self.prefs = Some(crate::net::get(ctx, "/api/me/preferences"));
            }
        }
        if let Some(r) = self.prefs.as_ref().and_then(|p| p.pronto()) {
            self.prefs = None;
            if let Ok(v) = r {
                self.aj.aplicar_prefs(&v);
            }
            self.prefs_salvas = Some((self.aj.tema, self.aj.paleta, self.aj.destaque));
            self.prefs_mudou = 0.0;
        }
        let agora = (self.aj.tema, self.aj.paleta, self.aj.destaque);
        if let Some(antes) = self.prefs_salvas {
            if antes != agora {
                if self.prefs_mudou == 0.0 {
                    self.prefs_mudou = t;
                }
                if t - self.prefs_mudou > 0.5 {
                    let mut corpo = serde_json::Map::new();
                    if antes.0 != agora.0 {
                        corpo.insert("theme".into(), agora.0.chave().into());
                    }
                    if antes.1 != agora.1 {
                        corpo.insert("preset".into(), agora.1.chave().map(Into::into).unwrap_or(serde_json::Value::Null));
                    }
                    if antes.2 != agora.2 {
                        corpo.insert("accentColor".into(), agora.2.map(|c| ajustes::cor_hex(c).into()).unwrap_or(serde_json::Value::Null));
                    }
                    let corpo = serde_json::Value::Object(corpo);
                    self.prefs_salvas = Some(agora);
                    self.prefs_mudou = 0.0;
                    self.enviar(ctx, move || ajustes::Envio::Prefs(crate::net::pedir_ja("PUT", "/api/me/preferences", Some(&corpo))));
                } else {
                    ctx.request_repaint_after(std::time::Duration::from_millis(550));
                }
            }
        }
        // a aparência também fica guardada neste computador, para o app abrir já no tema certo
        guardar_aparencia(agora);
    }

    /// Busca de novo (depois de salvar algo que muda a página).
    pub fn recarregar(&mut self) {
        self.carregado = None;
    }

    fn passo(&mut self, ctx: &egui::Context, geracao: u64) {
        let alvo = self.alvo;
        if self.carregado != Some((geracao, alvo)) {
            self.carregado = Some((geracao, alvo));
            let id = alvo.unwrap_or_else(crate::conta::perfil_id);
            if id > 0 {
                self.carga = Some(crate::net::rodar(ctx, move || buscar(id)));
            } else {
                self.m = MockFile::default();
            }
        }
        if let Some(r) = self.carga.as_ref().and_then(|c| c.pronto()) {
            self.carga = None;
            match r {
                Ok(m) => {
                    self.now = Local::now().timestamp();
                    self.since = NaiveDateTime::parse_from_str(&m.profile.since, "%Y-%m-%d %H:%M:%S").map(|d| d.and_utc().timestamp()).unwrap_or(self.now);
                    if self.alvo.is_none() {
                        self.aj.carregar_do_servidor(&m);
                    }
                    self.m = m;
                    self.erro = None;
                }
                Err(e) => self.erro = Some(e.msg),
            }
        }
        // voltou do navegador depois de ligar outra plataforma: busca o perfil de novo
        if let Some(desde) = self.religar_em {
            let t = ctx.input(|i| i.time);
            if t - desde > 2.0 && ctx.input(|i| i.focused) {
                self.religar_em = None;
                self.carregado = None;
            }
        }
        let mut i = 0;
        while i < self.envios.len() {
            let Some(r) = self.envios[i].pronto() else {
                i += 1;
                continue;
            };
            self.envios.remove(i);
            let t = ctx.input(|i| i.time);
            use ajustes::Envio;
            match r {
                Ok(Envio::Perfil(Ok(_))) => self.aj.status = Some(("Perfil atualizado.", t)),
                Ok(Envio::Privacidade(Ok(v))) => {
                    self.m.profile.is_public = v["isPublic"].as_bool().unwrap_or(self.m.profile.is_public);
                    self.aj.priv_status = Some(("Salvo.", t));
                }
                Ok(Envio::Principal(k, Ok(_))) => {
                    self.toast.say(&format!("O nome e a foto do perfil agora vêm da {}.", plat(&k).2), t);
                    self.carregado = None;
                }
                Ok(Envio::Desvincular(k, Ok(_))) => {
                    self.toast.say(&format!("{} desvinculada.", plat(&k).2), t);
                    self.carregado = None;
                }
                Ok(Envio::Prefs(Ok(_))) => {}
                Ok(Envio::Perfil(Err(e)) | Envio::Privacidade(Err(e)) | Envio::Principal(_, Err(e)) | Envio::Desvincular(_, Err(e)) | Envio::Prefs(Err(e))) | Err(e) => {
                    self.toast.say(&e.msg, t);
                    // o que estava na tela pode não ter pegado: busca o que está salvo
                    self.carregado = None;
                }
            }
        }
        if let Some(r) = self.mais.as_ref().and_then(|c| c.pronto()) {
            self.mais = None;
            if let Ok(p) = r {
                let now = self.now;
                self.m.my_posts.extend(p.posts.into_iter().map(|p| post_mini(p, now)));
                self.m.my_posts_total = p.total;
            }
        }
    }
}

fn tab_icon(key: &str) -> ImageSource<'static> {
    match key {
        "profile" => ic!("pf-tab-profile"),
        "account" => ic!("pf-tab-account"),
        "preferences" => ic!("pf-tab-preferences"),
        "security" => ic!("pf-tab-security"),
        "privacy" => ic!("pf-tab-privacy"),
        _ => ic!("pf-tab-connections"),
    }
}

/// O ícone e a cor de cada plataforma (o `ICON` do /u/).
fn plat(p: &str) -> (ImageSource<'static>, Color32, &'static str) {
    match p {
        "youtube" => (ic!("plat-youtube"), Color32::from_rgb(0xFF, 0x00, 0x00), "YouTube"),
        "kick" => (ic!("plat-kick"), Color32::from_rgb(0x53, 0xFC, 0x18), "Kick"),
        _ => (ic!("plat-twitch"), Color32::from_rgb(0x91, 0x46, 0xFF), "Twitch"),
    }
}

fn hex(s: &str) -> Color32 {
    let v = u32::from_str_radix(s.trim_start_matches('#'), 16).unwrap_or(0x8a857a);
    Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
}

fn num(n: i64) -> String {
    let s = n.abs().to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push('.');
        }
        out.push(c);
    }
    if n < 0 {
        format!("-{out}")
    } else {
        out
    }
}

fn local(ts: i64) -> chrono::DateTime<Local> {
    Local.timestamp_opt(ts, 0).single().unwrap_or_else(Local::now)
}

/// `toLocaleString` com dia, mês e ano de dois dígitos e a hora: "30/10/26, 20:47".
fn when(ts: i64) -> String {
    local(ts).format("%d/%m/%y, %H:%M").to_string()
}

/// Texto quebrado com a altura de linha do CSS.
fn para(p: &Painter, s: &str, font: egui::FontId, color: Color32, width: f32, line_h: f32) -> Arc<egui::Galley> {
    let mut job = LayoutJob::single_section(s.to_owned(), TextFormat { font_id: font, color, line_height: Some(line_h), valign: Align::Center, ..Default::default() });
    job.wrap.max_width = width;
    p.layout_job(job)
}

/* ================= a tela ================= */

pub fn show(ui: &mut egui::Ui, st: &mut PerfilState, body: Rect, t: f64, geracao: u64) {
    st.passo(ui.ctx(), geracao);
    if st.m.me.id == 0 {
        let msg = if let Some(e) = &st.erro { e.as_str() } else if st.carga.is_some() { "Carregando…" } else { "Conecte a conta para ver o seu perfil." };
        text(ui.painter(), body.center(), Align2::CENTER_CENTER, msg, f6(14.0), FAINT(), 0.0);
        return;
    }
    if !st.dono() {
        st.aba = 0;
    }
    let chlist = Rect::from_min_size(body.min, Vec2::new(CHLIST_W, body.height()));
    let members = Rect::from_min_max(Pos2::new(body.right() - MEMBERS_W, body.top()), body.max);
    let main = Rect::from_min_max(chlist.right_top(), members.left_bottom());
    tabs(ui, st, chlist);
    if st.aba == 0 {
        profile(ui, st, main, t);
    } else {
        ajustes::show(ui, st, main, t);
    }
    messages(ui, st, members);
    st.toast.show(ui, t);
}

/// O menu de abas (`.chlist` com `.ch-item`).
fn tabs(ui: &mut egui::Ui, st: &mut PerfilState, col: Rect) {
    let p = ui.painter().clone();
    p.rect_filled(col, 0.0, CREAM_2());
    let x = col.left() + 12.0;
    let w = col.width() - 24.0;
    let mut y = col.top() + 12.0;
    let mut cat = "";
    let n_abas = if st.dono() { ABAS.len() } else { 1 };
    for (i, (key, label, c)) in ABAS.iter().enumerate().take(n_abas) {
        let label = if i == 0 && !st.dono() { &"Perfil" } else { label };
        if *c != cat {
            cat = c;
            text(&p, Pos2::new(x + 8.0, y + 14.0 + 7.5), Align2::LEFT_CENTER, &c.to_uppercase(), f8(11.0), FAINT(), 0.66);
            y += 35.0;
        }
        let r = Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, 35.0));
        let resp = ui.interact(r, ui.id().with(("aba", i)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
        let on = st.aba == i;
        let (bg, fg, icf) = if on {
            (Some(SECONDARY()), ON_SECONDARY(), ON_SECONDARY())
        } else if resp.hovered() {
            (Some(AMBER_SOFT()), INK(), FAINT())
        } else {
            (None, SOFT(), FAINT())
        };
        if let Some(bg) = bg {
            p.rect_filled(r, R_XS(), bg);
        }
        ui::icon(ui, tab_icon(key), Pos2::new(r.left() + 10.0 + 8.0, r.center().y), 16.0, icf);
        text(&p, Pos2::new(r.left() + 36.0, r.center().y), Align2::LEFT_CENTER, label, f6(14.5), fg, 0.0);
        if resp.clicked() {
            st.aba = i;
        }
        y += 36.0;
    }
}

/// As últimas mensagens do chat (`.members` com `.msg`).
fn messages(ui: &mut egui::Ui, st: &PerfilState, col: Rect) {
    ui.painter().rect_filled(col, 0.0, CREAM_2());
    let mut child = ui.new_child(UiBuilder::new().max_rect(col));
    egui::ScrollArea::vertical().id_salt("perfil-mensagens").auto_shrink(false).show(&mut child, |ui| {
        let x = col.left() + 17.0;
        let w = col.width() - 33.0;
        ui.add_space(16.0);
        let (r, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 21.0 + 12.0), Sense::hover());
        ui::bold(ui.painter(), Pos2::new(x, r.top() + 10.5), Align2::LEFT_CENTER, "Mensagens", patua(18.0), INK(), 0.0);
        if st.m.profile.chat.is_empty() {
            let (r, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 30.0), Sense::hover());
            let msg = if st.m.profile.messages_hidden { "As mensagens deste perfil são privadas." } else { "Nenhuma mensagem no chat ainda." };
            text(ui.painter(), Pos2::new(x, r.center().y), Align2::LEFT_CENTER, msg, f6(12.5), FAINT(), 0.0);
        }
        for m in &st.m.profile.chat {
            let g = para(ui.painter(), &m.message, f6(14.0), INK(), w - 28.0 - 30.0, 20.3);
            let h = 1.0 + 11.0 + g.size().y + 3.0 + 15.0 + 11.0 + 1.0;
            let (row, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), h), Sense::hover());
            let r = Rect::from_min_size(Pos2::new(x, row.top()), Vec2::new(w, h));
            let p = ui.painter();
            p.rect_filled(r, R_MD(), CREAM_0());
            let (icon, c, _) = plat(&m.platform);
            ui::icon(ui, icon, Pos2::new(r.left() + 15.0 + 10.0, r.top() + 13.0 + 10.0), 20.0, c);
            let tx = r.left() + 15.0 + 20.0 + 10.0;
            let gh = g.size().y;
            p.galley(Pos2::new(tx, r.top() + 12.0), g, INK());
            let my = r.top() + 12.0 + gh + 3.0 + 7.5;
            let who = text(p, Pos2::new(tx, my), Align2::LEFT_CENTER, if m.username.is_empty() { &st.m.me.login } else { &m.username }, f6(11.5), SOFT(), 0.0);
            text(p, Pos2::new(who.right(), my), Align2::LEFT_CENTER, &format!(" · {}", when(st.now - m.ago_min * 60)), f6(11.5), FAINT(), 0.0);
            ui.add_space(9.0);
        }
        ui.add_space(8.0);
    });
}

/* ================= o perfil (Meu Perfil) ================= */

fn profile(ui: &mut egui::Ui, st: &mut PerfilState, main: Rect, t: f64) {
    let mut child = ui.new_child(UiBuilder::new().max_rect(main));
    let mut ver_mais = false;
    let mut area = egui::ScrollArea::vertical().id_salt("perfil").auto_shrink(false);
    if let Some(y) = st.rolar.take() {
        area = area.vertical_scroll_offset(y);
    }
    area.show(&mut child, |ui| {
        let inner = INNER_MAX.min(main.width());
        let x = main.center().x - inner / 2.0 + PAD;
        let w = inner - 2.0 * PAD;
        ui.add_space(PAD);
        card(ui, st, x, w);
        ui.add_space(20.0);
        stats(ui, st, x, w);
        ui.add_space(24.0);
        if !st.m.profile.activity_hidden {
            activity(ui, st, x, w);
            ui.add_space(24.0);
        }
        ver_mais = posts(ui, st, x, w);
        ui.add_space(90.0);
    });
    if ver_mais && st.mais.is_none() {
        let id = st.m.me.id;
        let off = st.m.my_posts.len();
        st.mais = Some(crate::net::get(ui.ctx(), format!("/api/profile/{id}/posts?limit=10&offset={off}")));
    }
    let _ = t;
}

/// Reserva uma faixa da largura toda da coluna e devolve o pedaço dela que é do conteúdo (`x`, `w`).
fn row(ui: &mut egui::Ui, x: f32, w: f32, h: f32) -> Rect {
    let (r, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), h), Sense::hover());
    Rect::from_min_size(Pos2::new(x, r.top()), Vec2::new(w, h))
}

/// O cartão (`.pcard`): a capa em degradê, a foto sobre ela, o nome com o cargo e o selo, a data, a bio e as plataformas.
fn card(ui: &mut egui::Ui, st: &PerfilState, x: f32, w: f32) {
    let pr = &st.m.profile;
    let bio = (!pr.bio.is_empty()).then(|| para(ui.painter(), &pr.bio, f6(14.0), SOFT(), (w - 48.0).min(456.0), 21.0));
    let bio_h = bio.as_ref().map(|g| 10.0 + g.size().y).unwrap_or(0.0);
    let h = 1.0 + 96.0 + 52.0 + 12.0 + 31.0 + 4.0 + 18.0 + bio_h + 16.0 + 34.0 + 24.0 + 1.0;
    let r = row(ui, x, w, h);
    let p = ui.painter().clone();
    p.rect_filled(r, 20.0, CREAM_0());

    let cover = Rect::from_min_size(r.min + Vec2::splat(1.0), Vec2::new(w - 2.0, 96.0));
    banner(&p, cover, r, 20.0, CREAM_1(), pr.banner_color.as_deref().map(hex));

    // a foto, com o anel da cor do cartão, metade sobre a capa
    let bx = r.left() + 25.0;
    let av = Rect::from_min_size(Pos2::new(bx, cover.bottom() - 52.0), Vec2::splat(104.0));
    p.circle_filled(av.center(), 52.0, CREAM_0());
    match crate::net::imagem(&st.m.me.avatar) {
        Some(src) => ui::image(ui, src, av.shrink(4.0), 48.0),
        None => ui::avatar(ui, &st.m.me.display_name, None, av.center(), 96.0, false, None),
    }

    // o nome, o cargo e o selo
    let ny = av.bottom() + 12.0 + 15.5;
    let name = ui::bold(&p, Pos2::new(bx, ny), Align2::LEFT_CENTER, &st.m.me.display_name, patua(26.0), INK(), -0.3);
    let mut nx = name.right() + 10.0;
    for role in &pr.roles {
        let tw = text_width(&p, &role.name, patua(13.0), 0.26) + 24.0;
        let tag = Rect::from_min_size(Pos2::new(nx, ny - 12.75), Vec2::new(tw, 25.5));
        p.rect_filled(tag.expand(1.0), 13.75, Color32::from_white_alpha(61));
        p.rect_filled(tag, 12.75, hex(&role.color));
        ui::bold(&p, tag.center() + Vec2::new(-0.2, 0.5), Align2::CENTER_CENTER, &role.name, patua(13.0), Color32::WHITE, 0.26);
        nx = tag.right() + 10.0;
    }
    if pr.claimed {
        let label = "REIVINDICADO";
        let tw = 11.0 + 13.0 + 5.0 + text_width(&p, label, f8(11.0), 0.44) + 11.0;
        let tag = Rect::from_min_size(Pos2::new(nx, ny - 12.0), Vec2::new(tw, 24.0));
        // `.claim-badge`: verde-claro no claro, verde-escuro no escuro
        let (bg, fg) = if DARK() { (CLAIM_BG, CLAIM_FG) } else { (Color32::from_rgb(0xe7, 0xf6, 0xec), Color32::from_rgb(0x1a, 0x7f, 0x37)) };
        p.rect_filled(tag, 12.0, bg);
        ui::icon(ui, ic!("plat-check"), Pos2::new(tag.left() + 11.0 + 6.5, tag.center().y), 13.0, fg);
        text(&p, Pos2::new(tag.left() + 29.0, tag.center().y + 0.5), Align2::LEFT_CENTER, label, f8(11.0), fg, 0.44);
    }

    // desde quando, a bio
    let my = ny + 15.5 + 4.0 + 9.0;
    let d = local(st.since);
    text(&p, Pos2::new(bx, my), Align2::LEFT_CENTER, &format!("Na comunidade desde {:02} de {} de {}", d.day(), MESES_CURTOS[d.month0() as usize], d.year()), f6(13.0), FAINT(), 0.0);
    let mut y = my + 9.0;
    if let Some(g) = bio {
        y += 10.0;
        let gh = g.size().y;
        p.galley(Pos2::new(bx, y), g, SOFT());
        y += gh;
    }

    // as contas de cada plataforma
    y += 16.0;
    let mut px = bx;
    for id in &pr.identities {
        let (icon, c, nome) = plat(&id.platform);
        let tw = 13.0 + 15.0 + 7.0 + text_width(&p, &id.display_name, f6(13.0), 0.0) + if id.verified { 7.0 + 14.0 } else { 0.0 } + 13.0;
        let pill = Rect::from_min_size(Pos2::new(px, y), Vec2::new(tw, 34.0));
        // a pílula é o link do canal na plataforma
        let resp = ui.interact(pill, ui.id().with(("plat", &id.platform)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand).on_hover_text(format!("{nome} · {}", if id.verified { "verificada" } else { "vista no chat" }));
        if resp.clicked() {
            let login = id.display_name.to_lowercase();
            let url = match id.platform.as_str() {
                "youtube" => format!("https://www.youtube.com/@{login}"),
                "kick" => format!("https://kick.com/{login}"),
                _ => format!("https://www.twitch.tv/{login}"),
            };
            ui.ctx().open_url(egui::OpenUrl::new_tab(url));
        }
        // a conta só vista no chat fica mais apagada (`opacity:.72`)
        let k = if id.verified { 1.0 } else { 0.72 };
        let fade = |c: Color32| mix(c, CREAM_0(), k);
        p.rect_filled(pill, 17.0, fade(CREAM_2()));
        ui::icon(ui, icon, Pos2::new(pill.left() + 13.0 + 7.5, pill.center().y), 15.0, fade(c));
        let t = text(&p, Pos2::new(pill.left() + 35.0, pill.center().y), Align2::LEFT_CENTER, &id.display_name, f6(13.0), fade(INK()), 0.0);
        if id.verified {
            // no claro o `.plat--twitch svg` do site pinta também o ✓ com a cor da plataforma; no escuro ele é verde
            ui::icon(ui, ic!("plat-check"), Pos2::new(t.right() + 7.0 + 7.0, pill.center().y), 14.0, if DARK() { CHECK } else { fade(c) });
        }
        px = pill.right() + 8.0;
    }
}

/// A capa do cartão: a cor que a pessoa escolheu ou o `linear-gradient(120deg, --cream-3, --cream-4)` do tema. Só os
/// cantos de cima do cartão `card` (raio `radius`) são redondos: eles são cortados pintando `outside`, a cor de fora.
fn banner(p: &Painter, cover: Rect, card: Rect, radius: f32, outside: Color32, solid: Option<Color32>) {
    match solid {
        Some(c) => {
            p.rect_filled(cover, 0.0, c);
        }
        None => {
            let mut mesh = egui::Mesh::default();
            let (a, b) = (CREAM_3(), CREAM_4());
            // o ângulo de 120° vai do canto de cima à esquerda para o de baixo à direita, mais para a direita do que para baixo
            let dir = Vec2::new((30f32).to_radians().cos(), (30f32).to_radians().sin());
            let span = cover.width() * dir.x.abs() + cover.height() * dir.y.abs();
            let at = |q: Pos2| {
                let k = ((q - cover.center()).dot(dir) / span + 0.5).clamp(0.0, 1.0);
                mix(b, a, k)
            };
            for q in [cover.left_top(), cover.right_top(), cover.right_bottom(), cover.left_bottom()] {
                mesh.colored_vertex(q, at(q));
            }
            mesh.add_triangle(0, 1, 2);
            mesh.add_triangle(0, 2, 3);
            p.add(mesh);
        }
    }
    for (corner, sx) in [(card.left_top(), 1.0f32), (card.right_top(), -1.0)] {
        let c = corner + Vec2::new(radius * sx, radius);
        let mut pts = vec![corner];
        for k in 0..=8 {
            let ang = std::f32::consts::FRAC_PI_2 * k as f32 / 8.0;
            pts.push(c + Vec2::new(-radius * sx * ang.cos(), -radius * ang.sin()));
        }
        let mut m = egui::Mesh::default();
        for q in &pts {
            m.colored_vertex(*q, outside);
        }
        for k in 1..pts.len() - 1 {
            m.add_triangle(0, k as u32, k as u32 + 1);
        }
        p.add(m);
    }
}

/// Os números (`.stats`): mensagens, pontos, pontos batidos e uma caixa por plataforma.
fn stats(ui: &mut egui::Ui, st: &PerfilState, x: f32, w: f32) {
    let pr = &st.m.profile;
    let mut items: Vec<(i64, String)> = vec![(pr.messages, "MENSAGENS".into()), (pr.points_total, "PONTOS".into()), (pr.points_batidas, "PONTOS BATIDOS".into())];
    for s in &pr.stats {
        items.push((s.count, plat(&s.platform).2.to_uppercase()));
    }
    // `repeat(auto-fit, minmax(120px, 1fr))` com 12 px entre as caixas
    let cols = (((w + 12.0) / (120.0 + 12.0)).floor() as usize).clamp(1, items.len());
    let bw = (w - 12.0 * (cols - 1) as f32) / cols as f32;
    let rows = (items.len() + cols - 1) / cols;
    let r = row(ui, x, w, rows as f32 * 75.0 + (rows - 1) as f32 * 12.0);
    let p = ui.painter();
    for (k, (n, label)) in items.iter().enumerate() {
        let b = Rect::from_min_size(r.min + Vec2::new((k % cols) as f32 * (bw + 12.0), (k / cols) as f32 * 87.0), Vec2::new(bw, 75.0));
        p.rect_filled(b, R_LG(), CREAM_0());
        ui::bold(p, Pos2::new(b.left() + 17.0, b.top() + 15.0 + 13.0), Align2::LEFT_CENTER, &num(*n), patua(22.0), INK(), 0.0);
        text(p, Pos2::new(b.left() + 17.0, b.top() + 43.0 + 8.5), Align2::LEFT_CENTER, label, f6(12.0), FAINT(), 0.36);
    }
}

/// A grade de um ano (`.act-card`): uma coluna por semana, de domingo a sábado, terminando no sábado desta semana.
fn activity(ui: &mut egui::Ui, st: &PerfilState, x: f32, w: f32) {
    let r = row(ui, x, w, 212.0);
    let p = ui.painter().clone();
    p.rect_filled(r, 20.0, CREAM_0());
    let o = r.min + Vec2::splat(21.0);
    let iw = w - 42.0;
    ui::bold(&p, Pos2::new(o.x, o.y + 10.0), Align2::LEFT_CENTER, "Atividade no Chat & Site", patua(16.0), INK(), 0.0);
    let total: u32 = st.m.profile.activity.iter().sum();
    text(&p, Pos2::new(o.x + iw, o.y + 11.5), Align2::RIGHT_CENTER, &format!("{} {}", num(total as i64), if total == 1 { "interação no último ano" } else { "interações no último ano" }), f6(12.0), FAINT(), 0.0);

    let today = Local::now().date_naive();
    let end = today + chrono::Duration::days(6 - today.weekday().num_days_from_sunday() as i64);
    let start = end - chrono::Duration::days(WEEKS * 7 - 1);
    let sq_x = o.x + 28.0;
    let sq_y = o.y + 53.0;

    // os meses: um rótulo por mês, na coluna em que ele aparece primeiro; o espaço que sobra se divide entre eles
    let mut months: Vec<(u32, i64, i64)> = Vec::new();
    for wk in 0..WEEKS {
        let m = (start + chrono::Duration::days(wk * 7)).month0();
        match months.last_mut() {
            Some(last) if last.0 == m => last.2 += 1,
            _ => months.push((m, wk, 1)),
        }
    }
    let k = (iw - 28.0) / (WEEKS as f32 * 13.0);
    for (m, wk, n) in months {
        if n > 1 {
            text(&p, Pos2::new(sq_x + wk as f32 * 13.0 * k, o.y + 32.0 + 7.5), Align2::LEFT_CENTER, MESES[m as usize], f6(11.0), FAINT(), 0.0);
        }
    }
    for (i, d) in ["Seg", "Qua", "Sex"].iter().enumerate() {
        text(&p, Pos2::new(o.x, o.y + 55.0 + 7.0 + 36.0 * i as f32), Align2::LEFT_CENTER, d, f6(10.0), FAINT(), 0.0);
    }

    // o verde do GitHub só no escuro; no claro, o âmbar misturado com `--cream-2` (30, 55, 80 e 100%)
    let color = |l: usize| match l {
        0 => CREAM_2(),
        _ if DARK() => GREENS[l - 1],
        _ => mix(SECONDARY(), CREAM_2(), [0.30, 0.55, 0.80, 1.0][l - 1]),
    };
    let level = |n: u32| match n {
        0 => 0,
        1..=4 => 1,
        5..=12 => 2,
        13..=29 => 3,
        _ => 4,
    };
    let mut hovered = None;
    for wk in 0..WEEKS {
        for d in 0..7 {
            let day: NaiveDate = start + chrono::Duration::days(wk * 7 + d);
            if day > today {
                continue;
            }
            let n = st.m.profile.activity.get((today - day).num_days() as usize).copied().unwrap_or(0);
            let q = Rect::from_min_size(Pos2::new(sq_x + wk as f32 * 13.0, sq_y + d as f32 * 13.0), Vec2::splat(10.0));
            let resp = ui.interact(q, ui.id().with(("dia-perfil", wk, d)), Sense::hover());
            let tip = if n > 0 { format!("{n} {} em {:02} de {}", if n == 1 { "interação" } else { "interações" }, day.day(), MESES_LONGOS[day.month0() as usize]) } else { format!("Nenhuma atividade em {:02} de {}", day.day(), MESES_LONGOS[day.month0() as usize]) };
            if resp.hovered() {
                hovered = Some((q, level(n)));
            }
            resp.on_hover_text(tip);
            p.rect_filled(q, 2.0, color(level(n)));
        }
    }
    // o quadrado com o mouse em cima cresce (`scale(1.3)`) por cima dos vizinhos
    if let Some((q, l)) = hovered {
        p.rect_filled(Rect::from_center_size(q.center(), Vec2::splat(13.0)), 2.6, color(l));
    }

    // a legenda, encostada à direita
    let fy = o.y + 155.0 + 7.5;
    let mais = text(&p, Pos2::new(o.x + iw, fy), Align2::RIGHT_CENTER, "Mais", f6(11.0), FAINT(), 0.0);
    let mut lx = mais.left() - 6.0;
    for l in (0..5).rev() {
        lx -= 10.0;
        p.rect_filled(Rect::from_min_size(Pos2::new(lx, fy - 5.0), Vec2::splat(10.0)), 2.0, color(l));
        lx -= 3.0;
    }
    text(&p, Pos2::new(lx + 3.0 - 6.0, fy), Align2::RIGHT_CENTER, "Menos", f6(11.0), FAINT(), 0.0);
}

/// As publicações no feed (`.post-mini`), só de leitura como no site. Devolve se o "Ver mais" foi clicado.
fn posts(ui: &mut egui::Ui, st: &PerfilState, x: f32, w: f32) -> bool {
    let list = &st.m.my_posts;
    if list.is_empty() {
        return false;
    }
    let head = row(ui, x, w, 21.0);
    let p = ui.painter().clone();
    ui::bold(&p, Pos2::new(head.left(), head.center().y), Align2::LEFT_CENTER, "Publicações no Feed", patua(18.0), INK(), 0.0);
    text(&p, Pos2::new(head.right(), head.center().y + 2.0), Align2::RIGHT_CENTER, &format!("{} de {}", list.len(), num(st.m.my_posts_total as i64)), f6(12.0), FAINT(), 0.0);
    ui.add_space(12.0);

    for (i, m) in list.iter().enumerate() {
        let thumb = image(&m.image);
        let tx = if thumb.is_some() { 14.0 + 56.0 + 12.0 } else { 14.0 };
        let tw = w - tx - 15.0;
        let title = (!m.title.is_empty()).then(|| para(&p, &m.title, patua(15.0), INK(), tw, 19.5));
        let body = (!m.message_text.is_empty()).then(|| para(&p, &m.message_text, f6(13.5), SOFT(), tw, 19.6));
        let mut content = 0.0;
        if let Some(g) = &title {
            content += g.size().y;
        }
        if let Some(g) = &body {
            content += if title.is_some() { 3.0 } else { 0.0 } + g.size().y;
        }
        content += 6.0 + 15.0;
        let inner_h = if thumb.is_some() { content.max(56.0) } else { content };
        let r = row(ui, x, w, 1.0 + 12.0 + inner_h + 12.0 + 1.0);
        let resp = ui.interact(r, ui.id().with(("post-mini", i)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand).on_hover_text("Abrir o post no navegador");
        if resp.clicked() {
            ui.ctx().open_url(egui::OpenUrl::new_tab(format!("https://brittico.xyz/{}/feed/{}", st.m.me.login, m.id)));
        }
        p.rect_filled(r, R_MD(), CREAM_0());
        if resp.hovered() {
            p.rect_stroke(r, R_MD(), Stroke::new(1.0_f32, HAIRLINE_2()), egui::StrokeKind::Inside);
        }
        if let Some((src, real)) = thumb {
            let th = Rect::from_min_size(r.min + Vec2::new(15.0, 13.0), Vec2::splat(56.0));
            p.rect_filled(th, R_SM(), CREAM_2());
            cover(ui, src, real, th);
        }
        let mut y = r.top() + 13.0;
        let bx = r.left() + tx + 1.0;
        if let Some(g) = title {
            let h = g.size().y;
            // a Patua em 700: o mesmo texto de novo, um pixel ao lado e quase transparente
            p.add(egui::epaint::TextShape::new(Pos2::new(bx + 1.0, y), g.clone(), INK()).with_override_text_color(INK().gamma_multiply(15.0 / 56.0)));
            p.galley(Pos2::new(bx, y), g, INK());
            y += h;
        }
        if let Some(g) = body {
            if m.title.is_empty() {
            } else {
                y += 3.0;
            }
            let h = g.size().y;
            p.galley(Pos2::new(bx, y), g, SOFT());
            y += h;
        }
        y += 6.0 + 7.5;
        let sc = if m.score > 0 { WIN } else if m.score < 0 { ACCENT() } else { FAINT() };
        let a = text(&p, Pos2::new(bx, y), Align2::LEFT_CENTER, &format!("{}{} pontos", if m.score > 0 { "+" } else { "" }, num(m.score)), f6(11.5), sc, 0.0);
        let b = text(&p, Pos2::new(a.right() + 12.0, y), Align2::LEFT_CENTER, &format!("{} {}", num(m.comment_count), if m.comment_count == 1 { "comentário" } else { "comentários" }), f6(11.5), FAINT(), 0.0);
        text(&p, Pos2::new(b.right() + 12.0, y), Align2::LEFT_CENTER, &when(st.now - m.ago_min * 60), f6(11.5), FAINT(), 0.0);
        ui.add_space(9.0);
    }

    // "Ver mais", quando há mais posts do que os da lista
    if (st.m.my_posts_total as usize) > list.len() {
        ui.add_space(5.0);
        let r = row(ui, x, w, 38.0);
        let label = "Ver mais";
        let bw = 16.0 + text_width(&p, label, f7(13.5), 0.0) + 16.0;
        let b = Rect::from_center_size(r.center(), Vec2::new(bw, 38.0));
        let resp = ui.interact(b, ui.id().with("ver-mais-posts"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
        p.rect_filled(b, 19.0, if resp.hovered() { mix(Color32::WHITE, CREAM_2(), 0.06) } else { CREAM_2() });
        text(&p, b.center(), Align2::CENTER_CENTER, label, f7(13.5), INK(), 0.0);
        return resp.clicked();
    }
    false
}

/// A miniatura de um post (o endereço da API), com o tamanho de verdade quando a imagem já chegou.
fn image(name: &str) -> Option<(ImageSource<'static>, Vec2)> {
    crate::net::imagem(name).map(|src| (src, Vec2::splat(56.0)))
}

/// `object-fit:cover` com os cantos de `--r-sm`.
fn cover(ui: &egui::Ui, src: ImageSource<'static>, real: Vec2, rect: Rect) {
    let (a, b) = (real.x / real.y, rect.width() / rect.height());
    let uv = if a > b {
        let w = b / a;
        Rect::from_min_max(Pos2::new((1.0 - w) / 2.0, 0.0), Pos2::new((1.0 + w) / 2.0, 1.0))
    } else {
        let h = a / b;
        Rect::from_min_max(Pos2::new(0.0, (1.0 - h) / 2.0), Pos2::new(1.0, (1.0 + h) / 2.0))
    };
    let src = match src {
        ImageSource::Bytes { uri, bytes } => {
            let (stem, ext) = uri.rsplit_once('.').unwrap_or((&uri, ""));
            ImageSource::Bytes { uri: format!("{stem}@{}x{}c.{ext}", rect.width() as u32, rect.height() as u32).into(), bytes }
        }
        other => other,
    };
    egui::Image::new(src).uv(uv).corner_radius(R_SM()).fit_to_exact_size(rect.size()).paint_at(ui, rect);
}

//! O Início do site (`public/index.html`): `assets/js/home.js` arma as três fatias, `home-panels.js` desenha cada uma,
//! `feed-core.js` desenha os posts e `home.css` + `feed-core.css` dão as medidas.
//!
//! escrever um post (`.cw`) · o feed da comunidade (`.post`, numa coluna de leitura de 600 px) · o meu perfil (`.pf`),
//! com o texto do que é o Brittico (`.home-legal`) embaixo do perfil. A pessoa é o Kennzy, como na Call.
//!
//! Os dados são os de `assets/mock/home.json`, o mesmo arquivo que `tools/site-home.cjs` usa para abrir a home do site
//! com os mesmos posts: é assim que as duas telas são comparadas. As medidas abaixo vêm de lá (janela de 1600×900).

use crate::theme::*;
use crate::ui::{self, mix, text, text_width};
use crate::ic;
use chrono::{Datelike, Local, NaiveDate, NaiveDateTime, TimeZone};
use eframe::egui::{
    self, text::LayoutJob, Align, Align2, Color32, CursorIcon, FontId, Galley,
    ImageSource, Order, Painter, Pos2, Rect, Sense, Stroke, StrokeKind, TextFormat, UiBuilder, Vec2,
};
use crate::net::{self, Tarefa};
use serde::Deserialize;
use std::sync::Arc;

/// A coluna de leitura do feed (`.feed-list{max-width:600px}`).
const FEED_W: f32 = 600.0;
/// O que fica entre um post e o outro, e entre as fatias.
const GAP: f32 = 10.0;
/// O composer (`.cw`) tem sempre a mesma altura: 261 px de conteúdo, 10 de folga e 1 de borda em cima e embaixo.
const CW_H: f32 = 283.0;
/// Quantas respostas o post mostra embaixo dele (`PREVIEW_MAX`).
const PREVIEW_MAX: usize = 5;
/// O texto do post e o título têm limite (`maxlength`).
const TEXT_MAX: usize = 2000;
const TITLE_MAX: usize = 140;
/// O azul do voto para baixo e o verde do `>` (`.greentext .quote`).
const DOWN: Color32 = Color32::from_rgb(0x3b, 0x6f, 0xd4);
const QUOTE: Color32 = Color32::from_rgb(0x78, 0x99, 0x22);
const QUOTE_TOOL: Color32 = Color32::from_rgb(0x3f, 0x8f, 0x3f);
/// `PALETTE` do feed-core.js: a cor do avatar sem foto sai do nome.
const PALETTE: [Color32; 10] = [
    Color32::from_rgb(0xF4, 0xA6, 0x2C),
    Color32::from_rgb(0xD3, 0x00, 0x00),
    Color32::from_rgb(0xE8, 0x89, 0x2B),
    Color32::from_rgb(0xC7, 0x7B, 0x3C),
    Color32::from_rgb(0xB8, 0x86, 0x0B),
    Color32::from_rgb(0x7A, 0x5C, 0x2E),
    Color32::from_rgb(0xA6, 0x64, 0x1E),
    Color32::from_rgb(0x91, 0x46, 0xFF),
    Color32::from_rgb(0x00, 0xC8, 0xAF),
    Color32::from_rgb(0x53, 0xa0, 0x18),
];
const MESES: [&str; 12] = ["Jan", "Fev", "Mar", "Abr", "Mai", "Jun", "Jul", "Ago", "Set", "Out", "Nov", "Dez"];
const MESES_LONGOS: [&str; 12] = ["janeiro", "fevereiro", "março", "abril", "maio", "junho", "julho", "agosto", "setembro", "outubro", "novembro", "dezembro"];
const SOBRE: &str = "Brittico é a plataforma de comunidade do canal do Kennzy, streamer de League of Legends, e foi pensada para receber outros streamers: perfis, feed, loja e jogos. Dá para navegar sem entrar; entrar com a Twitch ou o YouTube serve só para ter perfil e jogar, e lemos apenas o nome, a foto e o identificador da conta.";

/* ================= os dados ================= */

/// Um post como a API manda (`/api/feed`, `/api/feed/:id`).
#[derive(Deserialize, Default)]
#[serde(default)]
pub(crate) struct ApiPost {
    pub id: u32,
    pub username: String,
    pub post_type: String,
    pub timestamp: String,
    pub title: Option<String>,
    pub message_text: Option<String>,
    pub score: i32,
    pub my_vote: Option<i32>,
    pub comment_count: u32,
    pub image_filename: Option<String>,
    pub image_w: Option<f32>,
    pub image_h: Option<f32>,
    pub avatar_url: Option<String>,
    pub avatar_filename: Option<String>,
    pub profile_avatar: Option<String>,
    pub channel: Option<String>,
    pub profile_id: Option<i64>,
    pub mine: bool,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct FeedResp {
    posts: Vec<ApiPost>,
}

#[derive(Deserialize, Default, Clone)]
#[serde(default)]
pub(crate) struct ApiComment {
    pub id: u32,
    pub author_name: String,
    pub created_at: String,
    pub body: String,
    pub profile_id: Option<i64>,
    pub can_delete: bool,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct CommentsResp {
    comments: Vec<ApiComment>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct VoteResp {
    score: i32,
    my_vote: i32,
}

/// O resumo do perfil do lado direito: `/api/profile/:id`, a atividade e os 5 posts mais novos.
#[derive(Deserialize, Default)]
#[serde(default)]
pub(crate) struct ProfileResp {
    pub profile: ApiProfile,
    pub stats: Vec<serde_json::Value>,
    #[serde(rename = "totalMessages")]
    pub total_messages: i64,
}

#[derive(Deserialize, Default, Clone)]
#[serde(default)]
pub(crate) struct ApiProfile {
    pub id: i64,
    #[serde(rename = "displayName")]
    pub display_name: String,
    pub avatar: Option<String>,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    pub points: ApiPoints,
}

#[derive(Deserialize, Default, Clone)]
#[serde(default)]
pub(crate) struct ApiPoints {
    pub total: i64,
    pub month: i64,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub(crate) struct ActivityResp {
    pub days: Vec<ActivityDay>,
    pub hidden: bool,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub(crate) struct ActivityDay {
    pub day: String,
    pub count: u32,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub(crate) struct PostsResp {
    pub posts: Vec<ApiPost>,
    pub total: u32,
}

/// "2026-10-31 21:32:00" (UTC, do SQLite) ou ISO → segundos.
pub(crate) fn parse_ts(s: &str) -> i64 {
    let s = s.trim();
    if let Ok(d) = chrono::DateTime::parse_from_rfc3339(s) {
        return d.timestamp();
    }
    NaiveDateTime::parse_from_str(&s[..s.len().min(19)], "%Y-%m-%d %H:%M:%S")
        .or_else(|_| NaiveDateTime::parse_from_str(&s[..s.len().min(19)], "%Y-%m-%dT%H:%M:%S"))
        .map(|d| d.and_utc().timestamp())
        .unwrap_or_else(|_| Local::now().timestamp())
}

/// A foto de quem postou: a da plataforma, a do perfil, ou o arquivo guardado pelo bot.
fn avatar_of(p: &ApiPost) -> String {
    for a in [&p.avatar_url, &p.profile_avatar] {
        if let Some(a) = a.as_deref().filter(|a| !a.is_empty()) {
            return a.to_owned();
        }
    }
    p.avatar_filename.as_deref().filter(|a| !a.is_empty()).map(|f| format!("/api/feed/avatars/{f}")).unwrap_or_default()
}

/// A atividade por dia (`days`) virando a lista do mais novo (hoje = 0) para o mais velho, 371 dias.
pub(crate) fn activity_vec(a: &ActivityResp) -> Vec<u32> {
    let today = Local::now().date_naive();
    let mut v = vec![0u32; 371];
    for d in &a.days {
        if let Ok(day) = NaiveDate::parse_from_str(&d.day[..d.day.len().min(10)], "%Y-%m-%d") {
            let ago = (today - day).num_days();
            if (0..371).contains(&ago) {
                v[ago as usize] += d.count;
            }
        }
    }
    v
}

impl From<ApiPost> for Post {
    fn from(p: ApiPost) -> Post {
        let avatar = avatar_of(&p);
        let image = p.image_filename.clone().filter(|f| !f.is_empty()).map(|f| format!("/api/feed/images/{f}")).unwrap_or_default();
        let image_size = match (p.image_w, p.image_h) {
            (Some(w), Some(h)) if w > 0.0 && h > 0.0 => Some(Vec2::new(w, h)),
            _ => None,
        };
        Post {
            id: p.id,
            chat: p.post_type == "chat",
            ts: parse_ts(&p.timestamp),
            username: p.username,
            channel: p.channel.unwrap_or_default(),
            title: p.title.unwrap_or_default(),
            text: p.message_text.unwrap_or_default(),
            score: p.score,
            my_vote: p.my_vote.unwrap_or(0),
            comment_count: p.comment_count,
            image,
            image_size,
            avatar,
            replies: Vec::new(),
            profile_id: p.profile_id.unwrap_or(0),
            mine: p.mine,
        }
    }
}

struct Post {
    id: u32,
    username: String,
    /// O canal da live de onde o bot pescou o post; vazio quando foi escrito no site.
    channel: String,
    /// Veio do chat de uma live (tem "Ver o momento").
    chat: bool,
    ts: i64,
    title: String,
    text: String,
    score: i32,
    my_vote: i32,
    comment_count: u32,
    /// O endereço da imagem do post (ou o nome de um envio local), e o tamanho dela se a API disse.
    image: String,
    image_size: Option<Vec2>,
    avatar: String,
    replies: Vec<Reply>,
    profile_id: i64,
    mine: bool,
}

struct Reply {
    author: String,
    ts: i64,
    body: String,
}

struct MyPost {
    id: u32,
    title: String,
    text: String,
    score: i64,
    comments: i64,
    ts: i64,
    image: String,
}

/// O que volta de cada ação mandada ao servidor.
enum Volta {
    Voto(u32, Result<VoteResp, net::Erro>),
    Resposta(u32, Result<ApiComment, net::Erro>),
    Publicado(Result<ApiPost, net::Erro>),
    Apagado(u32, Result<serde_json::Value, net::Erro>),
    Denuncia(Result<serde_json::Value, net::Erro>),
}

/// Quantos posts vêm por página (`PAGE` do feed-core.js).
const PAGE: usize = 20;

pub struct HomeState {
    login: String,
    name: String,
    avatar: String,
    since: i64,
    points_month: i64,
    points_total: i64,
    messages: i64,
    activity: Vec<u32>,
    activity_hidden: bool,
    my_posts: Vec<MyPost>,
    my_posts_total: u32,
    posts: Vec<Post>,
    /// O rascunho do `.cw`.
    title: String,
    body: String,
    /// O post com o "Responder" aberto e o que está escrito nele.
    reply_to: Option<u32>,
    reply: String,
    /// O post com o menu do "…" aberto, e onde fica o botão.
    menu: Option<(u32, Rect)>,
    pub toast: ui::Toast,
    /// Alguém clicou num caminho para o perfil ("Meu perfil", "ver perfil"): o `main.rs` troca de seção.
    pub open_profile: bool,
    /// O perfil de outra pessoa pedido no feed (o id do perfil).
    pub open_other: Option<i64>,
    /// O "Tema" do composer: o `main.rs` alterna entre o claro e o escuro.
    pub alternar_tema: bool,
    /// O "Sair" do composer e o "Publicar" sem conta: o `main.rs` sai ou abre o cartão de entrar.
    pub pedir_sair: bool,
    pub pedir_login: bool,
    /// Um link que no site leva a uma página que o app não tem: abre no navegador.
    abrir_url: Option<String>,
    /// O post com a conversa inteira aberta ("12 comentários") e o com a janela do "Ver o momento".
    thread: Option<u32>,
    moment: Option<u32>,
    /// A imagem anexada ao rascunho (o nome dela no registro de `ENVIADAS`) e o diálogo de arquivo aberto.
    anexo: Option<String>,
    dialogo: Option<std::thread::JoinHandle<Option<std::path::PathBuf>>>,
    /* a conversa com o servidor */
    /// Para qual conta (geração da `Conta`) o que está na tela foi carregado.
    geracao: u64,
    pagina: Option<Tarefa<FeedResp>>,
    offset: usize,
    seed: u32,
    fim: bool,
    erro: Option<String>,
    comentarios: Vec<(u32, Tarefa<CommentsResp>)>,
    perfil: Option<Tarefa<(ProfileResp, ActivityResp, PostsResp)>>,
    voltas: Vec<Tarefa<Volta>>,
    enviando: bool,
    /// Quando o feed foi buscado (para buscar de novo de tempos em tempos).
    buscado_em: f64,
}

thread_local! {
    /// As imagens mandadas pelo composer: o nome que o post guarda, a textura, o tamanho de verdade e o arquivo.
    static ENVIADAS: std::cell::RefCell<std::collections::HashMap<String, (egui::TextureHandle, Vec2, std::path::PathBuf)>> = Default::default();
}

/// Lê a imagem escolhida (ou solta no composer), guarda a textura e devolve o nome dela.
fn anexar(ctx: &egui::Context, path: &std::path::Path) -> Option<String> {
    let img = image::open(path).ok()?.to_rgba8();
    let (w, h) = (img.width(), img.height());
    let nome = format!("envio-{}-{}", path.display(), ENVIADAS.with(|e| e.borrow().len()));
    let tex = ctx.load_texture(&nome, egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], img.as_raw()), egui::TextureOptions::LINEAR);
    ENVIADAS.with(|e| e.borrow_mut().insert(nome.clone(), (tex, Vec2::new(w as f32, h as f32), path.to_owned())));
    Some(nome)
}

/// O anexo como `data:` URL, que é como o site manda a imagem (`body.image`).
fn data_url(nome: &str) -> Option<String> {
    let path = ENVIADAS.with(|e| e.borrow().get(nome).map(|x| x.2.clone()))?;
    let bytes = std::fs::read(&path).ok()?;
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    let mime = match ext.as_str() {
        "png" => "image/png",
        "webp" => "image/webp",
        "gif" => "image/gif",
        _ => "image/jpeg",
    };
    use base64::Engine;
    Some(format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes)))
}

impl Default for HomeState {
    fn default() -> Self {
        Self {
            login: String::new(),
            name: String::new(),
            avatar: String::new(),
            since: 0,
            points_month: 0,
            points_total: 0,
            messages: 0,
            activity: Vec::new(),
            activity_hidden: false,
            my_posts: Vec::new(),
            my_posts_total: 0,
            posts: Vec::new(),
            title: String::new(),
            body: String::new(),
            reply_to: None,
            reply: String::new(),
            menu: None,
            toast: ui::Toast::default(),
            open_profile: false,
            open_other: None,
            alternar_tema: false,
            pedir_sair: false,
            pedir_login: false,
            abrir_url: None,
            thread: None,
            moment: None,
            anexo: None,
            dialogo: None,
            geracao: u64::MAX,
            pagina: None,
            offset: 0,
            seed: 1,
            fim: false,
            erro: None,
            comentarios: Vec::new(),
            perfil: None,
            voltas: Vec::new(),
            enviando: false,
            buscado_em: 0.0,
        }
    }
}

impl HomeState {
    /// Para as capturas (`BRITTICO_TELA=inicio-teste`): um rascunho e o "Responder" aberto no primeiro post.
    pub fn demo(&mut self) {
        self.title = "Treino de quinta".into();
        self.body = "quem topa scrim às 20h?\n> sem atraso dessa vez".into();
    }

    fn say(&mut self, msg: &str, t: f64) {
        self.toast.say(msg, t);
    }

    /// Começa (ou recomeça) o feed do topo, e o perfil do lado.
    fn recarregar(&mut self, ctx: &egui::Context, t: f64) {
        // a ordem do "para você" é sorteada uma vez por abertura (BRITTICO_SEED fixa, para os testes)
        self.seed = std::env::var("BRITTICO_SEED").ok().and_then(|v| v.parse().ok()).unwrap_or((Local::now().timestamp_subsec_nanos() ^ 0x5bd1e995) & 0x7fff_ffff);
        self.offset = 0;
        self.fim = false;
        self.erro = None;
        self.buscado_em = t;
        self.pagina = Some(net::get(ctx, format!("/api/feed?sort=foryou&limit={PAGE}&offset=0&seed={}", self.seed)));
        self.comentarios.clear();
        self.perfil = None;
        match crate::conta::eu() {
            Some(me) => {
                self.login = me.user.login.clone();
                self.name = me.nome().to_owned();
                self.avatar = me.user.avatar.clone();
                let id = me.profile_id;
                self.perfil = Some(net::rodar(ctx, move || {
                    let p: ProfileResp = serde_json::from_value(net::pedir_ja("GET", &format!("/api/profile/{id}"), None)?).unwrap_or_default();
                    let a: ActivityResp = net::pedir_ja("GET", &format!("/api/profile/{id}/activity"), None).ok().and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default();
                    let m: PostsResp = net::pedir_ja("GET", &format!("/api/profile/{id}/posts?limit=5&offset=0"), None).ok().and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default();
                    Ok((p, a, m))
                }));
            }
            None => {
                self.login.clear();
                self.name.clear();
                self.avatar.clear();
                self.my_posts.clear();
                self.my_posts_total = 0;
                self.activity.clear();
            }
        }
    }

    fn mais(&mut self, ctx: &egui::Context) {
        if self.pagina.is_some() || self.fim {
            return;
        }
        self.pagina = Some(net::get(ctx, format!("/api/feed?sort=foryou&limit={PAGE}&offset={}&seed={}", self.offset, self.seed)));
    }

    fn pedir_comentarios(&mut self, ctx: &egui::Context, id: u32) {
        if self.comentarios.iter().any(|(i, _)| *i == id) {
            return;
        }
        self.comentarios.push((id, net::get(ctx, format!("/api/feed/{id}/comments"))));
    }

    /// Uma vez por quadro: o que chegou do servidor.
    fn passo(&mut self, ctx: &egui::Context, geracao: u64, t: f64) {
        if self.geracao != geracao {
            self.geracao = geracao;
            self.posts.clear();
            self.recarregar(ctx, t);
        }
        // de dois em dois minutos, se ninguém está escrevendo uma resposta, o feed volta ao topo com o que há de novo
        if t - self.buscado_em > 120.0 && self.reply_to.is_none() && self.thread.is_none() && self.pagina.is_none() && self.menu.is_none() {
            self.buscado_em = t;
            self.pagina = Some(net::get(ctx, format!("/api/feed?sort=new&limit=10&offset=0")));
            self.offset = usize::MAX; // marca: é a busca do que há de novo
        }
        if let Some(r) = self.pagina.as_ref().and_then(|p| p.pronto()) {
            self.pagina = None;
            match r {
                Ok(f) if self.offset == usize::MAX => {
                    // o que há de novo entra no topo, sem repetir
                    self.offset = self.posts.len();
                    let novos: Vec<Post> = f.posts.into_iter().filter(|p| !self.posts.iter().any(|q| q.id == p.id) && parse_ts(&p.timestamp) > self.posts.iter().map(|q| q.ts).max().unwrap_or(0)).map(Post::from).collect();
                    for p in novos.into_iter().rev() {
                        let id = p.id;
                        let c = p.comment_count;
                        self.posts.insert(0, p);
                        self.offset += 1;
                        if c > 0 {
                            self.pedir_comentarios(ctx, id);
                        }
                    }
                }
                Ok(f) => {
                    let n = f.posts.len();
                    self.offset += n;
                    self.fim = n < PAGE;
                    for p in f.posts {
                        if self.posts.iter().any(|q| q.id == p.id) {
                            continue;
                        }
                        let (id, c) = (p.id, p.comment_count);
                        self.posts.push(p.into());
                        if c > 0 {
                            self.pedir_comentarios(ctx, id);
                        }
                    }
                    self.erro = None;
                }
                Err(e) => {
                    if self.offset == usize::MAX {
                        self.offset = self.posts.len();
                    }
                    self.erro = Some(e.msg);
                }
            }
        }
        let mut i = 0;
        while i < self.comentarios.len() {
            if let Some(r) = self.comentarios[i].1.pronto() {
                let (id, _) = self.comentarios.remove(i);
                if let (Ok(c), Some(p)) = (r, self.posts.iter_mut().find(|p| p.id == id)) {
                    p.replies = c.comments.into_iter().map(|c| Reply { author: c.author_name, ts: parse_ts(&c.created_at), body: c.body }).collect();
                    p.comment_count = p.comment_count.max(p.replies.len() as u32);
                }
            } else {
                i += 1;
            }
        }
        if let Some(r) = self.perfil.as_ref().and_then(|p| p.pronto()) {
            self.perfil = None;
            if let Ok((p, a, m)) = r {
                let since = p.stats.iter().filter_map(|s| s.get("firstAt").and_then(|v| v.as_str())).map(parse_ts).min().unwrap_or_else(|| parse_ts(&p.profile.created_at));
                self.since = since;
                self.points_month = p.profile.points.month;
                self.points_total = p.profile.points.total;
                self.messages = p.total_messages;
                if let Some(av) = p.profile.avatar.filter(|a| !a.is_empty()) {
                    self.avatar = av;
                }
                if !p.profile.display_name.is_empty() {
                    self.name = p.profile.display_name;
                }
                self.activity = activity_vec(&a);
                self.activity_hidden = a.hidden;
                self.my_posts_total = m.total;
                self.my_posts = m.posts.into_iter().map(|p| MyPost {
                    id: p.id,
                    ts: parse_ts(&p.timestamp),
                    title: p.title.unwrap_or_default(),
                    text: p.message_text.unwrap_or_default(),
                    score: p.score as i64,
                    comments: p.comment_count as i64,
                    image: p.image_filename.filter(|f| !f.is_empty()).map(|f| format!("/api/feed/images/{f}")).unwrap_or_default(),
                }).collect();
            }
        }
        let mut i = 0;
        while i < self.voltas.len() {
            let Some(v) = self.voltas[i].pronto() else {
                i += 1;
                continue;
            };
            self.voltas.remove(i);
            match v {
                Ok(v) => self.volta(ctx, v, t),
                Err(e) => self.say(&e.msg, t),
            }
        }
    }

    fn volta(&mut self, ctx: &egui::Context, v: Volta, t: f64) {
        match v {
            Volta::Voto(id, Ok(r)) => {
                if let Some(p) = self.posts.iter_mut().find(|p| p.id == id) {
                    p.score = r.score;
                    p.my_vote = r.my_vote;
                }
            }
            Volta::Voto(id, Err(e)) => {
                // o voto não pegou: busca o post de novo para a tela não mentir
                self.say(&e.msg, t);
                if let Some(i) = self.posts.iter().position(|p| p.id == id) {
                    let p = &mut self.posts[i];
                    p.score -= p.my_vote;
                    p.my_vote = 0;
                }
            }
            Volta::Resposta(id, Ok(c)) => {
                if let Some(p) = self.posts.iter_mut().find(|p| p.id == id) {
                    p.replies.push(Reply { author: c.author_name, ts: parse_ts(&c.created_at), body: c.body });
                    p.comment_count += 1;
                }
                self.say("Resposta enviada!", t);
            }
            Volta::Resposta(id, Err(e)) => {
                self.reply_to = Some(id);
                self.say(&e.msg, t);
            }
            Volta::Publicado(Ok(p)) => {
                self.enviando = false;
                self.title.clear();
                self.body.clear();
                self.anexo = None;
                let post: Post = p.into();
                self.my_posts.insert(0, MyPost { id: post.id, title: post.title.clone(), text: post.text.clone(), score: 0, comments: 0, ts: post.ts, image: post.image.clone() });
                self.my_posts.truncate(5);
                self.my_posts_total += 1;
                self.posts.insert(0, post);
                self.offset += 1;
                self.say("Publicado!", t);
            }
            Volta::Publicado(Err(e)) => {
                self.enviando = false;
                self.say(&e.msg, t);
            }
            Volta::Apagado(id, Ok(_)) => {
                self.posts.retain(|p| p.id != id);
                if self.my_posts.iter().any(|m| m.id == id) {
                    self.my_posts.retain(|m| m.id != id);
                    self.my_posts_total = self.my_posts_total.saturating_sub(1);
                }
                self.say("Post apagado.", t);
            }
            Volta::Apagado(_, Err(e)) => self.say(&e.msg, t),
            Volta::Denuncia(Ok(_)) => self.say("Denúncia enviada. Obrigado!", t),
            Volta::Denuncia(Err(e)) => self.say(&e.msg, t),
        }
        let _ = ctx;
    }

    fn publish(&mut self, ctx: &egui::Context, t: f64) {
        if crate::conta::eu().is_none() {
            self.pedir_login = true;
            return;
        }
        if self.enviando {
            return;
        }
        let title = self.title.trim().to_owned();
        let text = self.body.trim_end().to_owned();
        let mut corpo = serde_json::json!({ "title": title, "text": text });
        if let Some(n) = &self.anexo {
            match data_url(n) {
                Some(d) => corpo["image"] = d.into(),
                None => {
                    self.say("Não deu para ler a imagem anexada.", t);
                    return;
                }
            }
        }
        self.enviando = true;
        self.voltas.push(net::rodar(ctx, move || {
            let r = net::pedir_ja("POST", "/api/feed", Some(&corpo)).and_then(|v| serde_json::from_value::<ApiPost>(v.get("post").cloned().unwrap_or_default()).map_err(|e| net::Erro { status: 0, msg: e.to_string() }));
            Ok(Volta::Publicado(r))
        }));
    }
}

/// Uma imagem para o feed: um envio local (`envio-…`) ou um endereço da API; com o tamanho de verdade quando já se sabe.
fn mock_image(name: &str) -> Option<(ImageSource<'static>, Vec2)> {
    if name.is_empty() {
        return None;
    }
    if name.starts_with("envio-") {
        return ENVIADAS.with(|e| e.borrow().get(name).map(|(tex, size, _)| (ImageSource::Texture(egui::load::SizedTexture::new(tex.id(), *size)), *size)));
    }
    net::imagem(name).map(|src| (src, Vec2::new(600.0, 400.0)))
}

/// O tamanho de verdade de uma imagem remota, quando ela já chegou (até lá, `None`).
fn tamanho_remoto(ctx: &egui::Context, src: &ImageSource<'static>) -> Option<Vec2> {
    let ImageSource::Uri(u) = src else { return None };
    match ctx.try_load_texture(u, egui::TextureOptions::LINEAR, egui::SizeHint::default()) {
        Ok(egui::load::TexturePoll::Ready { texture }) => Some(texture.size),
        _ => None,
    }
}

/* ================= as contas do site ================= */

fn color_for(name: &str) -> Color32 {
    let mut h: u32 = 0;
    for u in name.encode_utf16() {
        h = h.wrapping_mul(31).wrapping_add(u as u32);
    }
    PALETTE[(h % PALETTE.len() as u32) as usize]
}

/// `initials` do feed-core.js: o que não é letra ou número vira espaço; duas palavras dão duas iniciais, uma dá as duas primeiras letras.
fn initials(name: &str) -> String {
    let clean: String = name.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { ' ' }).collect();
    let parts: Vec<&str> = clean.split_whitespace().collect();
    if parts.len() >= 2 {
        format!("{}{}", &parts[0][..1], &parts[1][..1]).to_uppercase()
    } else {
        clean.trim().chars().take(2).collect::<String>().to_uppercase()
    }
}

/// `toLocaleString("pt-BR")`: 18530 → "18.530".
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

/// O tempo curto do feed-core.js, o do cabeçalho do post: "agora", "5m", "3h", "2d", "1sem".
fn ago_short(ts: i64) -> String {
    let s = Local::now().timestamp() - ts;
    match s {
        s if s < 60 => "agora".into(),
        s if s < 3600 => format!("{}m", s / 60),
        s if s < 86400 => format!("{}h", s / 3600),
        s if s < 604800 => format!("{}d", s / 86400),
        s => format!("{}sem", s / 604800),
    }
}

/// O tempo por extenso do home-panels.js, o da lista "Seus posts": "5 horas atrás", "1 mês atrás".
fn ago_long(ts: i64) -> String {
    let s = Local::now().timestamp() - ts;
    if s < 60 {
        return "agora mesmo".into();
    }
    let plural = |n: i64, one: &str, many: &str| format!("{n} {}", if n == 1 { one } else { many });
    let m = s / 60;
    if m < 60 {
        return plural(m, "min atrás", "mins atrás");
    }
    let h = m / 60;
    if h < 24 {
        return plural(h, "hora atrás", "horas atrás");
    }
    let d = h / 24;
    if d < 30 {
        return plural(d, "dia atrás", "dias atrás");
    }
    let mo = d / 30;
    if mo < 12 {
        return plural(mo, "mês atrás", "meses atrás");
    }
    plural(mo / 12, "ano atrás", "anos atrás")
}

fn local(ts: i64) -> chrono::DateTime<Local> {
    Local.timestamp_opt(ts, 0).single().unwrap_or_else(Local::now)
}

/* ================= texto ================= */

/// Texto quebrado em linhas de até `width`, com a altura de linha do CSS (`line-height`) e a meia-entrelinha em cima e
/// embaixo, como o navegador faz. Linhas que começam com `>` ficam verdes quando `green` (o `greentext` do feed).
fn para(p: &Painter, s: &str, font: FontId, color: Color32, width: f32, line_h: f32, green: bool) -> Arc<Galley> {
    let mut job = LayoutJob::default();
    job.wrap.max_width = width;
    let lines: Vec<&str> = s.split('\n').collect();
    for (i, line) in lines.iter().enumerate() {
        let quote = green && line.trim_start().starts_with('>') && !line.trim_start().starts_with(">>");
        let mut l = line.to_string();
        if i + 1 < lines.len() {
            l.push('\n');
        }
        job.append(&l, 0.0, TextFormat { font_id: font.clone(), color: if quote { QUOTE } else { color }, line_height: Some(line_h), valign: Align::Center, ..Default::default() });
    }
    p.layout_job(job)
}

/// Um avatar do feed: a foto redonda, ou as iniciais em branco sobre a cor que sai do nome.
fn avatar(ui: &egui::Ui, name: &str, photo: &str, c: Pos2, size: f32, font: FontId) {
    match mock_image(photo) {
        Some((src, _)) => {
            ui.painter().circle_filled(c, size / 2.0, color_for(name));
            ui::image(ui, src, Rect::from_center_size(c, Vec2::splat(size)), size / 2.0)
        }
        None => {
            ui.painter().circle_filled(c, size / 2.0, color_for(name));
            text(ui.painter(), c, Align2::CENTER_CENTER, &initials(name), font, Color32::WHITE, 0.0);
        }
    }
}

/// Uma foto cortada para preencher a caixa (`object-fit:cover`).
fn cover(ui: &egui::Ui, src: ImageSource<'static>, real: Vec2, rect: Rect, radius: f32) {
    let (a, b) = (real.x / real.y, rect.width() / rect.height());
    let uv = if a > b {
        let w = b / a;
        Rect::from_min_max(Pos2::new((1.0 - w) / 2.0, 0.0), Pos2::new((1.0 + w) / 2.0, 1.0))
    } else {
        let h = a / b;
        Rect::from_min_max(Pos2::new(0.0, (1.0 - h) / 2.0), Pos2::new(1.0, (1.0 + h) / 2.0))
    };
    let s = rect.size();
    let src = match src {
        ImageSource::Bytes { uri, bytes } => {
            let (stem, ext) = uri.rsplit_once('.').unwrap_or((&uri, ""));
            ImageSource::Bytes { uri: format!("{stem}@{}x{}c.{ext}", s.x as u32, s.y as u32).into(), bytes }
        }
        other => other,
    };
    egui::Image::new(src).uv(uv).corner_radius(radius).fit_to_exact_size(s).paint_at(ui, rect);
}

fn hand(r: egui::Response) -> egui::Response {
    r.on_hover_cursor(CursorIcon::PointingHand)
}

/* ================= a tela ================= */

pub fn show(ui: &mut egui::Ui, st: &mut HomeState, body: Rect, t: f64, geracao: u64) {
    st.passo(ui.ctx(), geracao, t);
    if ui.input(|i| i.key_pressed(egui::Key::F5)) {
        st.posts.clear();
        st.recarregar(ui.ctx(), t);
    }
    // `.home-board`: grid 1fr 2fr 1fr, 10 px de folga em volta e entre as fatias
    let board = body.shrink(GAP);
    let w = (board.width() - 2.0 * GAP) / 4.0;
    let left = Rect::from_min_size(board.min, Vec2::new(w, board.height()));
    let center = Rect::from_min_size(Pos2::new(left.right() + GAP, board.top()), Vec2::new(2.0 * w, board.height()));
    let right = Rect::from_min_size(Pos2::new(center.right() + GAP, board.top()), Vec2::new(w, board.height()));
    composer(ui, st, Rect::from_min_size(left.min, Vec2::new(w, CW_H)), t);
    feed(ui, st, center, t);
    side(ui, st, right);
    post_menu(ui, st, t);
    moment(ui, st, body);
    if let Some(url) = st.abrir_url.take() {
        ui.ctx().open_url(egui::OpenUrl::new_tab(url));
    }
    st.toast.show(ui, t);
}

/// A janela do "Ver o momento" (`.kz-moment`): no site o player da Twitch toca ali dentro, 10 s antes do post; o app
/// não tem navegador, então o quadro mostra onde o vídeo vai abrir e o "Abrir na Twitch" leva para lá.
fn moment(ui: &mut egui::Ui, st: &mut HomeState, body: Rect) {
    let Some(id) = st.moment else { return };
    let Some(post) = st.posts.iter().find(|p| p.id == id) else {
        st.moment = None;
        return;
    };
    let canal = if post.channel.is_empty() { "kennzy".to_owned() } else { post.channel.clone() };
    let url = format!("https://www.twitch.tv/{canal}/videos?filter=archives&t={}", local(post.ts - 10).format("%Hh%Mm%Ss"));
    let w = (body.width() - 120.0).min(1100.0);
    let frame = Rect::from_center_size(body.center() - Vec2::new(0.0, 20.0), Vec2::new(w, w * 9.0 / 16.0).min(Vec2::new(w, body.height() - 160.0)));
    let mut fechar = false;
    egui::Area::new(ui.id().with("momento")).order(Order::Foreground).fixed_pos(body.min).constrain(false).show(ui.ctx(), |ui| {
        let (_, fundo) = ui.allocate_exact_size(body.size(), Sense::click());
        let p = ui.painter();
        p.rect_filled(body, 0.0, Color32::from_black_alpha(200));
        p.rect_filled(frame, R_MD(), Color32::from_rgb(14, 14, 16));
        ui::icon(ui, ic!("post-moment"), frame.center() - Vec2::new(0.0, 24.0), 48.0, Color32::from_white_alpha(140));
        text(p, frame.center() + Vec2::new(0.0, 26.0), Align2::CENTER_CENTER, &format!("A live de {canal}, 10 s antes deste post"), f7(15.0), Color32::WHITE, 0.0);
        text(p, frame.center() + Vec2::new(0.0, 50.0), Align2::CENTER_CENTER, "O player da Twitch toca no navegador.", f6(13.0), Color32::from_white_alpha(160), 0.0);
        let link = text(p, Pos2::new(frame.left(), frame.bottom() + 24.0), Align2::LEFT_CENTER, "Abrir na Twitch", f7(13.5), Color32::WHITE, 0.0);
        let lr = hand(ui.interact(link.expand(4.0), ui.id().with("momento-twitch"), Sense::click()));
        if lr.hovered() {
            p.rect_filled(Rect::from_min_size(Pos2::new(link.left(), link.bottom()), Vec2::new(link.width(), 1.0)), 0.0, Color32::WHITE);
        }
        if lr.clicked() {
            ui.ctx().open_url(egui::OpenUrl::new_tab(&url));
        }
        let x = Rect::from_center_size(Pos2::new(frame.right() - 16.0, frame.bottom() + 24.0), Vec2::splat(32.0));
        let xr = hand(ui.interact(x, ui.id().with("momento-x"), Sense::click())).on_hover_text("Fechar");
        p.circle_filled(x.center(), 16.0, if xr.hovered() { Color32::from_white_alpha(60) } else { Color32::from_white_alpha(30) });
        ui::icon(ui, ic!("x"), x.center(), 16.0, Color32::WHITE);
        if xr.clicked() || (fundo.clicked() && !ui.input(|i| i.pointer.interact_pos()).is_some_and(|q| frame.contains(q))) {
            fechar = true;
        }
    });
    if fechar || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        st.moment = None;
    }
}

/* ================= escrever um post (.cw) ================= */

fn composer(ui: &mut egui::Ui, st: &mut HomeState, panel: Rect, t: f64) {
    let p = ui.painter().clone();
    p.rect_filled(panel, R_MD(), CREAM_0());
    let o = panel.min + Vec2::splat(11.0);
    let iw = panel.width() - 22.0;

    // quem escreve
    let logado = !st.login.is_empty();
    avatar(ui, if logado { &st.name } else { "?" }, &st.avatar, o + Vec2::splat(17.0), 34.0, f8(12.0));
    ui::clip(&p, o + Vec2::new(42.0, 10.4), Align2::LEFT_CENTER, if logado { &st.name } else { "Você" }, f7(13.0), INK(), iw - 42.0);
    let sub = if logado { format!("@{}", st.login) } else { "Conecte a conta para publicar".to_owned() };
    ui::clip(&p, o + Vec2::new(42.0, 24.8), Align2::LEFT_CENTER, &sub, f6(11.0), FAINT(), iw - 42.0);

    // o título
    let title = Rect::from_min_size(o + Vec2::new(0.0, 40.0), Vec2::new(iw, 22.0));
    let hint = egui::RichText::new("Título (opcional)").font(patua(14.0)).color(FAINT());
    ui.put(
        title,
        egui::TextEdit::singleline(&mut st.title).id(ui.id().with("cw-titulo")).frame(false).hint_text(hint).font(patua(14.0)).text_color(INK()).char_limit(TITLE_MAX).vertical_align(Align::Center).margin(egui::Margin::ZERO),
    );

    // a caixa de escrever: 120 px, fundo da página, borda âmbar quando está escrevendo
    let field = Rect::from_min_size(o + Vec2::new(0.0, 68.0), Vec2::new(iw, 120.0));
    let text_id = ui.id().with("cw-texto");
    p.rect_filled(field, R_SM(), CREAM_1());
    if ui.memory(|m| m.has_focus(text_id)) {
        p.rect_stroke(field, R_SM(), Stroke::new(1.0_f32, SECONDARY()), StrokeKind::Inside);
    }
    // a alça do canto, que o navegador desenha no `resize:vertical`
    let g = field.right_bottom() - Vec2::new(3.0, 3.0);
    for k in [7.0, 3.5] {
        p.line_segment([g - Vec2::new(k, 0.0), g - Vec2::new(0.0, k)], Stroke::new(1.0_f32, FAINT()));
    }
    let inner = field.shrink2(Vec2::new(10.0, 8.0));
    let mut child = ui.new_child(UiBuilder::new().max_rect(inner));
    child.set_clip_rect(inner.expand(1.0));
    egui::ScrollArea::vertical().id_salt("cw-rolagem").auto_shrink(false).show(&mut child, |ui| {
        let mut layouter = |ui: &egui::Ui, s: &str, wrap: f32| {
            let mut job = LayoutJob::single_section(s.to_owned(), TextFormat { font_id: f6(13.0), color: INK(), line_height: Some(18.85), valign: Align::Center, ..Default::default() });
            job.wrap.max_width = wrap;
            ui.fonts(|f| f.layout_job(job))
        };
        let hint = egui::RichText::new("Escreva algo… ou solte uma imagem.").font(f6(13.0)).color(FAINT());
        ui.add(
            egui::TextEdit::multiline(&mut st.body)
                .id(text_id)
                .frame(false)
                .hint_text(hint)
                .char_limit(TEXT_MAX)
                .desired_width(inner.width())
                .min_size(inner.size())
                .margin(egui::Margin::ZERO)
                .layouter(&mut layouter),
        );
    });

    // as ferramentas: imagem, texto verde, o contador e o Publicar
    let y = o.y + 194.5;
    let img = Rect::from_min_size(Pos2::new(o.x, y), Vec2::splat(28.0));
    let r = hand(ui.interact(img, ui.id().with("cw-imagem"), Sense::click())).on_hover_text("Inserir imagem");
    p.rect_filled(img, R_SM(), if r.hovered() { AMBER_SOFT() } else { CREAM_2() });
    ui::icon(ui, ic!("cw-image"), img.center(), 15.0, if r.hovered() { INK() } else { SOFT() });
    if r.clicked() && st.dialogo.is_none() {
        st.dialogo = Some(std::thread::spawn(|| rfd::FileDialog::new().set_title("Inserir imagem").add_filter("imagem", &["png", "jpg", "jpeg", "webp", "gif"]).pick_file()));
    }
    if st.dialogo.as_ref().is_some_and(|h| h.is_finished()) {
        if let Some(Ok(Some(path))) = st.dialogo.take().map(|h| h.join()) {
            match anexar(ui.ctx(), &path) {
                Some(n) => st.anexo = Some(n),
                None => st.say("Esse arquivo não é uma imagem que dê para abrir.", t),
            }
        }
    }
    if st.dialogo.is_some() {
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(200));
    }
    // "ou solte uma imagem": o arquivo solto em cima do composer vira o anexo
    let soltos = ui.input(|i| if i.pointer.hover_pos().is_some_and(|q| panel.contains(q)) || i.pointer.hover_pos().is_none() { i.raw.dropped_files.clone() } else { vec![] });
    for f in soltos {
        if let Some(path) = f.path {
            if let Some(n) = anexar(ui.ctx(), &path) {
                st.anexo = Some(n);
            }
        }
    }
    // a miniatura do anexo, com o × para tirar
    if let Some((src, real)) = st.anexo.as_deref().and_then(mock_image) {
        let th = Rect::from_min_size(Pos2::new(o.x + 68.0, y), Vec2::new(40.0, 28.0));
        p.rect_filled(th, R_SM(), CREAM_2());
        cover(ui, src, real, th, R_SM());
        let x = Rect::from_center_size(th.right_top() + Vec2::new(-2.0, 2.0), Vec2::splat(16.0));
        let xr = hand(ui.interact(x, ui.id().with("cw-tirar"), Sense::click())).on_hover_text("Tirar a imagem");
        p.circle_filled(x.center(), 8.0, if xr.hovered() { ACCENT() } else { INK() });
        ui::icon(ui, ic!("x"), x.center(), 10.0, CREAM_0());
        if xr.clicked() {
            st.anexo = None;
        }
    }
    let q = Rect::from_min_size(Pos2::new(o.x + 34.0, y), Vec2::splat(28.0));
    let r = hand(ui.interact(q, ui.id().with("cw-verde"), Sense::click())).on_hover_text("Texto verde (começa a linha com >)");
    p.rect_filled(q, R_SM(), if r.hovered() { AMBER_SOFT() } else { CREAM_2() });
    ui::bold(&p, q.center() + Vec2::new(-0.25, 0.5), Align2::CENTER_CENTER, ">", patua(13.0), QUOTE_TOOL, 0.0);
    if r.clicked() {
        quote_line(ui.ctx(), text_id, &mut st.body);
    }

    let send = Rect::from_min_size(Pos2::new(o.x + iw - 71.8, o.y + 194.0), Vec2::new(71.8, 29.0));
    let can = (!st.body.trim().is_empty() || st.anexo.is_some()) && !st.enviando;
    let n = st.body.chars().count();
    if n > 0 {
        let c = if n > 1800 { ACCENT() } else { FAINT() };
        text(&p, Pos2::new(send.left() - 6.0, send.center().y), Align2::RIGHT_CENTER, &format!("{n}/{TEXT_MAX}"), f7(10.5), c, 0.0);
    }
    let r = ui.interact(send, ui.id().with("cw-publicar"), if can { Sense::click() } else { Sense::hover() });
    let r = if can { hand(r) } else { r.on_hover_cursor(CursorIcon::NotAllowed) };
    let (bg, fg) = if can { (SECONDARY(), ON_SECONDARY()) } else { (mix(SECONDARY(), CREAM_0(), 0.45), mix(ON_SECONDARY(), CREAM_0(), 0.45)) };
    p.rect_filled(send, R_SM(), bg);
    ui::bold(&p, send.center() + Vec2::new(-0.25, 0.5), Align2::CENTER_CENTER, if st.enviando { "…" } else { "Publicar" }, patua(12.0), fg, 0.0);
    if r.clicked() {
        st.publish(ui.ctx(), t);
    }

    // os atalhos embaixo
    let mut x = o.x;
    let links: [(ImageSource<'static>, &str, &str); 3] = [
        (ic!("cw-user"), "Meu perfil", ""),
        (ic!("cw-moon"), "Tema", "tema"),
        (ic!("cw-logout"), if logado { "Sair" } else { "Entrar" }, "sair"),
    ];
    for (icon, label, later) in links {
        let w = 8.0 + 13.0 + 5.0 + text_width(&p, label, f6(11.5), 0.0) + 8.0;
        let rect = Rect::from_min_size(Pos2::new(x, o.y + 236.0), Vec2::new(w, 25.0));
        let r = hand(ui.interact(rect, ui.id().with(("cw-atalho", label)), Sense::click()));
        if r.hovered() {
            p.rect_filled(rect, R_SM(), CREAM_2());
        }
        let c = if r.hovered() { INK() } else { SOFT() };
        ui::icon(ui, icon, Pos2::new(rect.left() + 8.0 + 6.5, rect.center().y), 13.0, mix(c, if r.hovered() { CREAM_2() } else { CREAM_0() }, 0.7));
        text(&p, Pos2::new(rect.left() + 26.0, rect.center().y), Align2::LEFT_CENTER, label, f6(11.5), c, 0.0);
        if r.clicked() {
            if later.is_empty() {
                if logado {
                    st.open_profile = true;
                } else {
                    st.pedir_login = true;
                }
            } else if later == "tema" {
                st.alternar_tema = true;
            } else if logado {
                st.pedir_sair = true;
            } else {
                st.pedir_login = true;
            }
        }
        x += w + 4.0;
    }
}

/// O botão `>`: põe "> " no começo da linha em que o cursor está e deixa o cursor onde estava.
fn quote_line(ctx: &egui::Context, id: egui::Id, body: &mut String) {
    let mut state = egui::TextEdit::load_state(ctx, id).unwrap_or_default();
    let at = state.cursor.char_range().map(|r| r.primary.index).unwrap_or_else(|| body.chars().count());
    let chars: Vec<char> = body.chars().collect();
    let start = chars[..at.min(chars.len())].iter().rposition(|&c| c == '\n').map(|i| i + 1).unwrap_or(0);
    let byte = body.char_indices().nth(start).map(|(b, _)| b).unwrap_or(body.len());
    body.insert_str(byte, "> ");
    state.cursor.set_char_range(Some(egui::text::CCursorRange::one(egui::text::CCursor::new(at + 2))));
    state.store(ctx, id);
    ctx.memory_mut(|m| m.request_focus(id));
}

/* ================= o feed (.post) ================= */

/// O que um post ocupa, medido antes de pintar: a lista precisa da altura de cada um para reservar o espaço.
struct Layout {
    title: Option<Arc<Galley>>,
    text: Option<Arc<Galley>>,
    media: Option<(ImageSource<'static>, Vec2, Vec2)>,
    replies: Vec<Arc<Galley>>,
    height: f32,
}

fn measure(p: &Painter, post: &Post, cw: f32, replying: bool, inteira: bool) -> Layout {
    let title = (!post.title.is_empty()).then(|| para(p, &post.title, patua(18.0), INK(), cw, 23.4, false));
    let text = (!post.text.is_empty()).then(|| para(p, &post.text, f6(15.0), INK(), cw, 22.5, true));
    // a imagem inteira, sem corte: até a largura do post e até 480 px de altura
    let media = mock_image(&post.image).map(|(src, real)| {
        let real = if post.image.starts_with("envio-") { real } else { post.image_size.or_else(|| tamanho_remoto(p.ctx(), &src)).unwrap_or(Vec2::new(cw, cw * 9.0 / 16.0)) };
        let k = (cw / real.x).min(480.0 / real.y).min(1.0);
        (src, real, real * k)
    });
    let shown = if inteira { &post.replies[..] } else { &post.replies[post.replies.len().saturating_sub(PREVIEW_MAX)..] };
    let replies: Vec<Arc<Galley>> = shown.iter().map(|r| para(p, &r.body, f6(13.0), INK(), cw - 12.0, 18.85, true)).collect();

    let mut h = 1.0 + 14.0 + 36.0;
    if let Some(g) = &title {
        h += GAP - 2.0 + g.size().y;
    }
    if let Some(g) = &text {
        h += 8.0 + g.size().y;
    }
    if let Some((_, _, s)) = &media {
        h += 8.0 + s.y;
    }
    h += 8.0 + 34.0;
    if replying {
        h += 8.0 + 38.0;
    }
    if !replies.is_empty() {
        // na conversa inteira cada comentário ganha o "Responder" embaixo
        let extra = if inteira { 18.0 } else { 0.0 };
        h += 8.0 + 2.0 + replies.iter().map(|g| 3.0 + 15.0 + 2.0 + g.size().y + 3.0 + extra).sum::<f32>() + 8.0 * (replies.len() - 1) as f32 + 2.0;
    }
    h += 10.0 + 1.0;
    Layout { title, text, media, replies, height: h }
}

fn feed(ui: &mut egui::Ui, st: &mut HomeState, col: Rect, t: f64) {
    let area = col.shrink(1.0);
    let mut child = ui.new_child(UiBuilder::new().max_rect(area));
    let mut actions = Vec::new();
    egui::ScrollArea::vertical().id_salt("feed").auto_shrink(false).show(&mut child, |ui| {
        let lw = FEED_W.min(area.width() - 8.0);
        let x0 = area.center().x - lw / 2.0;
        let n = st.posts.len();
        for i in 0..n {
            let replying = st.reply_to == Some(st.posts[i].id);
            let lay = measure(ui.painter(), &st.posts[i], lw - 34.0, replying, st.thread == Some(st.posts[i].id));
            let (row, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), lay.height), Sense::hover());
            let rect = Rect::from_min_size(Pos2::new(x0, row.top()), Vec2::new(lw, lay.height));
            if ui.is_rect_visible(rect) {
                post(ui, st, i, rect, lay, &mut actions);
            }
            if i + 1 < n {
                ui.add_space(GAP);
            }
        }
        let (r, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), if n == 0 { 106.0 } else { 60.0 }), Sense::hover());
        let msg = if let Some(e) = &st.erro {
            e.clone()
        } else if st.pagina.is_some() {
            "Carregando…".to_owned()
        } else if n == 0 {
            "Nada por aqui ainda. Seja o primeiro a postar!".to_owned()
        } else {
            String::new()
        };
        text(ui.painter(), r.center(), Align2::CENTER_CENTER, &msg, f6(14.0), FAINT(), 0.0);
        // chegou no fim da lista: a próxima página
        if n > 0 && ui.is_rect_visible(r) && st.erro.is_none() {
            st.mais(ui.ctx());
        }
    });
    for a in actions {
        act(ui.ctx(), st, a, t);
    }
}

enum Action {
    Vote(u32, i32),
    Reply(u32),
    SendReply(u32),
    Comments(u32),
    Moment(u32),
    Menu(u32, Rect),
    /// "Responder" num comentário da conversa: abre a caixa já citando o número dele.
    Quote(u32, usize),
    Perfil(i64),
    Abrir(String),
}

fn act(ctx: &egui::Context, st: &mut HomeState, a: Action, t: f64) {
    let find = |st: &mut HomeState, id: u32| st.posts.iter().position(|p| p.id == id);
    match a {
        Action::Vote(id, dir) => {
            if crate::conta::eu().is_none() {
                st.pedir_login = true;
                return;
            }
            if let Some(i) = find(st, id) {
                let p = &mut st.posts[i];
                // votar de novo no mesmo lado tira o voto
                let next = if p.my_vote == dir { 0 } else { dir };
                p.score += next - p.my_vote;
                p.my_vote = next;
                st.voltas.push(net::rodar(ctx, move || {
                    let r = net::pedir_ja("POST", &format!("/api/feed/{id}/vote"), Some(&serde_json::json!({ "value": next }))).and_then(|v| serde_json::from_value::<VoteResp>(v).map_err(|e| net::Erro { status: 0, msg: e.to_string() }));
                    Ok(Volta::Voto(id, r))
                }));
            }
        }
        Action::Reply(id) => {
            if crate::conta::eu().is_none() {
                st.pedir_login = true;
                return;
            }
            st.reply_to = if st.reply_to == Some(id) { None } else { Some(id) };
            st.reply.clear();
        }
        Action::SendReply(id) => {
            let body = st.reply.trim().to_owned();
            if body.is_empty() {
                return;
            }
            st.voltas.push(net::rodar(ctx, move || {
                let r = net::pedir_ja("POST", &format!("/api/feed/{id}/comments"), Some(&serde_json::json!({ "body": body }))).and_then(|v| serde_json::from_value::<ApiComment>(v.get("comment").cloned().unwrap_or_default()).map_err(|e| net::Erro { status: 0, msg: e.to_string() }));
                Ok(Volta::Resposta(id, r))
            }));
            st.reply_to = None;
            st.reply.clear();
        }
        Action::Comments(id) => {
            // abre a conversa inteira com a caixa de comentário pronta; o segundo clique fecha
            if st.thread == Some(id) {
                st.thread = None;
                if st.reply_to == Some(id) && st.reply.is_empty() {
                    st.reply_to = None;
                }
            } else {
                st.thread = Some(id);
                st.reply_to = Some(id);
            }
        }
        Action::Moment(id) => st.moment = Some(id),
        Action::Quote(id, n) => {
            st.reply_to = Some(id);
            st.reply = format!("> #{n}\n");
        }
        Action::Perfil(pid) => {
            if pid > 0 && pid == crate::conta::perfil_id() {
                st.open_profile = true;
            } else if pid > 0 {
                st.open_other = Some(pid);
            } else {
                st.say("Essa pessoa ainda não tem perfil no Brittico.", t);
            }
        }
        Action::Abrir(url) => st.abrir_url = Some(url),
        Action::Menu(id, at) => st.menu = if st.menu.map(|m| m.0) == Some(id) { None } else { Some((id, at)) },
    }
}

/// Um `.post`: quem postou · o título · o texto · a imagem · a barra (votos, "Ver o momento", conversa) · as respostas.
fn post(ui: &mut egui::Ui, st: &mut HomeState, i: usize, rect: Rect, lay: Layout, actions: &mut Vec<Action>) {
    let p = ui.painter().clone();
    let post = &st.posts[i];
    let id = post.id;
    p.rect_filled(rect, R_LG(), CREAM_0());
    let x = rect.left() + 17.0;
    let cw = rect.width() - 34.0;
    let mut y = rect.top() + 15.0;

    // o cabeçalho
    let hc = y + 18.0;
    avatar(ui, &post.username, &post.avatar, Pos2::new(x + 18.0, hc), 36.0, f8(12.5));
    let user = text(&p, Pos2::new(x + 46.0, hc), Align2::LEFT_CENTER, &post.username, f7(13.5), INK(), 0.0);
    let ur = hand(ui.interact(user.expand(2.0), ui.id().with(("autor", id)), Sense::click()));
    if ur.hovered() {
        p.rect_filled(Rect::from_min_size(Pos2::new(user.left(), user.bottom() - 1.0), Vec2::new(user.width(), 1.0)), 0.0, INK());
    }
    if ur.clicked() {
        actions.push(Action::Perfil(post.profile_id));
    }
    let tag = if post.chat { if post.channel.is_empty() { "kennzy" } else { &post.channel } } else { "brittico" };
    let tw = text_width(&p, tag, f7(11.0), 0.0) + 16.0;
    let pill = Rect::from_min_size(Pos2::new(user.right() + 6.0, hc - 8.6), Vec2::new(tw, 17.2));
    p.rect_filled(pill, 8.6, AMBER_SOFT());
    text(&p, pill.center(), Align2::CENTER_CENTER, tag, f7(11.0), SECONDARY(), 0.0);
    let dot = text(&p, Pos2::new(pill.right() + 6.0, hc), Align2::LEFT_CENTER, "·", f6(13.5), FAINT(), 0.0);
    let when = local(post.ts).format("%d/%m/%Y %H:%M").to_string();
    let time = text(&p, Pos2::new(dot.right() + 6.0, hc), Align2::LEFT_CENTER, &ago_short(post.ts), f6(13.5), FAINT(), 0.0);
    // a hora é o link do post (a página dele mora no site)
    if hand(ui.interact(time, ui.id().with(("quando", id)), Sense::click())).on_hover_text(format!("{when} · abrir o post no navegador")).clicked() {
        actions.push(Action::Abrir(format!("https://brittico.xyz/{}/feed/{id}", post.username)));
    }
    let more = Rect::from_center_size(Pos2::new(rect.right() - 17.0 - 16.0, hc), Vec2::splat(32.0));
    let r = hand(ui.interact(more, ui.id().with(("mais", id)), Sense::click())).on_hover_text("Mais");
    let open = st.menu.map(|m| m.0) == Some(id);
    if r.hovered() || open {
        p.circle_filled(more.center(), 16.0, CREAM_2());
    }
    ui::icon(ui, ic!("post-more"), more.center(), 18.0, if r.hovered() || open { INK() } else { FAINT() });
    if r.clicked() {
        actions.push(Action::Menu(id, more));
    }
    y += 36.0;

    if let Some(g) = lay.title {
        y += 8.0;
        let h = g.size().y;
        p.galley(Pos2::new(x, y), g, INK());
        y += h;
    }
    if let Some(g) = lay.text {
        y += 8.0;
        let h = g.size().y;
        p.galley(Pos2::new(x, y), g, INK());
        y += h;
    }
    if let Some((src, _, size)) = lay.media {
        y += 8.0;
        let r = Rect::from_min_size(Pos2::new(x, y), size);
        p.rect_filled(r, R_MD(), CREAM_2());
        ui::image(ui, src, r, R_MD());
        y += size.y;
    }

    // a barra: votos à esquerda, "Ver o momento" no meio, a conversa à direita
    y += 8.0;
    let bar = Rect::from_min_size(Pos2::new(x - 6.0, y), Vec2::new(cw + 12.0, 34.0));
    let vote = Rect::from_min_size(bar.min, Vec2::new(86.0, 34.0));
    p.rect_filled(vote, 17.0, CREAM_2());
    for (dir, cx, icon, on_c) in [(1, 17.0, ic!("vote-up"), SECONDARY()), (-1, 69.0, ic!("vote-down"), DOWN)] {
        let b = Rect::from_center_size(Pos2::new(vote.left() + cx, vote.center().y), Vec2::splat(30.0));
        let r = hand(ui.interact(b, ui.id().with(("voto", id, dir)), Sense::click())).on_hover_text(if dir > 0 { "Upvote" } else { "Downvote" });
        if r.hovered() {
            p.circle_filled(b.center(), 15.0, CREAM_3());
        }
        let c = if post.my_vote == dir { on_c } else if r.hovered() { INK() } else { SOFT() };
        // `transform:scale(.85)` enquanto aperta
        ui::icon(ui, icon, b.center(), if r.is_pointer_button_down_on() { 14.5 } else { 17.0 }, c);
        if r.clicked() {
            actions.push(Action::Vote(id, dir));
        }
    }
    let sc = match post.score {
        s if s > 0 => SECONDARY(),
        s if s < 0 => DOWN,
        _ => INK(),
    };
    text(&p, Pos2::new(vote.left() + 43.0, vote.center().y), Align2::CENTER_CENTER, &num(post.score as i64), f8(13.0), sc, 0.0);

    let pill_act = |ui: &mut egui::Ui, left: f32, icon: ImageSource<'static>, label: &str, key: &str, tip: &str| -> egui::Response {
        let w = 12.0 + 17.0 + 6.0 + text_width(ui.painter(), label, f7(13.0), 0.0) + 12.0;
        let r = Rect::from_min_size(Pos2::new(left, bar.top()), Vec2::new(w, 34.0));
        let resp = hand(ui.interact(r, ui.id().with((key, id)), Sense::click())).on_hover_text(tip);
        if resp.hovered() {
            ui.painter().rect_filled(r, 17.0, CREAM_2());
        }
        let c = if resp.hovered() { INK() } else { SOFT() };
        ui::icon(ui, icon, Pos2::new(r.left() + 12.0 + 8.5, r.center().y), 17.0, c);
        text(ui.painter(), Pos2::new(r.left() + 35.0, r.center().y), Align2::LEFT_CENTER, label, f7(13.0), c, 0.0);
        resp
    };
    let width_of = |ui: &egui::Ui, label: &str| 12.0 + 17.0 + 6.0 + text_width(ui.painter(), label, f7(13.0), 0.0) + 12.0;
    if post.chat {
        let w = width_of(ui, "Ver o momento");
        if pill_act(ui, bar.center().x - w / 2.0, ic!("post-moment"), "Ver o momento", "momento", "Ver na live, 10 s antes deste post").clicked() {
            actions.push(Action::Moment(id));
        }
    }
    let mut right = bar.right() - width_of(ui, "Responder");
    if pill_act(ui, right, ic!("post-reply"), "Responder", "responder", "Responder").clicked() {
        actions.push(Action::Reply(id));
    }
    if post.comment_count > 5 {
        // só o balão e o número: com a palavra, a direita fica larga e empurra o "Ver o momento"
        let label = num(post.comment_count as i64);
        right -= 6.0 + width_of(ui, &label);
        if pill_act(ui, right, ic!("post-comments"), &label, "conversa", &format!("{label} comentários")).clicked() {
            actions.push(Action::Comments(id));
        }
    }
    y += 34.0;

    // o "Responder" aberto: a caixa e o Enviar
    if st.reply_to == Some(id) {
        y += 8.0;
        let send_w = 16.0 + text_width(&p, "Enviar", f7(13.0), 0.0) + 16.0;
        let field = Rect::from_min_size(Pos2::new(x, y), Vec2::new(cw - 8.0 - send_w, 38.0));
        let fid = ui.id().with(("resposta", id));
        if ui.memory(|m| m.has_focus(fid)) {
            p.rect_stroke(field, R_MD(), Stroke::new(1.0_f32, SECONDARY()), StrokeKind::Inside);
        }
        let hint = egui::RichText::new("Escreva uma resposta… (use > para citar)").font(f6(13.5)).color(FAINT());
        // num Ui filho: o `put` direto na lista puxaria o cursor dela de volta para cá, e o post de baixo subiria
        let mut child = ui.new_child(UiBuilder::new().max_rect(field.shrink2(Vec2::new(12.0, 9.0))));
        let out = child.add(egui::TextEdit::multiline(&mut st.reply).id(fid).frame(false).hint_text(hint).font(f6(13.5)).text_color(INK()).margin(egui::Margin::ZERO).desired_rows(1).desired_width(field.width() - 24.0));
        if !ui.memory(|m| m.focused().is_some()) {
            out.request_focus();
        }
        let send = Rect::from_min_size(Pos2::new(field.right() + 8.0, field.bottom() - 36.0), Vec2::new(send_w, 36.0));
        let r = hand(ui.interact(send, ui.id().with(("enviar", id)), Sense::click()));
        p.rect_filled(send, R_SM(), SECONDARY());
        text(&p, send.center(), Align2::CENTER_CENTER, "Enviar", f7(13.0), ON_SECONDARY(), 0.0);
        if r.clicked() {
            actions.push(Action::SendReply(id));
        }
        y += 38.0;
    }

    // as últimas respostas, embaixo do post
    if !lay.replies.is_empty() {
        y += 8.0 + 2.0;
        let inteira = st.thread == Some(id);
        let first = if inteira { 0 } else { post.replies.len().saturating_sub(PREVIEW_MAX) };
        let shown = &post.replies[first..];
        for (k, (r, g)) in shown.iter().zip(lay.replies).enumerate() {
            if k > 0 {
                y += 8.0;
            }
            let h = 3.0 + 15.0 + 2.0 + g.size().y + 3.0 + if inteira { 18.0 } else { 0.0 };
            p.rect_filled(Rect::from_min_size(Pos2::new(x, y), Vec2::new(2.0, h)), 0.0, HAIRLINE_2());
            let op = r.author == post.username;
            let (who, c) = if op { (format!("{} (OP)", r.author), ACCENT()) } else { (r.author.clone(), SECONDARY()) };
            let mut ax = x + 12.0;
            if inteira {
                // a conversa inteira numera os comentários (é o número que o "> #n" cita)
                ax = text(&p, Pos2::new(ax, y + 3.0 + 7.5), Align2::LEFT_CENTER, &format!("#{}", first + k + 1), f7(11.5), FAINT(), 0.0).right() + 6.0;
            }
            let a = text(&p, Pos2::new(ax, y + 3.0 + 7.5), Align2::LEFT_CENTER, &who, f7(11.5), c, 0.0);
            text(&p, Pos2::new(a.right() + 6.0, y + 3.0 + 7.5), Align2::LEFT_CENTER, &local(r.ts).format("%d/%m/%Y %H:%M").to_string(), f6(11.5), FAINT(), 0.0);
            let gh = g.size().y;
            p.galley(Pos2::new(x + 12.0, y + 3.0 + 15.0 + 2.0), g, INK());
            if inteira {
                let ry = y + 3.0 + 15.0 + 2.0 + gh + 2.0 + 7.0;
                let rr = text(&p, Pos2::new(x + 12.0, ry), Align2::LEFT_CENTER, "Responder", f7(11.0), FAINT(), 0.0);
                let resp = hand(ui.interact(rr.expand(2.0), ui.id().with(("responder-n", id, k)), Sense::click()));
                if resp.hovered() {
                    text(&p, Pos2::new(x + 12.0, ry), Align2::LEFT_CENTER, "Responder", f7(11.0), SECONDARY(), 0.0);
                }
                if resp.clicked() {
                    actions.push(Action::Quote(id, first + k + 1));
                }
            }
            y += h;
        }
    }
}

/// O menu do "…": Denunciar e, para a moderação, Apagar post.
fn post_menu(ui: &mut egui::Ui, st: &mut HomeState, _t: f64) {
    let Some((id, at)) = st.menu else { return };
    let pode_apagar = st.posts.iter().find(|p| p.id == id).is_some_and(|p| p.mine || (p.profile_id > 0 && p.profile_id == crate::conta::perfil_id())) || crate::conta::eu().is_some_and(|m| m.is_mod);
    let n_items = if pode_apagar { 2 } else { 1 };
    let mut chosen = None;
    let area = egui::Area::new(ui.id().with("menu-do-post")).order(Order::Foreground).fixed_pos(Pos2::new(at.right() - 170.0, at.top() + 34.0)).show(ui.ctx(), |ui| {
        let (rect, _) = ui.allocate_exact_size(Vec2::new(170.0, n_items as f32 * 40.0 + 2.0), Sense::hover());
        let p = ui.painter();
        p.rect_filled(rect.translate(Vec2::new(0.0, 8.0)).expand(6.0), R_MD() + 6.0, Color32::from_black_alpha(40));
        p.rect_filled(rect, R_MD(), CREAM_0());
        p.rect_stroke(rect, R_MD(), Stroke::new(1.0_f32, HAIRLINE_2()), StrokeKind::Inside);
        let items: [(ImageSource<'static>, &str, bool); 2] = [(ic!("post-flag"), "Denunciar", false), (ic!("post-trash"), "Apagar post", true)];
        for (k, (icon, label, danger)) in items.into_iter().take(n_items).enumerate() {
            let r = Rect::from_min_size(rect.min + Vec2::new(1.0, 1.0 + 40.0 * k as f32), Vec2::new(168.0, 40.0));
            let resp = hand(ui.interact(r, ui.id().with(("item", k)), Sense::click()));
            if resp.hovered() {
                let round = if n_items == 1 { egui::CornerRadius::same(13) } else if k == 0 { egui::CornerRadius { nw: 13, ne: 13, sw: 0, se: 0 } } else { egui::CornerRadius { nw: 0, ne: 0, sw: 13, se: 13 } };
                ui.painter().rect_filled(r, round, CREAM_2());
            }
            let (ic_c, c) = if danger { (ACCENT(), ACCENT()) } else { (SOFT(), INK()) };
            ui::icon(ui, icon, Pos2::new(r.left() + 14.0 + 8.0, r.center().y), 16.0, ic_c);
            text(ui.painter(), Pos2::new(r.left() + 40.0, r.center().y), Align2::LEFT_CENTER, label, f6(13.0), c, 0.0);
            if resp.clicked() {
                chosen = Some(k);
            }
        }
    });
    match chosen {
        Some(0) => {
            st.menu = None;
            if crate::conta::eu().is_none() {
                st.pedir_login = true;
                return;
            }
            st.voltas.push(net::rodar(ui.ctx(), move || Ok(Volta::Denuncia(net::pedir_ja("POST", &format!("/api/feed/{id}/report"), Some(&serde_json::json!({ "reason": "" })))))));
        }
        Some(_) => {
            st.menu = None;
            st.voltas.push(net::rodar(ui.ctx(), move || Ok(Volta::Apagado(id, net::pedir_ja("DELETE", &format!("/api/feed/{id}"), None)))));
        }
        // clicar fora fecha (o próprio "…" já alterna sozinho)
        None if area.response.clicked_elsewhere() && !ui.rect_contains_pointer(at) => st.menu = None,
        None => {}
    }
}

/* ================= o perfil (.pf) e o texto do Brittico ================= */

fn side(ui: &mut egui::Ui, st: &mut HomeState, col: Rect) {
    if st.login.is_empty() {
        // sem conta: o convite para entrar no lugar do perfil
        let panel = Rect::from_min_size(col.min, Vec2::new(col.width(), 132.0));
        let p = ui.painter().clone();
        p.rect_filled(panel, R_MD(), CREAM_0());
        ui::bold(&p, panel.min + Vec2::new(14.0, 26.0), Align2::LEFT_CENTER, "Seu perfil", patua(16.0), INK(), 0.0);
        let g = p.layout("Conecte a conta para publicar, votar, jogar e entrar na call.".into(), f6(12.0), SOFT(), panel.width() - 28.0);
        p.galley(panel.min + Vec2::new(14.0, 44.0), g, SOFT());
        let b = Rect::from_min_size(panel.min + Vec2::new(14.0, 84.0), Vec2::new(150.0, 34.0));
        let r = hand(ui.interact(b, ui.id().with("lado-entrar"), Sense::click()));
        p.rect_filled(b, R_SM(), if r.hovered() { mix(SECONDARY(), Color32::WHITE, 0.08) } else { SECONDARY() });
        text(&p, b.center(), Align2::CENTER_CENTER, "Conectar conta", f7(13.0), ON_SECONDARY(), 0.0);
        if r.clicked() {
            st.pedir_login = true;
        }
        return;
    }
    let o = col.min + Vec2::splat(11.0);
    let iw = col.width() - 22.0;
    // a altura do painel sai da lista de posts, que é a única parte que muda
    let rows_top = o.y + 325.0;
    let rows: Vec<f32> = st.my_posts.iter().map(|m| if mock_image(&m.image).is_some() { 40.0 } else { 38.8 }).collect();
    let panel = Rect::from_min_max(col.min, Pos2::new(col.right(), rows_top + rows.iter().sum::<f32>().max(if st.my_posts.is_empty() { 20.0 } else { 0.0 }) + 11.0));
    let p = ui.painter().clone();
    p.rect_filled(panel, R_MD(), CREAM_0());

    // quem é, e o "ver perfil" no canto
    avatar(ui, &st.name, &st.avatar, o + Vec2::splat(19.0), 38.0, f8(13.0));
    ui::clip(&p, o + Vec2::new(47.0, 12.2), Align2::LEFT_CENTER, &st.name, f7(13.5), INK(), iw - 47.0 - 60.0);
    text(&p, o + Vec2::new(47.0, 27.5), Align2::LEFT_CENTER, &format!("no chat desde {}", local(st.since).format("%d/%m/%Y")), f6(11.0), FAINT(), 0.0);
    let link = text(&p, Pos2::new(panel.right() - 14.0, panel.top() + 14.0 + 7.5), Align2::RIGHT_CENTER, "ver perfil", f6(11.5), SOFT(), 0.0);
    let r = hand(ui.interact(link.expand(2.0), ui.id().with("ver-perfil"), Sense::click()));
    if r.clicked() {
        st.open_profile = true;
    }
    let c = if r.hovered() { SECONDARY() } else { HAIRLINE_2() };
    if r.hovered() {
        text(&p, link.left_center(), Align2::LEFT_CENTER, "ver perfil", f6(11.5), SECONDARY(), 0.0);
    }
    p.rect_filled(Rect::from_min_size(Pos2::new(link.left(), link.bottom() + 0.5), Vec2::new(link.width(), 1.0)), 0.0, c);

    // os três números
    let nw = (iw - 12.0) / 3.0;
    for (k, (v, label)) in [(st.points_month, "PTS NO MÊS"), (st.points_total, "PTS NO TOTAL"), (st.messages, "MENSAGENS")].into_iter().enumerate() {
        let b = Rect::from_min_size(o + Vec2::new(k as f32 * (nw + 6.0), 48.0), Vec2::new(nw, 51.0));
        p.rect_filled(b, R_SM(), CREAM_1());
        ui::bold(&p, Pos2::new(b.center().x - 0.25, b.top() + 18.0), Align2::CENTER_CENTER, &num(v), patua(16.0), INK(), 0.0);
        text(&p, Pos2::new(b.center().x, b.top() + 36.5), Align2::CENTER_CENTER, label, f7(9.5), FAINT(), 0.38);
    }

    // a porta da /stats/
    let gy = o.y + 114.5;
    let label = text(&p, Pos2::new(o.x + iw - 14.0, gy), Align2::RIGHT_CENTER, "Ver estatísticas globais", f6(11.0), FAINT(), 0.0);
    let hit = Rect::from_min_max(label.left_top(), Pos2::new(o.x + iw, label.bottom()));
    let r = hand(ui.interact(hit, ui.id().with("stats-globais"), Sense::click()));
    if r.hovered() {
        text(&p, label.left_center(), Align2::LEFT_CENTER, "Ver estatísticas globais", f6(11.0), INK(), 0.0);
        p.rect_filled(Rect::from_min_size(Pos2::new(label.left(), label.bottom() - 1.0), Vec2::new(label.width(), 1.0)), 0.0, INK());
    }
    ui::icon(ui, ic!("pf-chevron"), Pos2::new(o.x + iw - 6.0, gy), 12.0, if r.hovered() { INK() } else { FAINT() });
    if r.clicked() {
        ui.ctx().open_url(egui::OpenUrl::new_tab("https://brittico.xyz/stats/"));
    }

    if !st.activity_hidden {
        activity(ui, st, o + Vec2::new(0.0, 134.0), iw);
    }

    // os posts dela
    let gt = o.y + 312.0;
    let t = ui::bold(&p, Pos2::new(o.x, gt), Align2::LEFT_CENTER, "SEUS POSTS", patua(10.0), FAINT(), 0.5);
    if st.my_posts.is_empty() {
        text(&p, Pos2::new(o.x, gt + 22.0), Align2::LEFT_CENTER, "Nenhum post ainda.", f6(12.0), FAINT(), 0.0);
    }
    if st.my_posts_total as usize > st.my_posts.len() {
        text(&p, Pos2::new(t.right() + 3.0, gt), Align2::LEFT_CENTER, &format!("{} de {}", st.my_posts.len(), num(st.my_posts_total as i64)), patua(10.0), FAINT(), 0.0);
    }
    let mut y = rows_top;
    for (k, (m, h)) in st.my_posts.iter().zip(rows).enumerate() {
        let row = Rect::from_min_size(Pos2::new(o.x, y), Vec2::new(iw, h));
        let r = hand(ui.interact(row, ui.id().with(("meu-post", k)), Sense::click())).on_hover_text("Abrir o post no navegador");
        if r.clicked() {
            ui.ctx().open_url(egui::OpenUrl::new_tab(format!("https://brittico.xyz/{}/feed/{}", st.login, m.id)));
        }
        let mut mx = o.x;
        let my = if let Some((src, real)) = mock_image(&m.image) {
            let th = Rect::from_min_size(Pos2::new(o.x, y + 5.0), Vec2::new(38.0, 30.0));
            p.rect_filled(th, R_SM(), CREAM_2());
            cover(ui, src, real, th, R_SM());
            mx += 46.0;
            y + 5.65
        } else {
            y + 5.0
        };
        let first = m.text.lines().next().unwrap_or("");
        let title = if !m.title.is_empty() { m.title.as_str() } else if !first.is_empty() { first } else { "(imagem)" };
        let title: String = title.chars().take(90).collect();
        ui::clip(&p, Pos2::new(mx, my + 7.8), Align2::LEFT_CENTER, &title, f6(12.5), if r.hovered() { SECONDARY() } else { INK() }, o.x + iw - mx);
        let sub = format!("{} pts · {} coment. · {}", num(m.score), num(m.comments), ago_long(m.ts));
        ui::clip(&p, Pos2::new(mx, my + 22.15), Align2::LEFT_CENTER, &sub, f6(10.5), FAINT(), o.x + iw - mx);
        y += h;
    }

    // o que é o Brittico, embaixo do perfil
    let lw = col.width() - 20.0;
    let mut job = LayoutJob::single_section(SOBRE.to_owned(), TextFormat { font_id: f6(10.5), color: FAINT(), line_height: Some(15.75), valign: Align::Center, ..Default::default() });
    job.wrap.max_width = lw;
    job.halign = Align::Center;
    let g = p.layout_job(job);
    let about_h = g.size().y;
    let top = panel.bottom() + GAP;
    p.galley(Pos2::new(col.center().x, top), g, FAINT());
    let ly = top + about_h + 4.0 + 7.9;
    let names = ["Política de privacidade", "Termos de serviço"];
    let widths: Vec<f32> = names.iter().map(|s| text_width(&p, s, f6(10.5), 0.0)).collect();
    let mut x = col.center().x - (widths.iter().sum::<f32>() + 12.0) / 2.0;
    for (s, w) in names.iter().zip(widths) {
        let rect = Rect::from_min_size(Pos2::new(x, ly - 7.9), Vec2::new(w, 15.8));
        let r = hand(ui.interact(rect, ui.id().with(("legal", *s)), Sense::click()));
        if r.clicked() {
            ui.ctx().open_url(egui::OpenUrl::new_tab(if s.starts_with("Pol") { "https://brittico.xyz/privacidade/" } else { "https://brittico.xyz/termos/" }));
        }
        let c = if r.hovered() { INK() } else { FAINT() };
        text(&p, Pos2::new(x, ly), Align2::LEFT_CENTER, s, f6(10.5), c, 0.0);
        if r.hovered() {
            p.rect_filled(Rect::from_min_size(Pos2::new(x, ly + 7.0), Vec2::new(w, 1.0)), 0.0, INK());
        }
        x += w + 12.0;
    }
}

/// A grade de atividade (`.pf__act`): uma coluna por semana, de domingo a sábado, terminando no sábado desta semana.
/// Quantas semanas cabem sai da largura, como no site; os cortes de nível são os de /u/.
fn activity(ui: &mut egui::Ui, st: &HomeState, o: Pos2, iw: f32) {
    const SQ: f32 = 10.0;
    const STEP: f32 = 13.0;
    let p = ui.painter().clone();
    ui::bold(&p, Pos2::new(o.x, o.y + 15.0), Align2::LEFT_CENTER, "ATIVIDADE", patua(10.0), FAINT(), 0.5);

    let weeks = (((iw + 3.0) / STEP).floor() as i64).clamp(8, 53);
    let today = Local::now().date_naive();
    let end = today + chrono::Duration::days(6 - today.weekday().num_days_from_sunday() as i64);
    let start = end - chrono::Duration::days(weeks * 7 - 1);
    let level = |n: u32| match n {
        0 => 0,
        1..=4 => 1,
        5..=12 => 2,
        13..=29 => 3,
        _ => 4,
    };
    let color = |l: u8| match l {
        0 => CREAM_2(),
        1 => mix(SECONDARY(), CREAM_2(), 0.30),
        2 => mix(SECONDARY(), CREAM_2(), 0.55),
        3 => mix(SECONDARY(), CREAM_2(), 0.80),
        _ => SECONDARY(),
    };

    // os meses em cima: o nome só aparece quando o mês tem mais de uma coluna
    let mut months: Vec<(u32, i64, i64)> = Vec::new();
    for w in 0..weeks {
        let m = (start + chrono::Duration::days(w * 7)).month0();
        match months.last_mut() {
            Some(last) if last.0 == m => last.2 += 1,
            _ => months.push((m, w, 1)),
        }
    }
    for (m, w, n) in months {
        if n > 1 {
            text(&p, Pos2::new(o.x + w as f32 * STEP, o.y + 28.0 + 6.5), Align2::LEFT_CENTER, MESES[m as usize], f7(9.5), FAINT(), 0.0);
        }
    }

    let top = o.y + 44.0;
    let mut hovered = None;
    for w in 0..weeks {
        for d in 0..7 {
            let day: NaiveDate = start + chrono::Duration::days(w * 7 + d);
            if day > today {
                continue;
            }
            let ago = (today - day).num_days() as usize;
            let n = st.activity.get(ago).copied().unwrap_or(0);
            let r = Rect::from_min_size(Pos2::new(o.x + w as f32 * STEP, top + d as f32 * STEP), Vec2::splat(SQ));
            let resp = ui.interact(r, ui.id().with(("dia", w, d)), Sense::hover());
            let tip = if n > 0 { format!("{n} {} em {:02} de {}", if n == 1 { "interação" } else { "interações" }, day.day(), MESES_LONGOS[day.month0() as usize]) } else { format!("Nenhuma atividade em {:02} de {}", day.day(), MESES_LONGOS[day.month0() as usize]) };
            if resp.hovered() {
                hovered = Some((r, level(n)));
            }
            resp.on_hover_text(tip);
            p.rect_filled(r, 2.0, color(level(n)));
        }
    }
    // o quadrado com o mouse em cima cresce (`transform:scale(1.35)`) por cima dos vizinhos
    if let Some((r, l)) = hovered {
        p.rect_filled(Rect::from_center_size(r.center(), Vec2::splat(SQ * 1.35)), 2.7, color(l));
    }

    // o total e a legenda
    let fy = o.y + 146.0;
    let total: u32 = st.activity.iter().sum();
    text(&p, Pos2::new(o.x, fy), Align2::LEFT_CENTER, &format!("{} {}", num(total as i64), if total == 1 { "interação" } else { "interações" }), f6(10.5), FAINT(), 0.0);
    let mais = text(&p, Pos2::new(o.x + iw, fy), Align2::RIGHT_CENTER, "mais", f6(10.5), FAINT(), 0.0);
    let mut x = mais.left() - 3.0;
    for l in (0..5).rev() {
        x -= 9.0;
        p.rect_filled(Rect::from_min_size(Pos2::new(x, fy - 4.5), Vec2::splat(9.0)), 2.0, color(l));
        x -= 3.0;
    }
    text(&p, Pos2::new(x, fy), Align2::RIGHT_CENTER, "menos", f6(10.5), FAINT(), 0.0);
}

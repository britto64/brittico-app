//! O LivePixel (`public/livepixel/`: `livepixel.js` desenha, `livepixel.css` mede): o canvas infinito de pixels onde
//! cada comunidade pinta o território do seu streamer.
//!
//! No site ele é uma página cheia, sem a casca; no app ele mora no corpo, entre o topo e a trilha, e o HUD se ancora
//! nos cantos desse corpo com as mesmas folgas (`--lp-gap`). O jogo tem paleta e letra próprias (não segue o tema do
//! site): a paleta de 16 cores, Balsamiq Sans no lugar do Comic Sans, Inter no corpo, linha grossa e canto reto.
//!
//! Tudo é o jogo de verdade: os pixels, o chat, o placar e o cooldown vêm do WebSocket do site
//! (`/api/livepixel/ws`, `livepixel/rede.rs`) e das rotas `/api/livepixel/*`; pintar, apagar, moderar e conversar vão
//! por eles. Quem não entrou só olha.

use crate::theme::{comic, comic4, inter};
use crate::ui::{self, text, text_width};
use crate::ic;
use crate::net;
use eframe::egui::{self, text::LayoutJob, Align2, Color32, CursorIcon, FontId, ImageSource, Key, Painter, PointerButton, Pos2, Rect, Sense, Shape, Stroke, StrokeKind, TextFormat, UiBuilder, Vec2};
use serde::Deserialize;
use std::collections::HashMap;

mod janelas;
mod rede;
use janelas::{Busca, Denuncia, Envio, Janela, Moderacao, Roleta, Selo, Visual};

/// O que o botão do mouse está fazendo no canvas desde que desceu.
#[derive(Clone, Copy)]
enum Arrasto {
    /// Movendo a câmera: onde o mouse e a câmera estavam, e se já andou o bastante para não ser um clique.
    Pan(Pos2, (f32, f32), bool),
    /// A borracha da moderação, passando por cima.
    Apagar,
    /// A caixa de seleção da moderação, a partir desta célula.
    Caixa((i32, i32)),
    /// Movendo a seleção, que foi pega nesta célula.
    Mover((i32, i32)),
    /// A imagem de guia: de onde o mouse saiu, a posição e o tamanho de antes, e a alça (se for uma).
    Selo(Pos2, (f32, f32, u32, u32), Option<usize>),
}

/* ---------------- a paleta e os tokens do jogo ---------------- */

const fn rgb(v: u32) -> Color32 {
    Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
}
const fn rgba(v: u32, a: f32) -> Color32 {
    Color32::from_rgba_premultiplied(((v >> 16 & 0xff) as f32 * a + 0.5) as u8, ((v >> 8 & 0xff) as f32 * a + 0.5) as u8, ((v & 0xff) as f32 * a + 0.5) as u8, (a * 255.0 + 0.5) as u8)
}

/// As 16 cores do jogo (`PALETTE`).
pub const PALETTE: [Color32; 16] = [
    rgb(0x1a1c2c), rgb(0x5d275d), rgb(0xb13e53), rgb(0xef7d57),
    rgb(0xffcd75), rgb(0xa7f070), rgb(0x38b764), rgb(0x257179),
    rgb(0x29366f), rgb(0x3b5dc9), rgb(0x41a6f6), rgb(0x73eff7),
    rgb(0xf4f4f4), rgb(0x94b0c2), rgb(0x566c86), rgb(0x333c57),
];
const LOGO_C: [Color32; 8] = [rgb(0x5d275d), rgb(0xb13e53), rgb(0xef7d57), rgb(0x257179), rgb(0x29366f), rgb(0x3b5dc9), rgb(0x566c86), rgb(0x94b0c2)];
/// `--lp-bg`, `--lp-line`, `--lp-line-hi`, `--lp-ink`, `--lp-dim`, `--lp-accent`, `--lp-good`, `--lp-bad`.
const BG: Color32 = rgb(0x1a1c2c);
const LINE: Color32 = rgb(0x333c57);
const LINE_HI: Color32 = rgb(0x566c86);
const INK: Color32 = rgb(0xf4f4f4);
const DIM: Color32 = rgb(0x94b0c2);
const ACCENT: Color32 = rgb(0x41a6f6);
const BAD: Color32 = rgb(0xb13e53);
const GOLD: Color32 = rgb(0xe4ab00);
const LP_E: Color32 = rgb(0x566c86);
const LP_4: Color32 = rgb(0xffcd75);
/// `--lp-bw`, `--lp-gap`, `--lp-stack`, `--lp-hud-h`.
const BW: f32 = 3.0;
const GAP: f32 = 14.0;
const STACK: f32 = 8.0;
const HUD_H: f32 = 64.0;

/// `PX`, `MAX_PX`, `Z_MIN`, `Z_MAX`, `PAINT_MIN_PS`, `WORLD_LIMIT` do livepixel.js.
const PX: f32 = 16.0;
const MAX_PX: u32 = 24;
const Z_MIN: f32 = 0.06;
const Z_MAX: f32 = 12.0;
const PAINT_MIN_PS: f32 = 3.0;
const WORLD_LIMIT: i32 = 5000;
/// O ciclo do cooldown: os 24 pixels voltam um minuto depois do primeiro gasto.
const CICLO: f64 = 60.0;

/// Os pincéis, na ordem dos botões e das teclas 1..6: as células de cada um e o desenho na grade 3×3.
const BRUSHES: [(&str, &[(i32, i32)], &[usize]); 6] = [
    ("1 pixel", &[(0, 0)], &[4]),
    ("2 pixels (lado a lado)", &[(0, 0), (1, 0)], &[3, 4]),
    ("4 pixels (bloco 2×2)", &[(0, 0), (1, 0), (0, 1), (1, 1)], &[0, 1, 3, 4]),
    ("5 pixels (cruz)", &[(0, 0), (-1, 0), (1, 0), (0, -1), (0, 1)], &[1, 3, 4, 5, 7]),
    ("9 pixels (bloco 3×3)", &[(-1, -1), (0, -1), (1, -1), (-1, 0), (0, 0), (1, 0), (-1, 1), (0, 1), (1, 1)], &[0, 1, 2, 3, 4, 5, 6, 7, 8]),
    ("2 pixels (um sobre o outro)", &[(0, 0), (0, 1)], &[1, 4]),
];

/* ---------------- os dados de mentira ---------------- */

/// O que o jogo sabe de quem joga e dos streamers (montado das respostas do servidor).
#[derive(Default)]
struct Mock {
    me: MockMe,
    streamers: Vec<MockStreamer>,
    artists: Vec<MockArtist>,
    artists_me: MockArtist,
}
#[derive(Default)]
struct MockMe {
    name: String,
    /// O id com que esta pessoa joga (o dono de cada pixel é comparado com ele).
    id: String,
    avatar: String,
    #[allow(dead_code)]
    max: u32,
    moderador: bool,
    premium: bool,
}
#[derive(Deserialize, Clone, Default)]
#[serde(default)]
struct MockStreamer {
    id: String,
    #[serde(alias = "display_name")]
    name: String,
    #[serde(default)]
    username: String,
    #[serde(alias = "profile_image")]
    avatar: Option<String>,
    #[serde(default)]
    pixel_count: u64,
}
#[derive(Deserialize, Clone, Default)]
#[serde(default)]
struct MockArtist {
    display_name: String,
    pixel_count: u64,
    rank: Option<usize>,
    avatar: Option<String>,
}

/// Um pixel do canvas: a cor, quem pintou, por qual streamer e quando (segundos do relógio do app; negativo = antes de abrir).
#[derive(Clone)]
struct Pixel {
    c: Color32,
    /// O id de quem pintou e o nome dele.
    user: String,
    nome: String,
    foto: String,
    streamer: String,
    t: f64,
}

#[derive(Clone)]
struct Msg {
    id: i64,
    global: bool,
    user: String,
    streamer: String,
    text: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum LbTab {
    Streamers,
    Artistas,
}

pub struct LpState {
    m: Mock,
    /* a conversa com o servidor */
    rede: Option<rede::Rede>,
    online: bool,
    geracao: u64,
    view_enviada: Option<(i32, i32, i32)>,
    view_mudou: f64,
    pedidos: Vec<net::Tarefa<Volta>>,
    /// Quando o placar de artistas foi buscado (de meio em meio minuto, com o jogo na tela).
    artistas_em: f64,
    /// A busca de streamers da janela "quem você representa?" (o texto buscado e o que voltou).
    busca_feita: String,
    busca_res: Vec<MockStreamer>,
    busca_t: f64,
    pixels: HashMap<(i32, i32), Pixel>,
    /// A câmera: o centro em células e o zoom (`cam` do livepixel.js).
    cam: (f32, f32, f32),
    arrasto: Option<Arrasto>,
    hover: Option<(i32, i32)>,
    grid: bool,
    outlines: bool,
    color: Color32,
    brush: usize,
    erase: bool,
    eyedrop: bool,
    page: u8,
    custom: Vec<Color32>,
    /// Os pixels que sobram e quando o ciclo atual termina (devolve os 24).
    left: u32,
    cd_end: Option<f64>,
    streamer: Option<String>,
    lb_tab: LbTab,
    lb_counts: HashMap<String, u64>,
    my_count: u64,
    chat: Vec<Msg>,
    chat_input: String,
    chat_global: bool,
    chan_menu: bool,
    toast: Option<(String, bool, f64)>,
    /// O cartão do pixel aberto: a célula e onde o mouse estava.
    info: Option<((i32, i32), Pos2)>,
    pmenu: bool,
    janela: Option<Janela>,
    roleta: Roleta,
    /// Os pixels de bônus (a roleta): gastos depois dos 24 do ciclo.
    bonus: u32,
    selo: Option<Selo>,
    envio: Envio,
    envio_tex: Option<egui::TextureHandle>,
    moderacao: Moderacao,
    visual: Visual,
    busca: Busca,
    denuncia: Denuncia,
    fundo_picker: bool,
    grade_picker: bool,
    /// O seletor de cor aberto, e onde.
    cor_picker: Option<Pos2>,
}

/// O que volta das rotas `/api/livepixel/*`.
enum Volta {
    Eu(serde_json::Value),
    Placar(serde_json::Value),
    Artistas(serde_json::Value),
    Busca(String, serde_json::Value),
    Streamer(String, Result<serde_json::Value, net::Erro>),
    Roleta(serde_json::Value),
    Giro(Result<serde_json::Value, net::Erro>),
}

impl Default for LpState {
    fn default() -> Self {
        Self {
            m: Mock::default(),
            rede: None,
            online: false,
            geracao: u64::MAX,
            view_enviada: None,
            view_mudou: 0.0,
            pedidos: Vec::new(),
            artistas_em: -100.0,
            busca_feita: String::new(),
            busca_res: Vec::new(),
            busca_t: 0.0,
            pixels: HashMap::new(),
            cam: (0.0, 0.0, 1.6),
            arrasto: None,
            hover: None,
            grid: true,
            outlines: false,
            color: PALETTE[10],
            brush: 0,
            erase: false,
            eyedrop: false,
            page: 1,
            custom: vec![],
            left: 0,
            cd_end: None,
            streamer: None,
            lb_tab: LbTab::Streamers,
            lb_counts: HashMap::new(),
            my_count: 0,
            chat: Vec::new(),
            chat_input: String::new(),
            chat_global: true,
            chan_menu: false,
            toast: None,
            info: None,
            pmenu: false,
            janela: None,
            roleta: Roleta::default(),
            bonus: 0,
            selo: None,
            envio: Envio::default(),
            envio_tex: None,
            moderacao: Moderacao::default(),
            visual: Visual::default(),
            busca: Busca::default(),
            denuncia: Denuncia::default(),
            fundo_picker: false,
            grade_picker: false,
            cor_picker: None,
        }
    }
}

/// `#rrggbb` → cor.
fn hex_cor(s: &str) -> Option<Color32> {
    let s = s.strip_prefix('#')?;
    let v = u32::from_str_radix(s.get(..6)?, 16).ok()?;
    Some(rgb(v))
}

fn cor_hex(c: Color32) -> String {
    format!("#{:02x}{:02x}{:02x}", c.r(), c.g(), c.b())
}

/// Os ids dos pincéis no servidor, na ordem dos botões (`BRUSHES` do bot: 1, 2, 4, 5, 9, 6).
const BRUSH_ID: [u32; 6] = [1, 2, 4, 5, 9, 6];

impl LpState {
    /// Uma vez por quadro: liga (ou religa, quando a conta muda) o WebSocket e aplica o que chegou.
    fn passo(&mut self, ctx: &egui::Context, geracao: u64, t: f64) {
        if self.geracao != geracao {
            self.geracao = geracao;
            self.pixels.clear();
            self.chat.clear();
            self.lb_counts.clear();
            self.view_enviada = None;
            self.left = 0;
            self.bonus = 0;
            self.cd_end = None;
            self.streamer = None;
            self.m = Mock::default();
            if let Some(me) = crate::conta::eu() {
                self.m.me = MockMe { name: me.nome().to_owned(), id: me.livepixel_id.clone().unwrap_or_else(|| me.user.id.clone()), avatar: me.user.avatar.clone(), max: MAX_PX, moderador: me.is_mod, premium: false };
                self.pedidos.push(net::rodar(ctx, || net::pedir_ja("GET", "/api/livepixel/user/me", None).map(Volta::Eu)));
                self.pedidos.push(net::rodar(ctx, || net::pedir_ja("GET", "/api/livepixel/spin-status", None).map(Volta::Roleta)));
            }
            self.pedidos.push(net::rodar(ctx, || net::pedir_ja("GET", "/api/livepixel/leaderboard", None).map(Volta::Placar)));
            self.artistas_em = -100.0;
            // um WebSocket novo, já com a sessão de agora
            self.rede = Some(rede::ligar(ctx));
        }
        if t - self.artistas_em > 30.0 {
            self.artistas_em = t;
            self.pedidos.push(net::rodar(ctx, || net::pedir_ja("GET", "/api/livepixel/leaderboard/artists", None).map(Volta::Artistas)));
        }
        // a busca de streamers, 300 ms depois da última tecla
        let q = self.busca.q.trim().to_owned();
        if q != self.busca_feita {
            if self.busca_t == 0.0 {
                self.busca_t = t;
            }
            if t - self.busca_t > 0.3 {
                self.busca_feita = q.clone();
                self.busca_t = 0.0;
                if q.chars().count() >= 2 {
                    let path = format!("/api/livepixel/streamers?q={}", net::enc(&q));
                    self.pedidos.push(net::rodar(ctx, move || net::pedir_ja("GET", &path, None).map(|v| Volta::Busca(q, v))));
                } else {
                    self.busca_res.clear();
                }
            } else {
                ctx.request_repaint_after(std::time::Duration::from_millis(320));
            }
        }
        let mut chegou = Vec::new();
        if let Some(r) = &self.rede {
            while let Ok(c) = r.rx.try_recv() {
                chegou.push(c);
            }
        }
        for c in chegou {
            match c {
                rede::Chegou::Conectou => {
                    self.online = true;
                    self.view_enviada = None;
                }
                rede::Chegou::Caiu => self.online = false,
                rede::Chegou::Msg(v) => self.mensagem(v, t),
            }
        }
        let mut i = 0;
        while i < self.pedidos.len() {
            let Some(r) = self.pedidos[i].pronto() else {
                i += 1;
                continue;
            };
            self.pedidos.remove(i);
            match r {
                Ok(v) => self.volta(v, t),
                Err(e) => eprintln!("[livepixel] {e}"),
            }
        }
    }

    /// A parte do mapa que está na tela: pede ao servidor os pedaços que faltam (quando a câmera para de andar).
    fn pedir_vista(&mut self, area: Rect, t: f64) {
        let ps = (PX * self.cam.2).max(0.5);
        let r = ((area.width().max(area.height()) / 2.0 / ps) as i32 + 8).clamp(8, 250);
        let v = (self.cam.0.round() as i32, self.cam.1.round() as i32, r);
        if self.view_enviada == Some(v) || !self.online {
            return;
        }
        if self.view_mudou == 0.0 {
            self.view_mudou = t;
        }
        if t - self.view_mudou < 0.15 && self.view_enviada.is_some() {
            return;
        }
        self.view_mudou = 0.0;
        self.view_enviada = Some(v);
        self.mandar(serde_json::json!({ "type": "view", "cx": v.0, "cy": v.1, "r": v.2 }));
    }

    fn mandar(&self, v: serde_json::Value) {
        if let Some(r) = &self.rede {
            r.mandar(v);
        }
    }

    fn pixel_de(&mut self, p: &serde_json::Value, t: f64) {
        let (Some(x), Some(y)) = (p["x"].as_i64(), p["y"].as_i64()) else { return };
        let Some(c) = p["color"].as_str().and_then(hex_cor) else { return };
        let quando = p["placedAt"].as_str().map(crate::home::parse_ts).unwrap_or_else(|| chrono::Local::now().timestamp());
        let idade = (chrono::Local::now().timestamp() - quando) as f64;
        let user = p["userId"].as_str().unwrap_or("").to_owned();
        let nome = p["username"].as_str().unwrap_or("").to_owned();
        let streamer = p["streamerId"].as_str().unwrap_or("").to_owned();
        if let Some(n) = p["streamerName"].as_str().filter(|n| !n.is_empty()) {
            if !self.m.streamers.iter().any(|s| s.id == streamer) {
                self.m.streamers.push(MockStreamer { id: streamer.clone(), name: n.to_owned(), ..Default::default() });
            }
        }
        let foto = p["userAvatar"].as_str().unwrap_or("").to_owned();
        self.pixels.insert((x as i32, y as i32), Pixel { c, user, nome, foto, streamer, t: t - idade });
    }

    fn mensagem(&mut self, v: serde_json::Value, t: f64) {
        match v["type"].as_str().unwrap_or("") {
            "pixel_batch" => {
                for p in v["pixels"].as_array().into_iter().flatten() {
                    self.pixel_de(p, t);
                }
            }
            "pixel_placed" | "pixel_overwritten" => {
                let mine = v["userId"].as_str() == Some(self.m.me.id.as_str());
                self.pixel_de(&v, t);
                if mine {
                    self.my_count += 1;
                }
            }
            "pixel_removed" => {
                if let (Some(x), Some(y)) = (v["x"].as_i64(), v["y"].as_i64()) {
                    self.pixels.remove(&(x as i32, y as i32));
                }
            }
            "pixels_removed" => {
                for p in v["pixels"].as_array().into_iter().flatten() {
                    if let (Some(x), Some(y)) = (p["x"].as_i64(), p["y"].as_i64()) {
                        self.pixels.remove(&(x as i32, y as i32));
                    }
                }
            }
            "chunk_sync" => {
                // o que o servidor diz que existe em cada pedaço: o que sobrou aqui e não está na lista foi apagado
                for ch in v["chunks"].as_array().into_iter().flatten() {
                    let Some((bx, by)) = ch["b"].as_str().and_then(|b| b.split_once(',')).and_then(|(a, b)| Some((a.parse::<i32>().ok()?, b.parse::<i32>().ok()?))) else { continue };
                    let tem: std::collections::HashSet<(i32, i32)> = ch["k"].as_array().into_iter().flatten().filter_map(|k| Some((k[0].as_i64()? as i32, k[1].as_i64()? as i32))).collect();
                    self.pixels.retain(|&(x, y), _| x.div_euclid(32) != bx || y.div_euclid(32) != by || tem.contains(&(x, y)));
                }
            }
            "cooldown" => self.cooldown(&v, t),
            "leaderboard" => self.placar(&v),
            "chat_history" => {
                let global = v["channel"].as_str() == Some("global");
                self.chat.retain(|m| m.global != global || m.user.is_empty());
                let msgs: Vec<Msg> = v["messages"].as_array().into_iter().flatten().map(|m| self.msg_de(m)).collect();
                // o histórico vem do mais novo para o mais velho ou ao contrário: ordena pelo id
                let mut msgs = msgs;
                msgs.sort_by_key(|m| m.id);
                self.chat.extend(msgs);
                self.chat.sort_by_key(|m| m.id);
            }
            "chat_message" => {
                let m = self.msg_de(&v["message"]);
                if !self.chat.iter().any(|x| x.id == m.id && m.id != 0) {
                    self.chat.push(m);
                }
                if self.chat.len() > 200 {
                    self.chat.drain(..self.chat.len() - 200);
                }
            }
            "system_message" => {
                if let Some(text) = v["text"].as_str() {
                    self.chat.push(Msg { id: i64::MAX, global: true, user: String::new(), streamer: String::new(), text: text.to_owned() });
                }
            }
            "error" => {
                let msg = v["message"].as_str().unwrap_or("algo deu errado").to_owned();
                if v["code"].as_str() == Some("no_streamer") {
                    self.janela = Some(Janela::Streamer);
                }
                self.say(msg, true, t);
            }
            _ => {}
        }
    }

    fn msg_de(&self, m: &serde_json::Value) -> Msg {
        let ch = m["channel"].as_str().unwrap_or("global");
        Msg {
            id: m["id"].as_i64().unwrap_or(0),
            global: ch == "global",
            user: m["username"].as_str().unwrap_or("").to_owned(),
            streamer: m["streamerId"].as_str().unwrap_or("").to_owned(),
            text: m["content"].as_str().unwrap_or("").to_owned(),
        }
    }

    fn cooldown(&mut self, v: &serde_json::Value, t: f64) {
        let agora = chrono::Local::now().timestamp_millis() as f64;
        self.left = v["normalRemaining"].as_u64().or_else(|| v["normal_pixels_remaining"].as_u64()).unwrap_or(0) as u32;
        self.bonus = (v["dailyBonusRemaining"].as_u64().or_else(|| v["daily_bonus_pixels_remaining"].as_u64()).unwrap_or(0) + v["bonusRemaining"].as_u64().or_else(|| v["bonus_pixels_remaining"].as_u64()).unwrap_or(0)) as u32;
        let fim = v["normalEndsAt"].as_f64().or_else(|| v["normal_cooldown_ends_at"].as_f64());
        self.cd_end = fim.filter(|f| *f > agora).map(|f| t + (f - agora) / 1000.0);
        if let Some(p) = v["isPremium"].as_bool().or_else(|| v["is_premium"].as_bool()) {
            self.m.me.premium = p;
        }
    }

    fn placar(&mut self, v: &serde_json::Value) {
        let lista: Vec<MockStreamer> = serde_json::from_value(v["entries"].clone()).unwrap_or_default();
        self.lb_counts.retain(|k, _| Some(k) == self.streamer.as_ref());
        for s in lista {
            self.lb_counts.insert(s.id.clone(), s.pixel_count);
            match self.m.streamers.iter_mut().find(|x| x.id == s.id) {
                Some(x) => *x = s,
                None => self.m.streamers.push(s),
            }
        }
    }

    fn volta(&mut self, v: Volta, t: f64) {
        match v {
            Volta::Eu(v) => {
                if let Some(sid) = v["streamerId"].as_str().filter(|s| !s.is_empty()) {
                    self.streamer = Some(sid.to_owned());
                    self.lb_counts.entry(sid.to_owned()).or_insert(0);
                    if let Ok(s) = serde_json::from_value::<MockStreamer>(v["streamer"].clone()) {
                        if !s.id.is_empty() && !self.m.streamers.iter().any(|x| x.id == s.id) {
                            self.m.streamers.push(s);
                        }
                    }
                } else if self.janela.is_none() {
                    // como no site: quem entra sem representante escolhe um (ou só explora)
                    self.janela = Some(Janela::Streamer);
                }
                self.m.me.premium = v["premium"].as_bool().unwrap_or(false);
                self.cooldown(&v["cooldown"], t);
            }
            Volta::Placar(v) => self.placar(&v),
            Volta::Artistas(v) => {
                self.m.artists = serde_json::from_value(v["entries"].clone()).unwrap_or_default();
                self.m.artists_me = serde_json::from_value(v["me"].clone()).unwrap_or_default();
                self.my_count = self.m.artists_me.pixel_count;
            }
            Volta::Busca(q, v) => {
                if q == self.busca_feita {
                    self.busca_res = serde_json::from_value(v["streamers"].clone()).unwrap_or_default();
                    for s in &self.busca_res {
                        if !self.m.streamers.iter().any(|x| x.id == s.id) {
                            self.m.streamers.push(s.clone());
                        }
                    }
                }
            }
            Volta::Streamer(sid, Ok(_)) => {
                self.streamer = Some(sid.clone());
                self.lb_counts.entry(sid.clone()).or_insert(0);
                self.say(format!("agora você pinta para {}", self.name_of(&sid)), false, t);
                // o chat da guilda nova
                self.mandar(serde_json::json!({ "type": "chat_history", "channel": "streamer" }));
            }
            Volta::Streamer(_, Err(e)) => self.say(e.msg, true, t),
            Volta::Roleta(v) => {
                self.roleta.pode = v["canSpin"].as_bool().unwrap_or(false);
                let agora = chrono::Local::now().timestamp_millis() as f64;
                if let Some(n) = v["nextSpinAt"].as_f64() {
                    self.roleta.proximo = t + (n - agora).max(0.0) / 1000.0;
                }
            }
            Volta::Giro(Ok(v)) => {
                let premio = v["prizeIndex"].as_u64().unwrap_or(0) as usize;
                janelas::girar(self, premio.min(6), t);
                if v["cooldown"].is_object() {
                    // o saldo novo só aparece quando a roda parar (o `girar` soma); guarda o resto do cooldown
                    let c = v["cooldown"].clone();
                    self.left = c["normal_pixels_remaining"].as_u64().unwrap_or(self.left as u64) as u32;
                }
                let agora = chrono::Local::now().timestamp_millis() as f64;
                if let Some(n) = v["nextSpinAt"].as_f64() {
                    self.roleta.proximo = t + (n - agora).max(0.0) / 1000.0;
                }
            }
            Volta::Giro(Err(e)) => self.say(e.msg, true, t),
        }
    }

    /// Escolher quem a pessoa representa (`PUT /api/livepixel/user/streamer`).
    fn escolher_streamer(&mut self, ctx: &egui::Context, sid: String) {
        let corpo = serde_json::json!({ "streamerId": sid });
        self.pedidos.push(net::rodar(ctx, move || Ok(Volta::Streamer(sid, net::pedir_ja("PUT", "/api/livepixel/user/streamer", Some(&corpo))))));
    }

    fn pedir_giro(&mut self, ctx: &egui::Context) {
        self.pedidos.push(net::rodar(ctx, || Ok(Volta::Giro(net::pedir_ja("POST", "/api/livepixel/spin", None)))));
    }
}

/* ---------------- peças ---------------- */

/// Uma caixa do HUD: fundo e a borda grossa por dentro (canto reto, como tudo no jogo).
fn boxed(p: &Painter, r: Rect, bg: Color32, bw: f32, line: Color32) {
    p.rect_filled(r, 0.0, bg);
    if bw > 0.0 {
        p.rect_stroke(r, 0.0, Stroke::new(bw, line), StrokeKind::Inside);
    }
}

/// Um painel do HUD segura o clique: o que cai nos vãos dele não chega ao canvas embaixo (não pinta nem arrasta).
fn escudo(ui: &egui::Ui, r: Rect, id: &str) {
    ui.interact(r, ui.id().with(("lp-escudo", id)), Sense::click_and_drag());
}

fn hit(ui: &egui::Ui, r: Rect, id: impl std::hash::Hash, tip: &str) -> egui::Response {
    let resp = ui.interact(r, ui.id().with(("lp", id)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
    if tip.is_empty() {
        resp
    } else {
        resp.on_hover_text(tip)
    }
}

/// `streamerColor`: a cor de cada streamer sai de um hash do id (o site usa a cor dominante da foto quando tem; o
/// Kennzy fica com a que o site calcula da foto dele).
fn streamer_color(sid: &str) -> Color32 {
    if sid == "kennzy" {
        return rgb(0x9e6f4c);
    }
    const C: [u32; 10] = [0xb13e53, 0xef7d57, 0xffcd75, 0xa7f070, 0x38b764, 0x257179, 0x3b5dc9, 0x41a6f6, 0x5d275d, 0x73eff7];
    let mut h: i32 = 0;
    for ch in sid.encode_utf16() {
        h = (h << 5).wrapping_sub(h).wrapping_add(ch as i32);
    }
    rgb(C[(h.unsigned_abs() % 10) as usize])
}

/// `lighterColor`: 45% do caminho até o branco (o nome de quem fala no chat).
fn lighter(c: Color32) -> Color32 {
    let l = |v: u8| (v as f32 + (255.0 - v as f32) * 0.45).round() as u8;
    Color32::from_rgb(l(c.r()), l(c.g()), l(c.b()))
}

/// `fmtLBCount`: 48210 → "48,2K".
fn fmt_count(n: u64) -> String {
    let short = |v: f64, suf: &str| {
        if v.fract() == 0.0 {
            format!("{}{suf}", v as u64)
        } else {
            format!("{:.1}{suf}", v).replace('.', ",")
        }
    };
    if n >= 1_000_000 {
        short(n as f64 / 1e6, "M")
    } else if n >= 1000 {
        short(n as f64 / 1000.0, "K")
    } else {
        n.to_string()
    }
}

/// Como `outlined`, mas mantendo a cor de cada pedaço (a mensagem do sistema, amarela).
fn outlined_cor(p: &Painter, pos: Pos2, g: std::sync::Arc<egui::Galley>) {
    p.galley_with_override_text_color(pos + Vec2::new(0.0, 2.0), g.clone(), Color32::from_black_alpha(150));
    for (dx, dy) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
        p.galley_with_override_text_color(pos + Vec2::new(dx, dy), g.clone(), Color32::BLACK);
    }
    p.galley(pos, g, LP_4);
}

/// O texto do chat: branco com o contorno preto de quatro sombras e uma sombra embaixo (`text-shadow` do `.lp-chat__msg`).
fn outlined(p: &Painter, pos: Pos2, g: std::sync::Arc<egui::Galley>) {
    p.galley_with_override_text_color(pos + Vec2::new(0.0, 2.0), g.clone(), Color32::from_black_alpha(150));
    for (dx, dy) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
        p.galley_with_override_text_color(pos + Vec2::new(dx, dy), g.clone(), Color32::BLACK);
    }
    p.galley(pos, g, Color32::WHITE);
}

impl LpState {
    fn name_of(&self, sid: &str) -> String {
        self.m.streamers.iter().find(|s| s.id == sid).map(|s| s.name.clone()).unwrap_or_else(|| sid.to_owned())
    }

    fn avatar_of(&self, sid: &str) -> Option<ImageSource<'static>> {
        self.m.streamers.iter().find(|s| s.id == sid).and_then(|s| s.avatar.as_deref()).and_then(net::imagem)
    }

    fn say(&mut self, msg: impl Into<String>, bad: bool, t: f64) {
        self.toast = Some((msg.into(), bad, t));
    }

    /// Tela → célula, com a câmera no centro de `area`.
    fn s2w(&self, area: Rect, s: Pos2) -> (i32, i32) {
        let ps = PX * self.cam.2;
        (((s.x - area.center().x) / ps + self.cam.0).floor() as i32, ((s.y - area.center().y) / ps + self.cam.1).floor() as i32)
    }
    fn w2s(&self, area: Rect, x: f32, y: f32) -> Pos2 {
        let ps = PX * self.cam.2;
        Pos2::new((x - self.cam.0) * ps + area.center().x, (y - self.cam.1) * ps + area.center().y)
    }

    fn brush_cells(&self, at: (i32, i32)) -> Vec<(i32, i32)> {
        BRUSHES[self.brush].1.iter().map(|(dx, dy)| (at.0 + dx, at.1 + dy)).filter(|(x, y)| x.abs() <= WORLD_LIMIT && y.abs() <= WORLD_LIMIT).collect()
    }

    fn can_place(&self) -> bool {
        self.streamer.is_some() && (self.left + self.bonus) as usize >= BRUSHES[self.brush].1.len()
    }

    fn mine(&self, px: &Pixel) -> bool {
        !self.m.me.id.is_empty() && px.user == self.m.me.id
    }

    /// Pintar com o pincel no ponto (o clique sem arrastar).
    fn place(&mut self, at: (i32, i32), t: f64) {
        if self.m.me.id.is_empty() {
            self.say("entre para pintar", true, t);
            return;
        }
        let Some(sid) = self.streamer.clone() else {
            self.say("escolha quem você representa antes de pintar", true, t);
            return;
        };
        let cells = self.brush_cells(at);
        if ((self.left + self.bonus) as usize) < cells.len() {
            self.say(format!("sem pixels: espere o ciclo ({} s)", self.cd_left(t).ceil() as i64), true, t);
            return;
        }
        let mut n = 0;
        for c in cells {
            if self.pixels.get(&c).is_some_and(|p| p.c == self.color && p.streamer == sid) {
                continue;
            }
            self.pixels.insert(c, Pixel { c: self.color, user: self.m.me.id.clone(), nome: self.m.me.name.clone(), foto: self.m.me.avatar.clone(), streamer: sid.clone(), t });
            n += 1;
        }
        if n == 0 {
            return;
        }
        // o servidor confere o saldo e devolve o cooldown certo; aqui a tela já mostra o pixel
        self.mandar(serde_json::json!({ "type": "place_pixel", "x": at.0, "y": at.1, "color": cor_hex(self.color), "brush": BRUSH_ID[self.brush] }));
        let do_ciclo = n.min(self.left);
        self.left -= do_ciclo;
        self.bonus -= n - do_ciclo;
        if do_ciclo > 0 && self.cd_end.is_none() {
            self.cd_end = Some(t + CICLO);
        }
        *self.lb_counts.entry(sid).or_default() += n as u64;
    }

    /// O apagador: só os seus pixels, na área do pincel.
    fn erase_at(&mut self, at: (i32, i32)) {
        if self.m.me.id.is_empty() {
            return;
        }
        // o servidor apaga o que é seu (e da sua família); a tela já tira os seus
        if self.brush_cells(at).iter().any(|c| self.pixels.contains_key(c)) {
            self.mandar(serde_json::json!({ "type": "remove_pixel", "x": at.0, "y": at.1, "brush": BRUSH_ID[self.brush] }));
        }
        for c in self.brush_cells(at) {
            if self.pixels.get(&c).is_some_and(|p| p.user == self.m.me.id) {
                let p = self.pixels.remove(&c).unwrap();
                if let Some(v) = self.lb_counts.get_mut(&p.streamer) {
                    *v = v.saturating_sub(1);
                }
                self.my_count = self.my_count.saturating_sub(1);
            }
        }
    }

    /// A borracha da moderação: tira o pixel de qualquer pessoa.
    fn apagar_qualquer(&mut self, at: (i32, i32)) {
        if self.pixels.contains_key(&at) {
            self.mandar(serde_json::json!({ "type": "mod_delete_pixels", "pixels": [{ "x": at.0, "y": at.1 }] }));
        }
        if let Some(p) = self.pixels.remove(&at) {
            if let Some(v) = self.lb_counts.get_mut(&p.streamer) {
                *v = v.saturating_sub(1);
            }
            self.moderacao.sel.remove(&at);
        }
    }

    fn apagar_selecao(&mut self) {
        let sel: Vec<(i32, i32)> = self.moderacao.sel.drain().collect();
        for lote in sel.chunks(5000) {
            let px: Vec<serde_json::Value> = lote.iter().map(|(x, y)| serde_json::json!({ "x": x, "y": y })).collect();
            self.mandar(serde_json::json!({ "type": "mod_delete_pixels", "pixels": px }));
        }
        for at in sel {
            if let Some(p) = self.pixels.remove(&at) {
                if let Some(v) = self.lb_counts.get_mut(&p.streamer) {
                    *v = v.saturating_sub(1);
                }
            }
        }
    }

    /// Move os pixels selecionados `d` células (o que estava no destino é coberto).
    fn mover_selecao(&mut self, d: (i32, i32)) {
        if d == (0, 0) {
            return;
        }
        let sel: Vec<serde_json::Value> = self.moderacao.sel.iter().filter(|k| self.pixels.contains_key(k)).map(|(x, y)| serde_json::json!({ "x": x, "y": y })).collect();
        if !sel.is_empty() {
            self.mandar(serde_json::json!({ "type": "mod_move_pixels", "pixels": sel, "dx": d.0, "dy": d.1 }));
        }
        let tirados: Vec<((i32, i32), Pixel)> = self.moderacao.sel.iter().filter_map(|k| self.pixels.remove(k).map(|p| (*k, p))).collect();
        self.moderacao.sel.clear();
        for ((x, y), p) in tirados {
            let n = (x + d.0, y + d.1);
            self.pixels.insert(n, p);
            self.moderacao.sel.insert(n);
        }
    }

    fn cd_left(&self, t: f64) -> f64 {
        self.cd_end.map(|e| (e - t).max(0.0)).unwrap_or(0.0)
    }

    fn tick(&mut self, t: f64) {
        // o ciclo acabou: os 24 voltam (o servidor confirma no próximo pixel)
        if self.cd_end.is_some_and(|e| t >= e) {
            self.left = MAX_PX;
            self.cd_end = None;
        }
    }
}

impl LpState {
    fn roleta_pode(&self) -> bool {
        janelas::roleta_pode(&self.roleta)
    }

    /// Para as capturas (`BRITTICO_TELA=livepixel-<janela>`): abre uma janela ou um painel já no estado de mostrar.
    pub fn demo(&mut self, qual: &str) {
        match qual {
            "menu" => self.pmenu = true,
            "config" => self.janela = Some(Janela::Config(false)),
            "premium" => self.janela = Some(Janela::Config(true)),
            "notif" => self.janela = Some(Janela::Notificacoes),
            "roleta" => self.janela = Some(Janela::Roleta),
            "imagem" => self.janela = Some(Janela::Imagem),
            "streamer" => {
                self.janela = Some(Janela::Streamer);
                self.busca.escolhido = self.streamer.clone();
            }
            "denuncia" => self.janela = Some(Janela::Denuncia((-8, -12))),
            "info" => self.info = Some(((-6, -10), Pos2::new(700.0, 250.0))),
            "mod" => {
                self.moderacao.aberta = true;
                self.moderacao.selecionar = true;
                for x in 8..14 {
                    for y in -16..-12 {
                        if self.pixels.contains_key(&(x, y)) {
                            self.moderacao.sel.insert((x, y));
                        }
                    }
                }
            }
            "territorios" => self.outlines = true,
            _ => {}
        }
    }
}

/* ---------------- a tela ---------------- */

pub fn show(ui: &mut egui::Ui, st: &mut LpState, body: Rect, t: f64, geracao: u64) {
    st.passo(ui.ctx(), geracao, t);
    st.tick(t);
    st.pedir_vista(body, t);
    let p = ui.painter_at(body);
    canvas(ui, st, body, t);
    logo(&p, body.min + Vec2::new(GAP, GAP), 40.0);
    top_controls(ui, st, body);
    leaderboard(ui, st, body);
    dock(ui, st, body, t);
    coords(&p, st, body);
    let prof = hud_right(ui, st, body, t);
    if st.moderacao.aberta {
        janelas::barra_mod(ui, st, body);
    } else {
        janelas::barra_selo(ui, st, body, t);
    }
    janelas::info(ui, st, body, t);
    janelas::menu(ui, st, body, prof, t);
    janelas::janelas(ui, st, body, t);
    toast(ui, st, body, t);
    if st.janela.is_none() {
        keys(ui, st);
    }
    if st.cd_end.is_some() {
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(250));
    }
}

/// O canvas: o fundo, a grade, os eixos, os pixels e a prévia do pincel sob o mouse. Arrastar move, a roda aproxima,
/// o clique pinta, o direito mostra quem pintou.
fn canvas(ui: &mut egui::Ui, st: &mut LpState, area: Rect, t: f64) {
    let resp = ui.interact(area, ui.id().with("lp-canvas"), Sense::click_and_drag());
    let p = ui.painter_at(area);
    p.rect_filled(area, 0.0, st.visual.fundo);
    let ps = (PX * st.cam.2).max(1.0);
    let (hw, hh) = (area.width() / (2.0 * ps), area.height() / (2.0 * ps));
    let (x0, x1) = ((st.cam.0 - hw).floor() as i32, (st.cam.0 + hw).ceil() as i32);
    let (y0, y1) = ((st.cam.1 - hh).floor() as i32, (st.cam.1 + hh).ceil() as i32);

    if st.grid {
        let step = if ps < 1.2 { 128 } else if ps < 3.0 { 32 } else if ps < 6.0 { 8 } else { 1 };
        // a cor escolhida nas configurações entra com 15% de força; sem escolha, a do site
        let cor = st.visual.grade_cor.map(|c| c.gamma_multiply(0.15)).unwrap_or(rgba(0x1a1c2c, 0.08));
        let line = Stroke::new(st.visual.grade_w, cor);
        let mut gx = x0.div_euclid(step) * step;
        while gx <= x1 + step {
            let sx = st.w2s(area, gx as f32, 0.0).x.round() + 0.5;
            p.line_segment([Pos2::new(sx, area.top()), Pos2::new(sx, area.bottom())], line);
            gx += step;
        }
        let mut gy = y0.div_euclid(step) * step;
        while gy <= y1 + step {
            let sy = st.w2s(area, 0.0, gy as f32).y.round() + 0.5;
            p.line_segment([Pos2::new(area.left(), sy), Pos2::new(area.right(), sy)], line);
            gy += step;
        }
        let o = st.w2s(area, 0.0, 0.0);
        let axis = Stroke::new(1.0_f32, rgba(0x1a1c2c, 0.22));
        p.line_segment([Pos2::new(o.x.round() + 0.5, area.top()), Pos2::new(o.x.round() + 0.5, area.bottom())], axis);
        p.line_segment([Pos2::new(area.left(), o.y.round() + 0.5), Pos2::new(area.right(), o.y.round() + 0.5)], axis);
    }

    if st.outlines {
        territories(&p, st, area, ps);
    }
    let selo_r = janelas::desenhar_selo(ui, st, area);

    let side = ps.ceil();
    // na moderação, os pixels selecionados que estão sendo arrastados aparecem no lugar novo
    let mover = match st.arrasto {
        Some(Arrasto::Mover(a)) => st.hover.map(|h| (h.0 - a.0, h.1 - a.1)),
        _ => None,
    };
    for (&(x, y), px) in &st.pixels {
        if x < x0 - 64 || x > x1 + 64 || y < y0 - 64 || y > y1 + 64 {
            continue;
        }
        let (x, y) = match mover {
            Some(d) if st.moderacao.sel.contains(&(x, y)) => (x + d.0, y + d.1),
            _ => (x, y),
        };
        let s = st.w2s(area, x as f32, y as f32);
        p.rect_filled(Rect::from_min_size(Pos2::new(s.x.round(), s.y.round()), Vec2::splat(side)), 0.0, px.c);
    }
    // a seleção da moderação: verde por cima de cada pixel
    for &(x, y) in &st.moderacao.sel {
        let (x, y) = mover.map(|d| (x + d.0, y + d.1)).unwrap_or((x, y));
        let s = st.w2s(area, x as f32, y as f32).round();
        let r = Rect::from_min_size(s, Vec2::splat(side));
        p.rect_filled(r, 0.0, rgba(0x38b764, 0.35));
        p.rect_stroke(r, 0.0, Stroke::new(1.0_f32, PALETTE[6]), StrokeKind::Inside);
    }
    if let (Some(Arrasto::Caixa(a)), Some(h)) = (st.arrasto, st.hover) {
        let (ax, ay, bx, by) = (a.0.min(h.0), a.1.min(h.1), a.0.max(h.0) + 1, a.1.max(h.1) + 1);
        let r = Rect::from_min_max(st.w2s(area, ax as f32, ay as f32), st.w2s(area, bx as f32, by as f32));
        p.rect_filled(r, 0.0, rgba(0x38b764, 0.12));
        dashed(&p, r, PALETTE[6], true);
    }

    // o mouse
    let pointer = ui.input(|i| i.pointer.hover_pos()).filter(|q| area.contains(*q));
    st.hover = if resp.hovered() || st.arrasto.is_some() { ui.input(|i| i.pointer.interact_pos()).filter(|q| area.contains(*q)).or(pointer).map(|q| st.s2w(area, q)) } else { None };
    let mod_ligado = st.moderacao.aberta && (st.moderacao.apagar || st.moderacao.selecionar);
    let picking = !mod_ligado && (st.eyedrop || ui.input(|i| i.modifiers.alt || i.modifiers.shift));
    let espaco = ui.input(|i| i.key_down(Key::Space));
    if resp.hovered() {
        let pan = matches!(st.arrasto, Some(Arrasto::Pan(_, _, true)));
        ui.ctx().set_cursor_icon(if pan { CursorIcon::Grabbing } else if mod_ligado && espaco { CursorIcon::Grab } else { CursorIcon::Crosshair });
    }

    // começar a arrastar: a câmera, a borracha da moderação, a caixa de seleção, mover a seleção ou a imagem de guia
    let meio = resp.drag_started_by(PointerButton::Middle);
    if resp.drag_started_by(PointerButton::Primary) || meio {
        if let Some(o) = ui.input(|i| i.pointer.press_origin()) {
            let cel = st.s2w(area, o);
            let canto = selo_r.and_then(|r| [r.left_top(), r.right_top(), r.left_bottom(), r.right_bottom()].iter().position(|c| (*c - o).length() <= 8.0));
            let selo_livre = st.selo.as_ref().is_some_and(|s| !s.travado);
            st.arrasto = Some(if meio || (mod_ligado && espaco) {
                Arrasto::Pan(o, (st.cam.0, st.cam.1), false)
            } else if st.moderacao.aberta && st.moderacao.apagar {
                Arrasto::Apagar
            } else if st.moderacao.aberta && st.moderacao.selecionar {
                if st.moderacao.sel.contains(&cel) { Arrasto::Mover(cel) } else { Arrasto::Caixa(cel) }
            } else if selo_livre && (canto.is_some() || selo_r.is_some_and(|r| r.contains(o))) {
                let s = st.selo.as_ref().unwrap();
                Arrasto::Selo(o, (s.x, s.y, s.gw, s.gh), canto)
            } else {
                Arrasto::Pan(o, (st.cam.0, st.cam.1), false)
            });
        }
    }
    if let Some(q) = ui.input(|i| i.pointer.interact_pos()) {
        match st.arrasto {
            Some(Arrasto::Pan(o, c, _)) if resp.dragged() => {
                let d = q - o;
                let moved = d.length() > 4.0;
                st.arrasto = Some(Arrasto::Pan(o, c, moved));
                if moved {
                    st.cam.0 = (c.0 - d.x / ps).clamp(-WORLD_LIMIT as f32, WORLD_LIMIT as f32);
                    st.cam.1 = (c.1 - d.y / ps).clamp(-WORLD_LIMIT as f32, WORLD_LIMIT as f32);
                }
            }
            Some(Arrasto::Apagar) if resp.dragged() => {
                let at = st.s2w(area, q);
                st.apagar_qualquer(at);
            }
            Some(Arrasto::Selo(o, (sx, sy, gw, gh), canto)) if resp.dragged() => {
                let d = (q - o) / ps;
                if let Some(s) = st.selo.as_mut() {
                    match canto {
                        // a alça de baixo à direita (e as outras) muda o tamanho, mantendo a proporção da imagem
                        Some(c) => {
                            let dx = if c % 2 == 1 { d.x } else { -d.x };
                            let nw = (gw as f32 + dx).round().clamp(4.0, 512.0) as u32;
                            let nh = ((gh as f32 / gw as f32) * nw as f32).round().max(1.0) as u32;
                            s.gw = nw;
                            s.gh = nh;
                            if c % 2 == 0 {
                                s.x = sx + gw as f32 - nw as f32;
                            }
                            if c < 2 {
                                s.y = sy + gh as f32 - nh as f32;
                            }
                        }
                        None => {
                            s.x = (sx + d.x).round();
                            s.y = (sy + d.y).round();
                        }
                    }
                }
            }
            _ => {}
        }
    }
    if resp.drag_stopped() {
        match st.arrasto.take() {
            Some(Arrasto::Caixa(a)) => {
                if let Some(h) = st.hover {
                    if !ui.input(|i| i.modifiers.shift) {
                        st.moderacao.sel.clear();
                    }
                    for x in a.0.min(h.0)..=a.0.max(h.0) {
                        for y in a.1.min(h.1)..=a.1.max(h.1) {
                            if st.pixels.contains_key(&(x, y)) {
                                st.moderacao.sel.insert((x, y));
                            }
                        }
                    }
                }
            }
            Some(Arrasto::Mover(a)) => {
                if let Some(h) = st.hover {
                    st.mover_selecao((h.0 - a.0, h.1 - a.1));
                }
            }
            _ => {}
        }
    }

    // o clique sem arrastar: pintar, apagar, pegar a cor; o direito mostra quem pintou
    if resp.clicked_by(PointerButton::Primary) {
        if let Some(at) = pointer.map(|q| st.s2w(area, q)) {
            if st.moderacao.aberta && st.moderacao.apagar {
                st.apagar_qualquer(at);
            } else if st.moderacao.aberta && st.moderacao.selecionar {
                if !ui.input(|i| i.modifiers.shift) {
                    st.moderacao.sel.clear();
                }
                if st.pixels.contains_key(&at) {
                    st.moderacao.sel.insert(at);
                }
            } else if picking {
                match st.pixels.get(&at) {
                    Some(px) => {
                        st.color = px.c;
                        st.erase = false;
                        st.eyedrop = false;
                        if !PALETTE.contains(&px.c) && !st.custom.contains(&px.c) {
                            st.custom.insert(0, px.c);
                            st.custom.truncate(15);
                        }
                    }
                    None => st.say("esse lugar está vazio", true, t),
                }
            } else if ps < PAINT_MIN_PS {
                st.say("aproxime para pintar", true, t);
            } else if st.erase {
                st.erase_at(at);
            } else {
                st.place(at, t);
            }
        }
    }
    if resp.secondary_clicked() {
        if let Some(q) = pointer {
            let at = st.s2w(area, q);
            st.info = st.pixels.contains_key(&at).then_some((at, q));
        }
    }

    // a roda aproxima e afasta em volta do mouse
    if resp.hovered() {
        let scroll = ui.input(|i| i.smooth_scroll_delta.y);
        if scroll != 0.0 {
            if let Some(q) = pointer {
                let before = ((q.x - area.center().x) / ps + st.cam.0, (q.y - area.center().y) / ps + st.cam.1);
                st.cam.2 = (st.cam.2 * (scroll * 0.0025).exp()).clamp(Z_MIN, Z_MAX);
                let ps2 = PX * st.cam.2;
                st.cam.0 = before.0 - (q.x - area.center().x) / ps2;
                st.cam.1 = before.1 - (q.y - area.center().y) / ps2;
            }
        }
    }

    // a prévia: o pincel, o apagador ou o quadrado do conta-gotas
    if let Some(h) = st.hover {
        if st.arrasto.is_none() && !mod_ligado && ps >= PAINT_MIN_PS && resp.hovered() {
            if picking {
                let s = st.w2s(area, h.0 as f32, h.1 as f32).round();
                p.rect_stroke(Rect::from_min_size(s + Vec2::splat(1.0), Vec2::splat(side - 2.0)), 0.0, Stroke::new(2.0_f32, BG), StrokeKind::Middle);
            } else if st.erase {
                for c in st.brush_cells(h) {
                    let can = st.pixels.get(&c).is_some_and(|px| st.mine(px));
                    let s = st.w2s(area, c.0 as f32, c.1 as f32).round();
                    let r = Rect::from_min_size(s, Vec2::splat(side));
                    p.rect_filled(r, 0.0, rgba(0xb13e53, if can { 0.45 } else { 0.15 }));
                    dashed(&p, r.shrink(1.0), if can { BAD } else { rgba(0xb13e53, 0.5) }, !can);
                }
            } else if !selo_r.is_some_and(|r| r.contains(ui.input(|i| i.pointer.hover_pos()).unwrap_or_default())) || st.selo.as_ref().is_some_and(|s| s.travado) {
                let can = st.can_place();
                for c in st.brush_cells(h) {
                    let s = st.w2s(area, c.0 as f32, c.1 as f32).round();
                    let r = Rect::from_min_size(s, Vec2::splat(side));
                    p.rect_filled(r, 0.0, st.color.gamma_multiply(if can { 0.42 } else { 0.18 }));
                    dashed(&p, r.shrink(1.0), if can { BG } else { rgba(0xb13e53, 0.9) }, !can);
                }
            }
        }
    }
}

/// A borda de 2 px da prévia: cheia, ou tracejada (4 cheio, 3 vazio) quando não dá para pintar ali.
fn dashed(p: &Painter, r: Rect, c: Color32, dash: bool) {
    let s = Stroke::new(2.0_f32, c);
    if !dash {
        p.rect_stroke(r, 0.0, s, StrokeKind::Middle);
        return;
    }
    let pts = [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom(), r.left_top()];
    p.extend(Shape::dashed_line(&pts, s, 4.0, 3.0));
}

/// Os territórios (o botão de contorno): a caixa em volta dos pixels de cada streamer, na cor dele, com o nome em cima.
fn territories(p: &Painter, st: &LpState, area: Rect, ps: f32) {
    let mut boxes: HashMap<&str, (i32, i32, i32, i32)> = HashMap::new();
    for (&(x, y), px) in &st.pixels {
        let b = boxes.entry(px.streamer.as_str()).or_insert((x, y, x, y));
        *b = (b.0.min(x), b.1.min(y), b.2.max(x), b.3.max(y));
    }
    for (sid, (x0, y0, x1, y1)) in boxes {
        let c = streamer_color(sid);
        let a = st.w2s(area, x0 as f32 - 1.0, y0 as f32 - 1.0);
        let b = st.w2s(area, x1 as f32 + 2.0, y1 as f32 + 2.0);
        let r = Rect::from_min_max(a, b);
        p.rect_filled(r, 0.0, c.gamma_multiply(0.10));
        p.rect_stroke(r, 0.0, Stroke::new(2.0_f32, c), StrokeKind::Middle);
        let name = st.name_of(sid);
        let f = comic((ps * 0.7).clamp(11.0, 18.0));
        let g = p.layout_no_wrap(name, f, Color32::WHITE);
        let tag = Rect::from_min_size(Pos2::new(r.left(), r.top() - g.size().y - 6.0), g.size() + Vec2::new(10.0, 6.0));
        p.rect_filled(tag, 0.0, c);
        p.galley(tag.min + Vec2::new(5.0, 3.0), g, Color32::WHITE);
    }
}

/// O logo: cada letra inclinada e de uma cor, com o contorno duplo (escuro por fora, branco por dentro).
fn logo(p: &Painter, at: Pos2, size: f32) {
    let f = comic(size);
    let mut x = at.x;
    for (i, ch) in "LivePixel".chars().enumerate() {
        let s = ch.to_string();
        let g = p.layout_no_wrap(s.clone(), f.clone(), LOGO_C[i % 8]);
        let w = g.size().x;
        let ang = (((i * 37) % 9) as f32 - 4.0).to_radians();
        let c = Pos2::new(x + w / 2.0, at.y + size / 2.0);
        let paint = |off: Vec2, color: Color32| {
            let g = p.layout_no_wrap(s.clone(), f.clone(), color);
            // a letra gira em volta do próprio centro: o canto de cima gira junto
            let half = g.size() / 2.0;
            let rot = egui::emath::Rot2::from_angle(ang);
            let pos = c + off - rot * half;
            p.add(egui::epaint::TextShape::new(pos, g, color).with_angle(ang));
        };
        for k in 0..16 {
            let a = k as f32 / 16.0 * std::f32::consts::TAU;
            paint(Vec2::new(a.cos(), a.sin()) * 5.5 + Vec2::new(0.0, 1.0), LINE);
        }
        for k in 0..12 {
            let a = k as f32 / 12.0 * std::f32::consts::TAU;
            paint(Vec2::new(a.cos(), a.sin()) * 2.0, Color32::WHITE);
        }
        paint(Vec2::ZERO, LOGO_C[i % 8]);
        x += w + 1.0;
    }
}

/// O alto do meio: ligar e desligar a grade e os territórios.
fn top_controls(ui: &mut egui::Ui, st: &mut LpState, body: Rect) {
    let p = ui.painter().clone();
    let cx = body.center().x;
    for (i, (key, tip, icon)) in [("grade", "mostrar/ocultar grade", ic!("lp-gridToggle")), ("contorno", "mostrar/ocultar territórios e nomes", ic!("lp-outlineToggle"))].into_iter().enumerate() {
        let r = Rect::from_min_size(Pos2::new(cx - 44.0 + i as f32 * 48.0, body.top() + GAP), Vec2::splat(40.0));
        let resp = hit(ui, r, key, tip);
        let on = if i == 0 { st.grid } else { st.outlines };
        let (fg, line) = if on { (ACCENT, ACCENT) } else if resp.hovered() { (INK, LINE_HI) } else { (DIM, LINE) };
        boxed(&p, r, BG, BW, line);
        ui::icon(ui, icon, r.center(), 18.0, fg);
        if resp.clicked() {
            if i == 0 {
                st.grid = !st.grid;
            } else {
                st.outlines = !st.outlines;
            }
        }
    }
}

/// O placar (sempre aberto): as abas Streamers e Artistas e os cinco primeiros. Clicar num streamer voa até ele.
fn leaderboard(ui: &mut egui::Ui, st: &mut LpState, body: Rect) {
    let p = ui.painter().clone();
    let outer = Rect::from_min_size(Pos2::new(body.right() - GAP - 216.0, body.top() + GAP), Vec2::new(216.0, 239.0));
    escudo(ui, outer, "placar");
    boxed(&p, outer, BG, BW, LINE);
    let inner = outer.shrink(BW);
    // as abas
    let tabs = Rect::from_min_size(inner.min, Vec2::new(inner.width(), 31.0));
    p.rect_filled(Rect::from_min_max(Pos2::new(tabs.left(), tabs.bottom() - 2.0), tabs.max), 0.0, rgba(0x333c57, 0.5));
    for (i, (tab, label)) in [(LbTab::Streamers, "STREAMERS"), (LbTab::Artistas, "ARTISTAS")].into_iter().enumerate() {
        let r = Rect::from_min_size(Pos2::new(tabs.left() + i as f32 * tabs.width() / 2.0, tabs.top()), Vec2::new(tabs.width() / 2.0, 31.0));
        let resp = hit(ui, r, ("lbtab", i), if i == 1 { "quem tem mais pixels no canvas, de qualquer streamer" } else { "" });
        let on = st.lb_tab == tab;
        let c = if on { ACCENT } else if resp.hovered() { INK } else { LP_E };
        text(&p, Pos2::new(r.center().x, r.top() + 8.0 + 7.0), Align2::CENTER_CENTER, label, comic(11.0), c, 0.4);
        if on {
            p.rect_filled(Rect::from_min_max(Pos2::new(r.left(), r.bottom() - 3.0), r.right_bottom()), 0.0, ACCENT);
        }
        if resp.clicked() {
            st.lb_tab = tab;
        }
    }
    // as linhas
    struct Row {
        rank: usize,
        name: String,
        count: u64,
        avatar: Option<ImageSource<'static>>,
        mine: bool,
        sid: Option<String>,
    }
    let rows: Vec<Row> = match st.lb_tab {
        LbTab::Streamers => {
            let mut list: Vec<(String, u64)> = st.lb_counts.iter().map(|(k, v)| (k.clone(), *v)).collect();
            list.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
            let mine = st.streamer.clone();
            let my_rank = mine.as_ref().and_then(|m| list.iter().position(|(k, _)| k == m)).map(|i| i + 1);
            let mut top: Vec<Row> = list.iter().take(5).enumerate().map(|(i, (sid, n))| Row { rank: i + 1, name: st.name_of(sid), count: *n, avatar: st.avatar_of(sid), mine: mine.as_deref() == Some(sid), sid: Some(sid.clone()) }).collect();
            // a 5ª vaga é do streamer do jogador, se ele estiver mais para baixo
            if let (Some(m), Some(r)) = (mine, my_rank) {
                if r > 5 && top.len() >= 5 {
                    top[4] = Row { rank: r, name: st.name_of(&m), count: st.lb_counts[&m], avatar: st.avatar_of(&m), mine: true, sid: Some(m) };
                }
            }
            top
        }
        LbTab::Artistas => {
            let mut top: Vec<Row> = st.m.artists.iter().take(5).enumerate().map(|(i, a)| Row { rank: a.rank.unwrap_or(i + 1), name: a.display_name.clone(), count: a.pixel_count, avatar: a.avatar.as_deref().and_then(net::imagem), mine: a.display_name == st.m.artists_me.display_name && !a.display_name.is_empty(), sid: None }).collect();
            let me = &st.m.artists_me;
            if me.rank.unwrap_or(0) > 5 && top.len() >= 5 {
                top[4] = Row { rank: me.rank.unwrap_or(6), name: me.display_name.clone(), count: st.my_count, avatar: net::imagem(&st.m.me.avatar), mine: true, sid: None };
            }
            top
        }
    };
    let mut y = tabs.bottom() + 6.0;
    let x = inner.left() + 6.0;
    let w = inner.width() - 12.0;
    let mut fly = None;
    for row in rows {
        let r = Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, 38.0));
        p.rect_filled(Rect::from_min_size(r.min, Vec2::new(w, 2.0)), 0.0, rgba(0x333c57, 0.5));
        let body_r = Rect::from_min_max(r.min + Vec2::new(0.0, 2.0), r.max);
        let resp = if row.sid.is_some() { hit(ui, r, ("lbrow", &row.name), "ir para o território") } else { ui.interact(r, ui.id().with(("lbrow", &row.name)), Sense::hover()) };
        if row.mine {
            p.rect_filled(body_r, 0.0, rgba(0x41a6f6, 0.10));
            p.rect_stroke(body_r, 0.0, Stroke::new(2.0_f32, ACCENT), StrokeKind::Inside);
        }
        let cy = body_r.center().y;
        text(&p, Pos2::new(r.left() + 6.0 + 10.0, cy), Align2::CENTER_CENTER, &row.rank.to_string(), comic(12.0), DIM, 0.0);
        let av = Rect::from_min_size(Pos2::new(r.left() + 34.0, cy - 13.0), Vec2::splat(26.0));
        if let Some(src) = row.avatar {
            p.rect_filled(av, 0.0, Color32::WHITE);
            ui::image(ui, src, av.shrink(2.0), 0.0);
            p.rect_stroke(av, 0.0, Stroke::new(2.0_f32, LINE), StrokeKind::Inside);
        }
        let count = fmt_count(row.count);
        let cw = text_width(&p, &count, comic(13.0), 0.0);
        text(&p, Pos2::new(r.right() - 6.0, cy), Align2::RIGHT_CENTER, &count, comic(13.0), ACCENT, 0.0);
        ui::clip(&p, Pos2::new(r.left() + 68.0, cy), Align2::LEFT_CENTER, &row.name, inter(13.0, 600), INK, w - 68.0 - 6.0 - cw - 8.0);
        if resp.clicked() {
            fly = row.sid;
        }
        y += 38.0;
    }
    if let Some(sid) = fly {
        // o centro dos pixels do streamer
        let pts: Vec<(i32, i32)> = st.pixels.iter().filter(|(_, px)| px.streamer == sid).map(|(k, _)| *k).collect();
        if !pts.is_empty() {
            let n = pts.len() as f32;
            st.cam.0 = pts.iter().map(|p| p.0 as f32 + 0.5).sum::<f32>() / n;
            st.cam.1 = pts.iter().map(|p| p.1 as f32 + 0.5).sum::<f32>() / n;
        }
    }
}

/// A doca de baixo à esquerda: o chat em cima e a paleta embaixo, com o apagador e o conta-gotas ao lado.
fn dock(ui: &mut egui::Ui, st: &mut LpState, body: Rect, t: f64) {
    let pal = Rect::from_min_size(Pos2::new(body.left() + GAP, body.bottom() - GAP - 115.0), Vec2::new(285.0, 115.0));
    let chat_r = Rect::from_min_size(Pos2::new(pal.left(), pal.top() - STACK - 190.0), Vec2::new(337.0, 190.0));
    escudo(ui, pal, "paleta");
    // do chat, só o campo segura o clique: as mensagens são transparentes ao canvas, como no site
    escudo(ui, Rect::from_min_size(Pos2::new(chat_r.left(), chat_r.bottom() - 36.0), Vec2::new(337.0, 36.0)), "chat");
    chat(ui, st, chat_r, t);
    palette(ui, st, pal);
    cor_picker(ui, st);
    let tools = Rect::from_min_size(Pos2::new(pal.right() + 8.0, pal.top()), Vec2::new(44.0, 115.0));
    let p = ui.painter().clone();
    for (i, (key, tip, icon)) in [("apagar", "apagar meus pixels", ic!("lp-eraseBtn")), ("conta-gotas", "capturar cor do canvas (conta-gotas) — ou segure Alt ou Shift e clique", ic!("lp-eyedropBtn"))].into_iter().enumerate() {
        let r = Rect::from_min_size(Pos2::new(tools.left(), tools.top() + i as f32 * (54.5 + 6.0)), Vec2::new(44.0, 54.5));
        let resp = hit(ui, r, key, tip);
        let held = i == 1 && ui.input(|i| i.modifiers.alt || i.modifiers.shift);
        let on = if i == 0 { st.erase } else { st.eyedrop || held };
        let tint = if i == 0 { BAD } else { GOLD };
        if on {
            // o anel duplo: o fundo do HUD e depois a cor da ferramenta
            p.rect_stroke(r.expand(2.0), 0.0, Stroke::new(2.0_f32, BG), StrokeKind::Inside);
            p.rect_stroke(r.expand(4.0), 0.0, Stroke::new(2.0_f32, tint), StrokeKind::Inside);
            boxed(&p, r, ui::mix(tint, BG, 0.15), BW, tint);
        } else {
            boxed(&p, r, BG, BW, if resp.hovered() { LINE_HI } else { LINE });
        }
        ui::icon(ui, icon, r.center(), 18.0, if on { tint } else if resp.hovered() { INK } else { DIM });
        if resp.clicked() {
            if i == 0 {
                st.erase = !st.erase;
                st.eyedrop = false;
            } else {
                st.eyedrop = !st.eyedrop;
                st.erase = false;
            }
        }
    }
}

fn chat(ui: &mut egui::Ui, st: &mut LpState, r: Rect, t: f64) {
    let p = ui.painter().clone();
    let input_id = ui.id().with("lp-chat-input");
    let focused = ui.memory(|m| m.has_focus(input_id));
    // o fundo em degradê (mais escuro embaixo; mais forte com o campo em foco)
    let (a0, a1, a2) = if focused { (0.2, 0.6, 0.75) } else { (0.0, 0.3, 0.5) };
    let mid = Pos2::new(r.right(), r.top() + r.height() * if focused { 0.30 } else { 0.35 });
    ui::vgradient(&p, Rect::from_min_max(r.min, mid), rgba(0x1a1c2c, a0), rgba(0x1a1c2c, a1));
    ui::vgradient(&p, Rect::from_min_max(Pos2::new(r.left(), mid.y), r.max), rgba(0x1a1c2c, a1), rgba(0x1a1c2c, a2));
    if focused {
        p.rect_filled(Rect::from_min_size(r.min, Vec2::new(r.width(), 2.0)), 0.0, rgba(0x333c57, 0.5));
    }

    // as mensagens: as 20 últimas do canal, de baixo para cima
    let area = Rect::from_min_size(r.min + Vec2::new(10.0, 8.0 + 2.0), Vec2::new(r.width() - 18.0, 136.0));
    let msgs: Vec<&Msg> = st.chat.iter().filter(|m| if st.chat_global { true } else { !m.global }).filter(|m| m.global || st.streamer.as_deref() == Some(m.streamer.as_str())).collect();
    let mut y = area.bottom();
    let cp = ui.painter_at(area);
    for m in msgs.iter().rev().take(20) {
        let mut job = LayoutJob::default();
        let f = comic(13.0);
        let fmt = |c: Color32| TextFormat { font_id: f.clone(), color: c, line_height: Some(16.25), ..Default::default() };
        if m.user.is_empty() {
            job.append(&m.text, 0.0, fmt(LP_4));
            job.wrap.max_width = area.width();
            let g = cp.layout_job(job);
            y -= g.size().y;
            outlined_cor(&cp, Pos2::new(area.left(), y), g);
            y -= 4.0;
            continue;
        }
        let sc = streamer_color(&m.streamer);
        if m.global {
            job.append("[G] ", 0.0, fmt(LP_4));
        } else {
            job.append("[Guilda] ", 0.0, fmt(sc));
        }
        // o lugar do selo (a foto do streamer, 12×12): um espaço da largura dele
        let av_at = job.text.chars().count();
        // um "em" de 15 px: os 12 do selo e os 3 de margem
        job.append("\u{2003}", 0.0, TextFormat { font_id: FontId::new(15.0, f.family.clone()), ..fmt(Color32::TRANSPARENT) });
        job.append(&format!("{}: ", m.user), 0.0, fmt(lighter(sc)));
        job.append(&m.text, 0.0, fmt(Color32::WHITE));
        job.wrap.max_width = area.width();
        let g = cp.layout_job(job);
        y -= g.size().y;
        let pos = Pos2::new(area.left(), y);
        let cur = g.pos_from_cursor(&g.from_ccursor(egui::text::CCursor::new(av_at)));
        outlined(&cp, pos, g.clone());
        let av = Rect::from_min_size(pos + Vec2::new(cur.left() + 1.0, cur.top() + 2.6), Vec2::splat(12.0));
        cp.rect_filled(av.expand(1.0), 0.0, BG);
        match st.avatar_of(&m.streamer) {
            Some(src) => ui::image(ui, src, av, 0.0),
            None => {
                cp.rect_filled(av, 0.0, sc);
            }
        }
        y -= 4.0;
        if y < area.top() {
            break;
        }
    }

    // o campo: o canal, o texto e o enviar
    let wrap = Rect::from_min_size(Pos2::new(r.left(), r.bottom() - 36.0), Vec2::new(r.width(), 36.0));
    boxed(&p, wrap, if focused { BG } else { rgba(0x1a1c2c, 0.7) }, 2.0, if focused { ACCENT } else { LINE });
    let tag = if st.chat_global { ("[G]".to_owned(), ACCENT) } else { ("[Guilda]".to_owned(), st.streamer.as_deref().map(streamer_color).unwrap_or(DIM)) };
    let tw = text_width(&p, &tag.0, comic(11.0), 0.0);
    let chan = Rect::from_min_size(Pos2::new(wrap.left() + 10.0, wrap.center().y - 13.0), Vec2::new(7.0 + tw + 5.0 + 10.0 + 7.0 + 2.0, 26.0));
    let cresp = hit(ui, chan, "canal", "Escolher o canal do chat");
    let hot = cresp.hovered() || st.chan_menu;
    boxed(&p, chan, rgba(0x333c57, if hot { 0.7 } else { 0.4 }), 1.0, if hot { ACCENT } else { LINE });
    text(&p, Pos2::new(chan.left() + 8.0, chan.center().y), Align2::LEFT_CENTER, &tag.0, comic(11.0), tag.1, 0.0);
    let chev = Pos2::new(chan.right() - 8.0 - 5.0, chan.center().y);
    if st.chan_menu {
        ui::icon(ui, ic!("chevron-down"), chev, 10.0, DIM);
    } else {
        ui::icon(ui, ic!("chevron-up"), chev, 10.0, DIM);
    }
    if cresp.clicked() {
        st.chan_menu = !st.chan_menu;
    }
    let send = Rect::from_min_size(Pos2::new(wrap.right() - 6.0 - 28.0, wrap.center().y - 14.0), Vec2::splat(28.0));
    let field = Rect::from_min_max(Pos2::new(chan.right() + 6.0, wrap.top() + 6.5), Pos2::new(send.left() - 6.0, wrap.bottom() - 6.5));
    let hint = if st.chat_global { "Enviar mensagem no chat global..." } else { "Enviar mensagem para a guilda..." };
    let mut fui = ui.new_child(UiBuilder::new().max_rect(field));
    let out = fui.add(
        egui::TextEdit::singleline(&mut st.chat_input)
            .id(input_id)
            .frame(false)
            .hint_text(egui::RichText::new(hint).font(comic4(12.0)).color(DIM))
            .font(comic(12.0))
            .text_color(INK)
            .char_limit(200)
            .vertical_align(egui::Align::Center)
            .margin(egui::Margin::ZERO)
            .desired_width(field.width()),
    );
    let sresp = hit(ui, send, "enviar", "Enviar mensagem");
    boxed(&p, send, if sresp.hovered() { ui::mix(Color32::WHITE, ACCENT, 0.1) } else { ACCENT }, 2.0, LINE);
    ui::icon(ui, ic!("lp-chatSendBtn"), send.center(), 16.0, BG);
    let enter = out.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
    if (enter || sresp.clicked()) && !st.chat_input.trim().is_empty() {
        if !st.chat_global && st.streamer.is_none() {
            st.say("escolha um streamer para falar com a guilda", true, t);
        } else {
            if st.m.me.id.is_empty() {
                st.say("entre para conversar no chat", true, t);
            } else {
                let text = st.chat_input.trim().to_owned();
                st.mandar(serde_json::json!({ "type": "chat_send", "content": text, "channel": if st.chat_global { "global" } else { "streamer" } }));
                st.chat_input.clear();
            }
        }
        if enter {
            out.request_focus();
        }
    }

    // o menu do canal, acima do botão
    if st.chan_menu {
        let menu = Rect::from_min_size(Pos2::new(chan.left() - 6.0, chan.top() - 8.0 - (6.0 + 2.0 * 33.0 + 2.0 + 6.0)), Vec2::new(200.0, 6.0 + 2.0 * 33.0 + 2.0 + 6.0));
        let mp = ui.ctx().layer_painter(egui::LayerId::new(egui::Order::Foreground, ui.id().with("lp-chan-menu")));
        boxed(&mp, menu, BG, BW, LINE);
        let sname = st.streamer.as_deref().map(|s| st.name_of(s));
        let opts = [(true, "[G]".to_owned(), ACCENT, "Global".to_owned(), false), (false, "[Guilda]".to_owned(), st.streamer.as_deref().map(streamer_color).unwrap_or(DIM), sname.clone().unwrap_or("Sem streamer".into()), sname.is_none())];
        let mut fechou = false;
        for (i, (global, tag, tc, name, empty)) in opts.into_iter().enumerate() {
            let o = Rect::from_min_size(menu.min + Vec2::new(BW + 4.0, BW + 4.0 + i as f32 * 35.0), Vec2::new(menu.width() - 2.0 * (BW + 4.0), 33.0));
            let resp = ui.interact(o, ui.id().with(("lp-chan-opt", i)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
            if st.chat_global == global {
                mp.rect_filled(o, 0.0, rgba(0x41a6f6, 0.14));
            } else if resp.hovered() {
                mp.rect_filled(o, 0.0, rgba(0x333c57, 0.5));
            }
            let tr = text(&mp, Pos2::new(o.left() + 10.0, o.center().y), Align2::LEFT_CENTER, &tag, comic(12.0), tc, 0.0);
            text(&mp, Pos2::new(tr.right() + 8.0, o.center().y), Align2::LEFT_CENTER, &name, if empty { comic4(12.0) } else { comic(12.0) }, if empty { DIM } else { INK }, 0.0);
            if resp.clicked() {
                st.chat_global = global;
                fechou = true;
            }
        }
        if fechou || (ui.input(|i| i.pointer.any_pressed()) && !ui.input(|i| i.pointer.interact_pos()).is_some_and(|q| menu.contains(q) || chan.contains(q))) {
            st.chan_menu = false;
        }
    }
}

fn palette(ui: &mut egui::Ui, st: &mut LpState, r: Rect) {
    let p = ui.painter().clone();
    boxed(&p, r, BG, BW, LINE);
    // as páginas: (1) a paleta e (2) as cores recentes
    let mut x = r.left() + 13.0;
    for pg in [1u8, 2] {
        let label = format!("({pg})");
        let w = text_width(&p, &label, comic(11.0), 0.0) + 14.0;
        let b = Rect::from_min_size(Pos2::new(x, r.top() + 14.5), Vec2::new(w, 17.0));
        let resp = hit(ui, b, ("pagina", pg), if pg == 1 { "Paleta Principal" } else { "Cores Recentes" });
        let on = st.page == pg;
        let (bg, line, fg) = if on { (BG, ACCENT, ACCENT) } else if resp.hovered() { (rgba(0x333c57, 0.4), LINE_HI, INK) } else { (rgba(0x333c57, 0.4), LINE, DIM) };
        boxed(&p, b, bg, 1.5, line);
        text(&p, b.center(), Align2::CENTER_CENTER, &label, comic(11.0), fg, 0.0);
        if resp.clicked() {
            st.page = pg;
        }
        x = b.right() + 4.0;
    }
    // os pincéis, à direita
    for (i, (name, _, icon)) in BRUSHES.iter().enumerate() {
        let b = Rect::from_min_size(Pos2::new(r.right() - BW - 10.0 - (6 - i) as f32 * 28.0 + 4.0, r.top() + 11.0), Vec2::splat(24.0));
        let resp = hit(ui, b, ("pincel", i), &format!("{name} — tecla {}", i + 1));
        let on = st.brush == i;
        boxed(&p, b, if on { BG } else { rgba(0x333c57, 0.4) }, 2.0, if on { ACCENT } else if resp.hovered() { LINE_HI } else { LINE });
        let o = b.center() - Vec2::splat(7.0);
        for c in icon.iter() {
            let cell = Rect::from_min_size(o + Vec2::new((c % 3) as f32 * 5.0, (c / 3) as f32 * 5.0), Vec2::splat(4.0));
            p.rect_filled(cell, 0.0, if on { ACCENT } else { DIM });
        }
        if resp.clicked() {
            st.brush = i;
        }
    }
    // as cores: 8 por linha, 28 px, 5 entre elas
    let mut cells: Vec<Option<Color32>> = if st.page == 1 { PALETTE.iter().map(|c| Some(*c)).collect() } else { (0..15).map(|k| st.custom.get(k).copied()).collect() };
    if st.page == 2 {
        cells.push(None);
    }
    for (i, c) in cells.iter().enumerate() {
        let sw = Rect::from_min_size(Pos2::new(r.left() + 13.0 + (i % 8) as f32 * 33.0, r.top() + 41.0 + (i / 8) as f32 * 33.0), Vec2::splat(28.0));
        let rainbow = st.page == 2 && i == 15;
        let resp = hit(ui, sw, ("cor", st.page, i), &match c {
            Some(c) => format!("#{:02x}{:02x}{:02x}", c.r(), c.g(), c.b()),
            None if rainbow => "Color Picker (Escolher cor)".into(),
            None => "Slot vazio — clique para escolher cor".into(),
        });
        let sel = c.is_some_and(|c| c == st.color) && !st.erase && !st.eyedrop;
        let grow = if sel || resp.hovered() { if rainbow { 1.15 } else { 1.12 } } else { 1.0 };
        let sr = Rect::from_center_size(sw.center(), sw.size() * grow);
        match c {
            Some(c) => {
                p.rect_filled(sr, 0.0, *c);
                p.rect_stroke(sr, 0.0, Stroke::new(BW, if sel { INK } else { rgba(0x1a1c2c, 0.25) }), StrokeKind::Inside);
            }
            None if rainbow => {
                // o degradê do arco-íris em diagonal
                let stops = [rgb(0xff0000), rgb(0xff7f00), rgb(0xffff00), rgb(0x00ff00), rgb(0x0000ff), rgb(0x4b0082), rgb(0x9400d3)];
                let mut mesh = egui::Mesh::default();
                let n = 24;
                for yy in 0..n {
                    for xx in 0..n {
                        let k = ((xx + yy) as f32 / (2 * n - 2) as f32) * 6.0;
                        let (a, b) = (stops[(k.floor() as usize).min(6)], stops[(k.ceil() as usize).min(6)]);
                        let col = ui::mix(b, a, k.fract());
                        let cell = Rect::from_min_size(sr.min + Vec2::new(xx as f32, yy as f32) * sr.width() / n as f32, Vec2::splat(sr.width() / n as f32 + 0.5));
                        mesh.add_colored_rect(cell, col);
                    }
                }
                p.add(mesh);
                p.rect_stroke(sr, 0.0, Stroke::new(1.5_f32, Color32::from_white_alpha(178)), StrokeKind::Inside);
            }
            None => {
                p.rect_filled(sr, 0.0, rgba(0x1a1c2c, 0.15));
                let pts = [sr.left_top(), sr.right_top(), sr.right_bottom(), sr.left_bottom(), sr.left_top()];
                p.extend(Shape::dashed_line(&pts, Stroke::new(1.5_f32, if resp.hovered() { ACCENT } else { LINE }), 4.0, 3.0));
            }
        }
        if resp.clicked() {
            match c {
                Some(c) => {
                    st.color = *c;
                    st.erase = false;
                    st.eyedrop = false;
                }
                // o arco-íris e as casas vazias abrem o seletor de cor (o `<input type=color>` do site)
                None => st.cor_picker = Some(sw.left_top()),
            }
        }
    }
}

/// O seletor de cor da página (2): a cor escolhida vira a do pincel e entra nas recentes.
fn cor_picker(ui: &egui::Ui, st: &mut LpState) {
    let Some(at) = st.cor_picker else { return };
    let mut c = st.color;
    let area = egui::Area::new(ui.id().with("lp-cor")).order(egui::Order::Foreground).fixed_pos(at - Vec2::new(0.0, 270.0)).show(ui.ctx(), |ui| {
        egui::Frame::new().fill(BG).stroke(Stroke::new(BW, LINE)).inner_margin(10.0).show(ui, |ui| {
            ui.spacing_mut().item_spacing = Vec2::splat(6.0);
            egui::color_picker::color_picker_color32(ui, &mut c, egui::color_picker::Alpha::Opaque)
        })
    });
    if area.inner.inner {
        st.color = c;
        st.erase = false;
        st.eyedrop = false;
    }
    if ui.input(|i| i.pointer.any_pressed()) && !area.response.hovered() {
        // ao fechar, a cor nova entra nas recentes
        if !PALETTE.contains(&st.color) && !st.custom.contains(&st.color) {
            st.custom.insert(0, st.color);
            st.custom.truncate(15);
        }
        st.cor_picker = None;
    }
}

/// As coordenadas do mouse, no pé do meio.
fn coords(p: &Painter, st: &LpState, body: Rect) {
    let s = match st.hover {
        Some((x, y)) => format!("x {x} · y {y}"),
        None => format!("x {} · y {}", st.cam.0.floor() as i32, st.cam.1.floor() as i32),
    };
    let w = text_width(p, &s, comic(12.0), 0.0) + 24.0 + 2.0 * BW;
    let r = Rect::from_min_size(Pos2::new(body.center().x - w / 2.0, body.bottom() - GAP - 30.0), Vec2::new(w, 30.0));
    boxed(p, r, BG, BW, LINE);
    text(p, r.center(), Align2::CENTER_CENTER, &s, comic(12.0), DIM, 0.0);
}

/// O canto de baixo à direita: moderação, imagem, ajuda e o perfil com os pixels que sobram. Devolve onde está o perfil.
fn hud_right(ui: &mut egui::Ui, st: &mut LpState, body: Rect, t: f64) -> Rect {
    let p = ui.painter().clone();
    let prof = Rect::from_min_size(Pos2::new(body.right() - GAP - 270.0, body.bottom() - GAP - HUD_H), Vec2::new(270.0, HUD_H));
    let mut x = prof.left() - 10.0;
    let mut buttons: Vec<(&str, &str, ImageSource<'static>, bool)> = vec![("ajuda", if st.roleta_pode() { "Roletasso Diário (1 rodada disponível)" } else { "Roletasso Diário" }, ic!("lp-helpBtn"), false), ("imagem", "Overlay de Imagem", ic!("lp-stampBtn"), false)];
    if st.m.me.moderador {
        buttons.push(("mod", "ferramentas de moderador", ic!("lp-modBtn"), true));
    }
    for (key, tip, icon, green) in buttons {
        let r0 = Rect::from_min_size(Pos2::new(x - HUD_H, prof.top()), Vec2::splat(HUD_H));
        let resp = hit(ui, r0, key, tip);
        let r = if resp.is_pointer_button_down_on() { r0.translate(Vec2::new(0.0, 1.0)) } else if resp.hovered() { r0.translate(Vec2::new(0.0, -2.0)) } else { r0 };
        if green {
            if st.moderacao.aberta {
                p.rect_stroke(r.expand(2.0), 0.0, Stroke::new(2.0_f32, BG), StrokeKind::Inside);
                p.rect_stroke(r.expand(4.0), 0.0, Stroke::new(2.0_f32, PALETTE[6]), StrokeKind::Inside);
            }
            boxed(&p, r, if resp.hovered() { rgb(0x45c975) } else { rgb(0x38b764) }, BW, if resp.hovered() { rgb(0x38b764) } else { rgb(0x2b8a4e) });
            ui::icon(ui, icon, r.center(), 24.0, Color32::WHITE);
        } else {
            boxed(&p, r, if resp.hovered() { LINE } else { BG }, BW, if resp.hovered() { LINE_HI } else { LINE });
            ui::icon(ui, icon, r.center(), 24.0, INK);
        }
        if resp.clicked() {
            match key {
                "ajuda" => st.janela = Some(Janela::Roleta),
                "imagem" => {
                    st.moderacao.aberta = false;
                    if st.selo.is_none() {
                        st.janela = Some(Janela::Imagem);
                    } else {
                        st.say("a imagem já está no canvas: use a barra de cima para mexer nela", false, t);
                    }
                }
                _ => {
                    st.moderacao.aberta = !st.moderacao.aberta;
                    st.moderacao.apagar = false;
                    st.moderacao.selecionar = false;
                    st.moderacao.sel.clear();
                }
            }
        }
        x = r0.left() - 10.0;
    }

    // o perfil: clicar abre o menu
    let resp = hit(ui, prof, "perfil", "opções");
    boxed(&p, prof, BG, BW, if resp.hovered() || st.pmenu { LINE_HI } else { LINE });
    let av = Rect::from_min_size(Pos2::new(prof.left() + 15.0, prof.center().y - 20.0), Vec2::splat(40.0));
    p.rect_filled(av, 0.0, Color32::WHITE);
    match net::imagem(&st.m.me.avatar) {
        Some(src) => ui::image(ui, src, av.shrink(2.0), 0.0),
        None => {
            p.rect_filled(av.shrink(2.0), 0.0, streamer_color(&st.m.me.name));
            text(&p, av.center(), Align2::CENTER_CENTER, &ui::initials(if st.m.me.name.is_empty() { "?" } else { &st.m.me.name }), inter(14.0, 700), Color32::WHITE, 0.0);
        }
    }
    // a borda dourada e a coroa são de quem é Premium; o bônus da roleta só aparece no chip de "+N"
    p.rect_stroke(av, 0.0, Stroke::new(2.0_f32, if st.m.me.premium { GOLD } else { LINE }), StrokeKind::Inside);
    let nome = ui::clip(&p, Pos2::new(prof.left() + 65.0, prof.top() + 17.3 + 7.15), Align2::LEFT_CENTER, if st.m.me.name.is_empty() { "visitante" } else { &st.m.me.name }, inter(13.0, 700), INK, 58.0);
    if st.m.me.premium {
        ui::icon(ui, ic!("lp-profPremBadge"), Pos2::new(nome.right() + 4.0 + 7.5, nome.center().y), 15.0, GOLD);
    }
    let sname = st.streamer.as_deref().map(|s| st.name_of(s)).unwrap_or("sem representante".into());
    ui::clip(&p, Pos2::new(prof.left() + 65.0, prof.top() + 34.6 + 6.05), Align2::LEFT_CENTER, &sname, inter(11.0, 600), DIM, 58.0);
    let ammo = Rect::from_min_size(Pos2::new(prof.left() + 123.0, prof.top() + 13.5), Vec2::new(132.0, 37.0));
    p.rect_filled(Rect::from_min_size(ammo.min, Vec2::new(2.0, 37.0)), 0.0, rgba(0x333c57, 0.6));
    let ax = ammo.left() + 14.0;
    let n = st.left.to_string();
    let nr = text(&p, Pos2::new(ax, ammo.top() + 10.0), Align2::LEFT_CENTER, &n, comic(20.0), if st.left == 0 && st.bonus == 0 { BAD } else { ACCENT }, 0.0);
    let mr = text(&p, Pos2::new(nr.right() + 3.0, ammo.top() + 12.5), Align2::LEFT_CENTER, &format!("/{MAX_PX}"), comic(11.0), LP_E, 0.0);
    if st.cd_end.is_some() {
        let s = st.cd_left(t).ceil() as i64;
        text(&p, Pos2::new(ammo.right(), ammo.top() + 12.5), Align2::RIGHT_CENTER, &format!("{}:{:02}", s / 60, s % 60), comic(11.0), DIM, 0.0);
    }
    if st.bonus > 0 {
        // o chip dourado dos pixels de bônus
        let s = format!("+{}", st.bonus);
        let w = text_width(&p, &s, comic(10.0), 0.0) + 10.0;
        let r = Rect::from_min_size(Pos2::new(mr.right() + 3.0, ammo.top() + 12.5 - 7.0), Vec2::new(w, 14.0));
        p.rect_filled(r, 0.0, GOLD);
        text(&p, r.center(), Align2::CENTER_CENTER, &s, comic(10.0), BG, 0.0);
    }
    let bar = Rect::from_min_size(Pos2::new(ax, ammo.top() + 25.0), Vec2::new(118.0, 12.0));
    p.rect_filled(bar, 0.0, LINE);
    let inner = bar.shrink(2.0);
    let k = st.left as f32 / MAX_PX as f32;
    p.rect_filled(Rect::from_min_size(inner.min, Vec2::new(inner.width() * k, inner.height())), 0.0, ACCENT);
    for i in 1..MAX_PX {
        let sx = inner.left() + inner.width() * i as f32 / MAX_PX as f32 - 1.0;
        p.rect_filled(Rect::from_min_size(Pos2::new(sx, inner.top()), Vec2::new(1.0, inner.height())), 0.0, BG);
    }
    if resp.clicked() {
        st.pmenu = !st.pmenu;
    }
    prof
}

/// O aviso rápido, acima das coordenadas: some em 2,6 s.
fn toast(ui: &egui::Ui, st: &mut LpState, body: Rect, t: f64) {
    let Some((msg, bad, at)) = st.toast.clone() else { return };
    let age = t - at;
    if age > 2.6 {
        st.toast = None;
        return;
    }
    let p = ui.ctx().layer_painter(egui::LayerId::new(egui::Order::Tooltip, ui.id().with("lp-toast")));
    let k = (age / 0.18).min(1.0) as f32;
    let w = text_width(&p, &msg, inter(13.0, 600), 0.0) + 32.0 + 2.0 * BW;
    let r = Rect::from_min_size(Pos2::new(body.center().x - w / 2.0, body.bottom() - GAP - 30.0 - STACK - 37.0 + 8.0 * (1.0 - k)), Vec2::new(w, 37.0));
    boxed(&p, r, BG.gamma_multiply(k), BW, if bad { BAD } else { ACCENT }.gamma_multiply(k));
    text(&p, r.center(), Align2::CENTER_CENTER, &msg, inter(13.0, 600), INK.gamma_multiply(k), 0.0);
    ui.ctx().request_repaint();
}

/// O teclado: 1..6 trocam o pincel, WASD e as setas andam, + e − aproximam (fora do campo do chat).
fn keys(ui: &egui::Ui, st: &mut LpState) {
    if ui.ctx().wants_keyboard_input() {
        return;
    }
    ui.input(|i| {
        for (k, key) in [Key::Num1, Key::Num2, Key::Num3, Key::Num4, Key::Num5, Key::Num6].iter().enumerate() {
            if i.key_pressed(*key) {
                st.brush = k;
            }
        }
        let v = 40.0 / (PX * st.cam.2);
        if i.key_pressed(Key::ArrowLeft) || i.key_pressed(Key::A) {
            st.cam.0 -= v;
        }
        if i.key_pressed(Key::ArrowRight) || i.key_pressed(Key::D) {
            st.cam.0 += v;
        }
        if i.key_pressed(Key::ArrowUp) || i.key_pressed(Key::W) {
            st.cam.1 -= v;
        }
        if i.key_pressed(Key::ArrowDown) || i.key_pressed(Key::S) {
            st.cam.1 += v;
        }
        if i.key_pressed(Key::Plus) || i.key_pressed(Key::Equals) {
            st.cam.2 = (st.cam.2 * 1.2).min(Z_MAX);
        }
        if i.key_pressed(Key::Minus) {
            st.cam.2 = (st.cam.2 / 1.2).max(Z_MIN);
        }
        if i.key_pressed(Key::Escape) {
            st.erase = false;
            st.eyedrop = false;
            st.chan_menu = false;
            st.info = None;
            st.pmenu = false;
            st.moderacao.sel.clear();
        }
        if st.moderacao.aberta && (i.key_pressed(Key::Delete) || i.key_pressed(Key::Backspace)) {
            let sel: Vec<(i32, i32)> = st.moderacao.sel.drain().collect();
            for at in sel {
                st.apagar_qualquer(at);
            }
        }
    });
}

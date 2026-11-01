//! A Call do site (`public/call/`, `call/css/call.css`): as colunas do `.dis`.
//!
//! A coluna de grupos do site (`.grail`) não existe aqui: os grupos moram na trilha do app (`shell.rs`). Ficam os
//! canais (`.dz-channels`, 248 px) · o conteúdo (conversa do canal de texto, a porta do canal
//! de voz ou o palco de quem está dentro) · pessoas (`.dz-people`, 270 px). Os nomes das funções seguem as classes do CSS.
//!
//! Tudo é de verdade: as salas, os canais, as pessoas e as conversas vêm do gateway da Call (`call/rede.rs`, guardados em
//! `dados.rs`); escrever vai pelo REST; a voz e o vídeo são o LiveKit (`call/voz.rs`).

use crate::dados::{self, Kind, Role};
use crate::net;
use crate::theme::*;
use crate::ui::{self, mix, text, text_width};
use crate::ic;
use eframe::egui::{self, Align2, Color32, CornerRadius, CursorIcon, ImageSource, Painter, Pos2, Rect, Sense, Stroke, StrokeKind, UiBuilder, Vec2};
use std::collections::{HashMap, HashSet};

mod acoes;
pub mod rede;
pub mod voz;
use acoes::Convite;

const CHLIST_W: f32 = 248.0;
const MEMBERS_W: f32 = 270.0;
/// `.col-head`: `--sp-md * 2 + 30px`; as quatro colunas alinham por ele.
const HEAD_H: f32 = 62.0;
const MEBAR_H: f32 = 67.0;
const DOCK_H: f32 = 53.0;
const COMPOSER_H: f32 = 81.0;
const CONTROLS_H: f32 = 75.0;
const PRESI_H: f32 = 49.0;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum View {
    Channel(usize),
}

/// Um grupo de mensagens seguidas da mesma pessoa (como o site junta).
pub struct Msg {
    pub author: usize,
    pub time: String,
    pub lines: Vec<String>,
}

pub struct CallState {
    /// O grupo aberto (índice em `dados::grupos()`); a trilha do app também troca ele.
    pub group: usize,
    pub view: View,
    pub joined: Option<usize>,
    muted: bool,
    deafened: bool,
    cam: bool,
    sharing: bool,
    draft: String,
    /// O cartão aberto: de quem e onde.
    card: Option<(usize, Pos2)>,
    /// Os menus abertos (onde ficam).
    grupo_menu: Option<Pos2>,
    conta_menu: Option<Pos2>,
    lol_menu: Option<Pos2>,
    /// O menu de "Mostrar a tela" (Qualidade ou Fluidez) e o último modo escolhido (o da tecla S).
    tela_menu: Option<Pos2>,
    tela_modo: voz::Modo,
    /// O seletor de tela do app (Windows e Linux em X11): as telas e janelas (carregando) e a aba aberta.
    seletor: Option<acoes::Seletor>,
    convite: Option<Convite>,
    emotes_abertos: bool,
    emote_q: String,
    /// As categorias de canais recolhidas.
    fechadas: HashSet<String>,
    /// O quadro ampliado (os outros numa tira à direita), o modo palco, a call em tela cheia e um quadro em tela cheia.
    foco: Option<Tile>,
    palco: bool,
    cheia: bool,
    quadro_cheio: Option<Tile>,
    /// As estatísticas em cada vídeo (tecla I).
    stats: bool,
    presi_aberta: bool,
    pub toast: ui::Toast,
    /// Pedidos para o `main.rs`: abrir o perfil, abrir o LoL de proximidade, abrir o perfil de outra pessoa.
    pub abrir_perfil: bool,
    pub abrir_proximidade: bool,
    pub abrir_outro: Option<i64>,
    /* a conversa com o servidor */
    geracao: u64,
    gateway: Option<rede::Gateway>,
    pub online: bool,
    /// A voz: a da call em que se está, e a da Call Presidencial.
    voz: Option<voz::Voz>,
    presi: Option<voz::Voz>,
    /// Os canais de texto cuja conversa já foi pedida.
    pedidas: HashSet<String>,
    tarefas: Vec<net::Tarefa<String>>,
    /// As texturas do vídeo que chega: (pessoa, tela?) → textura e o número do último quadro subido.
    videos: HashMap<(i64, bool), (egui::TextureHandle, u64, Vec2)>,
    /// Os aparelhos escolhidos nas Configurações (nome; vazio = o padrão do sistema).
    pub auto_entrou: bool,
    pub auto_presi: bool,
    pub mic_nome: String,
    pub saida_nome: String,
    /// Quando mandou o último "digitando".
    digitou: f64,
    /// Um canal de voz para entrar (o clique pede; o `show`, que tem o contexto, entra).
    pedir_entrar: Option<usize>,
    /// O link do convite criado e a resposta que ainda vai chegar.
    link_convite: Option<String>,
    link_rx: Option<std::sync::mpsc::Receiver<Result<String, net::Erro>>>,
}

impl Default for CallState {
    fn default() -> Self {
        Self {
            group: 0,
            view: View::Channel(usize::MAX),
            joined: None,
            muted: false,
            deafened: false,
            cam: false,
            sharing: false,
            draft: String::new(),
            card: None,
            grupo_menu: None,
            conta_menu: None,
            lol_menu: None,
            tela_menu: None,
            tela_modo: voz::Modo::default(),
            seletor: None,
            convite: None,
            emotes_abertos: false,
            emote_q: String::new(),
            fechadas: HashSet::new(),
            foco: None,
            palco: false,
            cheia: false,
            quadro_cheio: None,
            stats: false,
            presi_aberta: false,
            toast: ui::Toast::default(),
            abrir_perfil: false,
            abrir_proximidade: false,
            abrir_outro: None,
            geracao: u64::MAX,
            gateway: None,
            online: false,
            voz: None,
            presi: None,
            pedidas: HashSet::new(),
            tarefas: Vec::new(),
            videos: HashMap::new(),
            mic_nome: String::new(),
            saida_nome: String::new(),
            digitou: 0.0,
            pedir_entrar: None,
            link_convite: None,
            link_rx: None,
            auto_entrou: false,
            auto_presi: false,
        }
    }
}

/// O id público do canal `i` (vazio se não existe).
fn cid(i: usize) -> String {
    dados::canal_ou(i).id
}

impl CallState {
    /// Abre o cartão de uma pessoa ao lado da coluna de pessoas (a busca do topo usa).
    pub fn abrir_cartao(&mut self, id: usize) {
        self.card = Some((id, Pos2::new(1600.0 - 270.0 - 8.0 - 260.0, 120.0)));
    }

    /// Começa a mostrar a tela no modo dado: no Wayland o seletor do sistema pergunta qual; nos outros abre o do app.
    fn comecar_tela(&mut self, ctx: &egui::Context, modo: voz::Modo) {
        self.tela_modo = modo;
        if voz::seletor_do_sistema() {
            if let Some(v) = &self.voz {
                v.mandar(voz::Cmd::Tela(Some((modo, None))));
            }
        } else if self.voz.is_some() {
            self.seletor = Some(acoes::Seletor::abrir(ctx));
        }
    }

    /// O id público da sala aberta.
    pub fn sala_id(&self) -> String {
        dados::grupos().get(self.group).map(|g| g.id.clone()).unwrap_or_default()
    }

    /// Uma vez por quadro: o gateway (ligado enquanto há conta), o que ele avisou e as respostas que chegaram.
    pub fn passo(&mut self, ctx: &egui::Context, geracao: u64, t: f64) {
        if self.geracao != geracao {
            self.geracao = geracao;
            self.sair_da_voz();
            self.presi = None;
            self.gateway = None;
            self.pedidas.clear();
            self.online = false;
            dados::limpar();
            if crate::conta::perfil_id() > 0 {
                self.gateway = Some(rede::ligar(ctx));
            }
        }
        let mut avisos = Vec::new();
        if let Some(g) = &self.gateway {
            while let Ok(a) = g.rx.try_recv() {
                avisos.push(a);
            }
        }
        for a in avisos {
            match a {
                rede::Aviso::Pronto => {
                    self.online = true;
                    // a conversa aberta é buscada de novo (o que chegou enquanto estava fora)
                    self.pedidas.clear();
                    let g = dados::grupos();
                    if self.group >= g.len() || !g[self.group].membro {
                        self.group = g.iter().position(|g| g.membro).unwrap_or(0);
                    }
                    if dados::canal(match self.view { View::Channel(c) => c }).is_none_or(|c| c.grupo != self.group) {
                        self.abrir_primeiro_canal();
                    }
                    // quem está na Call Presidencial agora (o evento só vem nas mudanças)
                    let sala = self.sala_id();
                    if !sala.is_empty() && dados::tem_perm("diskenzy.presidential") {
                        self.tarefas.push(net::rodar(ctx, move || {
                            let v = net::pedir_ja("GET", &format!("/api/diskenzy/v2/rooms/{sala}/presidential"), None)?;
                            dados::mudar(|st| {
                                let l: Vec<usize> = v["members"].as_array().into_iter().flatten().filter_map(|m| st.pessoa_de(m)).collect();
                                st.presidencial.insert(sala.clone(), l);
                            });
                            Ok(String::new())
                        }));
                    }
                }
                rede::Aviso::Caiu => self.online = false,
                rede::Aviso::Movido { para, por, url, token } => {
                    if let Some(i) = dados::com(|d| d.canal_idx(&para)) {
                        self.sair_da_voz();
                        self.entrar_com(ctx, i, Some((url, token)));
                        self.toast.say(&format!("Você foi movido por {por}."), t);
                    }
                }
                rede::Aviso::Derrubado { canal, por } => {
                    if self.voz.as_ref().is_some_and(|v| v.canal == canal) {
                        self.sair_da_voz();
                        self.toast.say(&if por == "Sistema" || por.is_empty() { "Você perdeu o acesso a esta call.".to_owned() } else { format!("{por} tirou você da call.") }, t);
                    }
                }
            }
        }
        let mut i = 0;
        while i < self.tarefas.len() {
            match self.tarefas[i].pronto() {
                Some(r) => {
                    self.tarefas.remove(i);
                    match r {
                        Ok(msg) if !msg.is_empty() => self.toast.say(&msg, t),
                        Ok(_) => {}
                        Err(e) => self.toast.say(&e.msg, t),
                    }
                }
                None => i += 1,
            }
        }
        // a voz caiu ou não entrou: avisa e solta
        let falhou = self.voz.as_ref().and_then(|v| match v.conexao() {
            voz::Conexao::Falhou(e) => Some(e),
            _ => None,
        });
        if let Some(e) = falhou {
            self.sair_da_voz();
            self.toast.say(&e, t);
        }
        for v in [&self.voz, &self.presi].into_iter().flatten() {
            if let Some(a) = v.estado.lock().ok().and_then(|mut e| e.aviso.take()) {
                self.toast.say(&a, t);
            }
        }
        let presi_falhou = self.presi.as_ref().and_then(|v| match v.conexao() {
            voz::Conexao::Falhou(e) => Some(e),
            _ => None,
        });
        if let Some(e) = presi_falhou {
            self.presi = None;
            self.toast.say(&e, t);
        }
        // quem fala agora
        let eu_id = crate::conta::perfil_id();
        let mut falando: Vec<i64> = Vec::new();
        let mic_aberto = !self.muted && !self.deafened;
        for v in [&self.voz, &self.presi].into_iter().flatten() {
            falando.extend(v.falando(eu_id, mic_aberto));
        }
        let antes = dados::com(|d| d.falando.clone());
        if antes != falando {
            dados::mudar(|d| d.falando = falando);
        }
        if self.voz.is_some() || self.presi.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
        self.sharing = self.voz.as_ref().is_some_and(|v| v.estado.lock().is_ok_and(|e| e.tela));
        self.subir_videos(ctx);
    }

    fn abrir_primeiro_canal(&mut self) {
        let chs = dados::canais();
        let mut deste: Vec<(usize, &dados::Channel)> = chs.iter().enumerate().filter(|(_, c)| c.grupo == self.group).collect();
        deste.sort_by_key(|(_, c)| (matches!(c.kind, Kind::Voice { .. }), c.position));
        if let Some((i, _)) = deste.first() {
            self.view = View::Channel(*i);
        }
    }

    /// O "+" da trilha: entrar numa sala com um convite.
    pub fn pedir_convite(&mut self) {
        self.convite = Some(Convite::Entrar(String::new()));
    }

    /// Troca de sala (a trilha chama).
    pub fn abrir_grupo(&mut self, g: usize) {
        self.group = g;
        self.abrir_primeiro_canal();
    }

    /// Sobe para textura o vídeo que chegou desde o último quadro.
    fn subir_videos(&mut self, ctx: &egui::Context) {
        let Some(v) = &self.voz else {
            self.videos.clear();
            return;
        };
        let Ok(e) = v.estado.lock() else { return };
        self.videos.retain(|k, _| e.videos.contains_key(k));
        for (k, q) in &e.videos {
            if self.videos.get(k).is_some_and(|(_, n, _)| *n == q.n) {
                continue;
            }
            let img = egui::ColorImage::from_rgba_unmultiplied([q.w, q.h], &q.rgba);
            match self.videos.get_mut(k) {
                Some((tex, n, size)) => {
                    tex.set(img, egui::TextureOptions::LINEAR);
                    *n = q.n;
                    *size = Vec2::new(q.w as f32, q.h as f32);
                }
                None => {
                    let tex = ctx.load_texture(format!("video-{}-{}", k.0, k.1), img, egui::TextureOptions::LINEAR);
                    self.videos.insert(*k, (tex, q.n, Vec2::new(q.w as f32, q.h as f32)));
                }
            }
        }
    }

    pub fn join(&mut self, ch: usize) {
        self.view = View::Channel(ch);
        if self.joined == Some(ch) {
            return;
        }
        self.pedir_entrar = Some(ch);
    }

    fn entrar_com(&mut self, ctx: &egui::Context, ch: usize, entrada: Option<(String, String)>) {
        let id = cid(ch);
        if id.is_empty() {
            return;
        }
        self.sair_da_voz();
        let path = format!("/api/diskenzy/v2/voice/{id}/token");
        self.voz = Some(voz::entrar(ctx, id, path, entrada, crate::config::escolhidos().0, crate::config::escolhidos().1, !self.muted && !self.deafened));
        if let Some(v) = &self.voz {
            if self.deafened {
                v.mandar(voz::Cmd::Surdo(true));
            }
        }
        self.joined = Some(ch);
        self.view = View::Channel(ch);
    }

    fn sair_da_voz(&mut self) {
        self.voz = None;
        self.joined = None;
        self.cheia = false;
        self.foco = None;
        self.palco = false;
        self.quadro_cheio = None;
        self.videos.clear();
    }

    /// Microfone e fone: o estado na tela vai para a voz de verdade.
    fn aplicar_som(&self) {
        let aberto = !self.muted && !self.deafened;
        for v in [&self.voz, &self.presi].into_iter().flatten() {
            v.mandar(voz::Cmd::Mic(aberto));
            v.mandar(voz::Cmd::Surdo(self.deafened));
        }
    }

    /// Quem está no canal de voz `ch` (você entra assim que conecta, mesmo antes de o servidor avisar).
    fn members(&self, ch: usize) -> Vec<usize> {
        let eu = dados::eu();
        let mut v: Vec<usize> = dados::com(|d| d.pessoas.iter().enumerate().filter(|(i, p)| p.room == Some(ch) && *i != eu).map(|(i, _)| i).collect());
        if self.joined == Some(ch) || dados::pessoa(eu).room == Some(ch) {
            v.push(eu);
        }
        v
    }

    fn room_of(&self, id: usize) -> Option<usize> {
        if id == dados::eu() {
            self.joined
        } else {
            dados::pessoa(id).room
        }
    }

    fn muted_of(&self, id: usize) -> bool {
        if id == dados::eu() {
            self.muted || self.deafened
        } else {
            dados::pessoa(id).muted
        }
    }

    fn cam_of(&self, id: usize) -> bool {
        let p = dados::pessoa(id);
        if id == dados::eu() {
            false
        } else {
            p.cam || self.videos.contains_key(&(p.profile_id, false))
        }
    }

    fn screen_of(&self, id: usize) -> bool {
        let p = dados::pessoa(id);
        if id == dados::eu() {
            self.sharing
        } else {
            p.screen || self.videos.contains_key(&(p.profile_id, true))
        }
    }

    fn talking(&self, id: usize, _t: f64) -> bool {
        self.room_of(id).is_some() && !self.muted_of(id) && dados::falando(dados::pessoa(id).profile_id)
    }

    /// As mensagens do canal, juntas por pessoa (seguidas e com menos de 7 minutos entre uma e outra).
    fn grupos_de_mensagens(&self, canal: &str) -> Vec<Msg> {
        let lista = dados::com(|d| d.mensagens.get(canal).cloned().unwrap_or_default());
        let mut out: Vec<Msg> = Vec::new();
        let mut ultimo: Option<(usize, i64)> = None;
        for m in lista {
            let junta = ultimo.is_some_and(|(a, ts)| a == m.author && m.ts - ts < 7 * 60 * 1000);
            if junta {
                if let Some(g) = out.last_mut() {
                    g.lines.push(m.text.clone());
                }
            } else {
                let quando = chrono::DateTime::from_timestamp_millis(m.ts).map(|d| d.with_timezone(&chrono::Local)).unwrap_or_else(chrono::Local::now);
                let hoje = chrono::Local::now().date_naive() == quando.date_naive();
                let time = if hoje { quando.format("%H:%M").to_string() } else { quando.format("%d/%m/%Y %H:%M").to_string() };
                out.push(Msg { author: m.author, time, lines: vec![m.text.clone()] });
            }
            ultimo = Some((m.author, m.ts));
        }
        out
    }

    fn enviar(&mut self, ctx: &egui::Context, ch: usize, texto: String) {
        let canal = cid(ch);
        if canal.is_empty() {
            return;
        }
        let nonce = format!("app{:x}{:x}", chrono::Local::now().timestamp_millis(), std::process::id());
        // já aparece na tela; a volta do gateway troca pela de verdade (pelo nonce)
        dados::mudar(|d| {
            if let Some(eu) = d.eu {
                d.mensagens.entry(canal.clone()).or_default().push(dados::Mensagem { mid: String::new(), author: eu, ts: chrono::Local::now().timestamp_millis(), text: texto.clone(), nonce: Some(nonce.clone()) });
            }
        });
        let corpo = serde_json::json!({ "text": texto, "nonce": nonce });
        self.tarefas.push(net::rodar(ctx, move || {
            match net::pedir_ja("POST", &format!("/api/diskenzy/v2/channels/{canal}/messages"), Some(&corpo)) {
                Ok(_) => Ok(String::new()),
                Err(e) => {
                    // não foi: tira a que estava esperando
                    dados::mudar(|d| {
                        if let Some(l) = d.mensagens.get_mut(&canal) {
                            l.retain(|m| m.nonce.as_deref() != Some(nonce.as_str()) || !m.mid.is_empty());
                        }
                    });
                    Err(e)
                }
            }
        }));
    }

    /// A Call Presidencial: entrar ou sair. Quem está nela continua ouvindo a call principal, como no site.
    fn presi_alternar(&mut self, ctx: &egui::Context) {
        if self.presi.is_some() {
            self.presi = None;
            return;
        }
        let sala = self.sala_id();
        let path = format!("/api/diskenzy/v2/rooms/{sala}/presidential/token");
        self.presi = Some(voz::entrar(ctx, format!("presidencial:{sala}"), path, None, crate::config::escolhidos().0, crate::config::escolhidos().1, !self.muted && !self.deafened));
    }
}

pub fn show(ui: &mut egui::Ui, st: &mut CallState, body: Rect, t: f64, _geracao: u64) {
    if let Some(ch) = st.pedir_entrar.take() {
        st.entrar_com(ui.ctx(), ch, None);
    }
    // para testar com duas cópias: BRITTICO_ENTRAR=<nome do canal> entra sozinho nele assim que a lista chega
    if st.voz.is_none() {
        if let Ok(nome) = std::env::var("BRITTICO_ENTRAR") {
            if let Some(ch) = (0..60).find(|&i| { let c = dados::canal_ou(i); !c.id.is_empty() && c.name == nome }) {
                if !st.auto_entrou {
                    st.auto_entrou = true;
                    st.entrar_com(ui.ctx(), ch, None);
                }
            }
        }
    } else if st.presi.is_none() && std::env::var("BRITTICO_PRESI").is_ok() && !st.auto_presi {
        st.auto_presi = true;
        st.presi_alternar(ui.ctx());
    }
    if crate::conta::perfil_id() == 0 {
        text(ui.painter(), body.center(), Align2::CENTER_CENTER, "Conecte a conta para entrar na Call.", f6(14.0), FAINT(), 0.0);
        return;
    }
    if dados::grupos().is_empty() {
        let msg = if st.online { "Você ainda não está em nenhuma sala. Entre por um convite." } else { "Conectando…" };
        text(ui.painter(), body.center(), Align2::CENTER_CENTER, msg, f6(14.0), FAINT(), 0.0);
        if st.online {
            acoes::convite(ui, st, body, t);
            let b = Rect::from_center_size(body.center() + Vec2::new(0.0, 40.0), Vec2::new(180.0, 38.0));
            let r = ui.interact(b, ui.id().with("sem-sala-convite"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
            ui.painter().rect_filled(b, R_SM(), SECONDARY());
            text(ui.painter(), b.center(), Align2::CENTER_CENTER, "Entrar com convite", f7(13.0), ON_SECONDARY(), 0.0);
            if r.clicked() {
                st.convite = Some(Convite::Entrar(String::new()));
            }
            st.toast.show(ui, t);
        }
        return;
    }
    // a call em tela cheia: o palco ocupa o corpo inteiro
    if st.cheia {
        if let Some(ch) = st.joined {
            stage(ui, st, body, ch, t);
            acoes::menus(ui, st, t);
            acoes::seletor_tela(ui, st, body);
            acoes::atalhos(ui, st);
            st.toast.show(ui, t);
            return;
        }
        st.cheia = false;
    }
    let chlist = Rect::from_min_size(body.min, Vec2::new(CHLIST_W, body.height()));
    let members = Rect::from_min_max(Pos2::new(body.right() - MEMBERS_W, body.top()), body.max);
    let main = Rect::from_min_max(chlist.right_top(), members.left_bottom());
    channels_col(ui, st, chlist, t);
    match st.view {
        View::Channel(ch) if dados::canal(ch).is_none_or(|c| c.grupo != st.group) => {
            ui.painter().rect_filled(main, 0.0, CREAM_1());
        }
        View::Channel(ch) => match dados::canal_ou(ch).kind {
            Kind::Voice { .. } if st.joined == Some(ch) => stage(ui, st, main, ch, t),
            Kind::Voice { .. } => voice_door(ui, st, main, ch),
            _ => chat(ui, st, main, ch),
        },
    }
    people_col(ui, st, members, t);
    acoes::cartao(ui, st, body);
    acoes::menus(ui, st, t);
    acoes::convite(ui, st, body, t);
    acoes::seletor_tela(ui, st, body);
    acoes::atalhos(ui, st);
    st.toast.show(ui, t);
}

/* ================= a lista de canais (.dz-channels) ================= */

fn col_head(ui: &egui::Ui, rect: Rect) -> Rect {
    let head = Rect::from_min_size(rect.min, Vec2::new(rect.width(), HEAD_H));
    ui.painter().rect_filled(head, 0.0, CREAM_0());
    head
}

fn head_act(ui: &egui::Ui, center: Pos2, src: ImageSource<'static>, tip: &str) -> egui::Response {
    let rect = Rect::from_center_size(center, Vec2::splat(28.0));
    let r = ui.interact(rect, ui.id().with(("act", tip)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand).on_hover_text(tip);
    if r.hovered() {
        ui.painter().rect_filled(rect, R_XS(), AMBER_SOFT());
    }
    ui::icon(ui, src, center, 15.0, if r.hovered() { SECONDARY() } else { FAINT() });
    r
}

fn channels_col(ui: &mut egui::Ui, st: &mut CallState, col: Rect, t: f64) {
    let p = ui.painter().clone();
    p.rect_filled(col, 0.0, CREAM_0());
    let head = col_head(ui, col);
    let name = dados::grupos().get(st.group).map(|g| g.name.clone()).unwrap_or_default();
    let name = name.as_str();
    let title = crate::nome::desenhar(ui, Pos2::new(head.left() + 20.0, head.center().y), Align2::LEFT_CENTER, name, patua(15.0), INK(), -0.1, 0.0);
    let alvo = Rect::from_min_max(Pos2::new(head.left() + 12.0, head.top() + 14.0), Pos2::new(title.right() + 26.0, head.bottom() - 14.0));
    let gr = ui.interact(alvo, ui.id().with("grupo-nome"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand).on_hover_text("Opções da sala");
    if gr.hovered() || st.grupo_menu.is_some() {
        p.rect_filled(alvo, R_SM(), CREAM_2());
        crate::nome::desenhar(ui, Pos2::new(head.left() + 20.0, head.center().y), Align2::LEFT_CENTER, name, patua(15.0), INK(), -0.1, 0.0);
    }
    ui::icon(ui, ic!("chevron-down"), Pos2::new(title.right() + 15.0, head.center().y), 15.0, FAINT());
    let gear = head_act(ui, Pos2::new(head.right() - 34.0, head.center().y), ic!("gear"), "Opções da sala");
    if gr.clicked() || gear.clicked() {
        st.grupo_menu = if st.grupo_menu.is_some() { None } else { Some(Pos2::new(head.left() + 12.0, head.bottom() - 6.0)) };
    }

    let foot = MEBAR_H + if st.joined.is_some() { DOCK_H } else { 0.0 };
    let list = Rect::from_min_max(Pos2::new(col.left(), head.bottom()), Pos2::new(col.right(), col.bottom() - foot));
    let mut child = ui.new_child(UiBuilder::new().max_rect(list));
    let mut clicked = None;
    let mut joined = None;
    egui::ScrollArea::vertical().id_salt("canais").auto_shrink(false).show(&mut child, |ui| {
        ui.add_space(12.0);
        let todos = dados::canais();
        for cat_name in dados::categorias(st.group) {
            let cat = cat_name.clone();
            // a categoria recolhe e abre no clique; recolhida, só o canal aberto e o da call continuam à vista
            let (r, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 28.0), Sense::click());
            let resp = resp.on_hover_cursor(CursorIcon::PointingHand);
            let p = ui.painter();
            let fechada = st.fechadas.contains(&cat);
            let c = if resp.hovered() { SOFT() } else { FAINT() };
            ui::icon(ui, if fechada { ic!("chevron-right") } else { ic!("chevron-down") }, Pos2::new(r.left() + 12.0 + 8.0 + 5.0, r.top() + 18.0), 10.0, c);
            let _ = p;
            crate::nome::desenhar(ui, Pos2::new(r.left() + 12.0 + 24.0, r.top() + 19.0), Align2::LEFT_CENTER, &cat_name.to_uppercase(), f8(10.0), c, 0.8, 0.0);
            if resp.clicked() {
                if fechada {
                    st.fechadas.remove(&cat);
                } else {
                    st.fechadas.insert(cat.clone());
                }
            }
            let mut deste: Vec<(usize, &dados::Channel)> = todos.iter().enumerate().filter(|(_, c)| c.grupo == st.group && c.category == cat).filter(|(i, _)| !fechada || st.view == View::Channel(*i) || st.joined == Some(*i)).collect();
            deste.sort_by_key(|(_, c)| c.position);
            for (i, ch) in deste {
                let active = st.view == View::Channel(i);
                let icon = match ch.kind {
                    Kind::Text => ic!("hash"),
                    Kind::News => ic!("megaphone"),
                    Kind::Voice { .. } => ic!("speaker"),
                };
                let members = st.members(i);
                let live = st.joined == Some(i);
                let resp = ch_row(ui, icon, &ch.name, active, live, |ui, r| {
                    if let Kind::Voice { locked, cap } = ch.kind {
                        let mut x = r.right() - 8.0;
                        let label = match cap {
                            Some(c) => Some(format!("{}/{}", members.len(), c)),
                            None if !members.is_empty() => Some(members.len().to_string()),
                            None => None,
                        };
                        if let Some(label) = label {
                            let w = text_width(ui.painter(), &label, f8(10.5), 0.0) + 14.0;
                            let pill = Rect::from_min_size(Pos2::new(x - w, r.center().y - 10.0), Vec2::new(w.max(20.0), 20.0));
                            let (bg, fg) = if live { (SECONDARY(), ON_SECONDARY()) } else { (CREAM_2(), FAINT()) };
                            ui.painter().rect_filled(pill, 10.0, bg);
                            text(ui.painter(), pill.center(), Align2::CENTER_CENTER, &label, f8(10.5), fg, 0.0);
                            x = pill.left() - 10.0;
                        }
                        if locked {
                            ui::icon(ui, ic!("lock"), Pos2::new(x - 6.0, r.center().y), 12.0, FAINT().gamma_multiply(0.8));
                        }
                    }
                });
                if resp.clicked() {
                    clicked = Some(i);
                }
                if resp.double_clicked() && matches!(ch.kind, Kind::Voice { .. }) {
                    joined = Some(i);
                }
                if !members.is_empty() {
                    ui.add_space(2.0);
                    for m in members {
                        if let Some(q) = vp_row(ui, st, m, t) {
                            st.card = Some((m, q));
                        }
                    }
                    ui.add_space(4.0);
                }
            }
            ui.add_space(6.0);
        }
        ui.add_space(16.0);
    });
    if let Some(i) = clicked {
        st.view = View::Channel(i);
    }
    if let Some(i) = joined {
        st.join(i);
    }

    // o pé da coluna: "Voz conectada" (só dentro de uma sala) e a barra "eu"
    let mut y = col.bottom() - foot;
    if let Some(room) = st.joined {
        let dock = Rect::from_min_size(Pos2::new(col.left(), y), Vec2::new(col.width(), DOCK_H));
        p.rect_filled(dock, 0.0, AMBER_SOFT());
        let dot = Pos2::new(dock.left() + 16.0, dock.center().y);
        p.circle_filled(dot, 5.5 + 1.5 * (t * 3.0).sin().abs() as f32, DIS_OK.gamma_multiply(0.22));
        p.circle_filled(dot, 4.0, DIS_OK);
        text(&p, Pos2::new(dock.left() + 36.0, dock.center().y - 8.0), Align2::LEFT_CENTER, "Voz conectada", f8(12.5), INK(), 0.0);
        let estado = match st.voz.as_ref().map(|v| v.conexao()) {
            Some(voz::Conexao::Entrando) => format!("{} · conectando…", dados::canal_ou(room).name),
            _ => dados::canal_ou(room).name,
        };
        crate::nome::desenhar(ui, Pos2::new(dock.left() + 36.0, dock.center().y + 8.0), Align2::LEFT_CENTER, &estado, f6(11.5), SOFT(), 0.0, dock.width() - 36.0 - 50.0);
        let hang = Rect::from_center_size(Pos2::new(dock.right() - 25.0, dock.center().y), Vec2::splat(34.0));
        let r = ui.interact(hang, ui.id().with("dock-sair"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand).on_hover_text("Sair da call");
        if r.hovered() {
            p.rect_filled(hang, R_SM(), CREAM_2());
        }
        ui::icon(ui, ic!("hangup"), hang.center(), 18.0, if r.hovered() { DIS_OFF } else { FAINT() });
        if r.clicked() {
            st.sair_da_voz();
        }
        y += DOCK_H;
    }
    let bar = Rect::from_min_size(Pos2::new(col.left(), y), Vec2::new(col.width(), MEBAR_H));
    p.rect_filled(bar, 0.0, CREAM_0());
    let eu = dados::eu();
    let me = dados::pessoa(eu);
    let c = Pos2::new(bar.left() + 20.0 + 16.0, bar.center().y);
    ui::avatar(ui, &me.name, me.photo(), c, 32.0, st.talking(eu, t), Some((DIS_OK, CREAM_0())));
    let nick = text(&p, Pos2::new(bar.left() + 64.0, bar.center().y - 8.0), Align2::LEFT_CENTER, &me.name, f7(12.5), INK(), 0.0);
    let lapis = Rect::from_min_max(nick.min - Vec2::new(2.0, 2.0), Pos2::new(nick.right() + 20.0, nick.bottom() + 2.0));
    let lr = ui.interact(lapis, ui.id().with("me-perfil"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand).on_hover_text("Abrir meu perfil para mudar nome e foto");
    ui::icon(ui, ic!("pencil"), Pos2::new(nick.right() + 12.0, nick.center().y), 12.0, if lr.hovered() { SECONDARY() } else { FAINT() });
    if lr.clicked() {
        st.abrir_perfil = true;
    }
    p.circle_filled(Pos2::new(bar.left() + 67.5, bar.center().y + 8.5), 3.5, DIS_OK);
    let estado = if !st.online { "conectando…" } else if st.joined.is_some() { "na call" } else { "online" };
    text(&p, Pos2::new(bar.left() + 77.0, bar.center().y + 8.5), Align2::LEFT_CENTER, estado, f7(10.0), FAINT(), 0.0);
    let gear = Rect::from_center_size(Pos2::new(bar.right() - 37.0, bar.center().y), Vec2::splat(34.0));
    let r = ui.interact(gear, ui.id().with("me-gear"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand).on_hover_text("Menu da conta");
    let on = st.conta_menu.is_some();
    if r.hovered() || on {
        p.rect_filled(gear, R_SM(), if on { AMBER_SOFT() } else { CREAM_2() });
    }
    ui::icon(ui, ic!("gear"), gear.center(), 18.0, if on { SECONDARY() } else if r.hovered() { INK() } else { FAINT() });
    if r.clicked() {
        st.conta_menu = if on { None } else { Some(Pos2::new(bar.left() + 8.0, bar.top() - 8.0 - 88.0)) };
    }
}

/// Um `.dz-ch`: ícone, nome e o que vier à direita (`extra`). Devolve o clique.
fn ch_row(ui: &mut egui::Ui, icon: ImageSource<'static>, name: &str, active: bool, live: bool, extra: impl FnOnce(&egui::Ui, Rect)) -> egui::Response {
    let (full, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 34.0), Sense::click());
    let r = full.shrink2(Vec2::new(12.0, 0.0));
    let resp = resp.on_hover_cursor(CursorIcon::PointingHand);
    let p = ui.painter();
    if active {
        p.rect_filled(r, R_SM(), AMBER_SOFT());
        p.rect_filled(Rect::from_min_size(Pos2::new(r.left() - 8.0, r.center().y - 10.0), Vec2::new(3.0, 20.0)), CornerRadius { nw: 0, sw: 0, ne: 3, se: 3 }, SECONDARY());
    } else if resp.hovered() {
        p.rect_filled(r, R_SM(), CREAM_2());
    }
    let ic_color = if active || live { SECONDARY() } else { FAINT() };
    let name_color = if active { SECONDARY() } else if resp.hovered() { INK() } else { SOFT() };
    ui::icon(ui, icon, Pos2::new(r.left() + 16.0, r.center().y), 15.0, ic_color);
    let _ = p;
    crate::nome::desenhar(ui, Pos2::new(r.left() + 33.0, r.center().y), Align2::LEFT_CENTER, name, f7(13.0), name_color, -0.1, r.width() - 33.0 - 52.0);
    extra(ui, r);
    resp
}

/// Um `.dz-vp`: quem está no canal de voz, embaixo dele.
fn vp_row(ui: &mut egui::Ui, st: &CallState, id: usize, t: f64) -> Option<Pos2> {
    let (full, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 29.0), Sense::click());
    let resp = resp.on_hover_cursor(CursorIcon::PointingHand);
    let r = Rect::from_min_max(Pos2::new(full.left() + 28.0, full.top()), Pos2::new(full.right() - 12.0, full.bottom() - 1.0));
    let p = ui.painter();
    if resp.hovered() {
        p.rect_filled(r, R_XS(), CREAM_2());
    }
    let person = dados::pessoa(id);
    let talking = st.talking(id, t);
    ui::avatar(ui, &person.name, person.photo(), Pos2::new(r.left() + 19.0, r.center().y), 22.0, talking, None);
    let c = if talking { SECONDARY() } else if resp.hovered() { INK() } else { FAINT() };
    text(p, Pos2::new(r.left() + 37.0, r.center().y), Align2::LEFT_CENTER, &person.name, f6(12.0), c, 0.0);
    // o estado à direita: tela (verde), câmera (âmbar), microfone fechado
    let mut x = r.right() - 8.0 - 6.5;
    let mut st_icon = |src: ImageSource<'static>, color: Color32| {
        ui::icon(ui, src, Pos2::new(x, r.center().y), 13.0, color);
        x -= 18.0;
    };
    if st.screen_of(id) {
        st_icon(ic!("screen"), DIS_OK);
    }
    if st.cam_of(id) {
        st_icon(ic!("video"), SECONDARY());
    }
    if st.muted_of(id) {
        st_icon(ic!("mic-off"), FAINT());
    }
    resp.clicked().then(|| Pos2::new(full.right() + 8.0, full.top()))
}

/* ================= o conteúdo ================= */

fn main_head(ui: &egui::Ui, main: Rect, icon: ImageSource<'static>, name: &str) -> (Rect, Rect) {
    let head = col_head(ui, main);
    ui::icon(ui, icon, Pos2::new(head.left() + 29.0, head.center().y), 18.0, SECONDARY());
    let title = crate::nome::desenhar(ui, Pos2::new(head.left() + 48.0, head.center().y), Align2::LEFT_CENTER, name, patua(15.0), INK(), -0.1, 0.0);
    (head, title)
}

fn chat(ui: &mut egui::Ui, st: &mut CallState, main: Rect, ch: usize) {
    let channel = dados::canal_ou(ch);
    // a conversa é buscada na primeira vez que o canal abre
    if !channel.id.is_empty() && st.online && st.pedidas.insert(channel.id.clone()) {
        let id = channel.id.clone();
        st.tarefas.push(net::rodar(ui.ctx(), move || rede::buscar_mensagens(&id, None).map(|_| String::new())));
    }
    let p = ui.painter().clone();
    p.rect_filled(main, 0.0, CREAM_1());
    let icon = if channel.kind == Kind::News { ic!("megaphone") } else { ic!("hash") };
    let (head, title) = main_head(ui, main, icon, &channel.name);
    text(&p, Pos2::new(title.right() + 4.0 + 12.0 + 12.0, head.center().y), Align2::LEFT_CENTER, &channel.topic, f6(12.5), SOFT(), 0.0);

    let composer = Rect::from_min_max(Pos2::new(main.left(), main.bottom() - COMPOSER_H), main.max);
    let list = Rect::from_min_max(Pos2::new(main.left(), head.bottom()), Pos2::new(main.right(), composer.top() - 20.0));
    let mut child = ui.new_child(UiBuilder::new().max_rect(list));
    egui::ScrollArea::vertical().id_salt(("chat", ch)).auto_shrink(false).stick_to_bottom(true).show(&mut child, |ui| {
        ui.add_space(12.0);
        let grupos = st.grupos_de_mensagens(&channel.id);
        // "mais antigas": a página de antes, quando o servidor diz que há
        if dados::com(|d| d.mais_antigas.get(&channel.id).copied().unwrap_or(false)) {
            let (r, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 30.0), Sense::click());
            let resp = resp.on_hover_cursor(CursorIcon::PointingHand);
            text(ui.painter(), r.center(), Align2::CENTER_CENTER, "Carregar mensagens mais antigas", f7(12.0), if resp.hovered() { SECONDARY() } else { FAINT() }, 0.0);
            if resp.clicked() {
                let id = channel.id.clone();
                let antes = dados::com(|d| d.mensagens.get(&id).and_then(|l| l.iter().find(|m| !m.mid.is_empty()).map(|m| m.mid.clone())));
                st.tarefas.push(net::rodar(ui.ctx(), move || rede::buscar_mensagens(&id, antes.as_deref()).map(|_| String::new())));
            }
        } else {
            chat_start(ui, &channel.name);
        }
        let mut abrir = None;
        for m in &grupos {
            if let Some(q) = msg_group(ui, m) {
                abrir = Some((m.author, q));
            }
        }
        if abrir.is_some() {
            st.card = abrir;
        }
        // quem está escrevendo
        let t = ui.input(|i| i.time);
        let escrevendo: Vec<String> = dados::com(|d| d.digitando.get(&channel.id).map(|l| l.iter().filter(|(p, ate)| *ate > t && Some(*p) != d.eu).map(|(p, _)| d.pessoas[*p].name.clone()).collect()).unwrap_or_default());
        if !escrevendo.is_empty() {
            let (r, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 20.0), Sense::hover());
            let s = if escrevendo.len() == 1 { format!("{} está escrevendo…", escrevendo[0]) } else { format!("{} pessoas estão escrevendo…", escrevendo.len()) };
            text(ui.painter(), Pos2::new(r.left() + 24.0, r.center().y), Align2::LEFT_CENTER, &s, f6(11.5), FAINT(), 0.0);
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(500));
        }
        ui.add_space(16.0);
    });

    // o compositor
    p.rect_filled(composer, 0.0, CREAM_0());
    let bx = Rect::from_min_size(composer.min + Vec2::new(20.0, 16.0), Vec2::new(composer.width() - 40.0, 48.0));
    let editing = ui.memory(|m| m.has_focus(ui.id().with("composer")));
    p.rect_filled(bx, R_SM(), CREAM_2());
    if editing {
        p.rect_stroke(bx, R_SM(), Stroke::new(1.0_f32, SECONDARY()), StrokeKind::Inside);
    }
    let smile = Rect::from_center_size(Pos2::new(bx.left() + 22.0, bx.center().y), Vec2::splat(30.0));
    let sr = ui.interact(smile, ui.id().with("emote-btn"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand).on_hover_text("Escolher um emote");
    ui::icon(ui, ic!("smile"), smile.center(), 18.0, if sr.hovered() || st.emotes_abertos { SECONDARY() } else { FAINT() });
    if sr.clicked() {
        st.emotes_abertos = !st.emotes_abertos;
    }
    let send = Rect::from_center_size(Pos2::new(bx.right() - 22.0, bx.center().y), Vec2::splat(30.0));
    let snd = ui.interact(send, ui.id().with("send-btn"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand).on_hover_text("Enviar mensagem");
    if snd.hovered() {
        p.rect_filled(send, R_XS(), AMBER_SOFT());
    }
    ui::icon(ui, ic!("send"), send.center(), 18.0, SECONDARY());
    let field = Rect::from_min_max(Pos2::new(bx.left() + 41.0, bx.top() + 4.0), Pos2::new(bx.right() - 44.0, bx.bottom() - 4.0));
    let hint = egui::RichText::new(if channel.can_post { format!("Conversar em #{}", crate::nome::simples(&channel.name)) } else { "Este canal é somente leitura.".to_owned() }).color(FAINT()).font(f6(13.5));
    let out = ui.put(field, egui::TextEdit::singleline(&mut st.draft).id(ui.id().with("composer")).frame(false).hint_text(hint).font(f6(13.5)).text_color(INK()).vertical_align(egui::Align::Center).margin(egui::Margin::ZERO));
    acoes::seletor_emotes(ui, st, composer);
    // "digitando…" para os outros, no máximo a cada 3 s
    if out.changed() && !st.draft.is_empty() {
        let t = ui.input(|i| i.time);
        if t - st.digitou > 3.0 {
            st.digitou = t;
            if let Some(g) = &st.gateway {
                g.mandar("typing", serde_json::json!({ "channel": channel.id }));
            }
        }
    }
    let enter = out.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
    if (enter || snd.clicked()) && !st.draft.trim().is_empty() {
        if channel.can_post {
            let line = std::mem::take(&mut st.draft).trim().to_owned();
            st.enviar(ui.ctx(), ch, line);
        } else {
            st.toast.say("Este canal é somente leitura.", ui.input(|i| i.time));
        }
        out.request_focus();
    }
}

/// O começo de um canal sem conversa: o `.dz-start`.
fn chat_start(ui: &mut egui::Ui, name: &str) {
    let (r, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 90.0), Sense::hover());
    let p = ui.painter();
    crate::nome::desenhar(ui, Pos2::new(r.left() + 24.0, r.top() + 40.0), Align2::LEFT_CENTER, &format!("Bem-vindo a #{name}"), patua(20.0), INK(), 0.0, 0.0);
    text(p, Pos2::new(r.left() + 24.0, r.top() + 66.0), Align2::LEFT_CENTER, "Este é o começo do canal.", f6(13.0), SOFT(), 0.0);
}

/// Um `.dz-grp`: avatar, nome na cor do cargo, as etiquetas, a hora e as linhas.
fn msg_group(ui: &mut egui::Ui, m: &Msg) -> Option<Pos2> {
    let author = dados::pessoa(m.author);
    let line_h = 24.0;
    // a linha com emote cresce para caber a imagem de 28 px
    let alturas: Vec<f32> = m.lines.iter().map(|l| if l.split(' ').any(|w| acoes::emote(w).is_some()) { 32.0 } else { line_h }).collect();
    let h = 6.0 + 22.0 + alturas.iter().sum::<f32>() + 2.0;
    ui.add_space(6.0);
    let (r, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), h), Sense::hover());
    let p = ui.painter();
    let left = r.left() + 24.0;
    ui::avatar(ui, &author.name, author.photo(), Pos2::new(left + 18.0, r.top() + 6.0 + 2.0 + 18.0), 36.0, false, None);
    let body_x = left + 36.0 + 12.0;
    let head_y = r.top() + 6.0 + 11.0;
    let name_c = author.roles.first().map(|ro| mix(ro.color(), INK(), 0.6)).unwrap_or(INK());
    let name = text(p, Pos2::new(body_x, head_y), Align2::LEFT_CENTER, &author.name, f8(13.5), name_c, 0.0);
    let nr = ui.interact(name.expand(2.0), ui.id().with(("autor", r.top() as i32)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
    if nr.hovered() {
        p.line_segment([name.left_bottom(), name.right_bottom()], Stroke::new(1.0_f32, name_c));
    }
    let mut x = name.right() + 8.0;
    for role in &author.roles {
        ui::role_chip(ui, role, Pos2::new(x + 11.0, head_y), 22.0);
        x += 22.0 + 6.0;
    }
    text(p, Pos2::new(x + 2.0, head_y + 1.0), Align2::LEFT_CENTER, &m.time, f6(10.5), FAINT(), 0.0);
    for (k, line) in m.lines.iter().enumerate() {
        let y = r.top() + 6.0 + 22.0 + alturas[..k].iter().sum::<f32>() + alturas[k] / 2.0;
        // os emotes viram imagem e o link fica âmbar (e abre no navegador), como no `ui-chat.js`
        acoes::linha(ui, body_x, y, line);
    }
    nr.clicked().then(|| Pos2::new(name.right() + 8.0, name.top()))
}

/// A porta do canal de voz: quem está lá e o botão "Entrar na call".
fn voice_door(ui: &mut egui::Ui, st: &mut CallState, main: Rect, ch: usize) {
    let p = ui.painter().clone();
    p.rect_filled(main, 0.0, CREAM_1());
    let channel = dados::canal_ou(ch);
    let members = st.members(ch);
    voice_head(ui, main, ch, members.len());
    let body = Rect::from_min_max(Pos2::new(main.left(), main.top() + HEAD_H), main.max);
    let c = Pos2::new(body.center().x, body.center().y);
    let sq = Rect::from_center_size(c - Vec2::new(0.0, 94.0), Vec2::splat(72.0));
    p.rect_filled(sq, R_LG(), SECONDARY());
    ui::icon(ui, ic!("speaker"), sq.center(), 34.0, Color32::WHITE);
    crate::nome::desenhar(ui, c - Vec2::new(0.0, 28.0), Align2::CENTER_CENTER, &channel.name, patua(22.0), INK(), 0.0, 0.0);
    let sub = match channel.kind {
        Kind::Voice { cap: Some(cap), .. } => format!("{} PESSOAS NA CALL · LIMITE DE {}", members.len(), cap),
        _ if members.is_empty() => "NINGUÉM NA CALL AINDA".to_string(),
        _ => format!("{} NA CALL", members.len()),
    };
    text(&p, c + Vec2::new(0.0, 7.0), Align2::CENTER_CENTER, &sub, f8(11.0), FAINT(), 1.0);
    // quem está lá: avatar e nome, lado a lado
    let widths: Vec<f32> = members.iter().map(|&m| 24.0 + 8.0 + text_width(&p, &dados::pessoa(m).name, f6(13.0), 0.0)).collect();
    let total: f32 = widths.iter().sum::<f32>() + 16.0 * (widths.len().max(1) as f32 - 1.0);
    let mut x = c.x - total / 2.0;
    for (k, &m) in members.iter().enumerate() {
        let y = c.y + 48.0;
        let pm = dados::pessoa(m);
        ui::avatar(ui, &pm.name, pm.photo(), Pos2::new(x + 12.0, y), 24.0, false, None);
        text(&p, Pos2::new(x + 32.0, y), Align2::LEFT_CENTER, &pm.name, f6(13.0), SOFT(), 0.0);
        x += widths[k] + 16.0;
    }
    let label = "Entrar na call";
    let w = 16.0 + 8.0 + text_width(&p, label, f8(14.0), 0.0) + 56.0;
    let btn = Rect::from_center_size(c + Vec2::new(0.0, 106.0), Vec2::new(w, 46.0));
    let r = ui.interact(btn, ui.id().with(("entrar", ch)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
    p.rect_filled(btn, R_SM(), if r.hovered() { mix(SECONDARY(), Color32::WHITE, 0.9) } else { SECONDARY() });
    let tw = text_width(&p, label, f8(14.0), 0.0);
    let x0 = btn.center().x - (16.0 + 8.0 + tw) / 2.0;
    ui::icon(ui, ic!("speaker"), Pos2::new(x0 + 8.0, btn.center().y), 16.0, ON_SECONDARY());
    text(&p, Pos2::new(x0 + 24.0, btn.center().y), Align2::LEFT_CENTER, label, f8(14.0), ON_SECONDARY(), 0.0);
    if r.clicked() {
        if channel.can_join {
            st.join(ch);
        } else {
            st.toast.say("Você não tem acesso a este canal de voz.", ui.input(|i| i.time));
        }
    }
}

fn voice_head(ui: &egui::Ui, main: Rect, ch: usize, n: usize) {
    let (head, title) = main_head(ui, main, ic!("speaker"), &dados::canal_ou(ch).name);
    let sub = text(ui.painter(), Pos2::new(title.right() + 10.0, head.center().y), Align2::LEFT_CENTER, &format!("{n} pessoas"), f6(11.5), FAINT(), 0.0);
    if matches!(dados::canal_ou(ch).kind, Kind::Voice { locked: true, .. }) {
        ui::icon(ui, ic!("lock"), Pos2::new(sub.right() + 17.0, head.center().y), 14.0, FAINT());
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tile {
    Person(usize),
    Screen(usize),
}

/// O palco de quem está dentro do canal de voz: a grade de quadros e a barra de controles.
fn stage(ui: &mut egui::Ui, st: &mut CallState, main: Rect, ch: usize, t: f64) {
    let p = ui.painter().clone();
    p.rect_filled(main, 0.0, CREAM_1());
    let members = st.members(ch);
    voice_head(ui, main, ch, members.len());
    // quem tem câmera ou tela vem primeiro, como no site
    let mut tiles: Vec<Tile> = Vec::new();
    for &m in &members {
        if st.cam_of(m) {
            tiles.push(Tile::Person(m));
        }
        if st.screen_of(m) {
            tiles.push(Tile::Screen(m));
        }
    }
    tiles.extend(members.iter().filter(|&&m| !st.cam_of(m)).map(|&m| Tile::Person(m)));

    let grid = Rect::from_min_max(Pos2::new(main.left(), main.top() + HEAD_H), Pos2::new(main.right(), main.bottom() - CONTROLS_H)).shrink(20.0);
    // um quadro em tela cheia cobre o palco inteiro (Esc ou o botão do canto voltam)
    let cheio = st.quadro_cheio.filter(|q| tiles.contains(q));
    // o ampliado: o escolhido no clique; no modo palco, a primeira tela compartilhada (ou o primeiro quadro)
    let foco = st.foco.filter(|f| tiles.contains(f)).or_else(|| st.palco.then(|| tiles.iter().copied().find(|t| matches!(t, Tile::Screen(_))).or(tiles.first().copied())).flatten());
    let mut acao: Option<(Tile, bool, bool)> = None;
    if let Some(q) = cheio {
        let r = Rect::from_min_max(Pos2::new(main.left(), main.top() + HEAD_H), Pos2::new(main.right(), main.bottom() - CONTROLS_H));
        if let Some(a) = tile_at(ui, st, r, q, t, true) {
            acao = Some(a);
        }
    } else if let Some(f) = foco {
        // o grande à esquerda e a tira à direita, como o `.stage--focus` do site
        let strip_w = (grid.width() * 0.2).clamp(150.0, 240.0);
        let big = Rect::from_min_max(grid.min, Pos2::new(grid.right() - strip_w - 10.0, grid.bottom()));
        if let Some(a) = tile_at(ui, st, big, f, t, false) {
            acao = Some(a);
        }
        let resto: Vec<Tile> = tiles.iter().copied().filter(|x| *x != f).collect();
        let th = ((grid.height() - 10.0 * (resto.len().max(1) as f32 - 1.0)) / resto.len().max(1) as f32).min(strip_w * 9.0 / 16.0 + 30.0);
        for (k, tile) in resto.into_iter().enumerate() {
            let r = Rect::from_min_size(Pos2::new(grid.right() - strip_w, grid.top() + k as f32 * (th + 10.0)), Vec2::new(strip_w, th));
            if let Some(a) = tile_at(ui, st, r, tile, t, false) {
                acao = Some(a);
            }
        }
    } else {
        let n = tiles.len().max(1);
        let cols = (n as f32).sqrt().ceil() as usize;
        let rows = n.div_ceil(cols);
        let gap = 10.0;
        let w = (grid.width() - gap * (cols as f32 - 1.0)) / cols as f32;
        let h = (grid.height() - gap * (rows as f32 - 1.0)) / rows as f32;
        for (k, tile) in tiles.into_iter().enumerate() {
            let (row, col) = (k / cols, k % cols);
            let r = Rect::from_min_size(Pos2::new(grid.left() + col as f32 * (w + gap), grid.top() + row as f32 * (h + gap)), Vec2::new(w, h));
            if let Some(a) = tile_at(ui, st, r, tile, t, false) {
                acao = Some(a);
            }
        }
    }
    // um clique amplia (ou desfaz), dois cliques ou o botão do canto põem o quadro em tela cheia
    match acao {
        Some((tile, _, true)) => st.quadro_cheio = if st.quadro_cheio == Some(tile) { None } else { Some(tile) },
        Some((tile, true, false)) => {
            if st.quadro_cheio.is_none() {
                st.foco = if foco == Some(tile) && !st.palco { None } else { Some(tile) };
                if foco == Some(tile) && st.palco {
                    st.palco = false;
                    st.foco = None;
                }
            }
        }
        _ => {}
    }
    controls(ui, st, Rect::from_min_max(Pos2::new(main.left(), main.bottom() - CONTROLS_H), main.max));
}

/// Desenha um quadro e devolve o que fizeram nele: (qual, um clique, tela cheia por dois cliques ou pelo botão do canto).
fn tile_at(ui: &egui::Ui, st: &CallState, r: Rect, tile: Tile, t: f64, cheio: bool) -> Option<(Tile, bool, bool)> {
    let key = match tile {
        Tile::Person(i) => (i, 0),
        Tile::Screen(i) => (i, 1),
    };
    let resp = ui.interact(r, ui.id().with(("quadro", key)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand).on_hover_text("Clique para ampliar. Clique duas vezes para tela cheia.");
    draw_tile(ui, st, r, tile, t);
    // o botão de tela cheia no canto de cima, que aparece com o mouse em cima
    let b = Rect::from_min_size(r.min + Vec2::new(10.0, 10.0), Vec2::splat(30.0));
    let br = ui.interact(b, ui.id().with(("quadro-cheio", key)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand).on_hover_text(if cheio { "Sair da tela cheia" } else { "Tela cheia" });
    if resp.hovered() || br.hovered() || cheio {
        let p = ui.painter();
        p.rect_filled(b, R_XS(), Color32::from_black_alpha(if br.hovered() { 200 } else { 140 }));
        ui::icon(ui, if cheio { ic!("compress") } else { ic!("expand") }, b.center(), 15.0, Color32::WHITE);
    }
    let video = match tile {
        Tile::Person(i) => st.videos.get(&(dados::pessoa(i).profile_id, false)),
        Tile::Screen(i) => st.videos.get(&(dados::pessoa(i).profile_id, true)),
    };
    if st.stats && video.is_some() {
        // o que se sabe do vídeo que chega: o tamanho de verdade
        let p = ui.painter();
        let size = video.map(|v| v.2).unwrap_or_default();
        let linhas = [format!("{}×{}", size.x as u32, size.y as u32), "recebendo".to_owned(), String::new()];
        let caixa = Rect::from_min_size(Pos2::new(r.right() - 10.0 - 150.0, r.top() + 10.0), Vec2::new(150.0, 58.0));
        p.rect_filled(caixa, R_XS(), Color32::from_black_alpha(170));
        for (k, l) in linhas.iter().enumerate() {
            text(p, Pos2::new(caixa.left() + 8.0, caixa.top() + 11.0 + k as f32 * 17.0), Align2::LEFT_CENTER, l, f6(11.0), Color32::WHITE, 0.0);
        }
    }
    if br.clicked() || resp.double_clicked() {
        Some((tile, false, true))
    } else if resp.clicked() {
        Some((tile, true, false))
    } else {
        None
    }
}

fn draw_tile(ui: &egui::Ui, st: &CallState, r: Rect, tile: Tile, t: f64) {
    let p = ui.painter().with_clip_rect(r);
    let (id, screen) = match tile {
        Tile::Person(id) => (id, false),
        Tile::Screen(id) => (id, true),
    };
    let person = dados::pessoa(id);
    let talking = !screen && st.talking(id, t);
    let video = st.videos.get(&(person.profile_id, screen)).map(|(tex, _, size)| (tex.id(), *size));
    p.rect_filled(r, R_MD(), CREAM_2());
    // a imagem fica em 16:9 no meio, com faixa preta em cima e embaixo (o `object-fit:contain`)
    let media = {
        let mh = (r.width() * 9.0 / 16.0).min(r.height());
        let mw = mh * 16.0 / 9.0;
        Rect::from_center_size(r.center(), Vec2::new(mw, mh))
    };
    if let Some((tex, size)) = video {
        // o vídeo de verdade, inteiro dentro do quadro (`object-fit:contain`)
        p.rect_filled(r.shrink(1.0), R_MD(), Color32::from_rgb(11, 11, 12));
        let k = (r.width() / size.x).min(r.height() / size.y);
        let m = Rect::from_center_size(r.center(), size * k);
        p.image(tex, m, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
    } else if screen {
        p.rect_filled(r.shrink(1.0), R_MD(), Color32::from_rgb(11, 11, 12));
        let msg = if id == dados::eu() { "Você está mostrando a sua tela" } else { "Esperando a tela…" };
        text(&p, media.center(), Align2::CENTER_CENTER, msg, f7(14.0), Color32::from_gray(170), 0.0);
    } else {
        ui::avatar(ui, &person.name, person.photo(), r.center(), (r.height() * 0.3).clamp(40.0, 96.0), false, None);
    }
    // a barra do nome: degradê escuro embaixo
    let bar = Rect::from_min_max(Pos2::new(r.left(), r.bottom() - 52.0), r.max);
    ui::vgradient(&p, bar, Color32::TRANSPARENT, Color32::from_black_alpha(184));
    let y = r.bottom() - 8.0 - 9.0;
    let mut x = r.left() + 10.0;
    if screen {
        ui::icon(ui, ic!("screen"), Pos2::new(x + 7.0, y), 14.0, Color32::WHITE);
        x += 14.0 + 8.0;
    }
    let name = text(&p, Pos2::new(x, y), Align2::LEFT_CENTER, &person.name, f7(12.5), Color32::WHITE, 0.0);
    let mut x = name.right() + 8.0;
    for role in &person.roles {
        ui::role_chip(ui, role, Pos2::new(x + 11.0, y), 22.0);
        x += 22.0 + 4.0;
    }
    if !screen && st.muted_of(id) {
        ui::icon(ui, ic!("mic-off"), Pos2::new(r.right() - 10.0 - 7.0, y), 14.0, Color32::from_rgb(0xff, 0x8a, 0x80));
    }
    let _ = media;
    let ui_painter = ui.painter();
    if talking {
        ui_painter.rect_stroke(r.expand(1.0), R_MD() + 1.0, Stroke::new(2.0_f32, AMBER_SOFT()), StrokeKind::Outside);
        ui_painter.rect_stroke(r, R_MD(), Stroke::new(1.0_f32, SECONDARY()), StrokeKind::Inside);
    }
}

/// O `.call-controls`: microfone, fone, câmera e tela no meio; ferramentas e sair à direita.
fn controls(ui: &mut egui::Ui, st: &mut CallState, bar: Rect) {
    ui.painter().rect_filled(bar, 0.0, CREAM_0());
    #[derive(PartialEq)]
    enum Look {
        Normal,
        On,
        Off,
        Hang,
    }
    let mid: [(ImageSource<'static>, Look, &str); 3] = [
        if st.muted || st.deafened { (ic!("mic-off"), Look::Off, "Ligar o microfone") } else { (ic!("mic"), Look::Normal, "Fechar o microfone") },
        if st.deafened { (ic!("headset-off"), Look::Off, "Voltar a ouvir") } else { (ic!("headset"), Look::Normal, "Parar de ouvir") },
        if st.sharing { (ic!("screen"), Look::On, "Parar de mostrar a tela") } else { (ic!("screen"), Look::Normal, "Mostrar a tela") },
    ];
    let right: [(ImageSource<'static>, Look, &str); 3] = [
        if st.palco { (ic!("grid"), Look::On, "Voltar para a grade") } else { (ic!("stage"), Look::Normal, "Modo palco") },
        if st.cheia { (ic!("compress"), Look::On, "Sair da tela cheia (Esc)") } else { (ic!("expand"), Look::Normal, "Tela cheia da call") },
        (ic!("hangup"), Look::Hang, "Sair da call"),
    ];
    let size = 42.0;
    let gap = 10.0;
    let mut clicked = None;
    let mut draw = |ui: &mut egui::Ui, k: usize, x: f32, (src, look, tip): (ImageSource<'static>, Look, &str)| {
        let rect = Rect::from_min_size(Pos2::new(x, bar.center().y - size / 2.0), Vec2::splat(size));
        let r = ui.interact(rect, ui.id().with(("ctl", k)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand).on_hover_text(tip);
        let p = ui.painter();
        let c = rect.center();
        let (bg, fg) = match look {
            Look::Normal => (if r.hovered() { AMBER_SOFT() } else { CREAM_2() }, INK()),
            Look::On => (SECONDARY(), ON_SECONDARY()),
            Look::Off | Look::Hang => (DIS_OFF, Color32::WHITE),
        };
        if look == Look::Off {
            p.circle_filled(c, size / 2.0 + 3.0, Color32::from_rgba_unmultiplied(229, 53, 43, 61));
        }
        p.circle_filled(c, size / 2.0, if look == Look::Hang && r.hovered() { mix(DIS_OFF, Color32::BLACK, 0.88) } else { bg });
        ui::icon(ui, src, c, 18.0, fg);
        if r.clicked() {
            clicked = Some(k);
        }
    };
    let total = size * 3.0 + gap * 2.0;
    let mut x = bar.center().x - total / 2.0;
    for (k, b) in mid.into_iter().enumerate() {
        draw(ui, k, x, b);
        x += size + gap;
    }
    let mut x = bar.right() - 20.0 - total;
    for (k, b) in right.into_iter().enumerate() {
        draw(ui, 3 + k, x, b);
        x += size + gap;
    }
    match clicked {
        Some(0) => {
            if st.deafened {
                st.deafened = false;
                st.muted = false;
            } else {
                st.muted = !st.muted;
            }
            st.aplicar_som();
        }
        Some(1) => {
            st.deafened = !st.deafened;
            st.aplicar_som();
        }
        Some(2) => {
            if st.sharing {
                if let Some(v) = &st.voz {
                    v.mandar(voz::Cmd::Tela(None));
                }
            } else if st.tela_menu.is_some() {
                st.tela_menu = None;
            } else {
                // o menu sobe em cima do botão da tela (o 3º do meio)
                let bx = bar.center().x - total / 2.0 + 2.0 * (size + gap) + size / 2.0;
                st.tela_menu = Some(Pos2::new(bx - acoes::TELA_MENU_W / 2.0, bar.top() - acoes::TELA_MENU_H - 8.0));
            }
        }
        Some(3) => {
            st.palco = !st.palco;
            st.foco = None;
        }
        Some(4) => st.cheia = !st.cheia,
        Some(5) => st.sair_da_voz(),
        _ => {}
    }
}

/* ================= pessoas (.dz-people) ================= */

fn people_col(ui: &mut egui::Ui, st: &mut CallState, col: Rect, t: f64) {
    let p = ui.painter().clone();
    p.rect_filled(col, 0.0, CREAM_0());
    let head = col_head(ui, col);
    ui::icon(ui, ic!("users"), Pos2::new(head.left() + 27.5, head.center().y), 15.0, SECONDARY());
    text(&p, Pos2::new(head.left() + 43.0, head.center().y), Align2::LEFT_CENTER, "PESSOAS", patua(13.0), INK(), 0.52);
    // as pessoas da sala aberta: os membros (e quem está na voz de um canal dela sem ser membro)
    let sala = st.sala_id();
    let grupo = st.group;
    let (pessoas, dono): (Vec<(usize, dados::Person)>, i64) = dados::com(|d| {
        let mut ids: Vec<usize> = d.membros.get(&sala).cloned().unwrap_or_default();
        for (i, p) in d.pessoas.iter().enumerate() {
            if p.room.is_some_and(|r| d.canais.get(r).is_some_and(|c| c.grupo == grupo)) && !ids.contains(&i) {
                ids.push(i);
            }
        }
        if let Some(eu) = d.eu {
            if !ids.contains(&eu) {
                ids.push(eu);
            }
        }
        (ids.into_iter().filter_map(|i| d.pessoas.get(i).map(|p| (i, p.clone()))).collect(), d.grupos.get(grupo).map(|g| g.owner).unwrap_or(0))
    });
    let online = pessoas.iter().filter(|(_, x)| x.online).count();
    text(&p, Pos2::new(head.right() - 59.0, head.center().y), Align2::RIGHT_CENTER, &format!("{online} online"), f6(11.5), FAINT(), 0.0);
    if head_act(ui, Pos2::new(head.right() - 34.0, head.center().y), ic!("gear"), "Convites e ajustes da sala").clicked() {
        st.abrir_convite(ui.ctx());
    }

    let presi_h = acoes::presidencial(ui, st, col, t);
    let list = Rect::from_min_max(Pos2::new(col.left(), head.bottom()), Pos2::new(col.right(), col.bottom() - PRESI_H - presi_h));
    let mut abrir = None;
    let mut child = ui.new_child(UiBuilder::new().max_rect(list));
    egui::ScrollArea::vertical().id_salt("pessoas").auto_shrink(false).show(&mut child, |ui| {
        ui.add_space(12.0);
        let mut cargos: Vec<Role> = Vec::new();
        for (_, p) in &pessoas {
            if let Some(r) = p.roles.first() {
                if p.online && !cargos.iter().any(|x| x.id == r.id) {
                    cargos.push(r.clone());
                }
            }
        }
        cargos.sort_by_key(|r| r.posicao);
        let ordem = |v: &mut Vec<usize>| v.sort_by_key(|i| dados::pessoa(*i).name.to_lowercase());
        for role in cargos {
            let mut ids: Vec<usize> = pessoas.iter().filter(|(_, p)| p.online && p.roles.first().is_some_and(|r| r.id == role.id)).map(|(i, _)| *i).collect();
            ordem(&mut ids);
            abrir = abrir.or(pgroup(ui, st, &role.nome.clone(), Some(role), &ids, dono));
        }
        let mut plain: Vec<usize> = pessoas.iter().filter(|(_, p)| p.online && p.roles.is_empty()).map(|(i, _)| *i).collect();
        ordem(&mut plain);
        abrir = abrir.or(pgroup(ui, st, "Online", None, &plain, dono));
        let mut off: Vec<usize> = pessoas.iter().filter(|(_, p)| !p.online).map(|(i, _)| *i).collect();
        ordem(&mut off);
        abrir = abrir.or(pgroup(ui, st, "Offline", None, &off, dono));
    });
    if let Some((id, q)) = abrir {
        // o cartão abre à esquerda da coluna, na altura da pessoa
        st.card = Some((id, Pos2::new(col.left() - 8.0 - 260.0, q.y)));
    }

    // a Call Presidencial, presa ao pé (só para quem tem a permissão)
    if !dados::tem_perm("diskenzy.presidential") {
        return;
    }
    let presi = Rect::from_min_max(Pos2::new(col.left(), col.bottom() - PRESI_H), col.max);
    let pr = ui.interact(presi, ui.id().with("presi"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand).on_hover_text(if st.presi_aberta { "Fechar a Call Presidencial" } else { "Abrir a Call Presidencial" });
    if pr.clicked() {
        st.presi_aberta = !st.presi_aberta;
    }
    p.rect_filled(presi, 0.0, if pr.hovered() { CREAM_2() } else { CREAM_0() });
    p.line_segment([presi.left_top(), presi.right_top()], Stroke::new(1.0_f32, HAIRLINE_2()));
    let c = Pos2::new(presi.left() + 31.0, presi.center().y);
    p.circle_filled(c, 15.0, SECONDARY());
    ui::icon(ui, ic!("phone"), c, 15.0, ON_SECONDARY());
    text(&p, Pos2::new(presi.left() + 56.0, presi.center().y - 7.0), Align2::LEFT_CENTER, "CALL PRESIDENCIAL", f8(11.5), INK(), 0.6);
    let n_presi = dados::com(|d| d.presidencial.get(&sala).map(|l| l.len()).unwrap_or(0));
    text(&p, Pos2::new(presi.left() + 56.0, presi.center().y + 9.0), Align2::LEFT_CENTER, &format!("{n_presi} {} na call", if n_presi == 1 { "pessoa" } else { "pessoas" }), f6(10.5), FAINT(), 0.0);
    let n = Rect::from_center_size(Pos2::new(presi.right() - 64.0, presi.center().y), Vec2::splat(20.0));
    p.rect_filled(n, 10.0, CREAM_2());
    text(&p, n.center(), Align2::CENTER_CENTER, &n_presi.to_string(), f8(11.0), SOFT(), 0.0);
    ui::icon(ui, if st.presi_aberta { ic!("chevron-down") } else { ic!("chevron-up") }, Pos2::new(presi.right() - 36.5, presi.center().y), 15.0, FAINT());
}

fn pgroup(ui: &mut egui::Ui, st: &CallState, title: &str, role: Option<Role>, ids: &[usize], dono: i64) -> Option<(usize, Pos2)> {
    if ids.is_empty() {
        return None;
    }
    let (r, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 26.0), Sense::hover());
    let p = ui.painter();
    let dot = Pos2::new(r.left() + 12.0 + 10.0 + 3.5, r.top() + 4.0 + 7.0);
    let off = title == "Offline";
    let (chip, tcolor) = match &role {
        Some(ro) => (ro.color(), mix(ro.color(), INK(), 0.62)),
        None => (if off { Color32::from_rgb(0x8a, 0x8a, 0x8a).gamma_multiply(0.7) } else { DIS_OK }, FAINT()),
    };
    if role.is_none() && !off {
        p.circle_filled(dot, 6.5, DIS_OK.gamma_multiply(0.18));
    }
    p.circle_filled(dot, 3.5, chip);
    let t = text(p, Pos2::new(dot.x + 3.5 + 8.0, dot.y), Align2::LEFT_CENTER, &title.to_uppercase(), f8(10.5), tcolor, 0.63);
    text(p, Pos2::new(t.right() + 8.0, dot.y), Align2::LEFT_CENTER, &ids.len().to_string(), f7(10.5), FAINT(), 0.0);
    let mut abrir = None;
    for &id in ids {
        if let Some(q) = person_row(ui, st, id, role.as_ref(), dono) {
            abrir = Some((id, q));
        }
    }
    ui.add_space(14.0);
    abrir
}

fn person_row(ui: &mut egui::Ui, st: &CallState, id: usize, role: Option<&Role>, dono: i64) -> Option<Pos2> {
    let (full, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 46.0), Sense::click());
    let r = full.shrink2(Vec2::new(12.0, 0.0));
    let resp = resp.on_hover_cursor(CursorIcon::PointingHand);
    let p = ui.painter();
    if resp.hovered() {
        p.rect_filled(r, R_SM(), AMBER_SOFT());
    }
    let person = dados::pessoa(id);
    let room = st.room_of(id);
    let alpha = if person.online { 1.0 } else { 0.6 };
    let dot = if !person.online {
        None
    } else if room.is_some() {
        Some((SECONDARY(), CREAM_0()))
    } else {
        Some((DIS_OK, CREAM_0()))
    };
    ui::avatar(ui, &person.name, person.photo(), Pos2::new(r.left() + 10.0 + 16.0, r.center().y), 32.0, false, dot);
    let name_c = match role {
        Some(ro) => mix(ro.color(), INK(), 0.5),
        None if person.online => INK(),
        None => SOFT(),
    };
    let label = if id == dados::eu() { format!("{} (você)", person.name) } else { person.name.to_string() };
    let y = if room.is_some() { r.center().y - 8.0 } else { r.center().y };
    let name = text(p, Pos2::new(r.left() + 54.0, y), Align2::LEFT_CENTER, &label, f7(13.0), name_c.gamma_multiply(alpha), 0.0);
    if person.profile_id == dono {
        ui::icon(ui, ic!("crown"), Pos2::new(name.right() + 6.0 + 6.5, y), 13.0, SECONDARY());
    }
    if let Some(room) = room {
        ui::icon(ui, ic!("speaker"), Pos2::new(r.left() + 54.0 + 5.0, r.center().y + 9.0), 10.0, FAINT());
        crate::nome::desenhar(ui, Pos2::new(r.left() + 54.0 + 14.0, r.center().y + 9.0), Align2::LEFT_CENTER, &dados::canal_ou(room).name, f6(10.5), FAINT(), 0.0, r.width() - 54.0 - 14.0 - 8.0);
    }
    resp.clicked().then(|| r.left_top())
}

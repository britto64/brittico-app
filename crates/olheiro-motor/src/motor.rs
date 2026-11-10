//! O motor: uma thread que gira a cada 50 ms. Acompanha o LoL (`session::Runner`), decide quando entrar e sair da sala de voz,
//! publica as posições que leu, junta com as que chegam dos outros e põe o volume de cada pessoa na mistura.
//!
//! Regras da voz (as mesmas do Olheiro de antes, agora calculadas aqui):
//!  - perto (até 900 unidades) ouve cheio; some em 1350; no meio cai em curva;
//!  - sem posição de alguém, ou leitura velha (mais de 5 s): volume 1. Falha nunca silencia;
//!  - só toca a voz de quem está na lista de jogadores da SUA partida (quem não está, o app não toca);
//!  - o volume que a pessoa escolheu para cada um multiplica o da distância.

use crate::ajustes::{Ajustes, Lado};
use crate::capture::ScreenCapture;
use crate::demo::{Demo, DemoCapture, DemoIcons, DemoLol};
use crate::icons::CdragonIcons;
use crate::lolapi::RealLol;
use crate::sala::{self, Comum, Conexao, InfoSala, ParaSala, Pedido};
use crate::session::{Out, Runner, Settings};
use crate::som::{self, Controles, Mistura, Som};
use olheiro_core::geometry::MinimapSide;
use olheiro_core::identity::{read_identity, RawIdentity};
use olheiro_core::protocol::{Phase, ToBot};
use olheiro_voz::{Fusao, Pacote, Ponto};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const PASSO: Duration = Duration::from_millis(50);
/// Depois de uma falha de entrada na sala, esperar isto antes de tentar de novo.
const DE_NOVO: Duration = Duration::from_secs(5);
/// O som que não abriu (aparelho sumiu, outro programa segurando) tenta de novo com este intervalo.
const SOM_DE_NOVO: Duration = Duration::from_secs(5);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Fase {
    /// O LoL está fechado (ou o Olheiro desligado).
    #[default]
    LolFechado,
    /// O cliente está aberto (lobby, seleção de campeões).
    Cliente,
    Carregando,
    EmPartida,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Leitura {
    /// Dá para ler o minimapa?
    pub ok: bool,
    /// O que mudar, em português (vazio quando está tudo certo).
    pub problema: String,
    pub minimapa_px: u32,
    pub icone_px: u32,
    /// Quantos aliados apareceram no último quadro.
    pub aneis: usize,
    /// Como cada aliado vivo foi achado no último quadro, pela chave do Riot ID. Quem não está aqui não foi achado.
    pub como: BTreeMap<String, Como>,
    /// Quantos aliados vivos (você incluído) o Olheiro deveria achar agora.
    pub esperados: usize,
}

pub use olheiro_core::How as Como;

impl Leitura {
    /// Quantos aliados foram vistos DE FATO (o ícone foi reconhecido agora há pouco), sem contar os palpites.
    pub fn vistos(&self) -> usize {
        self.como.values().filter(|c| **c == Como::Seen).count()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Jogador {
    /// O Riot ID como o jogo escreve.
    pub nome: String,
    pub chave: String,
    pub campeao: String,
    /// O nome do arquivo do ícone do campeão (o alias do Community Dragon).
    pub alias: String,
    /// "ORDER" (azul) ou "CHAOS" (vermelho).
    pub time: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Partida {
    pub id: String,
    pub personalizada: bool,
    pub meu_time: String,
    /// A sua chave (o seu Riot ID normalizado).
    pub voce: String,
    pub jogadores: Vec<Jogador>,
}

impl Partida {
    pub fn jogador(&self, chave: &str) -> Option<&Jogador> {
        self.jogadores.iter().find(|j| j.chave == chave)
    }
}

/// Alguém da sala, do ponto de vista de quem ouve.
#[derive(Clone, Debug, PartialEq)]
pub struct Ouvido {
    pub chave: String,
    pub nome: String,
    /// O volume pela distância (0 a 1), antes do que a pessoa escolheu. `None`: sem posição (fica cheio).
    pub distancia: Option<f64>,
    pub proximidade: f32,
    /// O volume que a pessoa escolheu para ele (0 a 2).
    pub escolhido: f32,
    /// Está na lista de jogadores da sua partida? (Quem não está não é tocado.)
    pub na_partida: bool,
    /// Fechou o próprio microfone.
    pub mudo: bool,
    /// Você o mutou.
    pub mutado: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Estado {
    pub ligado: bool,
    /// Rodando com a partida simulada (sem o LoL), e como qual dos cinco aliados.
    pub simulando: Option<usize>,
    pub fase: Fase,
    /// O Riot ID de quem está logado no cliente.
    pub riot_id: Option<String>,
    pub leitura: Leitura,
    pub partida: Option<Partida>,
    /// Onde cada jogador está agora (unidades do jogo), pela leitura mais fresca de qualquer Olheiro da sala.
    pub posicoes: BTreeMap<String, (f64, f64)>,
    pub sala: InfoSala,
    /// Quem está na sala e quanto você ouve de cada um.
    pub ouvidos: Vec<Ouvido>,
    pub microfone_aberto: bool,
    pub surdo: bool,
    pub testando_microfone: bool,
    /// O som não abriu: por quê.
    pub som_erro: Option<String>,
    /// Os aparelhos abertos de verdade (microfone, saída).
    pub som_aberto: Option<(String, String)>,
    pub aparelhos: (Vec<String>, Vec<String>),
    pub ajustes: Ajustes,
    /// As últimas linhas do registro, para a tela.
    pub registro: Vec<String>,
}

impl Estado {
    /// Está numa partida, com a sala de voz aberta.
    pub fn na_sala(&self) -> bool {
        self.sala.conexao == Conexao::Dentro
    }
}

pub enum Comando {
    Ligar,
    Desligar,
    /// Liga a partida simulada como o aliado `vaga` (0 a 4), ou volta ao LoL (`None`).
    Simular(Option<usize>),
    /// Troca os ajustes (salvos no disco e aplicados na hora).
    Ajustes(Ajustes),
    Microfone(bool),
    Surdo(bool),
    TestarMicrofone(bool),
    /// Relê a lista de microfones e saídas.
    Aparelhos,
}

/// A ponta que fica com quem desenha a tela.
pub struct Motor {
    tx: Sender<Comando>,
    estado: Arc<Mutex<Estado>>,
    comum: Arc<Comum>,
    demo: Arc<Mutex<Option<DemoArtes>>>,
}

/// As artes da partida simulada (a `Demo` não sai da thread do motor; a tela recebe só os ícones).
type DemoArtes = BTreeMap<String, olheiro_core::sim::Art>;

impl Motor {
    pub fn iniciar(acordar: impl Fn() + Send + Sync + 'static) -> Motor {
        let comum = Arc::new(Comum { mistura: Mistura::nova(), ctl: Controles::novos(), fusao: Mutex::new(Fusao::nova()), info: Mutex::new(InfoSala::default()), acordar: Box::new(acordar) });
        // os ajustes já carregados antes de devolver: quem desenha lê o estado na hora e não pode ver ajustes vazios
        // (e regravá-los por cima do que estava salvo)
        let ajustes = Ajustes::carregar();
        let estado = Arc::new(Mutex::new(Estado { ligado: ajustes.ligar_ao_abrir && ajustes.apresentado, microfone_aberto: true, ajustes, ..Default::default() }));
        let demo = Arc::new(Mutex::new(None));
        let (tx, rx) = channel();
        let (e, c, d) = (estado.clone(), comum.clone(), demo.clone());
        std::thread::Builder::new().name("olheiro-motor".into()).spawn(move || Laco::novo(rx, e, c, d).girar()).expect("thread do motor");
        Motor { tx, estado, comum, demo }
    }

    pub fn estado(&self) -> Estado {
        self.estado.lock().map(|e| e.clone()).unwrap_or_default()
    }

    pub fn mandar(&self, c: Comando) {
        let _ = self.tx.send(c);
    }

    /// O nível do microfone agora (0 a 1), depois da limpeza. Muda a cada 10 ms; a tela lê quando desenha.
    pub fn nivel_microfone(&self) -> f32 {
        self.comum.ctl.nivel_mic.get()
    }

    /// `(chave, nível do que chega, volume valendo)` de quem está falando agora.
    pub fn falando(&self) -> Vec<(String, f32, f32)> {
        self.comum.mistura.lock().map(|m| m.quadro()).unwrap_or_default()
    }

    /// O ícone de um campeão (RGBA), se já foi baixado (ou, na partida simulada, a arte dela).
    pub fn icone(&self, alias: &str) -> Option<olheiro_core::sim::Art> {
        if let Some(a) = self.demo.lock().ok().and_then(|d| d.as_ref().and_then(|d| d.get(alias).cloned())) {
            return Some(a);
        }
        let bytes = std::fs::read(crate::ajustes::dir().join("icons").join(format!("{alias}.png"))).ok()?;
        let img = image::load_from_memory_with_format(&bytes, image::ImageFormat::Png).ok()?.to_rgba8();
        Some(olheiro_core::sim::Art { w: img.width() as usize, h: img.height() as usize, rgba: img.into_raw() })
    }
}

/// O que o laço manda "para fora": aqui, uma fila que o próprio laço esvazia a cada passo.
#[derive(Clone, Default)]
struct Fila(Rc<RefCell<Vec<ToBot>>>);

impl Out for Fila {
    fn send(&mut self, m: ToBot) {
        self.0.borrow_mut().push(m);
    }
}

enum Fonte {
    Lol(Runner<RealLol, ScreenCapture, CdragonIcons, Fila>),
    Simulada(Runner<DemoLol, DemoCapture, DemoIcons, Fila>),
}

impl Fonte {
    fn tick(&mut self, ms: f64) {
        match self {
            Fonte::Lol(r) => r.tick(ms),
            Fonte::Simulada(r) => r.tick(ms),
        }
    }
    fn aneis(&self) -> usize {
        match self {
            Fonte::Lol(r) => r.rings(),
            Fonte::Simulada(r) => r.rings(),
        }
    }
    fn aliados(&self) -> Vec<(String, String)> {
        match self {
            Fonte::Lol(r) => r.aliados(),
            Fonte::Simulada(r) => r.aliados(),
        }
    }
    fn deteccao(&self) -> (Vec<(String, Como)>, usize) {
        match self {
            Fonte::Lol(r) => r.deteccao(),
            Fonte::Simulada(r) => r.deteccao(),
        }
    }
    fn set_sensibilidade(&mut self, s: f32) {
        match self {
            Fonte::Lol(r) => r.set_sensitivity(s),
            Fonte::Simulada(r) => r.set_sensitivity(s),
        }
    }
    fn set_debug(&mut self, on: bool) {
        match self {
            Fonte::Lol(r) => r.set_debug(on),
            Fonte::Simulada(r) => r.set_debug(on),
        }
    }
}

struct SalaViva {
    tx: tokio::sync::mpsc::UnboundedSender<ParaSala>,
    fio: tokio::task::JoinHandle<()>,
    pedido: Pedido,
}

struct Laco {
    rx: Receiver<Comando>,
    compartilhado: Arc<Mutex<Estado>>,
    comum: Arc<Comum>,
    demo_artes: Arc<Mutex<Option<DemoArtes>>>,
    rt: tokio::runtime::Runtime,
    est: Estado,
    fonte: Option<Fonte>,
    fila: Fila,
    sala: Option<SalaViva>,
    tentar_em: Instant,
    som: Option<Som>,
    som_quer: Option<(String, String, bool)>,
    som_tentar_em: Instant,
    seq: u64,
    t0: Instant,
}

fn chave(riot: &str) -> String {
    read_identity(&RawIdentity { riot_id: Some(riot.to_string()), ..Default::default() }).map(|i| i.key).unwrap_or_else(|| olheiro_voz::chave(riot))
}

impl Laco {
    fn novo(rx: Receiver<Comando>, compartilhado: Arc<Mutex<Estado>>, comum: Arc<Comum>, demo_artes: Arc<Mutex<Option<DemoArtes>>>) -> Laco {
        let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(2).thread_name("olheiro-voz").enable_all().build().expect("runtime da voz");
        let ajustes = Ajustes::carregar();
        comum.ctl.ganho_mic.set(ajustes.ganho_microfone);
        if let Ok(mut m) = comum.mistura.lock() {
            m.volume_geral = ajustes.volume_geral;
        }
        let est = Estado { ligado: ajustes.ligar_ao_abrir && ajustes.apresentado, microfone_aberto: true, ajustes, ..Default::default() };
        let agora = Instant::now();
        Laco { rx, compartilhado, comum, demo_artes, rt, est, fonte: None, fila: Fila::default(), sala: None, tentar_em: agora, som: None, som_quer: None, som_tentar_em: agora, seq: 0, t0: agora }
    }

    fn registrar(&mut self, linha: impl Into<String>) {
        let linha = linha.into();
        crate::logfile::write(&linha);
        self.est.registro.push(linha);
        let n = self.est.registro.len();
        if n > 40 {
            self.est.registro.drain(..n - 40);
        }
    }

    fn girar(mut self) {
        let mut proximo = Instant::now();
        loop {
            proximo += PASSO;
            loop {
                let espera = proximo.saturating_duration_since(Instant::now());
                match self.rx.recv_timeout(espera) {
                    Ok(c) => self.comando(c),
                    Err(RecvTimeoutError::Timeout) => break,
                    // a tela foi embora: sai da sala e encerra
                    Err(RecvTimeoutError::Disconnected) => {
                        self.sair_da_sala();
                        return;
                    }
                }
            }
            if Instant::now() > proximo + PASSO * 4 {
                proximo = Instant::now();
            }
            self.passo();
        }
    }

    fn comando(&mut self, c: Comando) {
        match c {
            Comando::Ligar => {
                self.est.ligado = true;
                self.registrar(format!("Olheiro {} ligado.", crate::VERSAO));
            }
            Comando::Desligar => {
                self.est.ligado = false;
                self.est.simulando = None;
                self.parar_fonte();
                self.registrar("Olheiro desligado.");
            }
            Comando::Simular(v) => {
                self.parar_fonte();
                self.est.simulando = v.map(|v| v.min(4));
                self.est.ligado = true;
                self.registrar(match v {
                    Some(v) => format!("Partida simulada ligada (você é o aliado {}).", v + 1),
                    None => "De volta ao LoL de verdade.".into(),
                });
            }
            Comando::Ajustes(a) => {
                let antes = self.est.ajustes.clone();
                if let Err(e) = a.salvar() {
                    self.registrar(format!("Não consegui salvar os ajustes: {e}"));
                }
                self.comum.ctl.ganho_mic.set(a.ganho_microfone);
                if let Ok(mut m) = self.comum.mistura.lock() {
                    m.volume_geral = a.volume_geral;
                }
                if (a.sensibilidade - antes.sensibilidade).abs() > f32::EPSILON {
                    if let Some(f) = self.fonte.as_mut() {
                        f.set_sensibilidade(a.sensibilidade);
                    }
                }
                if a.pacote_erro != antes.pacote_erro {
                    if let Some(f) = self.fonte.as_mut() {
                        f.set_debug(a.pacote_erro);
                    }
                }
                let lado_mudou = a.lado_minimapa != antes.lado_minimapa;
                self.est.ajustes = a;
                if lado_mudou {
                    self.parar_fonte();
                }
            }
            Comando::Microfone(aberto) => {
                self.est.microfone_aberto = aberto;
                self.comum.ctl.mudo.store(!aberto, Ordering::Relaxed);
                if let Some(s) = &self.sala {
                    let _ = s.tx.send(ParaSala::Microfone { aberto });
                }
            }
            Comando::Surdo(s) => {
                self.est.surdo = s;
                if let Ok(mut m) = self.comum.mistura.lock() {
                    m.surdo = s;
                }
            }
            Comando::TestarMicrofone(on) => self.est.testando_microfone = on,
            Comando::Aparelhos => self.est.aparelhos = som::aparelhos(),
        }
        self.publicar();
    }

    fn parar_fonte(&mut self) {
        self.fonte = None;
        self.fila.0.borrow_mut().clear();
        self.sair_da_sala();
        self.est.fase = Fase::LolFechado;
        self.est.partida = None;
        self.est.leitura = Leitura::default();
        self.est.posicoes.clear();
        if let Ok(mut d) = self.demo_artes.lock() {
            *d = None;
        }
        if let Ok(mut f) = self.comum.fusao.lock() {
            *f = Fusao::nova();
        }
    }

    fn sair_da_sala(&mut self) {
        if let Some(s) = self.sala.take() {
            let _ = s.tx.send(ParaSala::Sair);
            let _ = self.rt.block_on(async { tokio::time::timeout(Duration::from_secs(3), s.fio).await });
        }
    }

    fn criar_fonte(&mut self) {
        let lado = match self.est.ajustes.lado_minimapa {
            Lado::Direita => MinimapSide::Right,
            Lado::Esquerda => MinimapSide::Left,
        };
        let cfg = Settings { side: lado, debug: self.est.ajustes.pacote_erro, sensitivity: self.est.ajustes.sensibilidade, ..Settings::default() };
        self.fonte = Some(match self.est.simulando {
            Some(vaga) => {
                // a partida anda 3x mais rápido que o relógio, para os aliados se espalharem logo
                let d = Demo::time_fixo(7, vaga).speed(3.0);
                if let Ok(mut artes) = self.demo_artes.lock() {
                    *artes = Some(d.artes().into_iter().collect());
                }
                Fonte::Simulada(Runner::new(d.lol(), d.capture(), d.icons(), self.fila.clone(), cfg))
            }
            None => Fonte::Lol(Runner::new(RealLol::new(), ScreenCapture, CdragonIcons::new(), self.fila.clone(), cfg)),
        });
    }

    fn passo(&mut self) {
        let antes = self.est.clone();
        if self.est.ligado && self.fonte.is_none() {
            self.criar_fonte();
        }
        let ms = self.t0.elapsed().as_secs_f64() * 1000.0;
        if let Some(f) = self.fonte.as_mut() {
            f.tick(ms);
            self.est.leitura.aneis = f.aneis();
            let (como, esperados) = f.deteccao();
            self.est.leitura.como = como.into_iter().collect();
            self.est.leitura.esperados = esperados;
        }
        let recados: Vec<ToBot> = self.fila.0.borrow_mut().drain(..).collect();
        for r in recados {
            self.recado(r);
        }
        self.cuidar_da_sala();
        self.cuidar_do_som();
        self.volumes();
        if let Ok(i) = self.comum.info.lock() {
            self.est.sala = i.clone();
        }
        if self.est.sala.conexao != antes.sala.conexao && self.est.sala.conexao == Conexao::Dentro {
            self.registrar("Na sala de voz da partida.");
        }
        if self.est.sala.pessoas.len() != antes.sala.pessoas.len() && self.est.sala.conexao == Conexao::Dentro {
            let n = self.est.sala.pessoas.len();
            self.registrar(format!("Na sala: você e mais {n}."));
        }
        if self.est != antes {
            self.publicar();
        }
    }

    fn publicar(&mut self) {
        if let Ok(mut e) = self.compartilhado.lock() {
            *e = self.est.clone();
        }
        (self.comum.acordar)();
    }

    fn recado(&mut self, r: ToBot) {
        match r {
            ToBot::Status { phase, riot_id, calib } => {
                let fase = match phase {
                    Phase::None => Fase::LolFechado,
                    Phase::Client => Fase::Cliente,
                    Phase::Loading => Fase::Carregando,
                    Phase::Ingame => Fase::EmPartida,
                };
                if fase != self.est.fase {
                    match fase {
                        Fase::LolFechado => self.registrar("O LoL está fechado."),
                        Fase::Cliente => self.registrar("Cliente do LoL aberto."),
                        Fase::Carregando => self.registrar("A partida está carregando."),
                        Fase::EmPartida => self.registrar("Partida em andamento."),
                    }
                }
                self.est.fase = fase;
                if riot_id.is_some() || fase == Fase::LolFechado {
                    self.est.riot_id = riot_id;
                }
                match calib {
                    Some(c) => {
                        self.est.leitura.ok = c.ok;
                        self.est.leitura.problema = c.problem;
                        self.est.leitura.minimapa_px = c.minimap_px;
                        self.est.leitura.icone_px = c.icon_px;
                    }
                    None if fase != Fase::EmPartida => self.est.leitura = Leitura::default(),
                    None => {}
                }
                if fase != Fase::EmPartida && self.est.partida.is_some() {
                    self.registrar("A partida acabou.");
                    self.est.partida = None;
                    self.est.posicoes.clear();
                    if let Ok(mut f) = self.comum.fusao.lock() {
                        *f = Fusao::nova();
                    }
                }
            }
            ToBot::Roster { game_id, custom, you, players, .. } => {
                let aliados: BTreeMap<String, String> = self.fonte.as_ref().map(|f| f.aliados()).unwrap_or_default().into_iter().collect();
                let jogadores: Vec<Jogador> = players
                    .iter()
                    .map(|p| {
                        let k = chave(&p.riot_id);
                        let alias = aliados.get(&k).cloned().unwrap_or_else(|| crate::lol::champion_alias(None, &p.champ));
                        Jogador { nome: p.riot_id.clone(), chave: k, campeao: p.champ.clone(), alias, time: p.team.clone() }
                    })
                    .collect();
                let voce = chave(&you);
                let meu_time = jogadores.iter().find(|j| j.chave == voce).map(|j| j.time.clone()).unwrap_or_else(|| "ORDER".into());
                self.registrar(format!("Partida achada: {} jogadores{}.", jogadores.len(), if custom { ", personalizada" } else { "" }));
                self.est.riot_id = Some(you);
                self.est.partida = Some(Partida { id: game_id, personalizada: custom, meu_time, voce, jogadores });
            }
            ToBot::Frame { pos, dead, .. } => {
                let agora = crate::agora_ms();
                let pontos: Vec<Ponto> = pos.into_iter().map(|p| Ponto { k: p.k, x: p.x, y: p.y, c: p.c, s: p.s }).collect();
                if let Ok(mut f) = self.comum.fusao.lock() {
                    f.mortos(dead);
                    for p in &pontos {
                        f.observar(agora, p.clone(), agora);
                    }
                }
                if let Some(s) = &self.sala {
                    self.seq += 1;
                    let _ = s.tx.send(ParaSala::Posicoes(Pacote { t: agora, seq: self.seq, pos: pontos }));
                }
            }
            ToBot::Hello { .. } => {}
        }
    }

    /// O pedido de entrada que vale agora, ou `None` se não é hora de estar numa sala.
    fn pedido(&self) -> Option<Pedido> {
        let p = self.est.partida.as_ref()?;
        if !self.est.ligado || self.est.fase != Fase::EmPartida || !self.est.ajustes.porteiro_valido() {
            return None;
        }
        let riot_id = p.jogador(&p.voce).map(|j| j.nome.clone()).or_else(|| self.est.riot_id.clone())?;
        Some(Pedido {
            porteiro: self.est.ajustes.porteiro.trim().to_string(),
            riot_id,
            partida: p.id.clone(),
            time: p.meu_time.clone(),
            personalizada: p.personalizada,
            grupo: Some(self.est.ajustes.codigo_grupo.clone()).filter(|g| !g.trim().is_empty()),
        })
    }

    fn cuidar_da_sala(&mut self) {
        let quer = self.pedido();
        // a sala terminou sozinha (caiu, ou o porteiro recusou): tenta de novo daqui a pouco
        if self.sala.as_ref().is_some_and(|s| s.fio.is_finished()) {
            self.sala = None;
            let falha = self.comum.info.lock().ok().and_then(|i| if let Conexao::Falhou(e) = &i.conexao { Some(e.clone()) } else { None });
            if let Some(e) = falha {
                self.registrar(e);
            }
            self.tentar_em = Instant::now() + DE_NOVO;
        }
        match (&self.sala, quer) {
            (Some(s), Some(q)) if s.pedido == q => {}
            (Some(_), _) => {
                self.sair_da_sala();
                self.tentar_em = Instant::now();
            }
            (None, Some(q)) if Instant::now() >= self.tentar_em => {
                let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
                let fio = self.rt.spawn(sala::rodar(q.clone(), rx, self.comum.clone(), self.est.microfone_aberto));
                self.registrar("Entrando na sala de voz da partida…");
                self.sala = Some(SalaViva { tx, fio, pedido: q });
            }
            _ => {}
        }
        if self.sala.is_none() && self.pedido().is_none() {
            if let Ok(mut i) = self.comum.info.lock() {
                if i.conexao != Conexao::Fora {
                    *i = InfoSala::default();
                }
            }
        }
    }

    fn cuidar_do_som(&mut self) {
        let na_sala = self.sala.is_some();
        let quer = if na_sala || self.est.testando_microfone {
            Some((self.est.ajustes.microfone.clone(), self.est.ajustes.saida.clone(), na_sala))
        } else {
            None
        };
        if quer != self.som_quer {
            self.som = None;
            self.som_quer = quer.clone();
            self.est.som_aberto = None;
            self.est.som_erro = None;
            self.som_tentar_em = Instant::now();
        }
        if self.som.as_ref().is_some_and(|s| s.quebrado()) {
            // o fone foi desligado ou trocou de perfil: fecha e abre de novo, que é o que a pessoa faria à mão
            self.registrar("Um aparelho de som caiu; abrindo de novo.");
            self.som = None;
            self.est.som_aberto = None;
            self.som_tentar_em = Instant::now() + Duration::from_secs(2);
        }
        if let Some((mic, sai, com_saida)) = quer {
            if self.som.is_none() && Instant::now() >= self.som_tentar_em {
                match Som::abrir(&mic, &sai, com_saida.then(|| self.comum.mistura.clone()), self.comum.ctl.clone()) {
                    Ok(s) => {
                        self.est.som_aberto = Some((s.entrada.clone(), s.saida.clone()));
                        self.est.som_erro = s.aviso.as_ref().map(|e| format!("{}. Você está ouvindo, mas não fala até escolher outro microfone nos Ajustes.", e.trim_end_matches('.')));
                        if let Some(e) = s.aviso.clone() {
                            self.registrar(e);
                        }
                        self.som = Some(s);
                    }
                    Err(e) => {
                        if self.est.som_erro.as_deref() != Some(e.as_str()) {
                            self.registrar(e.clone());
                        }
                        self.est.som_erro = Some(e);
                        self.som_tentar_em = Instant::now() + SOM_DE_NOVO;
                    }
                }
            }
        } else {
            self.comum.ctl.nivel_mic.set(0.0);
        }
    }

    /// O volume de cada pessoa da sala: distância × o que você escolheu; zero para quem não está na sua partida.
    fn volumes(&mut self) {
        let agora = crate::agora_ms();
        let Ok(mut fusao) = self.comum.fusao.lock() else { return };
        let mut posicoes = BTreeMap::new();
        if let Some(p) = &self.est.partida {
            for j in &p.jogadores {
                if let Some(xy) = fusao.posicao(agora, &j.chave) {
                    posicoes.insert(j.chave.clone(), xy);
                }
            }
        }
        self.est.posicoes = posicoes;
        let pessoas: Vec<(String, String)> = self.comum.info.lock().map(|i| i.pessoas.iter().map(|(k, v)| (k.clone(), v.clone())).collect()).unwrap_or_default();
        let mudos: Vec<String> = self.comum.info.lock().map(|i| i.mudos.clone()).unwrap_or_default();
        let mut ouvidos = Vec::new();
        let voce = self.est.partida.as_ref().map(|p| p.voce.clone()).unwrap_or_default();
        for (k, nome) in pessoas {
            let na_partida = self.est.partida.as_ref().is_some_and(|p| p.jogador(&k).is_some());
            let tem_as_duas = fusao.posicao(agora, &voce).zip(fusao.posicao(agora, &k));
            let distancia = tem_as_duas.map(|((x1, y1), (x2, y2))| olheiro_voz::distancia(x1, y1, x2, y2));
            let proximidade = fusao.volume(agora, &voce, &k) as f32;
            let escolhido = self.est.ajustes.volume_de(&k);
            let mutado = self.est.ajustes.mutados.contains(&k);
            let alvo = if na_partida && !mutado { proximidade * escolhido } else { 0.0 };
            if let Ok(mut m) = self.comum.mistura.lock() {
                m.alvo(&k, alvo);
            }
            ouvidos.push(Ouvido { mudo: mudos.contains(&k), mutado, chave: k, nome, distancia, proximidade, escolhido, na_partida });
        }
        self.est.ouvidos = ouvidos;
    }
}

#[cfg(test)]
mod tests;

//! O laço do Olheiro: acompanha o LoL, monta o elenco da partida, carrega os ícones, calibra a captura e entrega as posições (no
//! Brittico, ao motor, que as manda à sala de voz). Os recados ainda têm o formato do tempo do bot (`ToBot`).
//!
//! Genérico sobre o LoL, a captura e os ícones (`Lol`, `Capture`, `Icons`): no Windows são o jogo de verdade, a tela e o Community
//! Dragon; no Arch (`--demo` e os testes) são o simulador do núcleo. É o mesmo código nos dois, então o que se testa aqui é o que roda.
//!
//! O que o bot vê, em ordem, a cada partida: `status` (carregando) → `roster` (os dez, dos dois times) → `status` (em partida, com a
//! calibração) → `frame`s a ~5 Hz → `status` (de volta ao cliente) quando a partida acaba. Falhas viram um `problem` em português na
//! calibração, para o painel do site e o app dizerem ao jogador o que mudar; nada aqui trava nem derruba o app.

use crate::lol::{self, GameInfo, LivePlayer};
use olheiro_core::geometry::{capture_bounds, icon_diam, minimap_region, region_fits, MapKind, MinimapSide};
use olheiro_core::protocol::{Calib, Phase, Pos, RosterPlayer, ToBot};
use olheiro_core::templates::TemplateSet;
use olheiro_core::tracker::{How, Scout, ScoutPlayer};
use olheiro_core::{Frame, Rect};

pub trait Lol {
    fn phase(&mut self) -> Phase;
    /// O Riot ID de quem está logado no cliente (não precisa de partida).
    fn client_riot_id(&mut self) -> Option<String>;
    fn game(&mut self) -> Option<GameInfo>;
    fn players(&mut self) -> Option<Vec<LivePlayer>>;
    /// `(fila, personalizada)`.
    fn queue(&mut self) -> Option<(i64, bool)>;
    fn minimap_scale(&mut self) -> Option<f64>;
}

pub trait Capture {
    /// A área do cliente da janela do jogo, em coordenadas da tela.
    fn window(&mut self) -> Option<Rect>;
    fn grab(&mut self, bounds: Rect) -> Result<Frame, String>;
}

pub trait Icons {
    /// Os modelos de cada campeão: `(número no time aliado, alias do campeão)`. Quem falhar fica de fora.
    fn load(&mut self, champs: &[(usize, String)]) -> Vec<(usize, Vec<Vec<f32>>)>;
}

pub trait Out {
    fn send(&mut self, m: ToBot);
}

#[derive(Clone, Debug)]
pub struct Settings {
    pub side: MinimapSide,
    /// De quanto em quanto tempo olha o LoL quando não há partida.
    pub poll_idle_ms: f64,
    /// ...e durante a partida (mortes e fim).
    pub poll_game_ms: f64,
    /// O período dos quadros de posição (5 Hz).
    pub frame_ms: f64,
    /// Esperar tanto antes de tentar de novo depois de uma falha ao montar a partida (ícones fora do ar…).
    pub retry_ms: f64,
    /// Gravar o pacote de depuração na partida (ver `debug.rs`).
    pub debug: bool,
    /// Onde gravá-lo (`None`: na pasta de configuração).
    pub debug_dir: Option<std::path::PathBuf>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { side: MinimapSide::Right, poll_idle_ms: 2000.0, poll_game_ms: 1000.0, frame_ms: 200.0, retry_ms: 5000.0, debug: false, debug_dir: None }
    }
}

const SCALE_POLL_MS: f64 = 5000.0;
/// Sem nenhum anel por tanto tempo (já em partida): o jogador precisa saber, e a frase diz o que fazer.
const NO_RINGS_MS: f64 = 20_000.0;
/// O quanto o ícone pode ser pequeno antes de a leitura ficar ruim (13 px deu 89,5% nas simulações; 16 px, 97%).
const MIN_GOOD_ICON_PX: f64 = 14.0;

struct Game {
    map: MapKind,
    map_number: i64,
    game_id: String,
    aliases: Vec<String>,
    you_key: String,
    my_team: String,
    /// Número do jogador no `Scout` → a chave dele no bot.
    keys: Vec<String>,
    scout: Scout,
    scale: Option<f64>,
    scale_at: f64,
    last_poll: f64,
    last_frame: f64,
    missing: u32,
    seq: u64,
    game_time0: f64,
    t0: f64,
    no_rings_since: Option<f64>,
    dead: Vec<String>,
    /// O pacote de depuração desta partida (só com `Settings.debug`).
    bundle: Option<crate::debug::Bundle>,
    /// A geometria usada, para o `meta.json` do pacote (preenchida quando o primeiro quadro é lido).
    meta_written: bool,
}

enum Mode {
    Idle,
    InGame(Box<Game>),
}

pub struct Runner<L, C, I, O> {
    pub lol: L,
    pub cap: C,
    pub icons: I,
    pub out: O,
    cfg: Settings,
    mode: Mode,
    last_poll: f64,
    retry_until: f64,
    last_status: Option<ToBot>,
    last_roster: Option<ToBot>,
}

impl<L: Lol, C: Capture, I: Icons, O: Out> Runner<L, C, I, O> {
    pub fn new(lol: L, cap: C, icons: I, out: O, cfg: Settings) -> Self {
        Runner { lol, cap, icons, out, cfg, mode: Mode::Idle, last_poll: f64::NEG_INFINITY, retry_until: 0.0, last_status: None, last_roster: None }
    }

    pub fn in_game(&self) -> bool {
        matches!(self.mode, Mode::InGame(_))
    }

    /// Liga ou desliga a gravação do pacote (vale a partir da próxima partida).
    pub fn set_debug(&mut self, on: bool) {
        self.cfg.debug = on;
    }

    /// O último `status` mandado ao bot (fase, Riot ID, calibração): é o que a janela e a bandeja mostram.
    pub fn last_status(&self) -> Option<&ToBot> {
        self.last_status.as_ref()
    }

    /// `(chave, alias do campeão)` de cada aliado da partida em andamento, na ordem do time.
    pub fn aliados(&self) -> Vec<(String, String)> {
        match &self.mode {
            Mode::InGame(g) => g.keys.iter().cloned().zip(g.aliases.iter().cloned()).collect(),
            Mode::Idle => Vec::new(),
        }
    }

    /// Quantos anéis de aliado o último quadro tinha (0 fora de partida).
    pub fn rings(&self) -> usize {
        match &self.mode {
            Mode::InGame(g) => g.scout.rings_seen(),
            Mode::Idle => 0,
        }
    }

    /// A ligação com o bot (re)abriu: ele não guarda nada de uma ligação para outra, então o que ele precisa saber é reenviado.
    pub fn on_connected(&mut self) {
        if let Some(s) = self.last_status.clone() {
            self.out.send(s);
        }
        if let Some(r) = self.last_roster.clone() {
            self.out.send(r);
        }
    }

    /// O `status` só sai quando muda.
    fn status(&mut self, phase: Phase, riot_id: Option<String>, calib: Option<Calib>) {
        let m = ToBot::Status { phase, riot_id, calib };
        if self.last_status.as_ref() != Some(&m) {
            self.last_status = Some(m.clone());
            self.out.send(m);
        }
    }

    fn problem(ok: bool, problem: &str, minimap_px: i32, icon_px: f64) -> Calib {
        Calib { ok, minimap_px: minimap_px.max(0) as u32, icon_px: icon_px.max(0.0) as u32, problem: problem.to_string() }
    }

    /// Um passo. `now_ms` é um relógio que só cresce. Chamar a cada ~50 ms; o laço decide o que está na hora.
    pub fn tick(&mut self, now_ms: f64) {
        if matches!(self.mode, Mode::Idle) {
            if now_ms - self.last_poll < self.cfg.poll_idle_ms {
                return;
            }
            self.last_poll = now_ms;
            let phase = self.lol.phase();
            let riot = if phase == Phase::None { None } else { self.lol.client_riot_id() };
            match phase {
                Phase::Ingame if now_ms >= self.retry_until => self.begin(now_ms, riot),
                Phase::Ingame => {}
                p => self.status(if p == Phase::Loading { Phase::Loading } else { p }, riot, None),
            }
        } else {
            self.play(now_ms);
        }
    }

    /// A partida está no ar: monta o elenco, baixa os ícones e passa a mandar posições.
    fn begin(&mut self, now_ms: f64, riot: Option<String>) {
        let Some(info) = self.lol.game() else {
            self.status(Phase::Loading, riot, None);
            return;
        };
        let you = info.you.display.clone();
        let Some(map) = MapKind::from_number(info.map_number) else {
            let c = Self::problem(false, "Este mapa ainda não tem proximidade: ela só funciona na Summoner's Rift.", 0, 0.0);
            self.status(Phase::Ingame, Some(you), Some(c));
            return;
        };
        let Some(my_team) = lol::my_team(&info) else {
            let c = Self::problem(false, "Não achei você entre os jogadores da partida. Confira o seu Riot ID no cliente do LoL.", 0, 0.0);
            self.status(Phase::Ingame, Some(you), Some(c));
            return;
        };
        self.status(Phase::Loading, Some(you.clone()), None);

        let ally: Vec<&LivePlayer> = info.players.iter().filter(|p| p.team == my_team).collect();
        let champs: Vec<(usize, String)> = ally.iter().enumerate().map(|(i, p)| (i, p.alias.clone())).collect();
        let loaded = self.icons.load(&champs);
        if loaded.len() < ally.len() {
            self.retry_until = now_ms + self.cfg.retry_ms;
            let c = Self::problem(false, "Não consegui baixar os ícones dos campeões do seu time. Confira a internet; tento de novo em instantes.", 0, 0.0);
            self.status(Phase::Ingame, Some(you), Some(c));
            return;
        }
        let players: Vec<ScoutPlayer> = loaded.into_iter().map(|(id, vecs)| ScoutPlayer { id, templates: TemplateSet::new(id, vecs) }).collect();
        let mut scout = Scout::new();
        scout.set_players(players);

        let queue = self.lol.queue();
        let roster = ToBot::Roster {
            game_id: lol::game_id(info.map_number, &info.players),
            map: info.map_number,
            mode: info.mode.clone(),
            queue: queue.map(|q| q.0),
            custom: queue.map(|q| q.1).unwrap_or(false),
            you: you.clone(),
            players: info.players.iter().map(|p| RosterPlayer { riot_id: p.identity.display.clone(), champ: p.champion.clone(), team: p.team.clone() }).collect(),
        };
        self.last_roster = Some(roster.clone());
        self.out.send(roster);
        self.mode = Mode::InGame(Box::new(Game {
            map,
            map_number: info.map_number,
            game_id: lol::game_id(info.map_number, &info.players),
            aliases: ally.iter().map(|p| p.alias.clone()).collect(),
            you_key: info.you.key.clone(),
            my_team,
            keys: ally.iter().map(|p| p.identity.key.clone()).collect(),
            scout,
            scale: None,
            scale_at: f64::NEG_INFINITY,
            last_poll: now_ms,
            last_frame: f64::NEG_INFINITY,
            missing: 0,
            seq: 0,
            game_time0: info.game_time,
            t0: now_ms,
            no_rings_since: None,
            dead: Vec::new(),
            bundle: None,
            meta_written: false,
        }));
        let _ = riot;
    }

    fn end_game(&mut self, phase: Phase) {
        self.mode = Mode::Idle;
        self.last_roster = None;
        let riot = if phase == Phase::None { None } else { self.lol.client_riot_id() };
        self.status(phase, riot, None);
    }

    fn play(&mut self, now_ms: f64) {
        let Mode::InGame(g) = &mut self.mode else { return };

        // O jogo acabou (ou o app perdeu a API por três checagens seguidas)? E quem morreu?
        if now_ms - g.last_poll >= self.cfg.poll_game_ms {
            g.last_poll = now_ms;
            let phase = self.lol.phase();
            if phase != Phase::Ingame {
                g.missing += 1;
                if g.missing >= 3 {
                    self.end_game(phase);
                    return;
                }
            } else {
                g.missing = 0;
                if let Some(list) = self.lol.players() {
                    let mut dead_keys = Vec::new();
                    let mut ids = Vec::new();
                    for p in list.iter().filter(|p| p.team == g.my_team && p.is_dead) {
                        if let Some(i) = g.keys.iter().position(|k| *k == p.identity.key) {
                            ids.push(i);
                            dead_keys.push(p.identity.key.clone());
                        }
                    }
                    g.scout.set_dead(ids);
                    g.dead = dead_keys;
                }
            }
        }
        if now_ms - g.scale_at >= SCALE_POLL_MS {
            g.scale_at = now_ms;
            if let Some(s) = self.lol.minimap_scale() {
                g.scale = Some(s);
            }
        }
        if now_ms - g.last_frame < self.cfg.frame_ms {
            return;
        }
        g.last_frame = now_ms;

        let riot = Some(g.you_key.clone());
        let Some(win) = self.cap.window() else {
            let c = Self::problem(false, "Não achei a janela do LoL. Jogue em modo Janela sem borda (não em tela cheia) e deixe o jogo visível.", 0, 0.0);
            self.status(Phase::Ingame, riot, Some(c));
            return;
        };
        // sem `MinimapScale` no game.cfg o padrão do jogo serve de palpite, mas só até o arquivo ser lido
        let scale = g.scale.unwrap_or(1.0);
        let cap_b = capture_bounds(win, self.cfg.side);
        let region = minimap_region(win, scale, cap_b, self.cfg.side);
        if !region_fits(&region, &cap_b) {
            let c = Self::problem(false, "O minimapa está grande demais para eu ler. Diminua a escala dele nas opções do LoL (Interface › Minimapa).", region.w, 0.0);
            self.status(Phase::Ingame, riot, Some(c));
            return;
        }
        let frame = match self.cap.grab(cap_b) {
            Ok(f) => f,
            Err(e) => {
                let c = Self::problem(false, &e, region.w, icon_diam(region.w));
                self.status(Phase::Ingame, riot, Some(c));
                return;
            }
        };
        let Mode::InGame(g) = &mut self.mode else { return };
        g.scout.on_frame(&frame, &region, now_ms);

        // a calibração: dá para ler? o ícone tem tamanho bom? há anéis na tela?
        let icon_px = icon_diam(region.w);
        if g.scout.rings_seen() > 0 {
            g.no_rings_since = None;
        } else if g.no_rings_since.is_none() {
            g.no_rings_since = Some(now_ms);
        }
        let blind = g.no_rings_since.is_some_and(|t| now_ms - t >= NO_RINGS_MS);
        let calib = if blind {
            Self::problem(false, "Não estou vendo ícones no minimapa. Use o modo Janela sem borda, deixe o minimapa visível e sem nada por cima.", region.w, icon_px)
        } else if icon_px < MIN_GOOD_ICON_PX {
            Self::problem(true, "O minimapa está pequeno e a leitura fica menos certa. Aumente a escala dele nas opções do LoL.", region.w, icon_px)
        } else {
            Self::problem(true, "", region.w, icon_px)
        };

        let places = g.scout.placements(now_ms);
        let pos: Vec<Pos> = places
            .iter()
            .filter_map(|p| {
                let key = g.keys.get(p.id)?.clone();
                let (gx, gy) = g.scout.to_game(p.x, p.y);
                let (c, s) = match p.how {
                    How::Seen => (0.9, "seen"),
                    How::Under => (0.6, "under"),
                    How::Inferred => (0.6, "inferred"),
                    How::Held => (0.3, "held"),
                };
                Some(Pos { k: key, x: gx.round(), y: gy.round(), c, s: s.into() })
            })
            .collect();
        if self.cfg.debug && !g.meta_written {
            g.meta_written = true;
            let meta = crate::debug::Meta {
                version: crate::VERSAO.into(),
                game_id: g.game_id.clone(),
                map: g.map_number,
                window: win,
                capture: cap_b,
                region,
                scale,
                allies: g.keys.iter().cloned().zip(g.aliases.iter().cloned()).collect(),
            };
            let base = self.cfg.debug_dir.clone().unwrap_or_else(crate::ajustes::dir);
            g.bundle = crate::debug::Bundle::start(&base, &meta, crate::agora_ms() / 1000);
        }
        if let Some(b) = g.bundle.as_mut() {
            b.frame(now_ms, &frame, g.scout.rings_seen(), &places, &g.keys);
        }
        g.seq += 1;
        let frame_msg = ToBot::Frame { seq: g.seq, gt: g.game_time0 + (now_ms - g.t0) / 1000.0, pos, dead: g.dead.clone() };
        let _ = g.map;
        self.status(Phase::Ingame, riot, Some(calib));
        self.out.send(frame_msg);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::demo::Demo;

    #[derive(Default)]
    struct Sink(Vec<ToBot>);
    impl Out for Sink {
        fn send(&mut self, m: ToBot) {
            self.0.push(m);
        }
    }

    fn rodar(demo: &Demo, segundos: f64) -> Vec<ToBot> {
        let mut r = Runner::new(demo.lol(), demo.capture(), demo.icons(), Sink::default(), Settings::default());
        let mut t = 0.0;
        while t < segundos * 1000.0 {
            r.tick(t);
            t += 50.0;
        }
        r.out.0
    }

    #[test]
    fn a_ordem_dos_recados_e_a_que_o_bot_espera() {
        let d = Demo::new(3, "Kennzy#BR1", false);
        let msgs = rodar(&d, 10.0);
        let tipo = |m: &ToBot| match m {
            ToBot::Status { phase, .. } => format!("status:{phase:?}"),
            ToBot::Roster { .. } => "roster".to_string(),
            ToBot::Frame { .. } => "frame".to_string(),
            ToBot::Hello { .. } => "hello".to_string(),
        };
        let seq: Vec<String> = msgs.iter().map(tipo).collect();
        assert_eq!(&seq[..3], ["status:Loading", "roster", "status:Ingame"], "{seq:?}");
        assert!(seq[3..].iter().all(|s| s == "frame" || s == "status:Ingame"), "{seq:?}");
        assert!(seq.iter().filter(|s| *s == "frame").count() > 30, "5 quadros por segundo");
    }

    #[test]
    fn o_elenco_tem_os_dez_jogadores_dos_dois_times_e_quem_e_voce() {
        let msgs = rodar(&Demo::new(3, "Kennzy#BR1", true), 5.0);
        let Some(ToBot::Roster { players, you, queue, custom, map, game_id, .. }) = msgs.iter().find(|m| matches!(m, ToBot::Roster { .. })) else { panic!("sem elenco") };
        assert_eq!(players.len(), 10);
        assert_eq!(players.iter().filter(|p| p.team == "ORDER").count(), 5);
        assert_eq!(you, "Kennzy#BR1");
        assert_eq!((*map, *custom), (11, true));
        assert_eq!(*queue, Some(0), "personalizada = fila 0");
        assert_eq!(game_id.len(), 16);
    }

    #[test]
    fn os_quadros_levam_so_aliados_em_unidades_do_jogo_e_acertam_a_posicao() {
        // uma partida de 5 min com semente fixa (e só depois dos 90 s da saída da base): 30 s de uma semente azarada oscilam demais
        let d = Demo::new(1, "Kennzy#BR1", false);
        let msgs = rodar(&d, 300.0);
        let aliados: Vec<String> = d.ally_keys();
        let mut ok = 0usize;
        let mut total = 0usize;
        for m in &msgs {
            if let ToBot::Frame { pos, seq, .. } = m {
                assert!(pos.iter().all(|p| aliados.contains(&p.k)), "só aliados");
                assert!(pos.iter().all(|p| (0.0..=14870.0).contains(&p.x) && (0.0..=14980.0).contains(&p.y)));
                if *seq > 450 {
                    // depois dos 90 s da saída da base
                    total += pos.len();
                    ok += pos.iter().filter(|p| d.near_truth(&p.k, p.x, p.y, *seq)).count();
                }
            }
        }
        assert!(total > 3000, "{total}");
        assert!(ok as f64 / total as f64 >= 0.95, "{ok}/{total}");
    }

    /// Diagnóstico (não roda no `cargo test`): o acerto por janela de 60 s, por quadro e por "como sabe", em várias sementes.
    #[test]
    #[ignore]
    fn diagnostico_do_acerto() {
        for seed in 1..=8u64 {
            let d = Demo::new(seed, "Kennzy#BR1", false);
            let msgs = rodar(&d, 300.0);
            let mut janela = vec![(0usize, 0usize); 5];
            let mut por_como = std::collections::BTreeMap::<String, (usize, usize)>::new();
            for m in &msgs {
                if let ToBot::Frame { pos, seq, .. } = m {
                    for p in pos {
                        let ok = d.near_truth(&p.k, p.x, p.y, *seq);
                        let w = (*seq as usize / 300).min(4);
                        janela[w].1 += 1;
                        janela[w].0 += ok as usize;
                        let e = por_como.entry(p.s.clone()).or_default();
                        e.1 += 1;
                        e.0 += ok as usize;
                    }
                }
            }
            let f = |(a, b): &(usize, usize)| if *b == 0 { "-".to_string() } else { format!("{:.0}%", *a as f64 * 100.0 / *b as f64) };
            eprintln!("semente {seed}: por 60 s {:?}  por como {:?}", janela.iter().map(f).collect::<Vec<_>>(), por_como.iter().map(|(k, v)| (k.clone(), f(v), v.1)).collect::<Vec<_>>());
        }
    }

    fn rodar_com(demo: &Demo, segundos: f64, cfg: Settings) -> Vec<ToBot> {
        let mut r = Runner::new(demo.lol(), demo.capture(), demo.icons(), Sink::default(), cfg);
        let mut t = 0.0;
        while t < segundos * 1000.0 {
            r.tick(t);
            t += 50.0;
        }
        r.out.0
    }

    #[test]
    fn com_a_depuracao_ligada_grava_o_pacote_e_sem_ela_nao_grava_nada() {
        let base = std::env::temp_dir().join(format!("olheiro-pacote-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let d = Demo::new(3, "Kennzy#BR1", false);
        rodar_com(&d, 8.0, Settings { debug: false, debug_dir: Some(base.clone()), ..Settings::default() });
        assert!(!base.join("debug").exists(), "desligada por padrão: nada é gravado");
        rodar_com(&Demo::new(3, "Kennzy#BR1", false), 14.0, Settings { debug: true, debug_dir: Some(base.clone()), ..Settings::default() });
        let pacote = std::fs::read_dir(base.join("debug")).unwrap().next().unwrap().unwrap().path();
        let meta: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(pacote.join("meta.json")).unwrap()).unwrap();
        assert_eq!((meta["region"]["w"].as_i64(), meta["map"].as_i64(), meta["allies"].as_array().map(|a| a.len())), (Some(341), Some(11), Some(5)));
        let linhas: Vec<serde_json::Value> = std::fs::read_to_string(pacote.join("frames.jsonl")).unwrap().lines().map(|l| serde_json::from_str(l).unwrap()).collect();
        assert!(linhas.len() >= 5, "um quadro a cada 2 s por 14 s: {}", linhas.len());
        assert!(linhas.iter().all(|l| pacote.join(l["file"].as_str().unwrap()).exists()));
        assert!(linhas.last().unwrap()["pos"].as_array().unwrap().iter().all(|p| p["key"].is_string()), "cada posição diz de quem é");
        let _ = std::fs::remove_dir_all(&base);
    }

    /// Gera um pacote de exemplo para a ferramenta `reproduzir` (não roda no `cargo test`):
    /// `OLHEIRO_PACOTE_SAIDA=/tmp/x cargo test -p olheiro gera_um_pacote_de_exemplo -- --ignored`
    #[test]
    #[ignore]
    fn gera_um_pacote_de_exemplo() {
        let out = std::path::PathBuf::from(std::env::var("OLHEIRO_PACOTE_SAIDA").expect("defina OLHEIRO_PACOTE_SAIDA"));
        let _ = std::fs::remove_dir_all(&out);
        rodar_com(&Demo::new(2, "Kennzy#BR1", false).speed(6.0), 130.0, Settings { debug: true, debug_dir: Some(out), ..Settings::default() });
    }

    #[test]
    fn a_calibracao_diz_que_esta_tudo_certo_com_um_minimapa_normal() {
        let msgs = rodar(&Demo::new(3, "Kennzy#BR1", false), 10.0);
        let c = msgs.iter().rev().find_map(|m| if let ToBot::Status { calib: Some(c), .. } = m { Some(c.clone()) } else { None }).unwrap();
        assert!(c.ok && c.problem.is_empty() && c.minimap_px == 341 && c.icon_px == 30, "{c:?}");
    }

    #[test]
    fn minimapa_pequeno_avisa_mas_continua() {
        let msgs = rodar(&Demo::with(3, "Kennzy#BR1", false, 768, 0.3), 10.0);
        let c = msgs.iter().rev().find_map(|m| if let ToBot::Status { calib: Some(c), .. } = m { Some(c.clone()) } else { None }).unwrap();
        assert!(c.ok && c.problem.contains("pequeno") && c.icon_px < 14, "{c:?}");
        assert!(msgs.iter().any(|m| matches!(m, ToBot::Frame { .. })));
    }

    #[test]
    fn quem_roda_o_app_e_achado_mesmo_no_modo_streamer() {
        // no modo streamer `summonerName` vira o nome do campeão; o Riot ID verdadeiro está nos campos novos
        let msgs = rodar(&Demo::new(3, "Kennzy#BR1", false).streamer_mode(), 5.0);
        assert!(msgs.iter().any(|m| matches!(m, ToBot::Roster { you, .. } if you == "Kennzy#BR1")));
    }

    #[test]
    fn ao_reconectar_o_bot_recebe_de_novo_o_status_e_o_elenco() {
        let d = Demo::new(3, "Kennzy#BR1", false);
        let mut r = Runner::new(d.lol(), d.capture(), d.icons(), Sink::default(), Settings::default());
        let mut t = 0.0;
        while t < 3000.0 {
            r.tick(t);
            t += 50.0;
        }
        r.out.0.clear();
        r.on_connected();
        let tipos: Vec<&str> = r.out.0.iter().map(|m| match m { ToBot::Status { .. } => "status", ToBot::Roster { .. } => "roster", _ => "?" }).collect();
        assert_eq!(tipos, ["status", "roster"]);
    }

    #[test]
    fn quando_a_partida_acaba_o_bot_e_avisado_e_o_elenco_some() {
        let d = Demo::new(3, "Kennzy#BR1", false);
        let mut r = Runner::new(d.lol(), d.capture(), d.icons(), Sink::default(), Settings::default());
        let mut t = 0.0;
        while t < 5000.0 {
            r.tick(t);
            t += 50.0;
        }
        assert!(r.in_game());
        d.end_game();
        while t < 12_000.0 {
            r.tick(t);
            t += 50.0;
        }
        assert!(!r.in_game(), "três checagens sem partida encerram");
        let ultimo = r.out.0.iter().rev().find(|m| matches!(m, ToBot::Status { .. })).unwrap();
        assert!(matches!(ultimo, ToBot::Status { phase: Phase::Client, .. }), "{ultimo:?}");
        r.out.0.clear();
        r.on_connected();
        assert!(!r.out.0.iter().any(|m| matches!(m, ToBot::Roster { .. })), "sem partida não se reenvia elenco velho");
    }

    #[test]
    fn mapa_sem_conversao_nao_manda_posicao_inventada() {
        let d = Demo::new(3, "Kennzy#BR1", false).with_map(30); // Arena
        let msgs = rodar(&d, 6.0);
        assert!(!msgs.iter().any(|m| matches!(m, ToBot::Roster { .. } | ToBot::Frame { .. })));
        assert!(msgs.iter().any(|m| matches!(m, ToBot::Status { calib: Some(c), .. } if !c.ok && c.problem.contains("Summoner"))));
    }

    #[test]
    fn sem_o_icone_de_alguem_nao_se_arrisca_nomear() {
        let d = Demo::new(3, "Kennzy#BR1", false).missing_icons(1);
        let msgs = rodar(&d, 6.0);
        assert!(!msgs.iter().any(|m| matches!(m, ToBot::Frame { .. })), "sem todos os ícones o ícone de um poderia passar pelo de outro");
        assert!(msgs.iter().any(|m| matches!(m, ToBot::Status { calib: Some(c), .. } if c.problem.contains("ícones"))));
    }

    #[test]
    fn janela_do_jogo_fora_do_lugar_diz_o_que_fazer() {
        let d = Demo::new(3, "Kennzy#BR1", false).without_window();
        let msgs = rodar(&d, 6.0);
        assert!(msgs.iter().any(|m| matches!(m, ToBot::Status { calib: Some(c), .. } if c.problem.contains("Janela sem borda"))));
        assert!(!msgs.iter().any(|m| matches!(m, ToBot::Frame { .. })));
    }
}

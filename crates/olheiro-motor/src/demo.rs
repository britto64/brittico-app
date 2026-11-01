//! Um LoL de mentira para o app rodar sem Windows e sem jogo: o simulador do núcleo faz de jogo, de tela e de Community Dragon.
//!
//! Serve à "partida simulada" da janela (ver tudo funcionar sem abrir o LoL, até a sala de voz) e aos testes do laço.
//! O que o "jogo" devolve é o MESMO JSON que a API local devolve, passando pelo mesmo leitor (`lol::parse_game`): o caminho que roda
//! no Windows é o que se testa aqui, só a fonte dos dados é outra.

use crate::lol::{self, GameInfo, LivePlayer};
use crate::session::{Capture, Icons, Lol};
use olheiro_core::geometry::{icon_diam, minimap_to_game, MapKind};
use olheiro_core::protocol::Phase;
use olheiro_core::sim::{synthetic_art, Art, Rng, SimConfig, SimGame};
use olheiro_core::templates::icon_templates;
use olheiro_core::{Frame, Rect};
use serde_json::{json, Value};
use std::cell::RefCell;
use std::rc::Rc;

const ALLIES: [&str; 5] = ["Mylon", "Ana", "Bia", "Caio", "Duda"];
const ENEMIES: [&str; 5] = ["Zed", "Lia", "Rui", "Caco", "Beto"];

struct Inner {
    sim: SimGame,
    cfg: SimConfig,
    /// Riot IDs completos (`Nome#BR1`): os cinco aliados (o primeiro é quem roda o app) e depois os cinco inimigos.
    ids: Vec<String>,
    aliases: Vec<String>,
    arts: Vec<Art>,
    custom: bool,
    streamer: bool,
    map_number: i64,
    drop_icons: usize,
    window_missing: bool,
    over: bool,
    /// Quantas vezes mais rápido que o relógio a partida anda (a demonstração; os testes usam 1).
    speed: f64,
    /// A cada quadro capturado: onde cada aliado estava, em unidades do jogo.
    history: Vec<Vec<(String, f64, f64)>>,
    /// Qual dos cinco aliados é quem roda o app (0 no `Demo::new`).
    me: usize,
}

#[derive(Clone)]
pub struct Demo(Rc<RefCell<Inner>>);

pub struct DemoLol(Rc<RefCell<Inner>>);
pub struct DemoCapture(Rc<RefCell<Inner>>);
pub struct DemoIcons(Rc<RefCell<Inner>>);

fn load_png(path: &std::path::Path) -> Option<Art> {
    let img = image::open(path).ok()?.to_rgba8();
    Some(Art { w: img.width() as usize, h: img.height() as usize, rgba: img.into_raw() })
}

/// A arte dos dez campeões: os ícones de verdade (se `fixtures/icons` existir, ver `scripts/fetch-icons.sh`) ou retratos inventados.
fn arts(seed: u64) -> (Vec<String>, Vec<Art>) {
    let dir = std::env::var("OLHEIRO_FIXTURES")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/icons"));
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .map(|d| d.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "png")).collect())
        .unwrap_or_default();
    files.sort();
    if files.len() >= 10 {
        let mut rng = Rng::new(seed);
        let mut idx: Vec<usize> = (0..files.len()).collect();
        for i in (1..idx.len()).rev() {
            idx.swap(i, rng.next_u64() as usize % (i + 1));
        }
        let picked: Vec<_> = idx[..10].iter().filter_map(|&i| load_png(&files[i]).map(|a| (files[i].file_stem().unwrap().to_string_lossy().to_string(), a))).collect();
        if picked.len() == 10 {
            return picked.into_iter().unzip();
        }
    }
    ((0..10).map(|i| format!("demo{i}")).collect(), (0..10).map(|i| synthetic_art(seed * 31 + i, 128)).collect())
}

impl Demo {
    pub fn new(seed: u64, you: &str, custom: bool) -> Demo {
        Demo::with(seed, you, custom, 1080, 1.92)
    }

    pub fn with(seed: u64, you: &str, custom: bool, window_h: i32, scale: f64) -> Demo {
        let (aliases, arts) = arts(seed);
        let cfg = SimConfig { seed, window_h, minimap_scale: scale, ..SimConfig::default() };
        let sim = SimGame::new(cfg.clone(), arts[..5].to_vec(), arts[5..].to_vec(), None);
        let tag = |n: &str| if n.contains('#') { n.to_string() } else { format!("{n}#BR1") };
        let mut ids = vec![tag(you)];
        ids.extend(ALLIES[..4].iter().map(|n| tag(n)));
        ids.extend(ENEMIES.iter().map(|n| tag(n)));
        Demo(Rc::new(RefCell::new(Inner {
            sim, cfg, ids, aliases, arts, custom, streamer: false, map_number: 11, drop_icons: 0, window_missing: false, over: false, speed: 1.0, history: Vec::new(), me: 0,
        })))
    }

    pub fn lol(&self) -> DemoLol {
        DemoLol(self.0.clone())
    }
    pub fn capture(&self) -> DemoCapture {
        DemoCapture(self.0.clone())
    }
    pub fn icons(&self) -> DemoIcons {
        DemoIcons(self.0.clone())
    }

    /// O modo streamer do LoL: `summonerName` vira o nome do campeão na máquina de quem transmite.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn streamer_mode(self) -> Demo {
        self.0.borrow_mut().streamer = true;
        self
    }
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn with_map(self, n: i64) -> Demo {
        self.0.borrow_mut().map_number = n;
        self
    }
    /// Os ícones dos últimos `n` aliados "não baixam".
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn missing_icons(self, n: usize) -> Demo {
        self.0.borrow_mut().drop_icons = n;
        self
    }
    /// A partida anda `x` vezes mais rápido que o relógio (os jogadores se espalham logo): só para ver a demonstração.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn speed(self, x: f64) -> Demo {
        self.0.borrow_mut().speed = x.clamp(1.0, 30.0);
        self
    }
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn without_window(self) -> Demo {
        self.0.borrow_mut().window_missing = true;
        self
    }
    /// Um time fixo (Duda, Mylon, Ana, Bia e Caio, todos `#BR1`) em que você é o aliado `vaga` (0 a 4): várias cópias do app na mesma
    /// partida simulada, cada uma como um jogador, caem na mesma sala de voz.
    pub fn time_fixo(seed: u64, vaga: usize) -> Demo {
        let d = Demo::new(seed, "Duda#BR1", false);
        d.0.borrow_mut().me = vaga.min(4);
        d
    }

    /// `(alias, arte)` dos dez campeões, para a tela desenhar os ícones.
    pub fn artes(&self) -> Vec<(String, Art)> {
        let inner = self.0.borrow();
        inner.aliases.iter().cloned().zip(inner.arts.iter().cloned()).collect()
    }

    /// A partida acaba: o cliente continua aberto.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn end_game(&self) {
        self.0.borrow_mut().over = true;
    }

    /// As chaves (normalizadas) dos cinco aliados, na ordem do time.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn ally_keys(&self) -> Vec<String> {
        self.0.borrow().ids[..5].iter().map(|i| i.to_lowercase()).collect()
    }

    /// A posição informada para o quadro `seq` está perto de onde o aliado estava de verdade (a 60% do diâmetro de um ícone)?
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn near_truth(&self, key: &str, x: f64, y: f64, seq: u64) -> bool {
        let inner = self.0.borrow();
        let Some(frame) = seq.checked_sub(1).and_then(|i| inner.history.get(i as usize)) else { return false };
        let Some((_, tx, ty)) = frame.iter().find(|(k, ..)| k == key) else { return false };
        let region = inner.sim.region();
        let tol = icon_diam(region.w) * 0.6 / region.w as f64 * MapKind::SummonersRift.dims().0;
        (x - tx).hypot(y - ty) <= tol
    }
}

fn player_json(inner: &Inner, i: usize) -> Value {
    let id = &inner.ids[i];
    let (name, tag) = id.split_once('#').unwrap();
    let alias = &inner.aliases[i];
    let champion = alias.to_uppercase();
    let dead = i < 5 && inner.sim.dead_ids().contains(&i);
    json!({
        "championName": champion, "rawChampionName": format!("game_character_displayname_{alias}"),
        "isBot": false, "isDead": dead, "respawnTimer": if dead { 12.0 } else { 0.0 }, "level": 5, "skinID": 0,
        "team": if i < 5 { "ORDER" } else { "CHAOS" },
        "riotId": id, "riotIdGameName": name, "riotIdTagLine": tag,
        // o modo streamer troca o nome mostrado pelo do campeão; os campos riotId* continuam verdadeiros
        "summonerName": if inner.streamer { champion } else { name.to_string() }
    })
}

impl Lol for DemoLol {
    fn phase(&mut self) -> Phase {
        if self.0.borrow().over { Phase::Client } else { Phase::Ingame }
    }

    fn client_riot_id(&mut self) -> Option<String> {
        let inner = self.0.borrow();
        Some(inner.ids[inner.me].clone())
    }

    fn game(&mut self) -> Option<GameInfo> {
        let inner = self.0.borrow();
        let all: Vec<Value> = (0..10).map(|i| player_json(&inner, i)).collect();
        let me = inner.me;
        let (name, tag) = inner.ids[me].split_once('#').unwrap();
        let data = json!({
            "activePlayer": { "riotId": inner.ids[me], "riotIdGameName": name, "riotIdTagLine": tag, "summonerName": if inner.streamer { inner.aliases[me].to_uppercase() } else { name.to_string() } },
            "allPlayers": all,
            "gameData": { "gameMode": "CLASSIC", "gameTime": 0.0, "mapName": format!("Map{}", inner.map_number), "mapNumber": inner.map_number }
        });
        lol::parse_game(&data)
    }

    fn players(&mut self) -> Option<Vec<LivePlayer>> {
        let inner = self.0.borrow();
        let list = Value::Array((0..10).map(|i| player_json(&inner, i)).collect());
        Some(lol::parse_players(&list))
    }

    fn queue(&mut self) -> Option<(i64, bool)> {
        Some(if self.0.borrow().custom { (0, true) } else { (440, false) })
    }

    fn minimap_scale(&mut self) -> Option<f64> {
        Some(self.0.borrow().cfg.minimap_scale)
    }
}

impl Capture for DemoCapture {
    fn window(&mut self) -> Option<Rect> {
        let inner = self.0.borrow();
        (!inner.window_missing).then(|| inner.sim.window())
    }

    /// Cada quadro anda a partida 0,2 s (5 Hz) e guarda onde cada aliado estava, para o teste conferir.
    fn grab(&mut self, _bounds: Rect) -> Result<Frame, String> {
        let mut inner = self.0.borrow_mut();
        let dt = 0.2 * inner.speed;
        inner.sim.step(dt);
        let frame = inner.sim.frame();
        let region = inner.sim.region();
        let truth: Vec<(String, f64, f64)> = inner
            .sim
            .truth()
            .iter()
            .map(|t| {
                let (gx, gy) = minimap_to_game(t.x, t.y, region.w as f64, region.h as f64, MapKind::SummonersRift);
                (inner.ids[t.id].to_lowercase(), gx, gy)
            })
            .collect();
        inner.history.push(truth);
        Ok(frame)
    }
}

impl Icons for DemoIcons {
    fn load(&mut self, champs: &[(usize, String)]) -> Vec<(usize, Vec<Vec<f32>>)> {
        let inner = self.0.borrow();
        let keep = champs.len().saturating_sub(inner.drop_icons);
        champs.iter().take(keep).map(|(i, _)| (*i, icon_templates(&inner.arts[*i].rgba, inner.arts[*i].w, inner.arts[*i].h))).collect()
    }
}

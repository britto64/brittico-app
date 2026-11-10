//! O olheiro: dado um quadro do minimapa por vez, diz onde cada aliado está.
//!
//! Porte do Camera mode do LoLProxChat (`camera-hearing.ts`), sem o retângulo de câmera e só para o time aliado. Nada aqui precisa
//! saber qual jogador é "eu": o que fazia o rastreio do original errar (uma skin, um colega por cima, uma torre, e ele seguia o
//! ícone errado) não existe, porque o alvo é todo mundo. Precisa de três coisas do minimapa:
//!
//! - cada ícone aliado, pela cor do anel;
//! - de quem é cada ícone, contra o ícone BASE dos campeões do time (o minimapa desenha o base, qualquer que seja a skin);
//! - para onde cada um foi, de um quadro para o outro.
//!
//! Os ícones são acompanhados (rastros), então um ícone nomeado uma vez mantém o nome enquanto passa por baixo de outro. Ninguém
//! some sem motivo: um aliado vivo é sempre desenhado, então quem some ao lado de outro aliado está embaixo dele e vai aonde ele
//! for; só a morte (a API do jogo diz) tira um aliado do mapa. O que continua sem nome se resolve por eliminação sobre o mapa
//! todo: cada ícone desconhecido é no máximo um jogador, vivo, que não esteja em outro lugar e que possa ter andado até ali.

use crate::blobs::{classify_icon_blobs, create_mask, find_blobs, ring_weight, single_icon_flags, Blob, Ring};
use crate::frame::Frame;
use crate::geometry::{icon_diam, Rect};
use crate::identify::identify_icon;
use crate::rings::{scan_region, Integral};
use crate::templates::{decide_match_with, MatchScore, TemplateSet, MATCH_MIN_MARGIN, MATCH_MIN_SCORE};
use crate::PlayerId;
use std::collections::{HashMap, HashSet};

/// Um aliado que o olheiro procura: o número dele no elenco e os modelos do ícone base do campeão.
#[derive(Clone, Debug)]
pub struct ScoutPlayer {
    pub id: PlayerId,
    pub templates: TemplateSet,
}

/// Como o olheiro sabe onde o jogador está.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum How {
    /// O ícone dele foi visto e reconhecido agora há pouco.
    Seen,
    /// O ícone dele está debaixo do de outro aliado: vai aonde o outro for.
    Under,
    /// Um ícone sem nome que só pode ser dele (eliminação).
    Inferred,
    /// Não aparece agora; a última posição vista.
    Held,
}

#[derive(Clone, Copy, Debug)]
pub struct Placement {
    pub id: PlayerId,
    /// Em pixels da região do minimapa (origem no canto de cima, à esquerda).
    pub x: f64,
    pub y: f64,
    pub how: How,
}

/// De quanto em quanto tempo os ícones são comparados com os jogadores. O original usa 250 ms; a captura do olheiro é a 5 Hz, então
/// o limite fica abaixo do período dela para não pular todo quadro par.
pub const MIN_IDENTIFY_INTERVAL_MS: f64 = 150.0;
/// Um ícone nomeado há menos que isso não é comparado de novo.
pub const RECHECK_MS: f64 = 2000.0;
/// No máximo tantos ícones avulsos comparados por quadro (~10 ms cada no original).
pub const MATCH_BUDGET: usize = 4;
/// No máximo tantas manchas fundidas dissecadas por quadro (é a parte cara).
pub const BLOB_BUDGET: usize = 2;
/// Um rastro que não aparece há tanto tempo é esquecido.
pub const TRACK_TTL_MS: f64 = 1000.0;
/// Quanto um ícone pode andar entre dois quadros e continuar o mesmo rastro, em diâmetros de ícone.
pub const TRACK_REACH_DIAMS: f64 = 0.9;
/// Um nome vivo num rastro longe só muda para um ícone novo com uma comparação tão forte: campeão não atravessa o mapa em um quarto de segundo.
pub const MOVE_NAME_MIN_SCORE: f32 = 0.8;
/// Um anel com tanto assim à vista está por cima de tudo: dá para compará-lo com o ícone do campeão. Menos que isso, parte do miolo
/// é de outro ícone e a nota enganaria.
pub const FULL_RING_VISIBLE: f64 = 0.92;
/// Um anel parcialmente coberto (à vista entre estes limites) também é comparado com os campeões, mas só vale se a nota for ALTA e clara: parte do miolo é
/// de outro ícone, então o mínimo comum (0,6) deixaria passar um nome errado. Nos quadros reais um aliado com 82% do anel à vista saiu com nota 0,94.
const PARTIAL_MIN_VISIBLE: f64 = 0.55;
const PARTIAL_MIN_SCORE: f32 = 0.8;
const PARTIAL_MIN_MARGIN: f32 = 0.25;
/// O halo e o retângulo da câmera em volta do ícone do jogador deixam, depois de o anel inteiro ser achado, "anéis fantasma" parciais a 4 ou 5 px dele (100% de
/// acerto, mas só uma lua crescente à vista). Dois aliados de verdade um sobre o outro deixam a mesma lua, então o anel NÃO é descartado (o simulador, que não
/// desenha halo, perdeu ~0,7 ponto quando era). O estrago era outro: a eliminação entregava a lua a uma aliada perdida que estava do outro lado do mapa (a
/// Morgana apareceu em cima do Maokai). Um anel parcial a menos disto (em diâmetros do ícone) de um aliado já nomeado só vai para quem foi visto bem perto.
const STACKED_RING_DIAMS: f64 = 0.3;
/// No máximo tantos anéis procurados na região por quadro (um time tem cinco; a folga absorve um falso achado).
const MAX_SCANNED_RINGS: usize = 8;
/// Fração mínima de pontos do círculo que têm de ser anel. O minimapa de verdade tem sombra e degradê que interrompem o anel (um ícone inteiro, na fonte,
/// acerta ~80%); um desenho que não é anel (torre, inibidor, tropa) fica muito abaixo.
const SCAN_MIN_HIT: f64 = 0.65;
/// Um anel que só aparece em parte (coberto por outro) precisa fechar melhor o círculo: uma meia-lua que acerta pouco é sombra, não ícone.
const PARTIAL_MIN_FIT: f64 = 0.75;
/// Quanto um campeão anda no minimapa por segundo, em diâmetros de ícone (~13 px/s num minimapa de 341 px, ~580 unidades/s)...
pub const REACH_DIAMS_PER_SEC: f64 = 0.45;
/// ...mais um Flash e um dash por cima.
pub const REACH_SLACK_DIAMS: f64 = 1.5;
/// Quem some tão perto (em diâmetros) de outro aliado está embaixo dele, não foi embora.
pub const HIDDEN_NEAR_DIAMS: f64 = 1.2;
/// Um aliado em nenhum ícone é posto embaixo do aliado mais perto de onde foi visto, se foi visto há tão pouco.
pub const ALLY_UNPLACED_MS: f64 = 3000.0;
/// Por quanto tempo a última posição vista de quem não aparece ainda vale como "retida".
pub const HELD_MAX_MS: f64 = 20_000.0;
/// Recolher leva ao fogo de base (cantos do mapa); um ícone novo ali pode ser de quem quer que tenha acabado de recolher.
const BASE_ZONE_FRACTION: f64 = 0.2;

/// Os limiares da leitura que a pessoa pode mexer. Um só botão, de 0 (rígido: só aceita o que tem certeza) a 1 (sensível: aceita mais, e erra mais);
/// 0,5 é o ajuste de fábrica, o mesmo de antes de existir o botão.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tuning {
    /// Fração mínima de pontos do círculo que têm de ser anel.
    pub scan_min_hit: f64,
    /// Nota e margem mínimas para nomear um ícone inteiro.
    pub match_min_score: f32,
    pub match_min_margin: f32,
    /// Idem, para um ícone coberto em parte.
    pub partial_min_score: f32,
    pub partial_min_margin: f32,
}

impl Tuning {
    pub const DEFAULT_SENSITIVITY: f32 = 0.5;

    pub fn from_sensitivity(s: f32) -> Tuning {
        let d = (s.clamp(0.0, 1.0) - Self::DEFAULT_SENSITIVITY) as f64;
        let f = d as f32;
        Tuning {
            scan_min_hit: SCAN_MIN_HIT - d * 0.30,
            match_min_score: MATCH_MIN_SCORE - f * 0.27,
            match_min_margin: MATCH_MIN_MARGIN - f * 0.18,
            partial_min_score: PARTIAL_MIN_SCORE - f * 0.18,
            partial_min_margin: PARTIAL_MIN_MARGIN - f * 0.18,
        }
    }
}

impl Default for Tuning {
    fn default() -> Self {
        Tuning::from_sensitivity(Self::DEFAULT_SENSITIVITY)
    }
}

#[derive(Clone, Debug)]
struct Obs {
    x: f64,
    y: f64,
    id: Option<PlayerId>,
    scores: Option<Vec<MatchScore>>,
}

impl Obs {
    fn score_of(&self, id: PlayerId) -> Option<f32> {
        self.scores.as_ref()?.iter().find(|m| m.id == id).map(|m| m.score)
    }
}

#[derive(Clone, Debug)]
struct Track {
    uid: u64,
    x: f64,
    y: f64,
    id: Option<PlayerId>,
    seen_ms: f64,
    /// Quando um ícone foi de fato observado para ele (`seen_ms` só se mexe também quando ele está coberto).
    observed_ms: f64,
    /// Quando o ícone foi comparado pela última vez (nomeado ou não).
    checked_ms: f64,
    scores: Option<Vec<MatchScore>>,
    born_ms: f64,
}

impl Track {
    fn score_of(&self, id: PlayerId) -> Option<f32> {
        self.scores.as_ref()?.iter().find(|m| m.id == id).map(|m| m.score)
    }
}

/// Um aliado escondido embaixo de um ícone: acompanha ele até aparecer de novo.
#[derive(Clone, Copy, Debug)]
struct Rider {
    host: u64,
    hx: f64,
    hy: f64,
    since_ms: f64,
}

pub struct Scout {
    players: Vec<ScoutPlayer>,
    sets: Vec<TemplateSet>,
    tracks: Vec<Track>,
    next_uid: u64,
    last_identify_ms: f64,
    last_seen: HashMap<PlayerId, (f64, f64, f64)>,
    dead: HashSet<PlayerId>,
    riders: HashMap<PlayerId, Rider>,
    prev_named: HashSet<PlayerId>,
    icon_diam: f64,
    region: Rect,
    map_w: f64,
    map_h: f64,
    /// Quantos anéis de aliado o último quadro tinha: a calibração diz "não estou vendo o minimapa" quando é zero por muito tempo.
    rings: usize,
    tuning: Tuning,
}

impl Default for Scout {
    fn default() -> Self {
        Scout::new()
    }
}

impl Scout {
    pub fn new() -> Self {
        Scout {
            players: Vec::new(),
            sets: Vec::new(),
            tracks: Vec::new(),
            next_uid: 1,
            last_identify_ms: f64::NEG_INFINITY,
            last_seen: HashMap::new(),
            dead: HashSet::new(),
            riders: HashMap::new(),
            prev_named: HashSet::new(),
            icon_diam: 30.0,
            region: Rect::new(0, 0, 0, 0),
            map_w: 14870.0,
            map_h: 14980.0,
            rings: 0,
            tuning: Tuning::default(),
        }
    }

    /// A sensibilidade da detecção, de 0 (rígida) a 1 (sensível). Vale já no próximo quadro.
    pub fn set_sensitivity(&mut self, s: f32) {
        self.tuning = Tuning::from_sensitivity(s);
    }

    /// Quantos anéis de aliado o último quadro analisado tinha.
    pub fn rings_seen(&self) -> usize {
        self.rings
    }

    /// O elenco aliado da partida. Zera tudo o que foi aprendido.
    pub fn set_players(&mut self, players: Vec<ScoutPlayer>) {
        self.sets = players.iter().map(|p| p.templates.clone()).collect();
        self.players = players;
        self.tracks.clear();
        self.last_seen.clear();
        self.riders.clear();
        self.prev_named.clear();
    }

    /// Quem o jogo diz que está morto agora: sem ícone, e nunca o ícone desconhecido de ninguém.
    pub fn set_dead(&mut self, ids: impl IntoIterator<Item = PlayerId>) {
        self.dead = ids.into_iter().collect();
    }

    pub fn has_players(&self) -> bool {
        !self.players.is_empty()
    }

    pub fn reset(&mut self) {
        self.tracks.clear();
        self.last_seen.clear();
        self.riders.clear();
        self.prev_named.clear();
        self.last_identify_ms = f64::NEG_INFINITY;
    }

    /// Um quadro capturado. `region` diz onde o minimapa está dentro de `frame`; `now_ms` é um relógio qualquer que só cresce.
    pub fn on_frame(&mut self, frame: &Frame, region: &Rect, now_ms: f64) {
        self.icon_diam = icon_diam(region.w).max(8.0);
        self.region = *region;
        if now_ms - self.last_identify_ms < MIN_IDENTIFY_INTERVAL_MS {
            return;
        }
        self.last_identify_ms = now_ms;
        let (w, h) = (region.w as usize, region.h as usize);
        let mask = create_mask(frame, region);
        let blobs: Vec<Blob> = find_blobs(&mask, w, h).into_iter().filter(|b| b.ring == Ring::Ally).collect();
        let singles = single_icon_flags(&blobs, self.icon_diam);
        let (obs, rings) = self.observe(frame, region, &mask, &blobs, &singles, now_ms);
        self.rings = rings;
        self.ingest(obs, now_ms);
    }

    /// Os ícones deste quadro. Os anéis são achados pela GEOMETRIA, na região inteira (rings.rs): um círculo do tamanho de um ícone, mesmo quebrado pela
    /// sombra do mapa ou parcialmente coberto por outro ícone. Foi o que o minimapa de verdade exigiu: lá a cor de aliado também pinta torres, inibidores e
    /// tropas (27 manchas por quadro) e o anel do campeão chega em pedaços, que nenhuma "mancha do tamanho de um ícone" reconhece. O ícone inteiro é
    /// comparado com o dos campeões (na ordem do orçamento); o parcialmente coberto entra só com a posição. Devolve também quantos anéis viu.
    fn observe(&self, frame: &Frame, region: &Rect, mask: &[u8], blobs: &[Blob], singles: &[bool], now_ms: f64) -> (Vec<Obs>, usize) {
        let diam = self.icon_diam;
        let (w, h) = (region.w as usize, region.h as usize);
        let ring_px = ring_weight(blobs, singles, diam);
        let (lone, _merged) = classify_icon_blobs(blobs, singles, diam, ring_px);
        let mut obs = Vec::new();

        // Um ponto para comparar com o ícone de um campeão: o centro de um anel por cima dos outros.
        struct Spot {
            x: f64,
            y: f64,
            strict: bool,
            /// Coberto em parte: só é nomeado com nota alta (ver `PARTIAL_MIN_SCORE`).
            partial: bool,
        }
        let mut spots: Vec<Spot> = Vec::new();
        let integral = Integral::new(mask, w, h);
        let whole = Blob { ring: Ring::Ally, pixels: 0, cx: 0.0, cy: 0.0, min_x: 0, max_x: region.w - 1, min_y: 0, max_y: region.h - 1, fill: 0.1 };
        let (found, ring_diam) = scan_region(mask, w, h, &whole, diam, MAX_SCANNED_RINGS, self.tuning.scan_min_hit, &integral);
        let mut rings = 0;
        for f in &found {
            if f.visible >= FULL_RING_VISIBLE {
                rings += 1;
                spots.push(Spot { x: f.x, y: f.y, strict: true, partial: false });
            } else if f.fit >= PARTIAL_MIN_FIT {
                rings += 1;
                if f.visible >= PARTIAL_MIN_VISIBLE {
                    spots.push(Spot { x: f.x, y: f.y, strict: true, partial: true });
                } else {
                    obs.push(Obs { x: f.x, y: f.y, id: None, scores: None });
                }
            }
        }
        // O que a geometria não pega: um ícone cuja ARTE tem a cor do anel (a Gwen de cabelo verde-água) enche o miolo e não é oco. Só vale se for reconhecido.
        for l in lone.iter().filter(|l| !l.strict) {
            let b = &blobs[l.blob];
            if found.iter().any(|f| (f.x - b.cx).hypot(f.y - b.cy) < diam * 0.6) {
                continue;
            }
            spots.push(Spot { x: b.cx, y: b.cy, strict: false, partial: false });
        }

        struct Q {
            spot: usize,
            strict: bool,
            must: bool,
            need: bool,
            prio: u8,
            checked: f64,
        }
        let mut queue: Vec<Q> = spots
            .iter()
            .enumerate()
            .map(|(k, sp)| {
                let prev = self.nearest_track(sp.x, sp.y, now_ms);
                let since = prev.map_or(f64::INFINITY, |t| now_ms - t.checked_ms);
                let named = prev.is_some_and(|t| t.id.is_some());
                let stale = !named || since >= RECHECK_MS;
                let must = !sp.strict && !named;
                Q {
                    spot: k,
                    strict: sp.strict,
                    must,
                    need: stale || must,
                    // quem ainda não tem nome primeiro; depois o comparado há mais tempo
                    prio: if named { 1 } else { 0 },
                    checked: prev.map_or(f64::NEG_INFINITY, |t| t.checked_ms),
                }
            })
            .collect();
        queue.sort_by(|a, b| a.prio.cmp(&b.prio).then(a.checked.partial_cmp(&b.checked).unwrap_or(std::cmp::Ordering::Equal)));
        let mut budget = MATCH_BUDGET;
        for q in &queue {
            let sp = &spots[q.spot];
            let mut o = Obs { x: sp.x, y: sp.y, id: None, scores: None };
            if q.need && budget > 0 && !self.sets.is_empty() {
                budget -= 1;
                let ranked = identify_icon(frame, region.x as f64 + sp.x, region.y as f64 + sp.y, ring_diam, &self.sets);
                o.id = if sp.partial { decide_partial(&ranked, &self.tuning) } else { decide_match_with(&ranked, self.tuning.match_min_score, self.tuning.match_min_margin) };
                o.scores = Some(ranked);
            }
            // Um ícone que o filtro recusou por cheio demais (arte na cor do anel, ou minions) só vale se foi reconhecido.
            if q.must && o.id.is_none() && !q.strict {
                continue;
            }
            obs.push(o);
        }

        (obs, rings)
    }

    fn nearest_track(&self, x: f64, y: f64, now_ms: f64) -> Option<&Track> {
        let mut best: Option<&Track> = None;
        let mut best_d = self.icon_diam * TRACK_REACH_DIAMS;
        for t in &self.tracks {
            if now_ms - t.seen_ms > TRACK_TTL_MS {
                continue;
            }
            let d = (t.x - x).hypot(t.y - y);
            if d <= best_d {
                best_d = d;
                best = Some(t);
            }
        }
        best
    }

    fn track(&self, uid: u64) -> Option<&Track> {
        self.tracks.iter().find(|t| t.uid == uid)
    }

    /// Junta as observações de um quadro aos rastros.
    fn ingest(&mut self, obs: Vec<Obs>, now_ms: f64) {
        let reach = self.icon_diam * TRACK_REACH_DIAMS;
        self.tracks.retain(|t| now_ms - t.seen_ms <= TRACK_TTL_MS);
        let mut free: Vec<u64> = self.tracks.iter().map(|t| t.uid).collect();

        // Os nomeados primeiro, para o nome cair no rastro a que pertence.
        let mut ordered: Vec<Obs> = obs
            .into_iter()
            .map(|mut o| {
                if let Some(id) = o.id {
                    // Um nome vivo num rastro longe daqui fica lá, a menos que a comparação seja forte.
                    let elsewhere = self.tracks.iter().any(|t| t.id == Some(id) && (t.x - o.x).hypot(t.y - o.y) > reach * 2.0);
                    if elsewhere && o.score_of(id).unwrap_or(f32::NEG_INFINITY) < MOVE_NAME_MIN_SCORE {
                        o.id = None;
                    }
                }
                o
            })
            .collect();
        ordered.sort_by_key(|o| u8::from(o.id.is_none()));

        for o in ordered {
            // O rastro que este ícone continua: o que já leva o nome dele, se estiver perto; senão o livre mais perto.
            let mut chosen: Option<u64> = None;
            if let Some(id) = o.id {
                chosen = free
                    .iter()
                    .copied()
                    .find(|&u| self.track(u).is_some_and(|t| t.id == Some(id) && (t.x - o.x).hypot(t.y - o.y) <= reach * 2.0));
            }
            if chosen.is_none() {
                let mut best_d = reach;
                for &u in &free {
                    let Some(t) = self.track(u) else { continue };
                    let d = (t.x - o.x).hypot(t.y - o.y);
                    if d <= best_d && (o.id.is_none() || t.id.is_none() || t.id == o.id) {
                        best_d = d;
                        chosen = Some(u);
                    }
                }
            }
            let uid = match chosen {
                Some(u) => {
                    free.retain(|&f| f != u);
                    u
                }
                None => {
                    let uid = self.next_uid;
                    self.next_uid += 1;
                    self.tracks.push(Track {
                        uid,
                        x: o.x,
                        y: o.y,
                        id: None,
                        seen_ms: now_ms,
                        observed_ms: now_ms,
                        checked_ms: f64::NEG_INFINITY,
                        scores: None,
                        born_ms: now_ms,
                    });
                    uid
                }
            };
            // Um jogador é um ícone só: o nome sai de qualquer outro rastro.
            if let Some(id) = o.id {
                for t in self.tracks.iter_mut() {
                    if t.uid != uid && t.id == Some(id) {
                        t.id = None;
                    }
                }
            }
            let t = self.tracks.iter_mut().find(|t| t.uid == uid).expect("rastro recém achado ou criado");
            t.x = o.x;
            t.y = o.y;
            t.seen_ms = now_ms;
            t.observed_ms = now_ms;
            if let Some(sc) = o.scores.clone() {
                t.scores = Some(sc);
                t.checked_ms = now_ms;
            }
            if let Some(id) = o.id {
                t.id = Some(id);
                t.checked_ms = now_ms;
            } else if let (Some(sc), Some(cur)) = (o.scores.as_ref(), t.id) {
                // Comparado e não reconhecido: mantém o nome (coberto, fundido), a menos que agora pareça claramente outro.
                let own = sc.iter().find(|m| m.id == cur).map_or(f32::NEG_INFINITY, |m| m.score);
                let top = sc.iter().map(|m| m.score).fold(f32::NEG_INFINITY, f32::max);
                if top - own >= 0.3 {
                    t.id = None;
                }
            }
        }
        for t in &self.tracks {
            if let (Some(id), true) = (t.id, t.seen_ms == now_ms) {
                self.last_seen.insert(id, (t.x, t.y, now_ms));
            }
        }
        self.update_riders(now_ms);
    }

    /// Quem está escondido embaixo do ícone de um aliado. Um aliado vivo é sempre desenhado, então quem some perto de outro aliado
    /// está embaixo dele, por mais que se empilhem, e dali em diante está onde aquele ícone estiver: dois campeões empilhados andam
    /// juntos. Param de carona quando nomeados de novo, mortos, ou quando o ícone que os cobria some sem outro ao lado para levá-los.
    fn update_riders(&mut self, now_ms: f64) {
        let near = self.icon_diam * HIDDEN_NEAR_DIAMS;
        let named: HashSet<PlayerId> = self.tracks.iter().filter_map(|t| t.id).collect();
        let host_near = |tracks: &[Track], x: f64, y: f64| -> Option<u64> {
            tracks
                .iter()
                .filter(|t| (t.x - x).hypot(t.y - y) <= near)
                .min_by(|a, b| {
                    let (da, db) = ((a.x - x).hypot(a.y - y), (b.x - x).hypot(b.y - y));
                    da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
                })
                .map(|t| t.uid)
        };
        let mut emerged: HashSet<u64> = HashSet::new();
        // Em ordem de jogador: quem aparece primeiro leva o ícone novo, e a ordem de um HashMap mudaria o resultado de uma execução para outra.
        let mut ids: Vec<PlayerId> = self.riders.keys().copied().collect();
        ids.sort_unstable();
        for id in ids {
            if named.contains(&id) || self.dead.contains(&id) {
                self.riders.remove(&id);
                continue;
            }
            let mut r = self.riders[&id];
            match self.track(r.host) {
                Some(h) => {
                    r.hx = h.x;
                    r.hy = h.y;
                }
                None => match host_near(&self.tracks, r.hx, r.hy) {
                    Some(next) => {
                        let t = self.track(next).expect("acabou de achar");
                        r.host = next;
                        r.hx = t.x;
                        r.hy = t.y;
                    }
                    None => {
                        self.riders.remove(&id);
                        continue;
                    }
                },
            }
            // Aparecendo de novo: um ícone novo e sem nome ao lado da pilha é ele, e dali em diante ele é aquele ícone.
            let shown = self
                .tracks
                .iter()
                .filter(|t| {
                    t.id.is_none()
                        && t.uid != r.host
                        && t.born_ms > r.since_ms
                        && !emerged.contains(&t.uid)
                        && (t.x - r.hx).hypot(t.y - r.hy) <= near
                })
                .max_by(|a, b| {
                    let sa = a.score_of(id).unwrap_or(0.0) as f64 - (a.x - r.hx).hypot(a.y - r.hy) / 1000.0;
                    let sb = b.score_of(id).unwrap_or(0.0) as f64 - (b.x - r.hx).hypot(b.y - r.hy) / 1000.0;
                    sa.partial_cmp(&sb).unwrap_or(std::cmp::Ordering::Equal)
                })
                .map(|t| (t.uid, t.x, t.y));
            if let Some((uid, x, y)) = shown {
                emerged.insert(uid);
                r.host = uid;
                r.hx = x;
                r.hy = y;
                r.since_ms = now_ms;
            }
            self.last_seen.insert(id, (r.hx, r.hy, now_ms));
            self.riders.insert(id, r);
        }
        let mut prev: Vec<PlayerId> = self.prev_named.iter().copied().collect();
        prev.sort_unstable();
        for id in prev {
            if named.contains(&id) || self.riders.contains_key(&id) || self.dead.contains(&id) {
                continue;
            }
            let Some(&(sx, sy, _)) = self.last_seen.get(&id) else { continue };
            if let Some(h) = host_near(&self.tracks, sx, sy) {
                let ht = self.track(h).expect("acabou de achar");
                if ht.id != Some(id) {
                    let (hx, hy) = (ht.x, ht.y);
                    self.riders.insert(id, Rider { host: h, hx, hy, since_ms: now_ms });
                    self.last_seen.insert(id, (hx, hy, now_ms));
                }
            }
        }
        self.prev_named = named;
    }

    fn in_base_zone(&self, x: f64, y: f64) -> bool {
        // coordenadas do minimapa -> fração do mapa; as bases ficam nos cantos de baixo à esquerda e de cima à direita
        let (fx, fy) = (x / self.region.w.max(1) as f64, y / self.region.h.max(1) as f64);
        let z = BASE_ZONE_FRACTION;
        (fx < z && fy > 1.0 - z) || (fx > 1.0 - z && fy < z)
    }

    /// `id` pode ser o ícone em `p`? Nunca nomeado ainda, ou visto há pouco tempo perto o bastante para ter andado até lá; ou o ícone
    /// está numa base (recolher leva até lá de qualquer ponto).
    fn could_be_at(&self, id: PlayerId, x: f64, y: f64, now_ms: f64) -> bool {
        let Some(&(sx, sy, ms)) = self.last_seen.get(&id) else { return true };
        if self.in_base_zone(x, y) {
            return true;
        }
        let reach = self.icon_diam * (REACH_SLACK_DIAMS + REACH_DIAMS_PER_SEC * (now_ms - ms).max(0.0) / 1000.0);
        (sx - x).hypot(sy - y) <= reach
    }

    /// Qual ícone desconhecido é cada candidato. Os pares mais certos primeiro: um ícone comparado vai para quem mais parecia; depois
    /// os que nunca foram comparados (dentro de uma mancha fundida) vão para quem foi nomeado mais perto.
    fn assign_unknown(&self, unknown: &[&Track], cands: &[PlayerId], now_ms: f64) -> HashMap<PlayerId, u64> {
        let mut placed: HashMap<PlayerId, u64> = HashMap::new();
        let mut used: HashSet<u64> = HashSet::new();
        let mut pairs: Vec<(u8, f64, u64, PlayerId)> = Vec::new();
        let stacked = |t: &Track| self.tracks.iter().any(|h| h.id.is_some() && h.uid != t.uid && (h.x - t.x).hypot(h.y - t.y) < STACKED_RING_DIAMS * self.icon_diam);
        let near_enough = |id: PlayerId, t: &Track| {
            self.last_seen.get(&id).is_some_and(|&(sx, sy, _)| (sx - t.x).hypot(sy - t.y) <= self.icon_diam * REACH_SLACK_DIAMS)
        };
        for t in unknown {
            let on_top = stacked(t);
            for &id in cands {
                if !self.could_be_at(id, t.x, t.y, now_ms) || (on_top && !near_enough(id, t)) {
                    continue;
                }
                if t.scores.is_some() {
                    pairs.push((0, -(t.score_of(id).unwrap_or(-1.0) as f64), t.uid, id));
                } else if let Some(&(sx, sy, _)) = self.last_seen.get(&id) {
                    pairs.push((1, (sx - t.x).hypot(sy - t.y), t.uid, id));
                }
            }
        }
        pairs.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal)));
        for (_, _, uid, id) in pairs {
            if placed.contains_key(&id) || used.contains(&uid) {
                continue;
            }
            placed.insert(id, uid);
            used.insert(uid);
        }
        // Quem nunca foi nomeado só é posto quando é o único que sobra para o único ícone que sobra: ali não há dúvida de quem é.
        let icons_left: Vec<&&Track> = unknown.iter().filter(|t| !used.contains(&t.uid) && t.scores.is_none() && !stacked(t)).collect();
        let never_seen: Vec<PlayerId> = cands.iter().copied().filter(|id| !placed.contains_key(id) && !self.last_seen.contains_key(id)).collect();
        if never_seen.len() == 1 && icons_left.len() == 1 {
            placed.insert(never_seen[0], icons_left[0].uid);
        }
        placed
    }

    /// Onde cada aliado está agora, até onde se pode saber: o ícone (rastro) em que cada um está, ou escondido embaixo de um, ou
    /// um dos ícones desconhecidos; mortos e quem ainda não apareceu ficam na última posição vista.
    pub fn placements(&self, now_ms: f64) -> Vec<Placement> {
        let live: Vec<&Track> = self.tracks.iter().filter(|t| now_ms - t.seen_ms <= TRACK_TTL_MS).collect();
        let mut at: HashMap<PlayerId, (u64, How)> = HashMap::new();
        for t in &live {
            if let Some(id) = t.id {
                let fresh = now_ms - t.observed_ms <= 600.0;
                at.insert(id, (t.uid, if fresh { How::Seen } else { How::Under }));
            }
        }
        let unknown: Vec<&Track> = live.iter().copied().filter(|t| t.id.is_none()).collect();
        let cands: Vec<PlayerId> =
            self.players.iter().map(|p| p.id).filter(|id| !at.contains_key(id) && !self.dead.contains(id)).collect();
        for (id, uid) in self.assign_unknown(&unknown, &cands, now_ms) {
            at.insert(id, (uid, How::Inferred));
        }
        for &id in &cands {
            if at.contains_key(&id) {
                continue;
            }
            if let Some(r) = self.riders.get(&id) {
                if live.iter().any(|t| t.uid == r.host) {
                    at.insert(id, (r.host, How::Under));
                }
            }
        }
        // Um aliado vivo é sempre desenhado: quem não está em nenhum ícone pouco depois de ser visto está embaixo do aliado mais perto.
        for &id in &cands {
            if at.contains_key(&id) {
                continue;
            }
            let Some(&(sx, sy, ms)) = self.last_seen.get(&id) else { continue };
            if now_ms - ms > ALLY_UNPLACED_MS {
                continue;
            }
            let host = live
                .iter()
                .filter(|t| self.could_be_at(id, t.x, t.y, now_ms))
                .min_by(|a, b| {
                    let (da, db) = ((a.x - sx).hypot(a.y - sy), (b.x - sx).hypot(b.y - sy));
                    da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
                });
            if let Some(h) = host {
                at.insert(id, (h.uid, How::Under));
            }
        }
        let mut out = Vec::new();
        for p in &self.players {
            if let (Some(&(uid, how)), false) = (at.get(&p.id), self.dead.contains(&p.id)) {
                if let Some(t) = self.track(uid) {
                    out.push(Placement { id: p.id, x: t.x, y: t.y, how });
                    continue;
                }
            }
            if let Some(&(x, y, ms)) = self.last_seen.get(&p.id) {
                if self.dead.contains(&p.id) || now_ms - ms <= HELD_MAX_MS {
                    out.push(Placement { id: p.id, x, y, how: How::Held });
                }
            }
        }
        out
    }

    /// Posição de jogo (unidades do mapa) de um ponto do minimapa em pixels da região.
    pub fn to_game(&self, x: f64, y: f64) -> (f64, f64) {
        let (fx, fy) = ((x / self.region.w.max(1) as f64).clamp(0.0, 1.0), (y / self.region.h.max(1) as f64).clamp(0.0, 1.0));
        (fx * self.map_w, self.map_h - fy * self.map_h)
    }

    /// Para o registro: cada rastro vivo, `id@x,y` ou `?@x,y`.
    pub fn describe(&self, now_ms: f64) -> String {
        self.tracks
            .iter()
            .filter(|t| now_ms - t.seen_ms <= TRACK_TTL_MS)
            .map(|t| format!("{}@{},{}", t.id.map_or("?".to_string(), |i| i.to_string()), t.x.round(), t.y.round()))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// A decisão para um ícone coberto em parte: nota alta e uma margem clara sobre o segundo.
fn decide_partial(ranked: &[MatchScore], t: &Tuning) -> Option<usize> {
    let first = ranked.first()?;
    let second = ranked.get(1).map_or(0.0, |m| m.score);
    (first.score >= t.partial_min_score && first.score - second >= t.partial_min_margin).then_some(first.id)
}

#[cfg(test)]
mod tuning_tests {
    use super::*;

    #[test]
    fn a_sensibilidade_de_fabrica_e_a_de_sempre() {
        let t = Tuning::default();
        assert!((t.scan_min_hit - SCAN_MIN_HIT).abs() < 1e-9);
        assert_eq!((t.match_min_score, t.match_min_margin), (MATCH_MIN_SCORE, MATCH_MIN_MARGIN));
        assert_eq!((t.partial_min_score, t.partial_min_margin), (PARTIAL_MIN_SCORE, PARTIAL_MIN_MARGIN));
    }

    #[test]
    fn mais_sensivel_exige_menos_e_fora_da_faixa_nao_estoura() {
        let (rigida, normal, sensivel) = (Tuning::from_sensitivity(0.0), Tuning::default(), Tuning::from_sensitivity(1.0));
        assert!(rigida.scan_min_hit > normal.scan_min_hit && normal.scan_min_hit > sensivel.scan_min_hit);
        assert!(rigida.match_min_score > normal.match_min_score && normal.match_min_score > sensivel.match_min_score);
        assert!(rigida.match_min_margin > sensivel.match_min_margin && sensivel.match_min_margin > 0.0);
        assert_eq!(Tuning::from_sensitivity(9.0), sensivel);
        assert_eq!(Tuning::from_sensitivity(-3.0), rigida);
    }
}

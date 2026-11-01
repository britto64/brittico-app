//! Um minimapa de mentira, com a verdade conhecida.
//!
//! Sem um Windows para gravar partidas, é assim que o reconhecimento se mede aqui: cinco aliados e cinco inimigos andam pelo
//! mapa (juntam, se separam, morrem, recolhem para a base), o quadro é desenhado como o jogo desenha (retrato redondo, anel da
//! cor do time por cima, aliados por cima dos inimigos) e a posição verdadeira de cada um é guardada para conferir.
//!
//! O que o simulador NÃO reproduz: o brilho e a compressão reais do minimapa, o desenho exato do anel, névoa de guerra, pings,
//! wards, minions nítidos. Por isso um bom número aqui prova que o método funciona, não que vai acertar na primeira partida de
//! verdade; essa segunda prova vem dos pacotes de depuração do app (etapa E6).
//!
//! Sem dependências: quem chama entrega as imagens já decodificadas.

use crate::frame::Frame;
use crate::geometry::{capture_bounds, game_to_minimap, minimap_region, MapKind, MinimapSide, Rect};

/// Uma imagem RGBA (arte de ícone ou fundo do minimapa).
#[derive(Clone, Debug)]
pub struct Art {
    pub w: usize,
    pub h: usize,
    pub rgba: Vec<u8>,
}

/// Gerador pseudoaleatório simples (xorshift64*), para o simulador ser repetível sem dependências.
#[derive(Clone, Debug)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    /// [0, 1)
    pub fn f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
    pub fn range(&mut self, a: f64, b: f64) -> f64 {
        a + (b - a) * self.f64()
    }
    pub fn chance(&mut self, p: f64) -> bool {
        self.f64() < p
    }
    /// Aproximadamente normal (soma de quatro uniformes), média 0, desvio ~0,58.
    fn bell(&mut self) -> f64 {
        self.f64() + self.f64() + self.f64() + self.f64() - 2.0
    }
}

#[derive(Clone, Debug)]
pub struct SimConfig {
    /// Altura da janela do jogo; a largura é 16:9.
    pub window_h: i32,
    pub minimap_scale: f64,
    /// Desvio do ruído somado a cada pixel (0 a ~10).
    pub noise: f64,
    /// Mistura cada pixel com os vizinhos (o minimapa real chega reamostrado).
    pub blur: bool,
    /// Chance de, ao escolher destino, um aliado ir atrás de outro (empilhar).
    pub stack_chance: f64,
    /// Mortes por jogador por minuto.
    pub deaths_per_min: f64,
    pub seed: u64,
}

impl Default for SimConfig {
    fn default() -> Self {
        SimConfig { window_h: 1080, minimap_scale: 1.92, noise: 5.0, blur: true, stack_chance: 0.35, deaths_per_min: 0.4, seed: 1 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Life {
    Alive,
    /// Morto até esse instante.
    Dead(f64),
    /// Recolhendo até esse instante: o ícone CONTINUA desenhado, parado onde está (no jogo o canalizar não esconde ninguém); no fim ele
    /// salta para a base.
    Recall(f64),
}

#[derive(Clone, Debug)]
struct Walker {
    x: f64,
    y: f64,
    dest: (f64, f64),
    speed: f64,
    pause_until: f64,
    follow: Option<usize>,
    life: Life,
}

/// A verdade de um jogador num quadro.
#[derive(Clone, Copy, Debug)]
pub struct Truth {
    pub id: usize,
    /// Pixels da região do minimapa.
    pub x: f64,
    pub y: f64,
    /// Aparece no minimapa (vivo: quem recolhe continua desenhado).
    pub drawn: bool,
    pub dead: bool,
}

pub struct SimGame {
    cfg: SimConfig,
    window: Rect,
    capture: Rect,
    region: Rect,
    ally_art: Vec<Art>,
    enemy_art: Vec<Art>,
    bg: Option<Art>,
    allies: Vec<Walker>,
    enemies: Vec<Walker>,
    rng: Rng,
    map: MapKind,
    t: f64,
}

const ALLY_RING: [u8; 3] = [24, 190, 200];
const ENEMY_RING: [u8; 3] = [210, 50, 50];
/// Pontos para onde os jogadores vão: rotas, selva, rios, bases (unidades do jogo).
const DESTS: [(f64, f64); 12] = [
    (2200.0, 12200.0),
    (7435.0, 7490.0),
    (12500.0, 2400.0),
    (4000.0, 8200.0),
    (10800.0, 6800.0),
    (3600.0, 4000.0),
    (11200.0, 11000.0),
    (7400.0, 9600.0),
    (7500.0, 5200.0),
    (5200.0, 11800.0),
    (9800.0, 3000.0),
    (1500.0, 7000.0),
];

impl SimGame {
    pub fn new(cfg: SimConfig, ally_art: Vec<Art>, enemy_art: Vec<Art>, bg: Option<Art>) -> Self {
        let window = Rect::new(0, 0, cfg.window_h * 16 / 9, cfg.window_h);
        let capture = capture_bounds(window, MinimapSide::Right);
        let region = minimap_region(window, cfg.minimap_scale, capture, MinimapSide::Right);
        let mut rng = Rng::new(cfg.seed);
        let spawn = |rng: &mut Rng, blue: bool| {
            let (bx, by) = if blue { (700.0, 700.0) } else { (14100.0, 14200.0) };
            Walker { x: bx + rng.range(-250.0, 250.0), y: by + rng.range(-250.0, 250.0), dest: (bx, by), speed: rng.range(330.0, 430.0), pause_until: rng.range(2.0, 12.0), follow: None, life: Life::Alive }
        };
        let allies = (0..ally_art.len()).map(|_| spawn(&mut rng, true)).collect();
        let enemies = (0..enemy_art.len()).map(|_| spawn(&mut rng, false)).collect();
        SimGame { cfg, window, capture, region, ally_art, enemy_art, bg, allies, enemies, rng, map: MapKind::SummonersRift, t: 0.0 }
    }

    pub fn region(&self) -> Rect {
        self.region
    }
    pub fn capture(&self) -> Rect {
        self.capture
    }
    pub fn window(&self) -> Rect {
        self.window
    }
    pub fn time(&self) -> f64 {
        self.t
    }
    pub fn allies(&self) -> usize {
        self.allies.len()
    }

    /// Região do minimapa DENTRO do quadro (relativo ao canto do quadro capturado).
    pub fn region_in_frame(&self) -> Rect {
        self.region
    }

    pub fn dead_ids(&self) -> Vec<usize> {
        self.allies.iter().enumerate().filter(|(_, w)| matches!(w.life, Life::Dead(_))).map(|(i, _)| i).collect()
    }

    pub fn truth(&self) -> Vec<Truth> {
        self.allies
            .iter()
            .enumerate()
            .map(|(id, w)| {
                let (x, y) = game_to_minimap(w.x, w.y, self.region.w as f64, self.region.h as f64, self.map);
                Truth { id, x, y, drawn: !matches!(w.life, Life::Dead(_)), dead: matches!(w.life, Life::Dead(_)) }
            })
            .collect()
    }

    /// Anda `dt` segundos.
    pub fn step(&mut self, dt: f64) {
        self.t += dt;
        let t = self.t;
        let n = self.allies.len();
        let (death_p, stack) = (self.cfg.deaths_per_min * dt / 60.0, self.cfg.stack_chance);
        for i in 0..n {
            Self::advance(&mut self.rng, &mut self.allies, i, t, dt, death_p, stack, true);
        }
        for i in 0..self.enemies.len() {
            Self::advance(&mut self.rng, &mut self.enemies, i, t, dt, death_p, 0.2, false);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn advance(rng: &mut Rng, team: &mut [Walker], i: usize, t: f64, dt: f64, death_p: f64, stack: f64, blue: bool) {
        let base = if blue { (700.0, 700.0) } else { (14100.0, 14200.0) };
        match team[i].life {
            Life::Dead(until) | Life::Recall(until) => {
                if t >= until {
                    let w = &mut team[i];
                    w.life = Life::Alive;
                    w.x = base.0 + rng.range(-250.0, 250.0);
                    w.y = base.1 + rng.range(-250.0, 250.0);
                    w.pause_until = t + rng.range(1.0, 6.0);
                    w.follow = None;
                }
                return;
            }
            Life::Alive => {}
        }
        if rng.chance(death_p) {
            team[i].life = Life::Dead(t + rng.range(12.0, 30.0));
            return;
        }
        if rng.chance(0.004 * dt) {
            team[i].life = Life::Recall(t + 8.0);
            return;
        }
        if t < team[i].pause_until {
            return;
        }
        // Quem segue outro vai até onde o outro está; senão, até o destino.
        if let Some(f) = team[i].follow {
            if team[f].life == Life::Alive {
                team[i].dest = (team[f].x + rng.range(-250.0, 250.0), team[f].y + rng.range(-250.0, 250.0));
            } else {
                team[i].follow = None;
            }
        }
        let (dx, dy) = (team[i].dest.0 - team[i].x, team[i].dest.1 - team[i].y);
        let d = dx.hypot(dy);
        let step = team[i].speed * dt;
        if d <= step.max(60.0) {
            // Chegou: espera um pouco e escolhe outro rumo (às vezes atrás de um colega).
            team[i].pause_until = t + rng.range(3.0, 25.0);
            if rng.chance(stack) && team.len() > 1 {
                let f = (i + 1 + (rng.next_u64() as usize % (team.len() - 1))) % team.len();
                team[i].follow = Some(f);
            } else {
                team[i].follow = None;
                team[i].dest = DESTS[rng.next_u64() as usize % DESTS.len()];
            }
        } else {
            team[i].x += dx / d * step;
            team[i].y += dy / d * step;
        }
    }

    /// Desenha o quadro capturado (o quadrado em volta do minimapa) deste instante.
    pub fn frame(&mut self) -> Frame {
        let (fw, fh) = (self.capture.w as usize, self.capture.h as usize);
        let mut f = Frame::new(fw, fh);
        // O resto da HUD em volta do minimapa: escuro, com textura.
        for y in 0..fh {
            for x in 0..fw {
                let v = 22.0 + 10.0 * self.rng.f64();
                f.set(x, y, [v as u8, (v + 3.0) as u8, (v + 6.0) as u8]);
            }
        }
        let r = self.region;
        // O fundo do minimapa.
        for y in 0..r.h as usize {
            for x in 0..r.w as usize {
                let px = match &self.bg {
                    Some(bg) => {
                        let (sx, sy) = (x * bg.w / r.w as usize, y * bg.h / r.h as usize);
                        let o = (sy.min(bg.h - 1) * bg.w + sx.min(bg.w - 1)) * 4;
                        [bg.rgba[o], bg.rgba[o + 1], bg.rgba[o + 2]]
                    }
                    None => [30, 60, 55],
                };
                f.set(r.x as usize + x, r.y as usize + y, px);
            }
        }
        let diam = crate::geometry::icon_diam(r.w);
        let (rw, rh) = (r.w as f64, r.h as f64);
        // Inimigos primeiro, aliados por cima (o jogo desenha os aliados sobre os inimigos).
        for (i, w) in self.enemies.iter().enumerate() {
            if !matches!(w.life, Life::Dead(_)) {
                let (x, y) = game_to_minimap(w.x, w.y, rw, rh, self.map);
                draw_icon(&mut f, r.x as f64 + x, r.y as f64 + y, diam, &self.enemy_art[i], ENEMY_RING);
            }
        }
        for (i, w) in self.allies.iter().enumerate() {
            if !matches!(w.life, Life::Dead(_)) {
                let (x, y) = game_to_minimap(w.x, w.y, rw, rh, self.map);
                draw_icon(&mut f, r.x as f64 + x, r.y as f64 + y, diam, &self.ally_art[i], ALLY_RING);
            }
        }
        // Ruído e reamostragem.
        let noise = self.cfg.noise;
        if self.cfg.blur {
            let src = f.data.clone();
            for y in 1..fh - 1 {
                for x in 1..fw - 1 {
                    for c in 0..3 {
                        let o = (y * fw + x) * 4 + c;
                        let s = src[o] as f64 * 0.6
                            + (src[o - 4] as f64 + src[o + 4] as f64 + src[o - fw * 4] as f64 + src[o + fw * 4] as f64) * 0.1;
                        f.data[o] = s.round() as u8;
                    }
                }
            }
        }
        if noise > 0.0 {
            for p in f.data.chunks_exact_mut(4) {
                for c in p.iter_mut().take(3) {
                    *c = (*c as f64 + self.rng.bell() * noise).round().clamp(0.0, 255.0) as u8;
                }
            }
        }
        f
    }
}

/// Um ícone de campeão: retrato redondo com o anel do time por cima.
fn draw_icon(f: &mut Frame, cx: f64, cy: f64, diam: f64, art: &Art, ring: [u8; 3]) {
    let r_out = diam / 2.0;
    let thick = (diam * 0.1).round().max(2.0);
    let r_in = r_out - thick;
    let (x0, x1) = ((cx - r_out - 1.0).floor().max(0.0) as usize, ((cx + r_out + 1.0).ceil() as usize).min(f.width - 1));
    let (y0, y1) = ((cy - r_out - 1.0).floor().max(0.0) as usize, ((cy + r_out + 1.0).ceil() as usize).min(f.height - 1));
    for y in y0..=y1 {
        for x in x0..=x1 {
            let (dx, dy) = (x as f64 + 0.5 - cx, y as f64 + 0.5 - cy);
            let d = dx.hypot(dy);
            if d > r_out + 0.5 {
                continue;
            }
            let src = if d >= r_in {
                ring
            } else {
                // o jogo mostra um pouco menos da arte que a imagem do círculo inteiro
                let zoom = 0.92;
                let (ax, ay) = (art.w as f64 / 2.0 + dx / r_in * art.w as f64 / 2.0 * zoom, art.h as f64 / 2.0 + dy / r_in * art.h as f64 / 2.0 * zoom);
                sample_art(art, ax, ay, art.w as f64 / (2.0 * r_in) * zoom * 0.5)
            };
            // borda suave para fora do anel
            let cover = if d > r_out - 0.5 { (r_out + 0.5 - d).clamp(0.0, 1.0) } else { 1.0 };
            let old = f.rgb(x, y);
            let mix = |a: u8, b: u8| (a as f64 * (1.0 - cover) + b as f64 * cover).round() as u8;
            f.set(x, y, [mix(old[0], src[0]), mix(old[1], src[1]), mix(old[2], src[2])]);
        }
    }
}

/// Média da arte numa janelinha em volta de (`ax`, `ay`), composta sobre um fundo escuro onde ela é transparente.
fn sample_art(art: &Art, ax: f64, ay: f64, half: f64) -> [u8; 3] {
    let (mut r, mut g, mut b, mut n) = (0.0, 0.0, 0.0, 0.0);
    let steps = 2;
    for j in 0..steps {
        for i in 0..steps {
            let sx = ax + (i as f64 / steps as f64 - 0.25) * half * 2.0;
            let sy = ay + (j as f64 / steps as f64 - 0.25) * half * 2.0;
            let (xi, yi) = (sx.round().clamp(0.0, art.w as f64 - 1.0) as usize, sy.round().clamp(0.0, art.h as f64 - 1.0) as usize);
            let o = (yi * art.w + xi) * 4;
            let a = art.rgba[o + 3] as f64 / 255.0;
            r += art.rgba[o] as f64 * a + 30.0 * (1.0 - a);
            g += art.rgba[o + 1] as f64 * a + 30.0 * (1.0 - a);
            b += art.rgba[o + 2] as f64 * a + 30.0 * (1.0 - a);
            n += 1.0;
        }
    }
    [(r / n) as u8, (g / n) as u8, (b / n) as u8]
}

/// Uma "arte" inventada (retrato de ruído suave, determinístico por semente), para testar sem baixar nada da Riot.
pub fn synthetic_art(seed: u64, size: usize) -> Art {
    let mut rng = Rng::new(seed ^ 0xABCD);
    // alguns pontos de cor espalhados, interpolados por distância
    let blobs: Vec<(f64, f64, [f64; 3])> = (0..7)
        .map(|_| (rng.range(0.0, 1.0), rng.range(0.0, 1.0), [rng.range(20.0, 235.0), rng.range(20.0, 235.0), rng.range(20.0, 235.0)]))
        .collect();
    let mut rgba = vec![0u8; size * size * 4];
    let c = size as f64 / 2.0;
    for y in 0..size {
        for x in 0..size {
            let (u, v) = (x as f64 / size as f64, y as f64 / size as f64);
            let (mut acc, mut wsum) = ([0.0f64; 3], 0.0);
            for (bx, by, col) in &blobs {
                let w = 1.0 / (0.002 + (u - bx).powi(2) + (v - by).powi(2)).powi(2);
                for k in 0..3 {
                    acc[k] += col[k] * w;
                }
                wsum += w;
            }
            let o = (y * size + x) * 4;
            for k in 0..3 {
                rgba[o + k] = (acc[k] / wsum).round().clamp(0.0, 255.0) as u8;
            }
            let inside = ((x as f64 + 0.5 - c).hypot(y as f64 + 0.5 - c)) <= c;
            rgba[o + 3] = if inside { 255 } else { 0 };
        }
    }
    Art { w: size, h: size, rgba }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn o_gerador_e_repetivel() {
        let (mut a, mut b) = (Rng::new(7), Rng::new(7));
        assert_eq!(a.next_u64(), b.next_u64());
        let x = Rng::new(7).f64();
        assert!((0.0..1.0).contains(&x));
    }

    #[test]
    fn o_simulador_desenha_aneis_que_a_mascara_acha() {
        use crate::blobs::{create_mask, find_blobs, Ring};
        let arts: Vec<Art> = (0..5).map(|i| synthetic_art(i, 64)).collect();
        let enemies: Vec<Art> = (10..15).map(|i| synthetic_art(i, 64)).collect();
        let mut g = SimGame::new(SimConfig::default(), arts, enemies, None);
        for _ in 0..40 {
            g.step(0.2);
        }
        let f = g.frame();
        let r = g.region();
        let blobs = find_blobs(&create_mask(&f, &r), r.w as usize, r.h as usize);
        assert!(blobs.iter().any(|b| b.ring == Ring::Ally), "nenhum anel de aliado achado");
        assert!(blobs.iter().any(|b| b.ring == Ring::Enemy), "nenhum anel de inimigo achado");
    }
}

//! De quem é este ícone? Compara o recorte em volta de um ponto com o ícone base de cada candidato.
//!
//! A pergunta é pequena: um entre no máximo cinco jogadores do time da cor do anel, e o jogo diz que campeão cada um joga. Um
//! centro de blob sai uns pixels fora do lugar quando outro ícone está por cima, e a escala do ícone na tela varia com o
//! `MinimapScale`, então a busca olha ±4 px e quatro escalas. Uma passada grossa (metade dos pixels, de dois em dois) acha onde
//! cada um encaixa melhor; só os poucos melhores são refinados em resolução cheia, e só as notas em resolução cheia são devolvidas.

use crate::blobs::Blob;
use crate::frame::Frame;
use crate::templates::{decide_match, dot, half_positions, sample_inner, MatchScore, TemplateSet};
use std::collections::HashSet;

/// Lados de recorte tentados, como fração do diâmetro esperado do ícone.
const SCALES: [f64; 4] = [0.9, 1.0, 1.1, 1.2];
const COARSE_SHIFTS: [f64; 5] = [-4.0, -2.0, 0.0, 2.0, 4.0];
const SCALE_STEP: f64 = 0.05;
/// Dentro de uma mancha de ícones fundidos, os lados tentados em cada ponto da grade.
const BLOB_SCALES: [f64; 2] = [1.0, 1.1];
/// Quantos dos melhores achados grossos o passe fino refina.
const REFINED: usize = 3;

/// A nota de cada candidato para um recorte, em resolução cheia ou em meia.
fn score_crop(frame: &Frame, x: f64, y: f64, side: f64, sets: &[TemplateSet], half: bool) -> Vec<f32> {
    let v = sample_inner(frame, x, y, side, if half { Some(half_positions()) } else { None });
    sets.iter()
        .map(|set| {
            let templates = if half { &set.half } else { &set.vecs };
            templates.iter().map(|t| dot(&v, t)).fold(f32::NEG_INFINITY, f32::max)
        })
        .collect()
}

/// A melhor nota de cada candidato para o ícone perto de (`cx`, `cy`), da maior para a menor. `sets`: os candidatos (o time que a
/// cor do anel diz). Coordenadas do QUADRO, não da região.
pub fn identify_icon(frame: &Frame, cx: f64, cy: f64, icon_diam: f64, sets: &[TemplateSet]) -> Vec<MatchScore> {
    if sets.is_empty() {
        return Vec::new();
    }
    // Grosso, em meia resolução: onde cada um encaixa melhor.
    let mut coarse = vec![f32::NEG_INFINITY; sets.len()];
    let mut at = vec![(0.0f64, 0.0f64, 1.0f64); sets.len()];
    for &k in &SCALES {
        for &dy in &COARSE_SHIFTS {
            for &dx in &COARSE_SHIFTS {
                for (i, s) in score_crop(frame, cx + dx, cy + dy, icon_diam * k, sets, true).into_iter().enumerate() {
                    if s > coarse[i] {
                        coarse[i] = s;
                        at[i] = (dx, dy, k);
                    }
                }
            }
        }
    }
    // Resolução cheia daqui: cada um no seu melhor ponto...
    let mut best = vec![f32::NEG_INFINITY; sets.len()];
    let mut tried: HashSet<(i64, i64, i64)> = HashSet::new();
    let mut try_at = |dx: f64, dy: f64, k: f64, best: &mut Vec<f32>| {
        let key = (dx.round() as i64, dy.round() as i64, (k * 100.0).round() as i64);
        if !tried.insert(key) {
            return;
        }
        for (i, s) in score_crop(frame, cx + dx, cy + dy, icon_diam * k, sets, false).into_iter().enumerate() {
            if s > best[i] {
                best[i] = s;
            }
        }
    };
    for &(dx, dy, k) in &at {
        try_at(dx, dy, k, &mut best);
    }
    // ...e um pixel e um passo de escala em volta dos três melhores: o vencedor e quem ainda pode virar o segundo, que é só o que
    // `decide_match` olha.
    let mut seeds: Vec<(f32, (f64, f64, f64))> = coarse.iter().copied().zip(at.iter().copied()).collect();
    seeds.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    for &(_, (dx, dy, k)) in seeds.iter().take(REFINED) {
        for kk in [k - SCALE_STEP, k, k + SCALE_STEP] {
            for ddy in -1..=1 {
                for ddx in -1..=1 {
                    try_at(dx + ddx as f64, dy + ddy as f64, kk, &mut best);
                }
            }
        }
    }
    let mut out: Vec<MatchScore> = sets.iter().enumerate().map(|(i, s)| MatchScore { id: s.id, score: best[i] }).collect();
    out.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    out
}

#[derive(Clone, Copy, Debug)]
pub struct NamedIcon {
    pub id: usize,
    pub x: f64,
    pub y: f64,
    pub score: f32,
}

/// Quem se reconhece dentro de uma mancha de vários ícones fundidos (dois ou três de largura; uma luta). Cada ponto de uma grade
/// sobre a mancha é pontuado contra cada candidato; o candidato é nomeado onde pontua mais, se for uma comparação clara
/// (`decide_match`, contra os outros naquele mesmo ponto) e não estiver em cima de alguém nomeado com mais clareza. No máximo
/// `max_icons`, o número de ícones que a mancha guarda. Coordenadas do QUADRO; `blob` já deslocado para o quadro.
pub fn identify_in_blob(frame: &Frame, blob: &Blob, icon_diam: f64, sets: &[TemplateSet], max_icons: usize) -> Vec<NamedIcon> {
    if sets.is_empty() || max_icons == 0 {
        return Vec::new();
    }
    let inset = icon_diam * 0.35;
    let (x0, x1) = (blob.min_x as f64 + inset, blob.max_x as f64 - inset);
    let (y0, y1) = (blob.min_y as f64 + inset, blob.max_y as f64 - inset);
    // Uma luta inteira numa mancha tem várias vezes a área: grade mais grossa mantém o custo em algumas dezenas de ms.
    let step = if blob.w().max(blob.h()) as f64 > icon_diam * 2.6 { 3.0 } else { 2.0 };
    let (lo_x, hi_x) = (x0.min(x1), x0.max(x1));
    let (lo_y, hi_y) = (y0.min(y1), y0.max(y1));

    // Grosso, em meia resolução: onde da mancha cada um encaixa melhor.
    let mut spot: Vec<(f32, f64, f64, f64)> = vec![(f32::NEG_INFINITY, 0.0, 0.0, 1.0); sets.len()];
    let mut y = lo_y;
    while y <= hi_y {
        let mut x = lo_x;
        while x <= hi_x {
            for &k in &BLOB_SCALES {
                for (i, s) in score_crop(frame, x, y, icon_diam * k, sets, true).into_iter().enumerate() {
                    if s > spot[i].0 {
                        spot[i] = (s, x, y, k);
                    }
                }
            }
            x += step;
        }
        y += step;
    }
    // Resolução cheia em volta dos pontos de quem pode estar nela (dois a mais do que ela guarda): ali cada um é pontuado contra
    // todos, para a margem.
    struct Per {
        score: f32,
        x: f64,
        y: f64,
        ranked: Vec<MatchScore>,
    }
    let mut per: Vec<Per> = sets.iter().map(|_| Per { score: f32::NEG_INFINITY, x: 0.0, y: 0.0, ranked: Vec::new() }).collect();
    let mut cands: Vec<(usize, (f32, f64, f64, f64))> = spot.iter().copied().enumerate().collect();
    cands.sort_by(|a, b| b.1 .0.partial_cmp(&a.1 .0).unwrap_or(std::cmp::Ordering::Equal));
    for &(i, (_, sx, sy, k)) in cands.iter().take(max_icons + 2) {
        for dy in -1..=1 {
            for dx in -1..=1 {
                let (px, py) = (sx + dx as f64, sy + dy as f64);
                let scores = score_crop(frame, px, py, icon_diam * k, sets, false);
                if scores[i] > per[i].score {
                    let mut ranked: Vec<MatchScore> = sets.iter().enumerate().map(|(j, s)| MatchScore { id: s.id, score: scores[j] }).collect();
                    ranked.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
                    per[i] = Per { score: scores[i], x: px, y: py, ranked };
                }
            }
        }
    }
    let mut order: Vec<usize> = (0..sets.len()).collect();
    order.sort_by(|&a, &b| per[b].score.partial_cmp(&per[a].score).unwrap_or(std::cmp::Ordering::Equal));
    let mut named: Vec<NamedIcon> = Vec::new();
    for i in order {
        if named.len() >= max_icons {
            break;
        }
        let p = &per[i];
        if p.ranked.is_empty() || decide_match(&p.ranked) != Some(sets[i].id) {
            continue;
        }
        if named.iter().any(|n| (n.x - p.x).hypot(n.y - p.y) < icon_diam * 0.4) {
            continue;
        }
        named.push(NamedIcon { id: sets[i].id, x: p.x, y: p.y, score: p.score });
    }
    named
}

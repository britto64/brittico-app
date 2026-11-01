//! Comparar um recorte do minimapa com o ícone de um campeão.
//!
//! O recorte e o ícone são reamostrados para o mesmo quadrado (32×32) e só o miolo (um círculo de raio 0,36 do lado) é comparado:
//! o anel é da cor do time, não faz parte da arte. Cada miolo vira um vetor de média zero e comprimento 1, então o produto escalar
//! de dois é a correlação deles, indiferente ao brilho e ao contraste que o minimapa aplica.

use crate::frame::Frame;
use std::sync::OnceLock;

pub const MATCH_SIZE: usize = 32;
const INNER_FRACTION: f64 = 0.36;
/// O melhor candidato só vale se tirar pelo menos isto...
pub const MATCH_MIN_SCORE: f32 = 0.6;
/// ...e vencer o segundo por pelo menos isto.
pub const MATCH_MIN_MARGIN: f32 = 0.2;
/// Fundo sobre o qual os cantos transparentes do ícone são compostos.
const ICON_BACKDROP: f32 = 30.0;
/// O ícone no jogo mostra um pouco menos da arte que a imagem do círculo inteiro, e quanto menos varia com a escala da HUD.
const TEMPLATE_ZOOMS: [f64; 3] = [0.8, 0.9, 1.0];

static INNER: OnceLock<Vec<usize>> = OnceLock::new();
static HALF: OnceLock<Vec<usize>> = OnceLock::new();

/// Índices (no quadrado 32×32) dos pixels dentro do círculo comparado.
pub fn inner_indices() -> &'static [usize] {
    INNER.get_or_init(|| {
        let s = MATCH_SIZE;
        let c = (s as f64 - 1.0) / 2.0;
        let mut out = Vec::new();
        for y in 0..s {
            for x in 0..s {
                if ((x as f64 - c).hypot(y as f64 - c)) <= s as f64 * INNER_FRACTION {
                    out.push(y * s + x);
                }
            }
        }
        out
    })
}

/// Posições (em `inner_indices`) dos pixels de linhas e colunas pares: um quarto do trabalho para a busca grossa.
pub fn half_positions() -> &'static [usize] {
    HALF.get_or_init(|| {
        let idx = inner_indices();
        (0..idx.len()).filter(|&n| idx[n] % MATCH_SIZE % 2 == 0 && idx[n] / MATCH_SIZE % 2 == 0).collect()
    })
}

/// Média zero e comprimento 1, no lugar.
pub fn normalize(v: &mut [f32]) {
    let mean = v.iter().sum::<f32>() / v.len() as f32;
    let mut norm = 0.0f32;
    for x in v.iter_mut() {
        *x -= mean;
        norm += *x * *x;
    }
    let norm = if norm.sqrt() == 0.0 { 1.0 } else { norm.sqrt() };
    for x in v.iter_mut() {
        *x /= norm;
    }
}

pub fn dot(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

/// O miolo de um quadrado 32×32 RGB (como [`sample_square`] devolve) em vetor normalizado.
pub fn normalized_inner(rgb: &[f32]) -> Vec<f32> {
    let idx = inner_indices();
    let mut v = Vec::with_capacity(idx.len() * 3);
    for &i in idx {
        v.extend_from_slice(&rgb[i * 3..i * 3 + 3]);
    }
    normalize(&mut v);
    v
}

/// Reamostra (bilinear) o quadrado de lado `side` centrado em (`cx`, `cy`) de um buffer RGBA para 32×32 RGB. Pixels fora do buffer
/// valem `fill`. Serve para o recorte do minimapa e para o ícone, de modo que os dois sejam preparados igual.
pub fn sample_square(data: &[u8], width: usize, height: usize, cx: f64, cy: f64, side: f64, fill: f32) -> Vec<f32> {
    let s = MATCH_SIZE;
    let mut out = vec![0.0f32; s * s * 3];
    let step = side / s as f64;
    let (x0, y0) = (cx - side / 2.0, cy - side / 2.0);
    let at = |x: i64, y: i64| -> Option<usize> {
        if x < 0 || y < 0 || x >= width as i64 || y >= height as i64 {
            None
        } else {
            Some((y as usize * width + x as usize) * 4)
        }
    };
    for j in 0..s {
        let sy = y0 + (j as f64 + 0.5) * step - 0.5;
        let ya = sy.floor();
        let fy = (sy - ya) as f32;
        for i in 0..s {
            let sx = x0 + (i as f64 + 0.5) * step - 0.5;
            let xa = sx.floor();
            let fx = (sx - xa) as f32;
            let (xa, ya_i) = (xa as i64, ya as i64);
            let p = [at(xa, ya_i), at(xa + 1, ya_i), at(xa, ya_i + 1), at(xa + 1, ya_i + 1)];
            let w = [(1.0 - fx) * (1.0 - fy), fx * (1.0 - fy), (1.0 - fx) * fy, fx * fy];
            let o = (j * s + i) * 3;
            for c in 0..3 {
                let mut acc = 0.0;
                for k in 0..4 {
                    acc += p[k].map_or(fill, |q| data[q + c] as f32) * w[k];
                }
                out[o + c] = acc;
            }
        }
    }
    out
}

/// Como [`sample_square`], mas só os pixels do miolo (todos, ou só os de `positions`), já normalizado. Dá os mesmos números com 40%
/// do trabalho, que é o grosso do custo de identificar.
pub fn sample_inner(frame: &Frame, cx: f64, cy: f64, side: f64, positions: Option<&[usize]>) -> Vec<f32> {
    let s = MATCH_SIZE;
    let idx = inner_indices();
    let count = positions.map_or(idx.len(), |p| p.len());
    let mut v = vec![0.0f32; count * 3];
    let step = side / s as f64;
    let (x0, y0) = (cx - side / 2.0, cy - side / 2.0);
    let (width, height) = (frame.width as i64, frame.height as i64);
    for m in 0..count {
        let n = positions.map_or(m, |p| p[m]);
        let (i, j) = (idx[n] % s, idx[n] / s);
        let sx = x0 + (i as f64 + 0.5) * step - 0.5;
        let sy = y0 + (j as f64 + 0.5) * step - 0.5;
        let (xa, ya) = (sx.floor() as i64, sy.floor() as i64);
        let (fx, fy) = ((sx - sx.floor()) as f32, (sy - sy.floor()) as f32);
        let w = [(1.0 - fx) * (1.0 - fy), fx * (1.0 - fy), (1.0 - fx) * fy, fx * fy];
        let inside = |x: i64, y: i64| x >= 0 && y >= 0 && x < width && y < height;
        let px = [(xa, ya), (xa + 1, ya), (xa, ya + 1), (xa + 1, ya + 1)];
        for c in 0..3 {
            let mut acc = 0.0;
            for k in 0..4 {
                let (x, y) = px[k];
                if inside(x, y) {
                    acc += frame.data[(y as usize * frame.width + x as usize) * 4 + c] as f32 * w[k];
                }
            }
            v[m * 3 + c] = acc;
        }
    }
    neutralize_white(&mut v);
    normalize(&mut v);
    v
}

/// O retângulo da câmera do jogo é desenhado em BRANCO por cima do minimapa, inclusive por cima do ícone do campeão de quem joga (a câmera o segue), e
/// uma linha branca no meio do ícone derrubava a nota do Maokai de 0,9 para 0,6. Amostra quase branca em todos os canais (acima de `WHITE_MIN`) não é
/// arte do ícone: vira a média das demais, que depois de normalizado vale zero (não conta nem a favor nem contra). Se mais da metade do recorte é
/// branca o ícone é mesmo claro e nada muda.
const WHITE_MIN: f32 = 222.0;

fn neutralize_white(v: &mut [f32]) {
    let n = v.len() / 3;
    let white: Vec<bool> = (0..n).map(|m| v[m * 3] >= WHITE_MIN && v[m * 3 + 1] >= WHITE_MIN && v[m * 3 + 2] >= WHITE_MIN).collect();
    let count = white.iter().filter(|w| **w).count();
    if count == 0 || count * 2 > n {
        return;
    }
    let mut mean = [0.0f32; 3];
    for m in (0..n).filter(|m| !white[*m]) {
        for c in 0..3 {
            mean[c] += v[m * 3 + c];
        }
    }
    for c in 0..3 {
        mean[c] /= (n - count) as f32;
    }
    for m in (0..n).filter(|m| white[*m]) {
        v[m * 3..m * 3 + 3].copy_from_slice(&mean);
    }
}

/// Os vetores-modelo de UMA imagem de ícone (RGBA, qualquer tamanho): cantos transparentes compostos sobre um fundo escuro, e o
/// miolo em cada um dos zooms.
pub fn icon_templates(rgba: &[u8], width: usize, height: usize) -> Vec<Vec<f32>> {
    let mut flat = vec![0u8; width * height * 4];
    for i in 0..width * height {
        let a = rgba[i * 4 + 3] as f32 / 255.0;
        for c in 0..3 {
            flat[i * 4 + c] = (rgba[i * 4 + c] as f32 * a + ICON_BACKDROP * (1.0 - a)).round() as u8;
        }
        flat[i * 4 + 3] = 255;
    }
    // Reduzir 120 px para 32 direto em bilinear serrilha; metade por metade até ficar a menos de 2x, como uma cadeia de mipmaps.
    let (mut src, mut w, mut h) = (flat, width, height);
    while w.min(h) >= MATCH_SIZE * 4 {
        let (hw, hh) = (w / 2, h / 2);
        let mut half = vec![0u8; hw * hh * 4];
        for y in 0..hh {
            for x in 0..hw {
                for c in 0..4 {
                    let sum = src[((2 * y) * w + 2 * x) * 4 + c] as u32
                        + src[((2 * y) * w + 2 * x + 1) * 4 + c] as u32
                        + src[((2 * y + 1) * w + 2 * x) * 4 + c] as u32
                        + src[((2 * y + 1) * w + 2 * x + 1) * 4 + c] as u32;
                    half[(y * hw + x) * 4 + c] = (sum / 4) as u8;
                }
            }
        }
        src = half;
        w = hw;
        h = hh;
    }
    let side = w.min(h) as f64;
    TEMPLATE_ZOOMS
        .iter()
        .map(|z| normalized_inner(&sample_square(&src, w, h, w as f64 / 2.0, h as f64 / 2.0, side * z, ICON_BACKDROP)))
        .collect()
}

/// Os modelos de um campeão: cada forma que ele toma, em cada zoom. Guarda também a versão em meia resolução, para a busca grossa.
#[derive(Clone, Debug)]
pub struct TemplateSet {
    pub id: usize,
    pub vecs: Vec<Vec<f32>>,
    pub half: Vec<Vec<f32>>,
}

impl TemplateSet {
    pub fn new(id: usize, vecs: Vec<Vec<f32>>) -> Self {
        let pos = half_positions();
        let half = vecs
            .iter()
            .map(|t| {
                let mut h = Vec::with_capacity(pos.len() * 3);
                for &p in pos {
                    h.extend_from_slice(&t[p * 3..p * 3 + 3]);
                }
                normalize(&mut h);
                h
            })
            .collect();
        TemplateSet { id, vecs, half }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct MatchScore {
    pub id: usize,
    pub score: f32,
}

/// Quem é, se a comparação for clara; `None` se não.
pub fn decide_match(ranked: &[MatchScore]) -> Option<usize> {
    let first = ranked.first()?;
    if first.score < MATCH_MIN_SCORE {
        return None;
    }
    let second = ranked.get(1).map_or(0.0, |m| m.score);
    (first.score - second >= MATCH_MIN_MARGIN).then_some(first.id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn o_miolo_tem_o_numero_certo_de_pixels() {
        // pi * (32 * 0,36)^2 = ~417; a contagem exata depende do centro em 15,5
        let n = inner_indices().len();
        assert!((380..460).contains(&n), "{n}");
        assert!(half_positions().len() < n / 3);
    }

    #[test]
    fn normalizado_tem_media_zero_e_comprimento_um() {
        let mut v: Vec<f32> = (0..300).map(|i| (i % 17) as f32 * 3.0 + 5.0).collect();
        normalize(&mut v);
        assert!(v.iter().sum::<f32>().abs() < 1e-3);
        assert!((dot(&v, &v) - 1.0).abs() < 1e-4);
    }

    // Um vetor constante (sem contraste) tem norma zero; não pode virar NaN e contaminar todas as notas.
    #[test]
    fn imagem_lisa_nao_vira_nan() {
        let mut v = vec![7.0f32; 90];
        normalize(&mut v);
        assert!(v.iter().all(|x| x.is_finite()));
    }

    #[test]
    fn decide_pede_nota_e_margem() {
        let r = |a: f32, b: f32| [MatchScore { id: 3, score: a }, MatchScore { id: 1, score: b }];
        assert_eq!(decide_match(&r(0.9, 0.3)), Some(3));
        assert_eq!(decide_match(&r(0.5, 0.0)), None, "nota baixa");
        assert_eq!(decide_match(&r(0.9, 0.8)), None, "margem curta");
        assert_eq!(decide_match(&[MatchScore { id: 2, score: 0.7 }]), Some(2), "um só candidato");
        assert_eq!(decide_match(&[]), None);
    }

    #[test]
    fn o_modelo_de_um_icone_correlaciona_com_ele_mesmo_e_nao_com_outro() {
        // duas "artes" diferentes: listras e xadrez
        let make = |f: &dyn Fn(usize, usize) -> u8| -> Vec<u8> {
            let (w, h) = (64, 64);
            let mut d = vec![255u8; w * h * 4];
            for y in 0..h {
                for x in 0..w {
                    let v = f(x, y);
                    d[(y * w + x) * 4] = v;
                    d[(y * w + x) * 4 + 1] = v / 2;
                    d[(y * w + x) * 4 + 2] = 255 - v;
                }
            }
            d
        };
        let stripes = make(&|x, _| if (x / 6) % 2 == 0 { 220 } else { 40 });
        let checker = make(&|x, y| if (x / 8 + y / 8) % 2 == 0 { 230 } else { 30 });
        let a = icon_templates(&stripes, 64, 64);
        let b = icon_templates(&checker, 64, 64);
        let same = dot(&a[2], &a[2]);
        let other = dot(&a[2], &b[2]);
        assert!(same > 0.99 && other < 0.5, "same {same} other {other}");
    }
}

#[cfg(test)]
mod white_tests {
    use super::*;

    #[test]
    fn a_linha_branca_da_camera_nao_conta_como_arte_do_icone() {
        // 8 amostras: 2 quase brancas (a linha da câmera) e 6 de arte; as brancas viram a média das outras e não pesam
        let mut v: Vec<f32> = vec![40.0, 30.0, 20.0, 60.0, 50.0, 40.0, 255.0, 255.0, 255.0, 80.0, 70.0, 60.0, 250.0, 248.0, 252.0, 20.0, 20.0, 20.0, 100.0, 90.0, 80.0, 30.0, 40.0, 50.0];
        neutralize_white(&mut v);
        assert!(v.iter().all(|x| *x < 200.0), "{v:?}");
        let mean_r = (40.0 + 60.0 + 80.0 + 20.0 + 100.0 + 30.0) / 6.0;
        assert!((v[6] - mean_r).abs() < 1e-3 && (v[12] - mean_r).abs() < 1e-3, "o branco vira a média");
    }

    #[test]
    fn um_icone_realmente_claro_nao_e_mexido() {
        let mut v: Vec<f32> = vec![255.0; 30];
        v[0] = 10.0;
        let before = v.clone();
        neutralize_white(&mut v);
        assert_eq!(v, before, "mais da metade branca: é o ícone, não a câmera");
    }
}

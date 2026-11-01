//! Achar os anéis de aliado pela geometria: onde estão os centros de círculos de raio conhecido, mesmo com um por cima do outro.
//!
//! O centro de uma mancha de dois ícones que se sobrepõem fica entre os dois; tomá-lo por "o ícone" cria um fantasma no meio e arrasta
//! o nome de quem estava ali. Mas o raio de um ícone é conhecido (`geometry::icon_diam`), e o ícone de cima aparece inteiro: acha-se
//! primeiro o círculo que mais anel tem, descontam-se os pontos que ele cobre, e o que sobra de anel (a meia-lua de quem estava
//! embaixo) vota no centro do próximo. Assim o aliado que começa a sair de baixo de outro tem posição exata desde que mostra uns
//! 3 px de anel, e não só quando a mancha já virou dois ícones.

use crate::blobs::Blob;

/// Pontos amostrados em volta de cada círculo candidato.
const SAMPLES: usize = 28;
/// Um círculo precisa de pelo menos tantos pontos à vista (não cobertos por outro) para valer.
const MIN_VISIBLE: usize = 7;
/// Fração mínima dos pontos à vista que têm de cair em anel.
const MIN_HIT_FRACTION: f64 = 0.8;
/// O que um círculo achado cobre do que vem depois, em raios de ícone, além da borda (o centro tem erro de um pixel).
const COVER_SLACK_PX: f64 = 0.8;
/// Um ícone (anel de aliado) tem o miolo da arte, não de anel: se o miolo visível é quase todo máscara, não é um anel oco.
const MAX_FILLED_INTERIOR: f64 = 0.6;

/// Um anel achado.
#[derive(Clone, Copy, Debug)]
pub struct FoundRing {
    /// Em pixels da região do minimapa (o mesmo sistema do centro de um `Blob`).
    pub x: f64,
    pub y: f64,
    /// Fração do anel que aparece, de 0 a 1. Um ícone por cima de tudo tem 1; um parcialmente coberto, menos.
    pub visible: f64,
    /// Fração dos pontos à vista que são anel (1 = o círculo fechado; um anel quebrado pela sombra do mapa fica em ~0,7).
    pub fit: f64,
}

/// A soma de pixels de anel em qualquer quadrado da máscara em tempo constante (imagem integral), para a varredura da região inteira não pontuar
/// os milhares de centros onde não há nada.
pub struct Integral {
    w: usize,
    h: usize,
    sums: Vec<u32>,
}

impl Integral {
    pub fn new(mask: &[u8], w: usize, h: usize) -> Integral {
        let mut sums = vec![0u32; (w + 1) * (h + 1)];
        for y in 0..h {
            let mut row = 0u32;
            for x in 0..w {
                row += u32::from(mask[y * w + x] == 1);
                sums[(y + 1) * (w + 1) + x + 1] = sums[y * (w + 1) + x + 1] + row;
            }
        }
        Integral { w, h, sums }
    }

    /// Pixels de anel no quadrado de lado `2r+1` centrado em (`cx`, `cy`).
    fn around(&self, cx: i32, cy: i32, r: i32) -> u32 {
        let (x0, y0) = ((cx - r).max(0) as usize, (cy - r).max(0) as usize);
        let (x1, y1) = (((cx + r + 1).max(0) as usize).min(self.w), ((cy + r + 1).max(0) as usize).min(self.h));
        if x1 <= x0 || y1 <= y0 {
            return 0;
        }
        let at = |x: usize, y: usize| self.sums[y * (self.w + 1) + x];
        at(x1, y1) + at(x0, y0) - at(x1, y0) - at(x0, y1)
    }
}

/// De quantos em quantos pixels a varredura olha um centro (o refino em volta do melhor acha o pixel exato).
const SCAN_STRIDE: usize = 2;
/// Um centro só é pontuado se há anel suficiente em volta: um círculo de 25 px tem uns 150 pixels de anel, e um arco quebrado ainda tem dezenas.
const MIN_RING_PIXELS_AROUND: u32 = 28;

struct Dirs {
    ring: Vec<(f64, f64)>,
    inner: Vec<(f64, f64)>,
}

fn directions() -> Dirs {
    let ring = (0..SAMPLES)
        .map(|k| {
            let a = k as f64 * std::f64::consts::TAU / SAMPLES as f64;
            (a.cos(), a.sin())
        })
        .collect();
    let inner = (0..8)
        .map(|k| {
            let a = (k as f64 + 0.5) * std::f64::consts::TAU / 8.0;
            (a.cos(), a.sin())
        })
        .collect();
    Dirs { ring, inner }
}

#[inline]
fn on(mask: &[u8], w: usize, h: usize, x: f64, y: f64) -> bool {
    let (xi, yi) = (x.round() as i64, y.round() as i64);
    xi >= 0 && yi >= 0 && (xi as usize) < w && (yi as usize) < h && mask[yi as usize * w + xi as usize] == 1
}

/// Quantos pontos do círculo em (`cx`, `cy`) estão à vista (fora dos círculos já achados) e quantos deles são anel; e se o miolo
/// à vista é oco.
fn score(mask: &[u8], w: usize, h: usize, cx: f64, cy: f64, r_out: f64, thick: f64, taken: &[(f64, f64)], dirs: &Dirs) -> (usize, usize, bool) {
    let cover = r_out + COVER_SLACK_PX;
    let covered = |px: f64, py: f64| taken.iter().any(|&(tx, ty)| (px - tx).hypot(py - ty) <= cover);
    let r_mid = r_out - thick / 2.0;
    let (mut seen, mut hit) = (0usize, 0usize);
    for &(dx, dy) in &dirs.ring {
        let (px, py) = (cx + dx * r_mid, cy + dy * r_mid);
        if covered(px, py) {
            continue;
        }
        seen += 1;
        // dois raios, para valer com anéis de 2 ou de 3 px
        if on(mask, w, h, px, py) || on(mask, w, h, cx + dx * (r_mid + 0.6), cy + dy * (r_mid + 0.6)) || on(mask, w, h, cx + dx * (r_mid - 0.6), cy + dy * (r_mid - 0.6)) {
            hit += 1;
        }
    }
    let (mut in_seen, mut in_hit) = (0usize, 0usize);
    for &(dx, dy) in &dirs.inner {
        let (px, py) = (cx + dx * r_out * 0.45, cy + dy * r_out * 0.45);
        if covered(px, py) {
            continue;
        }
        in_seen += 1;
        if on(mask, w, h, px, py) {
            in_hit += 1;
        }
    }
    let hollow = in_seen < 4 || (in_hit as f64) / (in_seen as f64) <= MAX_FILLED_INTERIOR;
    (seen, hit, hollow)
}

/// Oito pontos em vez de vinte e oito: descarta depressa os centros onde há só um pouco de cor solta. Dois acertos bastam para seguir (um ícone coberto
/// quase por inteiro ainda mostra uma meia-lua).
#[allow(clippy::too_many_arguments)]
fn quick_test(mask: &[u8], w: usize, h: usize, cx: f64, cy: f64, r_out: f64, thick: f64, dirs: &Dirs) -> bool {
    let r_mid = r_out - thick / 2.0;
    let mut hits = 0;
    for k in [0usize, 4, 7, 11, 14, 18, 21, 25] {
        let (dx, dy) = dirs.ring[k];
        if on(mask, w, h, cx + dx * r_mid, cy + dy * r_mid) || on(mask, w, h, cx + dx * (r_mid + 0.6), cy + dy * (r_mid + 0.6)) || on(mask, w, h, cx + dx * (r_mid - 0.6), cy + dy * (r_mid - 0.6)) {
            hits += 1;
            if hits >= 2 {
                return true;
            }
        }
    }
    false
}

/// Os anéis dentro de uma mancha, o que mais aparece primeiro. `mask`: 1 onde há anel de aliado (a máscara da região inteira,
/// `w` × `h`). Só os anéis cujo centro cabe na caixa da mancha (`blob`) são procurados. No máximo `max_icons`.
pub fn fit_rings(mask: &[u8], w: usize, h: usize, blob: &Blob, diam: f64, max_icons: usize) -> Vec<FoundRing> {
    fit_rings_with(mask, w, h, blob, diam, max_icons, MIN_HIT_FRACTION, None)
}

/// Os tamanhos de ícone tentados, como fração do esperado, quando o esperado não acha nada: o ícone do jogo é um pouco menor ou maior que a conta em
/// outras escalas do minimapa. Só entra em ação quando a primeira tentativa volta vazia, então não custa nada com o jogo normal.
const FALLBACK_SCALES: [f64; 2] = [0.88, 1.12];

/// A varredura da região com o tamanho esperado e, se nada aparecer, com tamanhos um pouco menores e maiores. Devolve os anéis e o diâmetro que valeu.
#[allow(clippy::too_many_arguments)]
pub fn scan_region(mask: &[u8], w: usize, h: usize, blob: &Blob, diam: f64, max_icons: usize, min_hit: f64, integral: &Integral) -> (Vec<FoundRing>, f64) {
    let first = fit_rings_with(mask, w, h, blob, diam, max_icons, min_hit, Some(integral));
    if !first.is_empty() {
        return (first, diam);
    }
    for k in FALLBACK_SCALES {
        let d = (diam * k).round();
        let found = fit_rings_with(mask, w, h, blob, d, max_icons, min_hit, Some(integral));
        if !found.is_empty() {
            return (found, d);
        }
    }
    (Vec::new(), diam)
}

/// Como `fit_rings`, com a fração mínima de acertos à escolha (o minimapa de verdade tem anéis que a sombra do mapa interrompe: ~70% de acerto ainda é um
/// ícone) e, opcionalmente, a imagem integral da máscara para pular os centros sem anel em volta (a varredura da região inteira).
#[allow(clippy::too_many_arguments)]
pub fn fit_rings_with(mask: &[u8], w: usize, h: usize, blob: &Blob, diam: f64, max_icons: usize, min_hit: f64, integral: Option<&Integral>) -> Vec<FoundRing> {
    let r_out = diam / 2.0;
    let thick = (diam * 0.1).round().max(2.0);
    let dirs = directions();
    // O centro de qualquer círculo dentro da mancha fica a um raio da borda dela.
    let range = |lo: i32, hi: i32| {
        let (a, b) = ((lo as f64 + r_out - 2.0).ceil() as i32, (hi as f64 - r_out + 2.0).floor() as i32);
        if a <= b {
            (a, b)
        } else {
            let mid = (lo + hi) / 2;
            (mid, mid)
        }
    };
    let ((x0, x1), (y0, y1)) = (range(blob.min_x, blob.max_x), range(blob.min_y, blob.max_y));

    // Os candidatos: todo centro com anel em volta que passa num teste barato (oito pontos), pontuado uma vez. Quando um anel é aceito só os candidatos que
    // se sobrepõem a ele são pontuados de novo (a parte coberta deixa de contar), em vez de varrer a região inteira a cada anel.
    struct Cand {
        cx: i32,
        cy: i32,
        seen: usize,
        hit: usize,
        hollow: bool,
    }
    let valid = |c: &Cand| c.hollow && c.seen >= MIN_VISIBLE && c.hit >= MIN_VISIBLE.min(c.seen) && (c.hit as f64) >= min_hit * c.seen as f64;
    let mut taken: Vec<(f64, f64)> = Vec::new();
    let mut cands: Vec<Cand> = Vec::new();
    // Uma grade de 2 em 2 pixels basta para achar onde há um anel (o acerto é um platô de uns 3 × 3); o ponto exato sai do refino em volta do melhor.
    for cy in (y0..=y1).step_by(SCAN_STRIDE) {
        for cx in (x0..=x1).step_by(SCAN_STRIDE) {
            if integral.is_some_and(|i| i.around(cx, cy, r_out.ceil() as i32) < MIN_RING_PIXELS_AROUND) {
                continue;
            }
            if !quick_test(mask, w, h, cx as f64, cy as f64, r_out, thick, &dirs) {
                continue;
            }
            let (seen, hit, hollow) = score(mask, w, h, cx as f64, cy as f64, r_out, thick, &taken, &dirs);
            cands.push(Cand { cx, cy, seen, hit, hollow });
        }
    }

    let mut found: Vec<FoundRing> = Vec::new();
    while found.len() < max_icons {
        let Some(best) = cands.iter().filter(|c| valid(c)).max_by(|a, b| a.hit.cmp(&b.hit).then(b.seen.cmp(&a.seen))) else { break };
        // Refino: todos os centros a até dois pixels do melhor da grade, já com o que os anéis anteriores cobrem.
        let mut local: Vec<Cand> = Vec::new();
        for dy in -(SCAN_STRIDE as i32)..=(SCAN_STRIDE as i32) {
            for dx in -(SCAN_STRIDE as i32)..=(SCAN_STRIDE as i32) {
                let (cx, cy) = (best.cx + dx, best.cy + dy);
                let (seen, hit, hollow) = score(mask, w, h, cx as f64, cy as f64, r_out, thick, &taken, &dirs);
                local.push(Cand { cx, cy, seen, hit, hollow });
            }
        }
        let Some(top) = local.iter().filter(|c| valid(c)).max_by(|a, b| a.hit.cmp(&b.hit).then(b.seen.cmp(&a.seen))) else { break };
        let (bhit, bseen, bx, by) = (top.hit, top.seen, top.cx, top.cy);
        // O centro é a média do platô: os pontos que acertam tanto quanto o melhor, vizinhos dele.
        let (mut sx, mut sy, mut n) = (0.0, 0.0, 0.0);
        for c in local.iter().filter(|c| valid(c)) {
            if c.hit + 1 >= bhit && ((c.cx - bx) as f64).hypot((c.cy - by) as f64) <= 2.5 {
                sx += c.cx as f64;
                sy += c.cy as f64;
                n += 1.0;
            }
        }
        let (fx, fy) = (sx / n, sy / n);
        found.push(FoundRing { x: fx, y: fy, visible: bseen as f64 / SAMPLES as f64, fit: bhit as f64 / bseen.max(1) as f64 });
        taken.push((fx, fy));
        for c in cands.iter_mut() {
            if ((c.cx as f64 - fx).hypot(c.cy as f64 - fy)) <= 2.0 * r_out + COVER_SLACK_PX + 1.0 {
                let (seen, hit, hollow) = score(mask, w, h, c.cx as f64, c.cy as f64, r_out, thick, &taken, &dirs);
                (c.seen, c.hit, c.hollow) = (seen, hit, hollow);
            }
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blobs::find_blobs;

    const W: usize = 160;
    const H: usize = 120;
    const DIAM: f64 = 30.0;

    /// Desenha um ícone por cima do que já está na máscara: o disco inteiro some e o anel volta (a arte do ícone cobre o de baixo).
    fn icon(mask: &mut [u8], cx: f64, cy: f64) {
        let (r_out, r_in) = (DIAM / 2.0, DIAM / 2.0 - 3.0);
        for y in 0..H {
            for x in 0..W {
                let d = (x as f64 - cx).hypot(y as f64 - cy);
                if d <= r_out {
                    mask[y * W + x] = u8::from(d >= r_in);
                }
            }
        }
    }

    fn blob_of(mask: &[u8]) -> Blob {
        let mut b = find_blobs(mask, W, H);
        assert_eq!(b.len(), 1, "a máscara de teste devia ter uma mancha só");
        b.remove(0)
    }

    fn near(found: &[FoundRing], x: f64, y: f64, tol: f64) -> bool {
        found.iter().any(|f| (f.x - x).hypot(f.y - y) <= tol)
    }

    #[test]
    fn um_anel_sozinho_tem_o_centro_certo() {
        let mut m = vec![0u8; W * H];
        icon(&mut m, 60.0, 50.0);
        let found = fit_rings(&m, W, H, &blob_of(&m), DIAM, 5);
        assert_eq!(found.len(), 1);
        assert!((found[0].x - 60.0).abs() <= 1.0 && (found[0].y - 50.0).abs() <= 1.0, "{:?}", found[0]);
        assert!(found[0].visible > 0.95);
    }

    #[test]
    fn dois_aneis_que_so_se_tocam_sao_dois() {
        let mut m = vec![0u8; W * H];
        icon(&mut m, 50.0, 50.0);
        icon(&mut m, 74.0, 52.0);
        let found = fit_rings(&m, W, H, &blob_of(&m), DIAM, 5);
        assert_eq!(found.len(), 2, "{found:?}");
        assert!(near(&found, 50.0, 50.0, 1.5) && near(&found, 74.0, 52.0, 1.5), "{found:?}");
    }

    #[test]
    fn quem_comeca_a_sair_de_baixo_de_outro_aparece_com_uns_poucos_pixels() {
        // o de baixo (50,50) e o de cima 5 px ao lado: a meia-lua é fina, mas o círculo dá o centro
        for shift in [4.0, 5.0, 6.0, 8.0, 10.0] {
            let mut m = vec![0u8; W * H];
            icon(&mut m, 50.0, 50.0);
            icon(&mut m, 50.0 + shift, 50.0);
            let found = fit_rings(&m, W, H, &blob_of(&m), DIAM, 5);
            assert_eq!(found.len(), 2, "deslocamento {shift}: {found:?}");
            assert!(near(&found, 50.0 + shift, 50.0, 1.5), "o de cima, deslocamento {shift}: {found:?}");
            assert!(near(&found, 50.0, 50.0, 2.0), "o de baixo, deslocamento {shift}: {found:?}");
            let under = found.iter().find(|f| (f.x - 50.0).abs() <= 2.0).unwrap();
            assert!(under.visible < 0.7, "o de baixo aparece pela metade: {under:?}");
        }
    }

    #[test]
    fn dois_aneis_quase_no_mesmo_lugar_sao_um_so() {
        // 1 px de diferença não se enxerga: quem chama conta os aliados vivos para saber que sobra um embaixo
        let mut m = vec![0u8; W * H];
        icon(&mut m, 50.0, 50.0);
        icon(&mut m, 51.0, 50.0);
        let found = fit_rings(&m, W, H, &blob_of(&m), DIAM, 5);
        assert_eq!(found.len(), 1, "{found:?}");
    }

    #[test]
    fn uma_luta_com_cinco_deixa_todos_a_vista() {
        let mut m = vec![0u8; W * H];
        let spots = [(40.0, 40.0), (62.0, 44.0), (51.0, 66.0), (80.0, 64.0), (72.0, 86.0)];
        for &(x, y) in &spots {
            icon(&mut m, x, y);
        }
        let found = fit_rings(&m, W, H, &blob_of(&m), DIAM, 5);
        assert_eq!(found.len(), 5, "{found:?}");
        for &(x, y) in &spots {
            assert!(near(&found, x, y, 2.0), "faltou ({x}, {y}): {found:?}");
        }
    }

    #[test]
    fn um_anel_menor_que_o_esperado_ainda_e_achado_na_segunda_tentativa() {
        // o ícone do jogo tem 25 px e a conta esperava 30: a primeira varredura volta vazia e a de 12% a menos acha (a de 28 achava sozinha: a tolerância do círculo cobre ~1 px)
        let mut m = vec![0u8; W * H];
        let (r_out, r_in, cx, cy) = (12.5, 10.5, 60.0, 50.0);
        for y in 0..H {
            for x in 0..W {
                let d = (x as f64 - cx).hypot(y as f64 - cy);
                m[y * W + x] = u8::from(d <= r_out && d >= r_in);
            }
        }
        let whole = Blob { ring: crate::blobs::Ring::Ally, pixels: 0, cx: 0.0, cy: 0.0, min_x: 0, max_x: W as i32 - 1, min_y: 0, max_y: H as i32 - 1, fill: 0.1 };
        let integral = Integral::new(&m, W, H);
        let (found, used) = scan_region(&m, W, H, &whole, 30.0, 5, 0.65, &integral);
        assert_eq!(found.len(), 1, "{found:?} (diâmetro usado {used})");
        assert!((found[0].x - cx).abs() <= 1.5 && (found[0].y - cy).abs() <= 1.5, "{found:?}");
        assert!(used < 30.0, "foi achado com um ícone menor que o esperado ({used})");
    }

    #[test]
    fn um_disco_cheio_nao_e_anel() {
        // torre, minion, grupo de minions: tudo cheio
        let mut m = vec![0u8; W * H];
        for y in 40..70 {
            for x in 40..70 {
                if (x as f64 - 55.0).hypot(y as f64 - 55.0) <= 15.0 {
                    m[y * W + x] = 1;
                }
            }
        }
        let found = fit_rings(&m, W, H, &blob_of(&m), DIAM, 5);
        assert!(found.is_empty(), "{found:?}");
    }

    #[test]
    fn pontos_soltos_nao_viram_anel() {
        let mut m = vec![0u8; W * H];
        icon(&mut m, 50.0, 50.0);
        for k in 0..40 {
            m[(30 + (k * 7) % 40) * W + 20 + (k * 13) % 70] = 1; // ruído de máscara
        }
        let found = fit_rings(&m, W, H, &Blob { ring: crate::blobs::Ring::Ally, pixels: 0, cx: 55.0, cy: 55.0, min_x: 35, max_x: 65, min_y: 35, max_y: 65, fill: 0.2 }, DIAM, 5);
        assert_eq!(found.len(), 1, "{found:?}");
    }
}

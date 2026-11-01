//! Achar os anéis dos ícones no minimapa: pinta de cada cor, junta pixels vizinhos em manchas ("blobs") e separa o que tem cara de
//! ícone do que é torre, minion ou ruído.

use crate::frame::Frame;
use crate::geometry::Rect;

/// A cor do anel diz o time: verde-água é aliado, vermelho é inimigo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ring {
    Ally,
    Enemy,
}

#[derive(Clone, Debug)]
pub struct Blob {
    pub ring: Ring,
    pub pixels: usize,
    pub cx: f64,
    pub cy: f64,
    pub min_x: i32,
    pub max_x: i32,
    pub min_y: i32,
    pub max_y: i32,
    /// pixels / área da caixa: baixo para um anel, alto para uma forma cheia (torre, grupo de minions).
    pub fill: f64,
}

impl Blob {
    pub fn w(&self) -> i32 {
        self.max_x - self.min_x + 1
    }
    pub fn h(&self) -> i32 {
        self.max_y - self.min_y + 1
    }
}

/// 0 = nada, 1 = anel de aliado, 2 = anel de inimigo.
#[inline]
pub fn classify_pixel(r: u8, g: u8, b: u8) -> u8 {
    let (r, g, b) = (r as i32, g as i32, b as i32);
    if r < 100 && g > 120 && b > 120 && g + b > 280 {
        1
    } else if r > 140 && g < 100 && b < 100 {
        2
    } else {
        0
    }
}

/// A máscara de cor da região do minimapa dentro do quadro.
pub fn create_mask(frame: &Frame, region: &Rect) -> Vec<u8> {
    let (w, h) = (region.w as usize, region.h as usize);
    let mut mask = vec![0u8; w * h];
    for y in 0..h {
        let row = (region.y as usize + y) * frame.width + region.x as usize;
        for x in 0..w {
            let o = (row + x) * 4;
            mask[y * w + x] = classify_pixel(frame.data[o], frame.data[o + 1], frame.data[o + 2]);
        }
    }
    mask
}

/// Componentes conexos (vizinhança de 4), com pelo menos 10 pixels.
pub fn find_blobs(mask: &[u8], w: usize, h: usize) -> Vec<Blob> {
    let mut visited = vec![false; w * h];
    let mut blobs = Vec::new();
    let mut stack: Vec<usize> = Vec::new();
    for y in 0..h {
        for x in 0..w {
            let idx = y * w + x;
            if visited[idx] || mask[idx] == 0 {
                continue;
            }
            let target = mask[idx];
            stack.clear();
            stack.push(idx);
            visited[idx] = true;
            let (mut sum_x, mut sum_y, mut count) = (0u64, 0u64, 0usize);
            let (mut min_x, mut max_x, mut min_y, mut max_y) = (x as i32, x as i32, y as i32, y as i32);
            while let Some(i) = stack.pop() {
                let (cx, cy) = ((i % w) as i32, (i / w) as i32);
                sum_x += cx as u64;
                sum_y += cy as u64;
                count += 1;
                min_x = min_x.min(cx);
                max_x = max_x.max(cx);
                min_y = min_y.min(cy);
                max_y = max_y.max(cy);
                let mut push = |nx: i32, ny: i32| {
                    if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 {
                        return;
                    }
                    let ni = ny as usize * w + nx as usize;
                    if !visited[ni] && mask[ni] == target {
                        visited[ni] = true;
                        stack.push(ni);
                    }
                };
                push(cx - 1, cy);
                push(cx + 1, cy);
                push(cx, cy - 1);
                push(cx, cy + 1);
            }
            if count >= 10 {
                let area = ((max_x - min_x + 1) * (max_y - min_y + 1)) as f64;
                blobs.push(Blob {
                    ring: if target == 1 { Ring::Ally } else { Ring::Enemy },
                    pixels: count,
                    cx: (sum_x as f64 / count as f64).round(),
                    cy: (sum_y as f64 / count as f64).round(),
                    min_x,
                    max_x,
                    min_y,
                    max_y,
                    fill: if area > 0.0 { count as f64 / area } else { 1.0 },
                });
            }
        }
    }
    blobs
}

/// O tamanho e o formato de UM ícone, sem olhar o preenchimento: caixa perto do diâmetro esperado, quase quadrada, com pelo menos
/// um arco de anel e nem esparsa demais para ser ruído.
fn is_icon_sized(b: &Blob, diam: f64) -> bool {
    let (min, max) = (diam * 0.6, diam * 1.6);
    let (bw, bh) = (b.w() as f64, b.h() as f64);
    if bw < min || bw > max || bh < min || bh > max {
        return false;
    }
    let aspect = bw / bh;
    if !(0.6..=1.7).contains(&aspect) {
        return false;
    }
    b.pixels >= 15 && b.fill >= 0.08
}

/// Quais blobs são um ícone avulso (anel oco: preenchimento até 40%). Devolve um vetor de "sim/não" na ordem de `blobs`.
/// Um ícone com arte na cor do anel (Gwen, de cabelo verde-água) enche demais e cai fora daqui; `classify` o aceita pelo caminho
/// frouxo, mas só se for reconhecido.
pub fn single_icon_flags(blobs: &[Blob], diam: f64) -> Vec<bool> {
    if diam < 5.0 {
        return vec![true; blobs.len()];
    }
    blobs.iter().map(|b| is_icon_sized(b, diam) && b.fill <= 0.40).collect()
}

/// Um blob frouxo: tamanho de ícone, preenchido até 60%.
pub fn is_icon_shaped(b: &Blob, diam: f64) -> bool {
    let (bw, bh) = (b.w() as f64, b.h() as f64);
    bw >= diam * 0.6
        && bw <= diam * 1.6
        && bh >= diam * 0.6
        && bh <= diam * 1.6
        && bw / bh >= 0.6
        && bw / bh <= 1.7
        && b.pixels >= 15
        && b.fill >= 0.08
        && b.fill <= 0.6
}

/// Vários ícones fundidos (dois na rota, até um time inteiro numa luta).
pub const MAX_ICONS_IN_BLOB: usize = 5;

pub fn is_merged_icons(b: &Blob, diam: f64) -> bool {
    if b.pixels < 15 || b.fill > 0.7 || b.fill < 0.08 {
        return false;
    }
    let (bw, bh) = (b.w() as f64, b.h() as f64);
    let span = bw.max(bh) / diam;
    bw.min(bh) >= diam * 0.6 && span > 1.6 && span <= 5.0
}

/// Dois ícones empilhados a poucos pixels: uma mancha de até um ícone e meio, mais larga que um. O peso sozinho não basta (um
/// ícone cuja arte tem a cor do anel pesa como dois).
pub fn is_stacked_icons(b: &Blob, diam: f64, ring_px: f64) -> bool {
    let (bw, bh) = (b.w() as f64, b.h() as f64);
    let span = bw.max(bh) / diam;
    if b.fill > 0.75 || bw.min(bh) < diam * 0.6 || span > 1.6 {
        return false;
    }
    span >= 1.3 || (span >= 1.2 && ring_px > 0.0 && b.pixels as f64 >= ring_px * 1.6)
}

/// Quantos ícones uma mancha fundida guarda: pelo quanto se estende, ou pelo quanto de anel carrega, o que disser mais (uma luta
/// empilha em vez de alargar). Anéis que se sobrepõem dividem pixels, por isso o arredondamento passa de meio.
pub fn icons_in_blob(b: &Blob, diam: f64, ring_px: f64) -> usize {
    let span = (b.w().max(b.h()) as f64) / diam;
    let by_span = if span <= 1.6 {
        1
    } else if span <= 2.1 {
        2
    } else {
        3
    };
    let by_pixels = if ring_px > 0.0 { (b.pixels as f64 / ring_px + 0.4).floor() as usize } else { 1 };
    by_span.max(by_pixels).clamp(1, MAX_ICONS_IN_BLOB)
}

/// Um ícone sozinho. `strict` = passou no filtro de ícone avulso; senão só foi aceito pela forma, e quem chama só acredita nele
/// se for reconhecido.
#[derive(Clone, Debug)]
pub struct Lone {
    pub blob: usize,
    pub strict: bool,
}

#[derive(Clone, Debug)]
pub struct Merged {
    pub blob: usize,
    pub n: usize,
}

/// Quais manchas são ícones e quantos cada uma guarda. `singles` vem de [`single_icon_flags`]; `ring_px` é o peso de um anel.
pub fn classify_icon_blobs(blobs: &[Blob], singles: &[bool], diam: f64, ring_px: f64) -> (Vec<Lone>, Vec<Merged>) {
    let (mut lone, mut merged) = (Vec::new(), Vec::new());
    for (i, b) in blobs.iter().enumerate() {
        let strict = singles[i];
        let loose = !strict && is_icon_shaped(b, diam);
        let stacked = is_stacked_icons(b, diam, ring_px);
        if is_merged_icons(b, diam) || stacked {
            merged.push(Merged { blob: i, n: icons_in_blob(b, diam, ring_px).max(2) });
        } else if strict || loose {
            lone.push(Lone { blob: i, strict });
        }
    }
    (lone, merged)
}

/// O peso de um anel em pixels: a mediana dos ícones claramente sozinhos deste quadro; sem nenhum, dez por pixel de diâmetro
/// (medido em minimapas reais e simulados).
pub fn ring_weight(blobs: &[Blob], singles: &[bool], diam: f64) -> f64 {
    let mut w: Vec<usize> = blobs
        .iter()
        .zip(singles)
        .filter(|(b, s)| **s && (b.w().max(b.h()) as f64) <= diam * 1.15)
        .map(|(b, _)| b.pixels)
        .collect();
    if w.is_empty() {
        return diam * 10.0;
    }
    w.sort_unstable();
    w[w.len() / 2] as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn as_cores_dos_aneis_sao_classificadas() {
        assert_eq!(classify_pixel(30, 200, 190), 1); // aliado
        assert_eq!(classify_pixel(200, 60, 60), 2); // inimigo
        assert_eq!(classify_pixel(120, 120, 120), 0);
        assert_eq!(classify_pixel(10, 130, 140), 0); // g+b = 270, escuro demais para ser anel
    }

    fn ring(frame: &mut Frame, cx: f64, cy: f64, diam: f64, rgb: [u8; 3]) {
        let (r1, r0) = (diam / 2.0, diam / 2.0 - 3.0);
        for y in 0..frame.height {
            for x in 0..frame.width {
                let d = ((x as f64 - cx).powi(2) + (y as f64 - cy).powi(2)).sqrt();
                if d <= r1 && d >= r0 {
                    frame.set(x, y, rgb);
                }
            }
        }
    }

    #[test]
    fn um_anel_vira_um_blob_de_ícone_avulso() {
        let mut f = Frame::new(120, 120);
        ring(&mut f, 60.0, 60.0, 30.0, [30, 200, 190]);
        let region = Rect::new(0, 0, 120, 120);
        let mask = create_mask(&f, &region);
        let blobs = find_blobs(&mask, 120, 120);
        assert_eq!(blobs.len(), 1);
        assert_eq!(blobs[0].ring, Ring::Ally);
        assert!((blobs[0].cx - 60.0).abs() <= 1.0 && (blobs[0].cy - 60.0).abs() <= 1.0);
        let flags = single_icon_flags(&blobs, 30.0);
        assert_eq!(flags, vec![true]);
        let (lone, merged) = classify_icon_blobs(&blobs, &flags, 30.0, ring_weight(&blobs, &flags, 30.0));
        assert_eq!((lone.len(), merged.len()), (1, 0));
    }

    #[test]
    fn dois_aneis_colados_viram_uma_pilha() {
        let mut f = Frame::new(120, 120);
        ring(&mut f, 55.0, 60.0, 30.0, [30, 200, 190]);
        ring(&mut f, 70.0, 60.0, 30.0, [30, 200, 190]);
        let mask = create_mask(&f, &Rect::new(0, 0, 120, 120));
        let blobs = find_blobs(&mask, 120, 120);
        assert_eq!(blobs.len(), 1, "os dois anéis encostam");
        let flags = single_icon_flags(&blobs, 30.0);
        let (lone, merged) = classify_icon_blobs(&blobs, &flags, 30.0, 0.0);
        assert!(lone.is_empty());
        assert_eq!(merged.len(), 1);
        assert!(merged[0].n >= 2);
    }

    #[test]
    fn um_grupo_de_minions_cheio_nao_e_icone() {
        let mut f = Frame::new(120, 120);
        for y in 50..70 {
            for x in 50..70 {
                f.set(x, y, [200, 60, 60]); // quadrado cheio
            }
        }
        let blobs = find_blobs(&create_mask(&f, &Rect::new(0, 0, 120, 120)), 120, 120);
        let flags = single_icon_flags(&blobs, 20.0);
        assert_eq!(flags, vec![false]);
        let (lone, merged) = classify_icon_blobs(&blobs, &flags, 20.0, 0.0);
        assert!(lone.is_empty() && merged.is_empty());
    }
}

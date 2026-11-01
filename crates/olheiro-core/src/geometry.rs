//! Onde o minimapa fica na tela, que tamanho tem e como um pixel dele vira posição no mapa.
//!
//! Tudo calibrado em medidas reais da referência: a 1080p, `MinimapScale` 0 dá 200 px e 3 dá 420 px; a 1440p, 280 e 560.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    pub const fn new(x: i32, y: i32, w: i32, h: i32) -> Self {
        Rect { x, y, w, h }
    }

    pub fn intersects(&self, o: &Rect) -> bool {
        self.x < o.x + o.w && o.x < self.x + self.w && self.y < o.y + o.h && o.y < self.y + self.h
    }
}

/// De que lado da janela do jogo o minimapa fica. O jogo deixa trocar; o padrão é a direita.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum MinimapSide {
    #[default]
    Right,
    Left,
}

/// Fração da altura da janela capturada como o quadrado onde o minimapa cabe. Com 0,40 cabe toda a faixa de `MinimapScale`
/// (0 a 3) que o jogo oferece, em qualquer resolução (a conta fecha em 7/18 = 0,389 para a escala 3).
pub const MINIMAP_CAPTURE_FACTOR: f64 = 0.40;

/// O lado do ícone de campeão, como fração do lado do minimapa.
pub const ICON_DIAM_FRACTION: f64 = 0.087;

/// Lado do minimapa para um `MinimapScale` do `game.cfg`.
pub fn minimap_size_for_height(height: f64, scale: f64) -> i32 {
    let base = height * 2.0 / 9.0 - 40.0;
    let rate = height / 18.0 + 40.0 / 3.0;
    (base + scale * rate).round() as i32
}

/// O quadrado a capturar, em coordenadas da tela (a origem pode ser negativa: monitor à esquerda ou acima do principal).
/// Ancorado no canto de baixo da área do cliente da janela, do lado do minimapa.
pub fn capture_bounds(window: Rect, side: MinimapSide) -> Rect {
    let size = (window.h as f64 * MINIMAP_CAPTURE_FACTOR).round() as i32;
    let x = match side {
        MinimapSide::Right => window.x + window.w - size,
        MinimapSide::Left => window.x,
    };
    Rect::new(x, window.y + window.h - size, size, size)
}

/// Onde o minimapa está DENTRO do quadrado capturado (relativo à origem dele, não à tela). `window.x`/`y` se cancelam contra os
/// mesmos termos de `capture`, então o resultado só depende do TAMANHO da janela: é isso que torna a origem negativa inofensiva.
pub fn minimap_region(window: Rect, scale: f64, capture: Rect, side: MinimapSide) -> Rect {
    let size = minimap_size_for_height(window.h as f64, scale);
    let left = match side {
        MinimapSide::Right => window.x + window.w - size,
        MinimapSide::Left => window.x,
    };
    Rect::new(left - capture.x, (window.y + window.h - size) - capture.y, size, size)
}

/// O minimapa cabe inteiro no quadrado capturado? Um minimapa maior que a captura teria origem negativa e a leitura da máscara
/// "dobraria" para a linha anterior em vez de falhar: quem chama deve recusar a região, não varrê-la.
pub fn region_fits(region: &Rect, capture: &Rect) -> bool {
    region.x >= 0 && region.y >= 0 && region.x + region.w <= capture.w && region.y + region.h <= capture.h
}

/// O diâmetro esperado de um ícone, para um minimapa de lado `region_w`.
pub fn icon_diam(region_w: i32) -> f64 {
    (region_w as f64 * ICON_DIAM_FRACTION).round()
}

/// Os mapas para os quais sabemos converter pixel em posição. Os outros (Arena, Swarm…) ficam sem proximidade em vez de
/// receberem coordenadas erradas com cara de certas.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MapKind {
    SummonersRift,
    HowlingAbyss,
}

impl MapKind {
    /// `mapNumber` do `gamestats`/`allgamedata`: 11 = Summoner's Rift, 12 = Howling Abyss.
    pub fn from_number(n: i64) -> Option<MapKind> {
        match n {
            11 => Some(MapKind::SummonersRift),
            12 => Some(MapKind::HowlingAbyss),
            _ => None,
        }
    }

    /// Largura e altura do mapa em unidades do jogo.
    pub fn dims(self) -> (f64, f64) {
        match self {
            MapKind::SummonersRift => (14870.0, 14980.0),
            MapKind::HowlingAbyss => (12988.0, 12988.0),
        }
    }
}

/// Posição no mapa de um ponto do minimapa (`rx`, `ry` relativos ao canto de cima, à esquerda do minimapa). O `y` do jogo cresce
/// para o norte, o do desenho para baixo.
pub fn minimap_to_game(rx: f64, ry: f64, region_w: f64, region_h: f64, map: MapKind) -> (f64, f64) {
    let fx = (rx / region_w).clamp(0.0, 1.0);
    let fy = (ry / region_h).clamp(0.0, 1.0);
    let (w, h) = map.dims();
    (fx * w, h - fy * h)
}

/// O inverso, para o simulador desenhar.
pub fn game_to_minimap(gx: f64, gy: f64, region_w: f64, region_h: f64, map: MapKind) -> (f64, f64) {
    let (w, h) = map.dims();
    (gx / w * region_w, (h - gy) / h * region_h)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Medidas reais da referência.
    #[test]
    fn o_tamanho_do_minimapa_bate_com_as_medidas() {
        assert_eq!(minimap_size_for_height(1080.0, 0.0), 200);
        assert_eq!(minimap_size_for_height(1080.0, 3.0), 420);
        assert_eq!(minimap_size_for_height(1440.0, 0.0), 280);
        assert_eq!(minimap_size_for_height(1440.0, 3.0), 560);
        // gravações reais da referência: 1920x1080, MinimapScale 1,92 -> 341
        assert_eq!(minimap_size_for_height(1080.0, 1.92), 341);
    }

    #[test]
    fn o_ancoramento_a_direita_cabe_em_toda_a_faixa_de_escala() {
        for h in [768, 900, 1080, 1440, 2160] {
            let win = Rect::new(0, 0, h * 16 / 9, h);
            let cap = capture_bounds(win, MinimapSide::Right);
            let reg = minimap_region(win, 3.0, cap, MinimapSide::Right);
            assert!(region_fits(&reg, &cap), "altura {h}: {reg:?} fora de {cap:?}");
        }
    }

    // Um monitor à esquerda do principal tem x negativo; o resultado relativo não pode mudar.
    #[test]
    fn a_origem_negativa_nao_muda_a_regiao_relativa() {
        let a = Rect::new(0, 0, 1920, 1080);
        let b = Rect::new(-1920, -400, 1920, 1080);
        let (ca, cb) = (capture_bounds(a, MinimapSide::Right), capture_bounds(b, MinimapSide::Right));
        assert_eq!(minimap_region(a, 1.92, ca, MinimapSide::Right), minimap_region(b, 1.92, cb, MinimapSide::Right));
        assert_eq!(cb.x, -1920 + 1920 - 432);
    }

    #[test]
    fn minimapa_a_esquerda_ancora_na_borda_esquerda() {
        let win = Rect::new(100, 50, 1920, 1080);
        let cap = capture_bounds(win, MinimapSide::Left);
        assert_eq!(cap.x, 100);
        let reg = minimap_region(win, 1.92, cap, MinimapSide::Left);
        assert_eq!(reg.x, 0);
        assert_eq!(reg.y + reg.h, cap.h);
    }

    #[test]
    fn pixel_vira_posicao_no_mapa_e_volta() {
        let (gx, gy) = minimap_to_game(0.0, 0.0, 341.0, 341.0, MapKind::SummonersRift);
        assert_eq!((gx, gy), (0.0, 14980.0)); // canto de cima à esquerda = noroeste
        let (gx, gy) = minimap_to_game(341.0, 341.0, 341.0, 341.0, MapKind::SummonersRift);
        assert_eq!((gx, gy), (14870.0, 0.0));
        let (px, py) = game_to_minimap(7435.0, 7490.0, 341.0, 341.0, MapKind::SummonersRift);
        assert!((px - 170.5).abs() < 1e-9 && (py - 170.5).abs() < 1e-9);
    }

    #[test]
    fn mapa_desconhecido_nao_converte() {
        assert_eq!(MapKind::from_number(11), Some(MapKind::SummonersRift));
        assert_eq!(MapKind::from_number(30), None); // Arena
        assert_eq!(MapKind::from_number(33), None); // Swarm
    }
}

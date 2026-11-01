//! Um quadro capturado: pixels RGBA, de cima para baixo, sem preenchimento no fim das linhas.

#[derive(Clone, Debug)]
pub struct Frame {
    pub width: usize,
    pub height: usize,
    /// `width * height * 4` bytes, R G B A.
    pub data: Vec<u8>,
}

impl Frame {
    pub fn new(width: usize, height: usize) -> Self {
        Frame { width, height, data: vec![0; width * height * 4] }
    }

    /// Quadro de BGRA (como o GDI entrega) para RGBA, com alfa opaco. O canal alfa do GDI é indefinido; um alfa 0 apagaria a cor
    /// em qualquer conta que o respeite. Trocar de lugar duas vezes devolve o original: converta em um lado só.
    pub fn from_bgra(width: usize, height: usize, mut bgra: Vec<u8>) -> Self {
        for p in bgra.chunks_exact_mut(4) {
            p.swap(0, 2);
            p[3] = 255;
        }
        Frame { width, height, data: bgra }
    }

    #[inline]
    pub fn rgb(&self, x: usize, y: usize) -> [u8; 3] {
        let o = (y * self.width + x) * 4;
        [self.data[o], self.data[o + 1], self.data[o + 2]]
    }

    #[inline]
    pub fn set(&mut self, x: usize, y: usize, rgb: [u8; 3]) {
        let o = (y * self.width + x) * 4;
        self.data[o] = rgb[0];
        self.data[o + 1] = rgb[1];
        self.data[o + 2] = rgb[2];
        self.data[o + 3] = 255;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // O anel de aliado (verde-água) lido na ordem do GDI parece vermelho; sem a troca nenhum ícone aparece e o rastreio nunca começa.
    #[test]
    fn bgra_vira_rgba_com_alfa_opaco() {
        // verde-água (30,200,190) e vermelho (200,60,60) como o GDI entrega
        let f = Frame::from_bgra(2, 1, vec![190, 200, 30, 0, 60, 60, 200, 7]);
        assert_eq!(f.data, vec![30, 200, 190, 255, 200, 60, 60, 255]);
    }
}

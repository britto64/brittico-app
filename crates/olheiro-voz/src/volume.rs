//! Curva de volume pela distância no mapa, em unidades do jogo.
//!
//! Cheio até 900, zero em 1350. No meio a queda é quadrática: `t` vai de 0 a 1 nesse intervalo e o ganho
//! é `(1 - t)²`. Em 1125 (o meio) o volume é 0,25. Fora da curva, ou com número que não é distância, o
//! volume é 1: falha nunca silencia.

/// Distância até a qual o colega é ouvido por inteiro.
pub const CHEIO_ATE: f64 = 900.0;
/// Distância a partir da qual o colega some.
pub const ZERO_EM: f64 = 1350.0;

/// Distância em linha reta no mapa. `y` cresce para o norte, como no jogo; a conta não se importa com o sentido.
pub fn distancia(x1: f64, y1: f64, x2: f64, y2: f64) -> f64 {
    let (dx, dy) = (x1 - x2, y1 - y2);
    (dx * dx + dy * dy).sqrt()
}

/// Ganho de 0 a 1. Qualquer entrada que não seja uma distância de verdade devolve 1.
pub fn ganho(distancia: f64) -> f64 {
    if !distancia.is_finite() || distancia < 0.0 || CHEIO_ATE >= ZERO_EM {
        return 1.0;
    }
    if distancia <= CHEIO_ATE {
        return 1.0;
    }
    if distancia >= ZERO_EM {
        return 0.0;
    }
    let t = (distancia - CHEIO_ATE) / (ZERO_EM - CHEIO_ATE);
    (1.0 - t) * (1.0 - t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cheio_ate_900_e_zero_em_1350() {
        assert_eq!(ganho(0.0), 1.0);
        assert_eq!(ganho(900.0), 1.0);
        assert_eq!(ganho(899.0), 1.0);
        assert_eq!(ganho(1350.0), 0.0);
        assert_eq!(ganho(10_000.0), 0.0);
    }

    #[test]
    fn no_meio_do_caminho_o_volume_e_um_quarto() {
        let meio = (CHEIO_ATE + ZERO_EM) / 2.0;
        let g = ganho(meio);
        assert!((g - 0.25).abs() < 1e-12, "em {meio} o ganho foi {g}, esperava 0,25");
    }

    #[test]
    fn numero_ruim_nao_silencia() {
        assert_eq!(ganho(f64::NAN), 1.0);
        assert_eq!(ganho(f64::INFINITY), 1.0);
        assert_eq!(ganho(-10.0), 1.0);
    }

    #[test]
    fn a_distancia_e_em_linha_reta() {
        assert!((distancia(0.0, 0.0, 300.0, 400.0) - 500.0).abs() < 1e-9);
    }
}

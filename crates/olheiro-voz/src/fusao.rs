//! Junta o que vários olheiros viram do mesmo jogador.
//!
//! Vale a leitura mais fresca. Empate no instante: ganha a de maior confiança. Leitura com mais de 5 s,
//! ou com relógio adiantado demais, é ignorada. Sem posição fresca dos dois lados, o volume fica em 1.
//!
//! Um campeão não some de um canto do mapa e aparece no outro: uma leitura que exigiria correr (e dar Flash) mais do que
//! dá no tempo que passou fica em QUARENTENA e não mexe na posição. Ela só vale se uma leitura seguinte (de qualquer olheiro)
//! cair no mesmo lugar, que é o que um teleporte, um recall ou um respawn fazem; um nome errado de um quadro só não se repete.
//! Quem acabou de morrer ou de nascer de novo (o jogo diz) fica fora da regra: o ponto de volta é a base.

use crate::pacote::Ponto;
use crate::volume::{distancia, ganho};
use crate::{chave, VELHO_MS};
use std::collections::{HashMap, HashSet};

/// Quanto o relógio de quem manda pode estar à frente do nosso antes de a leitura ser recusada.
const FUTURO_MS: u64 = 2_000;

/// Quanto um campeão anda, em unidades do jogo por milissegundo (550 de velocidade, com bota e buff: um pouco mais)...
const VELOCIDADE: f64 = 0.8;
/// ...mais o que cabe de uma vez: um Flash (400), um dash e o erro de ler o centro de um ícone no minimapa.
const FOLGA: f64 = 1_800.0;
/// Uma leitura em quarentena é confirmada por outra a menos disto dela (dois ícones do mesmo campeão no minimapa).
const CONFIRMA_RAIO: f64 = 900.0;
/// A quarentena vale por este tempo; passou, a leitura esquecida.
const QUARENTENA_MS: u64 = 3_000;

#[derive(Clone, Debug)]
struct Guardada {
    ponto: Ponto,
    t: u64,
}

#[derive(Clone, Debug, Default)]
pub struct Fusao {
    por: HashMap<String, Guardada>,
    /// A leitura longe demais que espera uma segunda para valer, por jogador.
    quarentena: HashMap<String, Guardada>,
    /// Quem o jogo diz que está morto agora, e quem acabou de nascer (a próxima leitura vale sem conferir a distância).
    mortos: HashSet<String>,
    livres: HashSet<String>,
}

impl Fusao {
    pub fn nova() -> Self {
        Self::default()
    }

    /// Incorpora uma posição lida no instante `t_ms` (o `t` do pacote). `agora_ms` é o relógio de quem recebe.
    pub fn observar(&mut self, agora_ms: u64, mut ponto: Ponto, t_ms: u64) {
        if agora_ms.saturating_sub(t_ms) > VELHO_MS || t_ms > agora_ms.saturating_add(FUTURO_MS) {
            return;
        }
        if !ponto.x.is_finite() || !ponto.y.is_finite() || !ponto.c.is_finite() {
            return;
        }
        let k = chave(&ponto.k);
        if k.is_empty() {
            return;
        }
        ponto.k = k;
        ponto.c = ponto.c.clamp(0.0, 1.0);
        if !self.plausivel(&ponto, t_ms) {
            return;
        }
        let fica = match self.por.get(&ponto.k) {
            Some(velha) if t_ms < velha.t || (t_ms == velha.t && ponto.c <= velha.ponto.c) => false,
            _ => true,
        };
        if fica {
            self.por.insert(ponto.k.clone(), Guardada { ponto, t: t_ms });
        }
    }

    /// Quem o jogo diz que está morto agora (a lista inteira, a cada quadro). Quem sai da lista acabou de nascer.
    pub fn mortos(&mut self, agora: impl IntoIterator<Item = String>) {
        let agora: HashSet<String> = agora.into_iter().map(|k| chave(&k)).collect();
        for k in self.mortos.difference(&agora) {
            self.livres.insert(k.clone());
            self.quarentena.remove(k);
        }
        self.mortos = agora;
    }

    /// A leitura é possível para quem estava onde estava há pouco? Se não, vai para a quarentena (e `false`); se uma leitura que
    /// espera concorda com esta, as duas valem.
    fn plausivel(&mut self, ponto: &Ponto, t_ms: u64) -> bool {
        let k = &ponto.k;
        if self.mortos.contains(k) || self.livres.contains(k) {
            // sem regra enquanto morto; a primeira leitura depois do respawn vale e fecha a exceção
            if !self.mortos.contains(k) && ponto.s != "held" {
                self.livres.remove(k);
            }
            self.quarentena.remove(k);
            return true;
        }
        let Some(velha) = self.por.get(k) else { return true };
        let dt = t_ms.saturating_sub(velha.t) as f64;
        let longe = distancia(velha.ponto.x, velha.ponto.y, ponto.x, ponto.y);
        if longe <= VELOCIDADE * dt + FOLGA {
            self.quarentena.remove(k);
            return true;
        }
        // uma "retida" repete o último lugar visto, não confirma nada
        if ponto.s == "held" {
            return false;
        }
        if let Some(espera) = self.quarentena.get(k) {
            let recente = t_ms.saturating_sub(espera.t) <= QUARENTENA_MS;
            if recente && distancia(espera.ponto.x, espera.ponto.y, ponto.x, ponto.y) <= CONFIRMA_RAIO {
                self.quarentena.remove(k);
                return true;
            }
        }
        self.quarentena.insert(k.clone(), Guardada { ponto: ponto.clone(), t: t_ms });
        false
    }

    /// As chaves que ainda estão na mesa. Quem chama confere a idade com `posicao` ou `volume`.
    pub fn chaves(&self) -> Vec<String> {
        self.por.keys().cloned().collect()
    }

    /// Onde `k` está agora, se a leitura ainda está fresca.
    pub fn posicao(&mut self, agora_ms: u64, k: &str) -> Option<(f64, f64)> {
        let k = chave(k);
        self.expira(agora_ms, &k);
        self.por.get(&k).map(|g| (g.ponto.x, g.ponto.y))
    }

    /// Quanto `ouvinte` ouve de `falante`, de 0 a 1. Sem posição fresca de um dos dois, 1.
    pub fn volume(&mut self, agora_ms: u64, ouvinte: &str, falante: &str) -> f64 {
        match (self.posicao(agora_ms, ouvinte), self.posicao(agora_ms, falante)) {
            (Some((x1, y1)), Some((x2, y2))) => ganho(distancia(x1, y1, x2, y2)),
            _ => 1.0,
        }
    }

    fn expira(&mut self, agora_ms: u64, k: &str) {
        let velha = self.por.get(k).is_some_and(|g| agora_ms.saturating_sub(g.t) > VELHO_MS);
        if velha {
            self.por.remove(k);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(k: &str, x: f64, y: f64, c: f64) -> Ponto {
        Ponto { k: k.into(), x, y, c, s: "seen".into() }
    }

    #[test]
    fn a_leitura_mais_fresca_vence_mesmo_com_menos_confianca() {
        let mut f = Fusao::nova();
        f.observar(1_000, p("Ana#BR1", 0.0, 0.0, 0.99), 1_000);
        f.observar(2_000, p("ana#br1", 100.0, 0.0, 0.2), 2_000);
        assert_eq!(f.posicao(2_000, "ANA#br1"), Some((100.0, 0.0)));
    }

    #[test]
    fn no_empate_de_instante_fica_a_maior_confianca() {
        let mut f = Fusao::nova();
        f.observar(1_000, p("ana#br1", 0.0, 0.0, 0.4), 1_000);
        f.observar(1_000, p("ana#br1", 50.0, 0.0, 0.9), 1_000);
        f.observar(1_000, p("ana#br1", 80.0, 0.0, 0.3), 1_000);
        assert_eq!(f.posicao(1_000, "ana#br1"), Some((50.0, 0.0)));
    }

    #[test]
    fn pacote_atrasado_nao_desfaz_a_posicao_nova() {
        let mut f = Fusao::nova();
        f.observar(5_000, p("ana#br1", 10.0, 0.0, 0.5), 5_000);
        f.observar(5_100, p("ana#br1", 0.0, 0.0, 1.0), 4_000);
        assert_eq!(f.posicao(5_100, "ana#br1"), Some((10.0, 0.0)));
    }

    #[test]
    fn mais_de_cinco_segundos_deixa_de_valer_e_o_volume_volta_a_um() {
        let mut f = Fusao::nova();
        f.observar(10_000, p("ana#br1", 0.0, 0.0, 1.0), 10_000);
        f.observar(10_000, p("bia#br1", 0.0, 5_000.0, 1.0), 10_000);
        // no instante exato dos 5 s ainda vale: estão longe, volume zero
        assert_eq!(f.volume(15_000, "ana#br1", "bia#br1"), 0.0);
        // passou de 5 s: a posição some e a falha não silencia
        assert_eq!(f.volume(15_001, "ana#br1", "bia#br1"), 1.0);
        assert!(f.posicao(15_001, "ana#br1").is_none());
    }

    #[test]
    fn leitura_que_ja_nasce_velha_ou_no_futuro_e_ignorada() {
        let mut f = Fusao::nova();
        f.observar(10_000, p("ana#br1", 1.0, 0.0, 1.0), 10_000 - VELHO_MS - 1);
        f.observar(10_000, p("bia#br1", 2.0, 0.0, 1.0), 10_000 + FUTURO_MS + 1);
        assert!(f.posicao(10_000, "ana#br1").is_none());
        assert!(f.posicao(10_000, "bia#br1").is_none());
        assert_eq!(f.volume(10_000, "ana#br1", "bia#br1"), 1.0);
    }

    #[test]
    fn o_volume_segue_a_curva_e_falta_de_posicao_nao_silencia() {
        let mut f = Fusao::nova();
        f.observar(1_000, p("ana#br1", 7000.0, 7000.0, 1.0), 1_000);
        f.observar(1_000, p("bia#br1", 7000.0, 7000.0 + 1125.0, 1.0), 1_000);
        let g = f.volume(1_000, "ana#br1", "bia#br1");
        assert!((g - 0.25).abs() < 1e-9, "{g}");
        assert_eq!(f.volume(1_000, "ana#br1", "caio#br1"), 1.0);
    }

    #[test]
    fn perto_e_longe_batem_com_os_extremos() {
        let mut f = Fusao::nova();
        let t = 1_000;
        f.observar(t, p("ana#br1", 0.0, 0.0, 1.0), t);
        f.observar(t, p("perto#br1", 0.0, 900.0, 1.0), t);
        f.observar(t, p("longe#br1", 0.0, 1350.0, 1.0), t);
        assert_eq!(f.volume(t, "ana#br1", "perto#br1"), 1.0);
        assert_eq!(f.volume(t, "ana#br1", "longe#br1"), 0.0);
    }

    #[test]
    fn um_salto_impossivel_fica_em_quarentena_e_some_se_nao_se_repete() {
        let mut f = Fusao::nova();
        f.observar(1_000, p("ana#br1", 2_000.0, 2_000.0, 0.9), 1_000);
        // 12 mil unidades em 200 ms: um nome errado de um quadro
        f.observar(1_200, p("ana#br1", 12_000.0, 12_000.0, 0.9), 1_200);
        assert_eq!(f.posicao(1_200, "ana#br1"), Some((2_000.0, 2_000.0)));
        // o quadro seguinte volta ao lugar de sempre
        f.observar(1_400, p("ana#br1", 2_100.0, 2_000.0, 0.9), 1_400);
        assert_eq!(f.posicao(1_400, "ana#br1"), Some((2_100.0, 2_000.0)));
    }

    #[test]
    fn um_teleporte_que_se_repete_vale_na_segunda_leitura() {
        let mut f = Fusao::nova();
        f.observar(1_000, p("ana#br1", 2_000.0, 2_000.0, 0.9), 1_000);
        f.observar(1_200, p("ana#br1", 12_000.0, 12_000.0, 0.9), 1_200);
        f.observar(1_400, p("ana#br1", 12_100.0, 12_050.0, 0.9), 1_400);
        assert_eq!(f.posicao(1_400, "ana#br1"), Some((12_100.0, 12_050.0)));
    }

    #[test]
    fn dois_olheiros_que_concordam_confirmam_o_salto() {
        let mut f = Fusao::nova();
        f.observar(1_000, p("ana#br1", 2_000.0, 2_000.0, 0.9), 1_000);
        f.observar(1_100, p("ana#br1", 12_000.0, 12_000.0, 0.6), 1_100);
        f.observar(1_150, p("ana#br1", 12_050.0, 12_000.0, 0.9), 1_150);
        assert_eq!(f.posicao(1_150, "ana#br1"), Some((12_050.0, 12_000.0)));
    }

    #[test]
    fn correr_e_dar_flash_cabe_e_nao_passa_pela_quarentena() {
        let mut f = Fusao::nova();
        f.observar(1_000, p("ana#br1", 2_000.0, 2_000.0, 0.9), 1_000);
        // 2 s depois: 1.600 de corrida e mais um Flash
        f.observar(3_000, p("ana#br1", 2_000.0, 4_000.0, 0.9), 3_000);
        assert_eq!(f.posicao(3_000, "ana#br1"), Some((2_000.0, 4_000.0)));
    }

    #[test]
    fn quem_morreu_e_nasceu_na_base_vale_na_primeira_leitura() {
        let mut f = Fusao::nova();
        f.observar(1_000, p("ana#br1", 9_000.0, 9_000.0, 0.9), 1_000);
        f.mortos(["ana#br1".to_string()]);
        f.observar(1_500, p("ana#br1", 9_000.0, 9_000.0, 0.3), 1_500);
        f.mortos(Vec::<String>::new());
        f.observar(2_000, p("ana#br1", 500.0, 500.0, 0.9), 2_000);
        assert_eq!(f.posicao(2_000, "ana#br1"), Some((500.0, 500.0)), "o respawn não espera confirmação");
        // e a exceção fecha: um salto depois disso volta a precisar de confirmação
        f.observar(2_200, p("ana#br1", 12_000.0, 12_000.0, 0.9), 2_200);
        assert_eq!(f.posicao(2_200, "ana#br1"), Some((500.0, 500.0)));
    }

    #[test]
    fn uma_retida_nao_confirma_um_salto() {
        let mut f = Fusao::nova();
        f.observar(1_000, p("ana#br1", 2_000.0, 2_000.0, 0.9), 1_000);
        f.observar(1_200, p("ana#br1", 12_000.0, 12_000.0, 0.9), 1_200);
        let mut r = p("ana#br1", 12_000.0, 12_000.0, 0.3);
        r.s = "held".into();
        f.observar(1_400, r, 1_400);
        assert_eq!(f.posicao(1_400, "ana#br1"), Some((2_000.0, 2_000.0)));
    }
}

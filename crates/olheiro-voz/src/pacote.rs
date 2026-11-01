//! O recado que viaja no canal de dados da sala, sem garantia de entrega.
//!
//! Um olheiro vê vários aliados no mesmo quadro, então o pacote leva a lista inteira e o instante da
//! leitura. O canal pode reordenar e perder; o instante é o que a fusão usa para ficar com a mais fresca.
//!
//! ```json
//! { "t": 1710000000000, "seq": 12, "pos": [ { "k": "ana#br1", "x": 7000, "y": 7000, "c": 0.9, "s": "seen" } ] }
//! ```
//!
//! `k` é o Riot ID normalizado, `c` a confiança de 0 a 1, `s` o estado da leitura (`seen`, `under`, `held`, …).

use serde::{Deserialize, Serialize};

/// Uma posição, no formato curto do plano.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Ponto {
    pub k: String,
    pub x: f64,
    pub y: f64,
    /// Confiança de que o nome está certo, de 0 a 1.
    pub c: f64,
    /// "seen" | "under" | "held" | "inferred", ou o que a leitura disser.
    pub s: String,
}

/// Uma leitura do minimapa, pronta para publicar.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Pacote {
    /// Instantes desde a época Unix, em milissegundos, no relógio de quem leu.
    pub t: u64,
    /// Sobe a cada envio de quem manda. Só para olhar o teste; a fusão não usa.
    pub seq: u64,
    pub pos: Vec<Ponto>,
}

/// No máximo dez jogadores numa sala. O que passar disto não entra no pacote.
const MAX_PONTOS: usize = 10;

pub fn codificar(pacote: &Pacote) -> Vec<u8> {
    let mut enxuto = pacote.clone();
    enxuto.pos.truncate(MAX_PONTOS);
    serde_json::to_vec(&enxuto).unwrap_or_default()
}

/// `None` quando o que chegou não é um pacote nosso. Pacote perdido ou lixo não pode derrubar quem ouve.
pub fn decodificar(bytes: &[u8]) -> Option<Pacote> {
    let mut pacote: Pacote = serde_json::from_slice(bytes).ok()?;
    pacote.pos.truncate(MAX_PONTOS);
    Some(pacote)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn o_pacote_vai_e_volta_com_os_nomes_curtos() {
        let p = Pacote {
            t: 10,
            seq: 3,
            pos: vec![Ponto { k: "ana#br1".into(), x: 1.5, y: 2.5, c: 0.8, s: "seen".into() }],
        };
        let bytes = codificar(&p);
        let texto = String::from_utf8(bytes.clone()).unwrap();
        assert!(texto.contains("\"k\"") && texto.contains("\"s\""));
        assert_eq!(decodificar(&bytes).as_ref(), Some(&p));
    }

    #[test]
    fn lixo_nao_vira_pacote() {
        assert!(decodificar(b"").is_none());
        assert!(decodificar(b"nao e json").is_none());
        assert!(decodificar(b"{\"t\":1}").is_none());
    }

    #[test]
    fn passa_de_dez_e_cortado() {
        let pos = (0..12).map(|i| Ponto { k: format!("p{i}#br1"), x: 0.0, y: 0.0, c: 1.0, s: "seen".into() }).collect();
        let p = Pacote { t: 1, seq: 1, pos };
        let de_volta = decodificar(&codificar(&p)).unwrap();
        assert_eq!(de_volta.pos.len(), 10);
    }
}

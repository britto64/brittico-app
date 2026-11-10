//! A pasta do Olheiro e o que a pessoa escolheu (os ajustes), num `ajustes.json` dentro dela.
//!
//! Tudo aqui é da máquina de quem joga: nada sobe para servidor nenhum. Um arquivo estragado vira o padrão, sem quebrar o app.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

/// Onde o Olheiro guarda os ícones baixados, o registro, os pacotes de erro e os ajustes.
/// `OLHEIRO_DIR` troca a pasta (os testes e as capturas usam).
pub fn dir() -> PathBuf {
    if let Ok(d) = std::env::var("OLHEIRO_DIR") {
        return PathBuf::from(d);
    }
    #[cfg(windows)]
    {
        if let Ok(a) = std::env::var("APPDATA") {
            return PathBuf::from(a).join("Brittico").join("olheiro");
        }
    }
    if let Ok(x) = std::env::var("XDG_CONFIG_HOME") {
        return PathBuf::from(x).join("brittico").join("olheiro");
    }
    PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".into())).join(".config").join("brittico").join("olheiro")
}

/// A pasta do LoL que a pessoa escolheu, se escolheu (só quando o app não acha sozinho).
pub fn lol_escolhida() -> Option<String> {
    Ajustes::carregar().pasta_lol.filter(|s| !s.trim().is_empty())
}

/// O porteiro do Brittico, na Contabo (o mesmo endereço é o do LiveKit: o Caddy manda `/entrar` e `/saude` para o porteiro).
pub const PORTEIRO_PADRAO: &str = "https://voz.brittico.xyz";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Lado {
    #[default]
    Direita,
    Esquerda,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Ajustes {
    /// O endereço do porteiro (`https://…`). Vazio vira o [`PORTEIRO_PADRAO`] ao carregar.
    pub porteiro: String,
    /// Código do grupo (opcional): só encontra a sala quem tiver o mesmo código.
    pub codigo_grupo: String,
    /// O nome do microfone e da saída escolhidos. Vazio: o padrão do sistema.
    pub microfone: String,
    pub saida: String,
    /// De 0 a 2: quanto o microfone é amplificado antes de ir para a sala.
    pub ganho_microfone: f32,
    /// De 0 a 2: o volume geral de quem fala.
    pub volume_geral: f32,
    /// Volume de cada pessoa (pela chave do Riot ID), de 0 a 2. Quem não está aqui fica em 1.
    pub volumes: BTreeMap<String, f32>,
    /// Quem você mutou (pela chave do Riot ID): não toca, perto ou longe.
    pub mutados: BTreeSet<String>,
    pub lado_minimapa: Lado,
    /// Gravar o pacote de erro na próxima partida.
    pub pacote_erro: bool,
    /// Ligar o Olheiro quando o Brittico abre.
    pub ligar_ao_abrir: bool,
    /// Já passou pela apresentação.
    pub apresentado: bool,
    pub pasta_lol: Option<String>,
    /// De 0 (rígida: só aceita o que tem certeza) a 1 (sensível: aceita mais, e erra mais). 0,5 é o ajuste de fábrica.
    pub sensibilidade: f32,
}

impl Default for Ajustes {
    fn default() -> Self {
        Ajustes {
            porteiro: std::env::var("OLHEIRO_PORTEIRO").unwrap_or_else(|_| PORTEIRO_PADRAO.into()),
            codigo_grupo: String::new(),
            microfone: String::new(),
            saida: String::new(),
            ganho_microfone: 1.0,
            volume_geral: 1.0,
            volumes: BTreeMap::new(),
            mutados: BTreeSet::new(),
            lado_minimapa: Lado::Direita,
            pacote_erro: false,
            ligar_ao_abrir: true,
            apresentado: false,
            pasta_lol: None,
            sensibilidade: 0.5,
        }
    }
}

impl Ajustes {
    pub fn caminho() -> PathBuf {
        dir().join("ajustes.json")
    }

    pub fn carregar() -> Ajustes {
        let mut a: Ajustes = std::fs::read_to_string(Self::caminho()).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        // `OLHEIRO_PORTEIRO` manda quando existe (os testes e quem roda o servidor em casa)
        if let Ok(p) = std::env::var("OLHEIRO_PORTEIRO") {
            a.porteiro = p;
        } else if a.porteiro.trim().is_empty() {
            a.porteiro = PORTEIRO_PADRAO.into();
        }
        a.ganho_microfone = a.ganho_microfone.clamp(0.0, 2.0);
        a.volume_geral = a.volume_geral.clamp(0.0, 2.0);
        a.sensibilidade = if a.sensibilidade.is_finite() { a.sensibilidade.clamp(0.0, 1.0) } else { 0.5 };
        a
    }

    pub fn salvar(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(dir())?;
        let tmp = dir().join("ajustes.json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(self).unwrap_or_default())?;
        std::fs::rename(tmp, Self::caminho())
    }

    /// O volume que a pessoa deu a alguém (1 se nunca mexeu).
    pub fn volume_de(&self, chave: &str) -> f32 {
        self.volumes.get(chave).copied().unwrap_or(1.0).clamp(0.0, 2.0)
    }

    /// O porteiro parece um endereço de verdade? (Só `https://`, ou `http://` para o próprio PC nos testes.)
    pub fn porteiro_valido(&self) -> bool {
        let p = self.porteiro.trim();
        p.starts_with("https://") && p.len() > 10 || p.starts_with("http://127.0.0.1") || p.starts_with("http://localhost")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arquivo_estragado_vira_o_padrao() {
        let a: Ajustes = serde_json::from_str("{\"volume_geral\": \"alto\"}").unwrap_or_default();
        assert_eq!(a.volume_geral, 1.0);
        let b: Ajustes = serde_json::from_str("{\"codigo_grupo\": \"ab\"}").unwrap();
        assert_eq!((b.codigo_grupo.as_str(), b.ligar_ao_abrir), ("ab", true), "o que falta vem do padrão");
    }

    #[test]
    fn so_aceita_porteiro_com_https_ou_no_proprio_pc() {
        let mut a = Ajustes::default();
        for (p, ok) in [("https://voz.exemplo.com", true), ("http://127.0.0.1:8090", true), ("http://1.2.3.4:8090", false), ("", false), ("voz.exemplo.com", false)] {
            a.porteiro = p.into();
            assert_eq!(a.porteiro_valido(), ok, "{p}");
        }
    }
}

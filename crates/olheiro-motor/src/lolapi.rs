//! Falar com o LoL de verdade: a API local do jogo (Live Client Data), o cliente (LCU) e o `game.cfg`.
//!
//! O jogo publica `https://127.0.0.1:2999/liveclientdata/…` durante a partida, sem senha. O cliente guarda a porta e a senha dele no
//! `lockfile` da pasta de instalação. Nada daqui lê a memória do jogo nem injeta coisa alguma.

use crate::lol::{self, GameInfo, LivePlayer};
use crate::net;
use crate::session::Lol;
use olheiro_core::protocol::Phase;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::time::Duration;

const DEFAULT_LIVE: &str = "https://127.0.0.1:2999";

pub struct RealLol {
    agent: ureq::Agent,
    live: String,
    install: Option<PathBuf>,
}

impl Default for RealLol {
    fn default() -> Self {
        Self::new()
    }
}

impl RealLol {
    /// `OLHEIRO_LIVE_API` troca o endereço da API do jogo (só para testar com um jogo de mentira).
    pub fn new() -> Self {
        RealLol {
            agent: net::local(Duration::from_secs(2)),
            live: std::env::var("OLHEIRO_LIVE_API").unwrap_or_else(|_| DEFAULT_LIVE.into()),
            install: None,
        }
    }

    fn live_get(&self, path: &str) -> Option<Value> {
        let resp = self.agent.get(format!("{}{}", self.live, path)).call().ok()?;
        serde_json::from_str(&net::read_text(resp, 4 * 1024 * 1024)?).ok()
    }

    /// A pasta do LoL: a que o jogador escolheu (`OLHEIRO_LOL_DIR`), ou a primeira dos lugares comuns que tem o cliente.
    pub fn install_dir(&mut self) -> Option<PathBuf> {
        if let Some(d) = &self.install {
            if looks_like_install(d) {
                return Some(d.clone());
            }
        }
        let found = candidates().into_iter().find(|p| looks_like_install(p));
        if found.is_some() {
            self.install = found.clone();
        }
        found
    }

    fn lcu_get(&mut self, path: &str) -> Option<Value> {
        let dir = self.install_dir()?;
        let (port, password) = lol::parse_lockfile(&std::fs::read_to_string(dir.join("lockfile")).ok()?)?;
        let auth = format!("Basic {}", base64(format!("riot:{password}").as_bytes()));
        let resp = self.agent.get(format!("https://127.0.0.1:{port}{path}")).header("authorization", auth).call().ok()?;
        serde_json::from_str(&net::read_text(resp, 1024 * 1024)?).ok()
    }
}

pub fn looks_like_install(dir: &Path) -> bool {
    dir.join("lockfile").exists() || dir.join("LeagueClient.exe").exists() || dir.join("Config").join("game.cfg").exists()
}

/// Os lugares onde o LoL costuma estar: a variável de ambiente e os discos mais comuns.
fn candidates() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(d) = std::env::var("OLHEIRO_LOL_DIR") {
        out.push(PathBuf::from(d));
    }
    // a que a pessoa escolheu na janela
    if let Some(d) = crate::ajustes::lol_escolhida() {
        out.push(PathBuf::from(d));
    }
    for drive in ["C", "D", "E", "F", "G", "H"] {
        for tail in [r"Riot Games\League of Legends", r"Program Files\Riot Games\League of Legends", r"Program Files (x86)\Riot Games\League of Legends", r"Games\Riot Games\League of Legends"] {
            out.push(PathBuf::from(format!(r"{drive}:\{tail}")));
        }
    }
    out
}

impl Lol for RealLol {
    fn phase(&mut self) -> Phase {
        // A API do jogo só responde com a partida carregada; o cliente aberto responde pelo lockfile.
        if self.live_get("/liveclientdata/gamestats").is_some() {
            return Phase::Ingame;
        }
        match self.lcu_get("/lol-gameflow/v1/gameflow-phase") {
            Some(Value::String(p)) if p == "InProgress" || p == "Reconnect" || p == "GameStart" => Phase::Loading,
            Some(_) => Phase::Client,
            None => Phase::None,
        }
    }

    fn client_riot_id(&mut self) -> Option<String> {
        let v = self.lcu_get("/lol-summoner/v1/current-summoner")?;
        let name = v.get("gameName").and_then(Value::as_str)?;
        let tag = v.get("tagLine").and_then(Value::as_str)?;
        (!name.is_empty() && !tag.is_empty()).then(|| format!("{name}#{tag}"))
    }

    fn game(&mut self) -> Option<GameInfo> {
        lol::parse_game(&self.live_get("/liveclientdata/allgamedata")?)
    }

    fn players(&mut self) -> Option<Vec<LivePlayer>> {
        let v = self.live_get("/liveclientdata/playerlist")?;
        let list = lol::parse_players(&v);
        (!list.is_empty()).then_some(list)
    }

    /// `(fila, personalizada)`: a fila do Flex é 440; uma personalizada tem `isCustomGame`. Melhor esforço: sem cliente, `None`.
    fn queue(&mut self) -> Option<(i64, bool)> {
        let v = self.lcu_get("/lol-gameflow/v1/session")?;
        let gd = v.get("gameData")?;
        let id = gd.pointer("/queue/id").and_then(Value::as_i64).unwrap_or(-1);
        let custom = gd.get("isCustomGame").and_then(Value::as_bool).unwrap_or(false);
        Some((if custom { 0 } else { id }, custom))
    }

    fn minimap_scale(&mut self) -> Option<f64> {
        let dir = self.install_dir()?;
        lol::parse_minimap_scale(&std::fs::read_to_string(dir.join("Config").join("game.cfg")).ok()?)
    }
}

/// Base64 padrão (RFC 4648): só para o cabeçalho `Authorization: Basic` do cliente; evita uma dependência por 20 linhas.
pub fn base64(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for c in bytes.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if c.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if c.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    // Os vetores de teste do RFC 4648.
    #[test]
    fn base64_bate_com_o_rfc() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
        assert_eq!(base64(b"riot:s3nh4"), "cmlvdDpzM25oNA==");
    }
}

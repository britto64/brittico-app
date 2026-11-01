//! O que o app e o bot dizem um ao outro (o contrato é `docs/PROXIMIDADE.md` §6.2 do site; o lado do bot é `diskenzy-prox-rules.ts`).
//!
//! Só dados: nada aqui abre conexão. O bot trata tudo o que o app manda como lixo até prova em contrário e recalcula as chaves dos
//! jogadores a partir do Riot ID, então o que importa aqui é a forma, não a confiança.

use serde::{Deserialize, Serialize};

/// Onde o WebSocket do olheiro mora, no servidor da API.
pub const SCOUT_PATH: &str = "/api/diskenzy/v2/prox/scout";

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Phase {
    /// O LoL está fechado.
    None,
    /// O cliente está aberto (lobby, seleção de campeões).
    Client,
    /// A partida está carregando.
    Loading,
    /// Em partida.
    Ingame,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Calib {
    pub ok: bool,
    pub minimap_px: u32,
    pub icon_px: u32,
    /// Em português, para o jogador: o que mudar (vazio quando está tudo certo).
    pub problem: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RosterPlayer {
    pub riot_id: String,
    pub champ: String,
    /// "ORDER" (azul) ou "CHAOS" (vermelho).
    pub team: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Pos {
    pub k: String,
    /// Unidades do jogo; `y` cresce para o norte.
    pub x: f64,
    pub y: f64,
    /// Confiança de que o nome está certo, de 0 a 1.
    pub c: f64,
    /// "seen" | "under" | "inferred" | "held"
    pub s: String,
}

/// O que o app manda ao bot.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "t", rename_all = "lowercase")]
pub enum ToBot {
    Hello {
        token: String,
        app: String,
        os: String,
    },
    #[serde(rename_all = "camelCase")]
    Status {
        phase: Phase,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        riot_id: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        calib: Option<Calib>,
    },
    #[serde(rename_all = "camelCase")]
    Roster {
        game_id: String,
        map: i64,
        mode: String,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        queue: Option<i64>,
        custom: bool,
        /// O Riot ID de quem roda o app, como o jogo escreve.
        you: String,
        players: Vec<RosterPlayer>,
    },
    Frame {
        seq: u64,
        /// Segundos de jogo.
        gt: f64,
        pos: Vec<Pos>,
        dead: Vec<String>,
    },
}

#[derive(Deserialize, Clone, Debug, PartialEq)]
pub struct Profile {
    pub id: i64,
}

/// O que o bot manda ao app.
#[derive(Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "t")]
pub enum FromBot {
    #[serde(rename = "welcome", rename_all = "camelCase")]
    Welcome {
        profile: Profile,
        #[serde(default)]
        hz: u32,
        #[serde(default)]
        min_version: String,
        #[serde(default)]
        up_to_date: bool,
    },
    #[serde(rename = "bye")]
    Bye {
        #[serde(default)]
        reason: String,
    },
    #[serde(other)]
    Other,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn to_json(m: &ToBot) -> serde_json::Value {
        serde_json::to_value(m).unwrap()
    }

    // O bot lê `t`, `token`, `app`, `phase`, `riotId`… por nome: se um deles mudar aqui, o bot ignora o recado em silêncio.
    #[test]
    fn o_hello_tem_a_forma_que_o_bot_espera() {
        let v = to_json(&ToBot::Hello { token: "abc".into(), app: "olheiro/0.1.0".into(), os: "windows".into() });
        assert_eq!(v, json!({ "t": "hello", "token": "abc", "app": "olheiro/0.1.0", "os": "windows" }));
    }

    #[test]
    fn o_status_omite_o_que_nao_tem_e_usa_camel_case() {
        let v = to_json(&ToBot::Status { phase: Phase::Client, riot_id: None, calib: None });
        assert_eq!(v, json!({ "t": "status", "phase": "client" }));
        let v = to_json(&ToBot::Status {
            phase: Phase::Ingame,
            riot_id: Some("Kennzy#BR1".into()),
            calib: Some(Calib { ok: true, minimap_px: 341, icon_px: 30, problem: String::new() }),
        });
        assert_eq!(v["riotId"], "Kennzy#BR1");
        assert_eq!(v["calib"], json!({ "ok": true, "minimapPx": 341, "iconPx": 30, "problem": "" }));
    }

    #[test]
    fn o_elenco_e_o_quadro_tem_a_forma_que_o_bot_espera() {
        let v = to_json(&ToBot::Roster {
            game_id: "BR1_1".into(),
            map: 11,
            mode: "CLASSIC".into(),
            queue: Some(440),
            custom: false,
            you: "Kennzy#BR1".into(),
            players: vec![RosterPlayer { riot_id: "Kennzy#BR1".into(), champ: "Ahri".into(), team: "ORDER".into() }],
        });
        assert_eq!(v["t"], "roster");
        assert_eq!(v["gameId"], "BR1_1");
        assert_eq!(v["players"][0], json!({ "riotId": "Kennzy#BR1", "champ": "Ahri", "team": "ORDER" }));
        let f = to_json(&ToBot::Frame {
            seq: 7,
            gt: 61.5,
            pos: vec![Pos { k: "kennzy#br1".into(), x: 7000.0, y: 7000.0, c: 0.9, s: "seen".into() }],
            dead: vec!["mylon#br1".into()],
        });
        assert_eq!(f, json!({ "t": "frame", "seq": 7, "gt": 61.5, "pos": [{ "k": "kennzy#br1", "x": 7000.0, "y": 7000.0, "c": 0.9, "s": "seen" }], "dead": ["mylon#br1"] }));
    }

    #[test]
    fn le_as_boas_vindas_do_bot_e_ignora_o_que_nao_conhece() {
        let w: FromBot = serde_json::from_str(r#"{"t":"welcome","profile":{"id":2},"hz":5,"minVersion":"0.1.0","upToDate":true}"#).unwrap();
        assert_eq!(w, FromBot::Welcome { profile: Profile { id: 2 }, hz: 5, min_version: "0.1.0".into(), up_to_date: true });
        assert_eq!(serde_json::from_str::<FromBot>(r#"{"t":"algo-novo","x":1}"#).unwrap(), FromBot::Other);
        assert_eq!(serde_json::from_str::<FromBot>(r#"{"t":"bye","reason":"x"}"#).unwrap(), FromBot::Bye { reason: "x".into() });
    }
}

//! O LoL visto de fora: o que a API local do jogo devolve (Live Client Data), o `game.cfg` e o `lockfile` do cliente.
//!
//! Só leitura e só o que o próprio jogo publica: nada de memória, nada de injeção. Aqui moram as partes PURAS (JSON de entrada,
//! texto de saída), que se testam sem LoL; falar com a API de verdade é de `lolapi.rs`.

use olheiro_core::identity::{find_in_roster, read_identity, Identity, RawIdentity};
use serde_json::Value;

/// Um jogador da partida, como a API local o lista (os dez, dos dois times).
#[derive(Clone, Debug)]
pub struct LivePlayer {
    pub identity: Identity,
    /// O nome do campeão como o jogo mostra (pode vir traduzido).
    pub champion: String,
    /// O nome da pasta do campeão no Community Dragon (`ahri`, `leesin`, `monkeyking`…).
    pub alias: String,
    /// Hoje o minimapa usa o ícone base, qualquer que seja a skin; guardado para o dia em que isso mudar.
    #[allow(dead_code)]
    pub skin_id: i64,
    /// "ORDER" (azul) ou "CHAOS" (vermelho).
    pub team: String,
    pub is_dead: bool,
    #[cfg_attr(not(test), allow(dead_code))]
    pub respawn: f64,
}

/// A partida inteira, como a API local a conta no começo.
#[derive(Clone, Debug)]
pub struct GameInfo {
    pub map_number: i64,
    pub mode: String,
    pub game_time: f64,
    /// Quem roda o app.
    pub you: Identity,
    pub players: Vec<LivePlayer>,
}

fn text(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(str::to_string)
}

/// Os campos de nome que esta versão do jogo trouxer (o modo streamer troca `summonerName` pelo nome do campeão; os `riotId*` ficam).
pub fn raw_identity(v: &Value) -> RawIdentity {
    RawIdentity {
        summoner_name: text(v, "summonerName"),
        riot_id: text(v, "riotId"),
        riot_id_game_name: text(v, "riotIdGameName"),
        riot_id_tag_line: text(v, "riotIdTagLine"),
    }
}

/// O nome da pasta do campeão no Community Dragon: `rawChampionName` é `game_character_displayname_<Alias>` (o Wukong é `MonkeyKing`).
/// Sem ele, o nome mostrado só com letras e números, que é o alias de quase todos.
pub fn champion_alias(raw: Option<&str>, name: &str) -> String {
    let base = match raw.and_then(|r| r.strip_prefix("game_character_displayname_")) {
        Some(a) => a,
        None => name,
    };
    base.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>().to_lowercase()
}

pub fn parse_player(v: &Value) -> Option<LivePlayer> {
    let identity = read_identity(&raw_identity(v))?;
    let champion = text(v, "championName").unwrap_or_default();
    let team = text(v, "team")?;
    if team != "ORDER" && team != "CHAOS" {
        return None;
    }
    Some(LivePlayer {
        identity,
        alias: champion_alias(v.get("rawChampionName").and_then(Value::as_str), &champion),
        champion,
        skin_id: v.get("skinID").and_then(Value::as_i64).unwrap_or(0),
        team,
        is_dead: v.get("isDead").and_then(Value::as_bool).unwrap_or(false),
        respawn: v.get("respawnTimer").and_then(Value::as_f64).unwrap_or(0.0),
    })
}

/// `/playerlist` (um vetor) ou `allPlayers` de `/allgamedata`. Quem não dá para ler fica de fora.
pub fn parse_players(list: &Value) -> Vec<LivePlayer> {
    list.as_array().map(|a| a.iter().filter_map(parse_player).collect()).unwrap_or_default()
}

/// `/allgamedata`: o jogador ativo, os dez jogadores e os dados do mapa.
pub fn parse_game(data: &Value) -> Option<GameInfo> {
    let you = read_identity(&raw_identity(data.get("activePlayer")?))?;
    let players = parse_players(data.get("allPlayers")?);
    if players.is_empty() {
        return None;
    }
    let gd = data.get("gameData");
    let map_number = gd
        .and_then(|g| g.get("mapNumber"))
        .and_then(|n| n.as_i64().or_else(|| n.as_str().and_then(|s| s.trim().parse().ok())))
        .unwrap_or(0);
    Some(GameInfo {
        map_number,
        mode: gd.and_then(|g| text(g, "gameMode")).unwrap_or_default(),
        game_time: gd.and_then(|g| g.get("gameTime")).and_then(Value::as_f64).unwrap_or(0.0),
        you,
        players,
    })
}

/// O time de quem roda o app, achado pelo Riot ID dele na lista.
pub fn my_team(game: &GameInfo) -> Option<String> {
    let ids: Vec<Identity> = game.players.iter().map(|p| p.identity.clone()).collect();
    find_in_roster(&ids, &game.you).map(|i| game.players[i].team.clone())
}

/// Um identificador da partida que os dois olheiros (um por time) calculam igual: o mapa e os Riot IDs ordenados, num hash FNV-1a.
pub fn game_id(map_number: i64, players: &[LivePlayer]) -> String {
    let mut keys: Vec<&str> = players.iter().map(|p| p.identity.key.as_str()).collect();
    keys.sort_unstable();
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut eat = |b: u8| {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    };
    for b in map_number.to_le_bytes() {
        eat(b);
    }
    for k in keys {
        for b in k.bytes() {
            eat(b);
        }
        eat(0);
    }
    format!("{h:016x}")
}

/// `MinimapScale` da seção `[HUD]` do `game.cfg`.
pub fn parse_minimap_scale(cfg: &str) -> Option<f64> {
    cfg_value(cfg, "HUD", "MinimapScale")?.parse().ok()
}

/// O valor de `chave=` dentro de `[seção]` (sem caixa, sem espaços).
pub fn cfg_value(cfg: &str, section: &str, key: &str) -> Option<String> {
    let mut inside = false;
    for line in cfg.lines() {
        let l = line.trim();
        if l.starts_with('[') {
            inside = l.trim_matches(|c| c == '[' || c == ']').eq_ignore_ascii_case(section);
            continue;
        }
        if inside {
            if let Some((k, v)) = l.split_once('=') {
                if k.trim().eq_ignore_ascii_case(key) {
                    return Some(v.trim().to_string());
                }
            }
        }
    }
    None
}

/// `nome:pid:porta:senha:protocolo` do `lockfile` do cliente. Espaço no fim da senha dá 401 sem dizer por quê: tudo é aparado.
pub fn parse_lockfile(content: &str) -> Option<(u16, String)> {
    let parts: Vec<&str> = content.trim().split(':').collect();
    if parts.len() < 4 {
        return None;
    }
    Some((parts[2].trim().parse().ok()?, parts[3].trim().to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn jogador(riot: &str, champ: &str, raw: &str, team: &str, dead: bool) -> Value {
        let (g, t) = riot.split_once('#').unwrap();
        json!({
            "championName": champ, "rawChampionName": raw, "isBot": false, "isDead": dead, "respawnTimer": if dead { 12.5 } else { 0.0 },
            "level": 3, "position": "MIDDLE", "skinID": 0, "team": team,
            "riotId": riot, "riotIdGameName": g, "riotIdTagLine": t, "summonerName": g
        })
    }

    fn partida() -> Value {
        json!({
            "activePlayer": { "riotId": "Kennzy#BR1", "riotIdGameName": "Kennzy", "riotIdTagLine": "BR1", "summonerName": "Kennzy", "level": 3 },
            "allPlayers": [
                jogador("Kennzy#BR1", "Ahri", "game_character_displayname_Ahri", "ORDER", false),
                jogador("Mylon#BR1", "Wukong", "game_character_displayname_MonkeyKing", "ORDER", true),
                jogador("Zed#BR1", "Zed", "game_character_displayname_Zed", "CHAOS", false)
            ],
            "gameData": { "gameMode": "CLASSIC", "gameTime": 61.5, "mapName": "Map11", "mapNumber": 11, "mapTerrain": "Default" }
        })
    }

    #[test]
    fn le_a_partida_inteira_da_api_local() {
        let g = parse_game(&partida()).unwrap();
        assert_eq!((g.map_number, g.mode.as_str(), g.game_time), (11, "CLASSIC", 61.5));
        assert_eq!(g.you.key, "kennzy#br1");
        assert_eq!(g.players.len(), 3);
        assert_eq!(g.players[0].alias, "ahri");
        assert!(g.players[1].is_dead && g.players[1].respawn == 12.5);
        assert_eq!(my_team(&g).as_deref(), Some("ORDER"));
    }

    #[test]
    fn o_alias_do_wukong_vem_do_nome_interno() {
        let g = parse_game(&partida()).unwrap();
        assert_eq!(g.players[1].champion, "Wukong");
        assert_eq!(g.players[1].alias, "monkeyking");
        assert_eq!(champion_alias(None, "Kai'Sa"), "kaisa");
        assert_eq!(champion_alias(None, "Cho'Gath"), "chogath");
    }

    // O modo streamer põe o nome do campeão em `summonerName` na máquina de quem transmite; o Riot ID verdadeiro continua nos campos novos.
    #[test]
    fn o_modo_streamer_nao_engana() {
        let mut p = partida();
        p["allPlayers"][0]["summonerName"] = json!("Ahri");
        p["activePlayer"]["summonerName"] = json!("Ahri");
        let g = parse_game(&p).unwrap();
        assert_eq!(g.you.key, "kennzy#br1");
        assert_eq!(g.players[0].identity.key, "kennzy#br1");
        assert_eq!(my_team(&g).as_deref(), Some("ORDER"));
    }

    #[test]
    fn mapa_vem_como_numero_ou_como_texto() {
        let mut p = partida();
        p["gameData"]["mapNumber"] = json!("12");
        assert_eq!(parse_game(&p).unwrap().map_number, 12);
        p["gameData"].as_object_mut().unwrap().remove("mapNumber");
        assert_eq!(parse_game(&p).unwrap().map_number, 0, "sem mapa o olheiro recusa em vez de adivinhar");
    }

    #[test]
    fn recusa_o_que_nao_e_uma_partida() {
        assert!(parse_game(&json!({})).is_none());
        assert!(parse_game(&json!({ "activePlayer": {}, "allPlayers": [] })).is_none());
        assert!(parse_game(&json!({ "activePlayer": { "riotId": "A#B" }, "allPlayers": [] })).is_none());
        // um time inesperado não entra (Arena, modos novos): o jogador é descartado, não adivinhado
        let mut p = partida();
        p["allPlayers"][2]["team"] = json!("NEUTRAL");
        assert_eq!(parse_game(&p).unwrap().players.len(), 2);
    }

    #[test]
    fn o_id_da_partida_e_igual_para_os_dois_olheiros() {
        let g = parse_game(&partida()).unwrap();
        let mut outro = g.clone();
        outro.players.reverse(); // o outro time pode listar em outra ordem
        assert_eq!(game_id(11, &g.players), game_id(11, &outro.players));
        assert_ne!(game_id(11, &g.players), game_id(12, &g.players));
        assert_eq!(game_id(11, &g.players).len(), 16);
    }

    #[test]
    fn le_o_minimapscale_do_game_cfg() {
        let cfg = "[General]\r\nWindowMode=2\r\n\r\n[HUD]\r\nMinimapScale=1.9200\r\nFlipMiniMap=0\r\n[Volume]\r\nMinimapScale=9\r\n";
        assert_eq!(parse_minimap_scale(cfg), Some(1.92));
        assert_eq!(cfg_value(cfg, "General", "windowmode").as_deref(), Some("2"));
        assert_eq!(parse_minimap_scale("[HUD]\nMinimapScale=abc\n"), None);
        assert_eq!(parse_minimap_scale("[General]\nMinimapScale=3\n"), None, "só vale na seção [HUD]");
    }

    #[test]
    fn le_o_lockfile_sem_deixar_espaco_na_senha() {
        assert_eq!(parse_lockfile("LeagueClient:1234:52000:s3nh4 :https\n"), Some((52000, "s3nh4".into())));
        assert_eq!(parse_lockfile("curto:1"), None);
        assert_eq!(parse_lockfile("a:1:porta:x:https"), None);
    }
}

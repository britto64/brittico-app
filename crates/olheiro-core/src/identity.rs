//! O Riot ID de um jogador, lido do que a API local do jogo entrega.
//!
//! O LoL já mudou mais de uma vez a grafia canônica do nome: `summonerName` sozinho, `summonerName` com "nome#tag",
//! `riotId`, e o par `riotIdGameName` / `riotIdTagLine`. O jogador ativo e a lista de jogadores nem sempre concordam em qual
//! deles trazem. Por isso tudo aqui compara por uma chave normalizada ou recusa; nunca adivinha.
//!
//! O modo streamer do LoL troca `summonerName` pelo nome do campeão na máquina de quem transmite; os campos `riotId*` continuam
//! verdadeiros, e por isso são eles que mandam.

use unicode_normalization::UnicodeNormalization;

#[derive(Clone, Debug, Default)]
pub struct RawIdentity {
    pub summoner_name: Option<String>,
    pub riot_id: Option<String>,
    pub riot_id_game_name: Option<String>,
    pub riot_id_tag_line: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Identity {
    /// "nome#tag" normalizado quando há tag, senão só o nome.
    pub key: String,
    pub game_name: String,
    pub tag_line: String,
    /// Como o jogo escreveu, para mostrar e para a rede.
    pub display: String,
}

/// Só para comparar: NFKC, minúsculas, sem espaços nas pontas.
pub fn normalize_name(v: &str) -> String {
    v.trim().nfkc().collect::<String>().to_lowercase()
}

fn trimmed(v: &Option<String>) -> &str {
    v.as_deref().map(str::trim).unwrap_or("")
}

/// Riot IDs são "nome#tag"; divide no último `#`.
fn split_riot_id(v: &str) -> (String, String) {
    match v.rfind('#') {
        None => (v.to_string(), String::new()),
        Some(i) => (v[..i].trim().to_string(), v[i + 1..].trim().to_string()),
    }
}

/// Junta o que esta versão do jogo trouxer numa forma só. `None` quando nenhum campo trouxe nome usável: quem chama trata como
/// recusa, não como texto vazio.
pub fn read_identity(raw: &RawIdentity) -> Option<Identity> {
    let split_game = trimmed(&raw.riot_id_game_name);
    let split_tag = trimmed(&raw.riot_id_tag_line);
    let riot_id = trimmed(&raw.riot_id);
    let summoner = trimmed(&raw.summoner_name);

    let (game_name, mut tag_line) = if !split_game.is_empty() {
        (split_game.to_string(), split_tag.to_string())
    } else if !riot_id.is_empty() {
        split_riot_id(riot_id)
    } else if !summoner.is_empty() {
        split_riot_id(summoner)
    } else {
        return None;
    };
    if tag_line.is_empty() {
        tag_line = if !split_tag.is_empty() { split_tag.to_string() } else { split_riot_id(summoner).1 };
    }
    if game_name.is_empty() {
        return None;
    }

    let g = normalize_name(&game_name);
    let t = normalize_name(&tag_line);
    let display = if !riot_id.is_empty() {
        riot_id.to_string()
    } else if !summoner.is_empty() {
        summoner.to_string()
    } else if !tag_line.is_empty() {
        format!("{game_name}#{tag_line}")
    } else {
        game_name.clone()
    };
    Some(Identity { key: if t.is_empty() { g.clone() } else { format!("{g}#{t}") }, game_name: g, tag_line: t, display })
}

/// Iguais quando o Riot ID inteiro bate, ou quando o nome bate e um dos lados nunca disse a tag. Com tag nos DOIS lados e tags
/// diferentes são pessoas diferentes: casar só pelo nome só poderia ligar a pessoa errada.
pub fn identity_equals(a: &Identity, b: &Identity) -> bool {
    a.key == b.key || (a.game_name == b.game_name && (a.tag_line.is_empty() || b.tag_line.is_empty()))
}

/// Acha `who` na lista. Chave inteira primeiro; depois só pelo nome, e só se for único. Ambíguo recusa (`None`).
pub fn find_in_roster(roster: &[Identity], who: &Identity) -> Option<usize> {
    let by_key: Vec<usize> = (0..roster.len()).filter(|&i| roster[i].key == who.key).collect();
    match by_key.len() {
        1 => return Some(by_key[0]),
        n if n > 1 => return None,
        _ => {}
    }
    let by_name: Vec<usize> = (0..roster.len())
        .filter(|&i| roster[i].game_name == who.game_name && !(!roster[i].tag_line.is_empty() && !who.tag_line.is_empty()))
        .collect();
    if by_name.len() == 1 {
        Some(by_name[0])
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(game: &str, tag: &str) -> RawIdentity {
        RawIdentity { riot_id_game_name: Some(game.into()), riot_id_tag_line: Some(tag.into()), ..Default::default() }
    }

    #[test]
    fn o_par_dividido_manda_sobre_o_resto() {
        let id = read_identity(&RawIdentity {
            summoner_name: Some("Ahri".into()), // modo streamer: o nome do campeão
            riot_id_game_name: Some("Kennzy".into()),
            riot_id_tag_line: Some("BR1".into()),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(id.key, "kennzy#br1");
    }

    #[test]
    fn le_do_riot_id_inteiro_e_do_summoner_name_com_tag() {
        let a = read_identity(&RawIdentity { riot_id: Some("Nome#TAG".into()), ..Default::default() }).unwrap();
        let b = read_identity(&RawIdentity { summoner_name: Some("nome#tag".into()), ..Default::default() }).unwrap();
        assert_eq!(a.key, b.key);
        assert_eq!(a.display, "Nome#TAG");
    }

    #[test]
    fn sem_nenhum_campo_recusa() {
        assert!(read_identity(&RawIdentity::default()).is_none());
        assert!(read_identity(&RawIdentity { summoner_name: Some("  ".into()), ..Default::default() }).is_none());
    }

    #[test]
    fn acentos_e_caixa_nao_atrapalham() {
        let a = read_identity(&raw("Ação", "BR1")).unwrap();
        let b = read_identity(&raw("AÇÃO", "br1")).unwrap();
        assert!(identity_equals(&a, &b));
    }

    #[test]
    fn tags_diferentes_sao_pessoas_diferentes() {
        let a = read_identity(&raw("Joao", "BR1")).unwrap();
        let b = read_identity(&raw("Joao", "EUW")).unwrap();
        assert!(!identity_equals(&a, &b));
        assert_eq!(find_in_roster(&[a.clone()], &b), None);
    }

    #[test]
    fn digitado_sem_tag_acha_pelo_nome_se_for_unico() {
        let roster: Vec<Identity> = [("Joao", "BR1"), ("Maria", "BR1"), ("Pedro", "BR1")]
            .iter()
            .map(|(g, t)| read_identity(&raw(g, t)).unwrap())
            .collect();
        let typed = read_identity(&RawIdentity { summoner_name: Some("maria".into()), ..Default::default() }).unwrap();
        assert_eq!(find_in_roster(&roster, &typed), Some(1));
    }

    #[test]
    fn nome_repetido_sem_tag_recusa_em_vez_de_adivinhar() {
        let roster: Vec<Identity> =
            [("Joao", "BR1"), ("Joao", "EUW")].iter().map(|(g, t)| read_identity(&raw(g, t)).unwrap()).collect();
        let typed = read_identity(&RawIdentity { summoner_name: Some("joao".into()), ..Default::default() }).unwrap();
        assert_eq!(find_in_roster(&roster, &typed), None);
    }
}

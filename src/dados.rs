//! O que a Call sabe do mundo, vivo: as salas (os "grupos" da trilha), os canais, as pessoas, quem está em cada canal de
//! voz e as conversas. Quem enche é o gateway da Call (`call/rede.rs`: o `ready` e os eventos) e as rotas
//! `/api/diskenzy/v2/*`; quem lê são as telas, pelas funções daqui (uma cópia pequena a cada quadro, sem segurar trava).
//!
//! As telas falam de pessoa e de canal por índice (a posição nas listas daqui); o servidor fala por `profileId` e pelo
//! id público do canal. Os índices nunca mudam de dono: quem aparece entra no fim da lista.

use crate::ic;
use eframe::egui::{Color32, ImageSource};
use std::collections::HashMap;
use std::sync::RwLock;

/// Um cargo (a etiqueta colorida ao lado do nome).
#[derive(Clone, Debug, PartialEq)]
pub struct Role {
    pub id: i64,
    pub key: String,
    pub nome: String,
    pub cor: Color32,
    pub posicao: i64,
}

impl Role {
    pub fn color(&self) -> Color32 {
        self.cor
    }
    pub fn name(&self) -> &str {
        &self.nome
    }
    /// O desenho branco dentro do círculo: os dos cargos de fábrica; os outros ficam só com a cor.
    pub fn glyph(&self) -> Option<ImageSource<'static>> {
        match self.key.as_str() {
            "admin" => Some(ic!("role-admin")),
            "streamer" => Some(ic!("role-streamer")),
            "treinador" => Some(ic!("role-treinador")),
            "jogador" => Some(ic!("role-jogador")),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Person {
    pub profile_id: i64,
    pub name: String,
    pub login: String,
    pub avatar: String,
    pub roles: Vec<Role>,
    pub online: bool,
    /// O canal de voz em que está (índice em `canais()`).
    pub room: Option<usize>,
    pub cam: bool,
    pub screen: bool,
    pub muted: bool,
    /// Dono da sala aberta (a coroa).
    pub owner: bool,
    /// Membro da sala aberta (quem não é só aparece se estiver na voz).
    pub membro: bool,
}

impl Person {
    pub fn photo(&self) -> Option<ImageSource<'static>> {
        crate::net::imagem(&self.avatar)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Text,
    News,
    Voice { locked: bool, cap: Option<u8> },
}

#[derive(Clone, Debug)]
pub struct Channel {
    pub id: String,
    /// O índice da sala em `grupos()`.
    pub grupo: usize,
    pub name: String,
    pub kind: Kind,
    pub category: String,
    pub position: i64,
    pub topic: String,
    pub can_post: bool,
    pub can_join: bool,
}

#[derive(Clone, Debug)]
pub struct Grupo {
    pub id: String,
    pub name: String,
    /// Um emoji, um endereço de imagem, ou vazio (aí a trilha mostra as iniciais).
    pub icon: String,
    pub cor: Option<Color32>,
    pub owner: i64,
    pub membro: bool,
}

#[derive(Clone, Debug)]
pub struct Mensagem {
    pub mid: String,
    pub author: usize,
    pub ts: i64,
    pub text: String,
    /// O nonce de quem mandou daqui (para casar a volta pelo gateway com a que já está na tela).
    pub nonce: Option<String>,
}

#[derive(Default)]
pub struct Dados {
    pub grupos: Vec<Grupo>,
    pub canais: Vec<Channel>,
    pub pessoas: Vec<Person>,
    pub por_perfil: HashMap<i64, usize>,
    pub eu: Option<usize>,
    /// Quem está falando agora (identidades do LiveKit = profileId), de cada canal em que se está dentro.
    pub falando: Vec<i64>,
    pub mensagens: HashMap<String, Vec<Mensagem>>,
    /// Até onde a conversa de cada canal foi buscada (`hasMore`).
    pub mais_antigas: HashMap<String, bool>,
    /// Quem está digitando em cada canal e até quando (segundos do relógio do app).
    pub digitando: HashMap<String, Vec<(usize, f64)>>,
    pub perms: Vec<String>,
    pub admin: bool,
    /// Os membros de cada sala (id público → índices), da rota `/rooms/:room` e dos eventos `member.*`.
    pub membros: HashMap<String, Vec<usize>>,
    /// Quem está na Call Presidencial de cada sala (o `GET` e o `presidential.state`).
    pub presidencial: HashMap<String, Vec<usize>>,
}

static D: RwLock<Option<Dados>> = RwLock::new(None);

pub fn com<R>(f: impl FnOnce(&Dados) -> R) -> R {
    let g = D.read().unwrap();
    match g.as_ref() {
        Some(d) => f(d),
        None => f(&Dados::default()),
    }
}

pub fn mudar<R>(f: impl FnOnce(&mut Dados) -> R) -> R {
    let mut g = D.write().unwrap();
    f(g.get_or_insert_with(Dados::default))
}

pub fn limpar() {
    *D.write().unwrap() = None;
}

pub fn pessoas() -> Vec<Person> {
    com(|d| d.pessoas.clone())
}

pub fn pessoa(i: usize) -> Person {
    com(|d| d.pessoas.get(i).cloned().unwrap_or_default())
}

pub fn canais() -> Vec<Channel> {
    com(|d| d.canais.clone())
}

pub fn canal(i: usize) -> Option<Channel> {
    com(|d| d.canais.get(i).cloned())
}

/// O canal `i`, ou um vazio se ele não existe mais (as telas desenham sem perguntar).
pub fn canal_ou(i: usize) -> Channel {
    canal(i).unwrap_or(Channel { id: String::new(), grupo: usize::MAX, name: String::new(), kind: Kind::Text, category: String::new(), position: 0, topic: String::new(), can_post: false, can_join: false })
}

/// Você (índice), ou um índice que não existe quando ainda não se sabe.
pub fn eu() -> usize {
    me().unwrap_or(usize::MAX)
}

pub fn grupos() -> Vec<Grupo> {
    com(|d| d.grupos.clone())
}

/// Você (o índice em `pessoas()`), quando já se sabe.
pub fn me() -> Option<usize> {
    com(|d| d.eu)
}

pub fn falando(profile_id: i64) -> bool {
    com(|d| d.falando.contains(&profile_id))
}

pub fn tem_perm(p: &str) -> bool {
    com(|d| d.admin || d.perms.iter().any(|x| x == p))
}

/// Os cargos que aparecem como grupos na coluna de pessoas, na ordem deles (menor posição primeiro).
pub fn cargos_em_uso() -> Vec<Role> {
    com(|d| {
        let mut v: Vec<Role> = Vec::new();
        for p in &d.pessoas {
            if let Some(r) = p.roles.first() {
                if !v.iter().any(|x| x.id == r.id) {
                    v.push(r.clone());
                }
            }
        }
        v.sort_by_key(|r| r.posicao);
        v
    })
}

/// As categorias de canais de uma sala, na ordem em que aparecem.
pub fn categorias(grupo: usize) -> Vec<String> {
    com(|d| {
        let mut chs: Vec<&Channel> = d.canais.iter().filter(|c| c.grupo == grupo).collect();
        chs.sort_by_key(|c| c.position);
        let mut v: Vec<String> = Vec::new();
        for c in chs {
            if !v.contains(&c.category) {
                v.push(c.category.clone());
            }
        }
        v
    })
}

pub fn hex(s: &str) -> Option<Color32> {
    let s = s.strip_prefix('#')?;
    let v = u32::from_str_radix(s.get(..6)?, 16).ok()?;
    Some(Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8))
}

/* ================= o que vem do servidor ================= */

use serde_json::Value;

pub fn role_de(v: &Value) -> Role {
    Role {
        id: v["id"].as_i64().unwrap_or(0),
        key: v["key"].as_str().unwrap_or("").to_owned(),
        nome: v["name"].as_str().unwrap_or("").to_owned(),
        cor: v["color"].as_str().and_then(hex).unwrap_or(Color32::from_gray(0x8a)),
        posicao: v["position"].as_i64().unwrap_or(99),
    }
}

impl Dados {
    /// A pessoa de um `Person`/`Member` do servidor: cria ou atualiza, e devolve o índice.
    pub fn pessoa_de(&mut self, v: &Value) -> Option<usize> {
        let id = v["profileId"].as_i64()?;
        let i = self.indice(id);
        let p = &mut self.pessoas[i];
        if let Some(n) = v["name"].as_str().filter(|s| !s.is_empty()) {
            p.name = n.to_owned();
        }
        if let Some(l) = v["login"].as_str() {
            p.login = l.to_owned();
        }
        if let Some(a) = v["avatar"].as_str() {
            p.avatar = a.to_owned();
        }
        if let Some(rs) = v["roles"].as_array() {
            let mut roles: Vec<Role> = rs.iter().map(role_de).collect();
            roles.sort_by_key(|r| r.posicao);
            p.roles = roles;
        }
        Some(i)
    }

    /// O índice de um perfil (cria um "Perfil N" se ainda não se sabe quem é).
    pub fn indice(&mut self, id: i64) -> usize {
        if let Some(&i) = self.por_perfil.get(&id) {
            return i;
        }
        let i = self.pessoas.len();
        self.pessoas.push(Person { profile_id: id, name: format!("Perfil {id}"), ..Default::default() });
        self.por_perfil.insert(id, i);
        i
    }

    pub fn canal_idx(&self, id: &str) -> Option<usize> {
        self.canais.iter().position(|c| c.id == id)
    }

    pub fn grupo_idx(&self, id: &str) -> Option<usize> {
        self.grupos.iter().position(|g| g.id == id)
    }

    pub fn sala_de(&mut self, v: &Value) {
        let Some(id) = v["id"].as_str() else { return };
        let g = Grupo {
            id: id.to_owned(),
            name: v["name"].as_str().unwrap_or("").to_owned(),
            icon: v["icon"].as_str().unwrap_or("").to_owned(),
            cor: v["color"].as_str().and_then(hex),
            owner: v["ownerProfileId"].as_i64().unwrap_or(0),
            membro: v["me"]["role"].is_string(),
        };
        match self.grupo_idx(id) {
            Some(i) => self.grupos[i] = g,
            None => self.grupos.push(g),
        }
    }

    pub fn canal_de(&mut self, v: &Value) {
        let (Some(id), Some(sala)) = (v["id"].as_str(), v["room"].as_str()) else { return };
        let Some(grupo) = self.grupo_idx(sala) else { return };
        let voz = v["kind"].as_str() == Some("voice");
        let ro = v["readOnly"].as_bool().unwrap_or(false);
        let kind = if voz {
            let cap = v["userLimit"].as_u64().filter(|n| *n > 0).map(|n| n.min(99) as u8);
            Kind::Voice { locked: v["access"].as_str() == Some("restricted"), cap }
        } else if ro {
            Kind::News
        } else {
            Kind::Text
        };
        let c = Channel {
            id: id.to_owned(),
            grupo,
            name: v["name"].as_str().unwrap_or("").to_owned(),
            kind,
            category: v["category"].as_str().filter(|s| !s.is_empty()).unwrap_or(if voz { "Voz" } else { "Texto" }).to_owned(),
            position: v["position"].as_i64().unwrap_or(0),
            topic: v["topic"].as_str().unwrap_or("").to_owned(),
            can_post: v["me"]["canPost"].as_bool().unwrap_or(!voz),
            can_join: v["me"]["canJoin"].as_bool().unwrap_or(voz),
        };
        match self.canal_idx(id) {
            Some(i) => self.canais[i] = c,
            None => self.canais.push(c),
        }
    }

    /// A lista inteira de quem está num canal de voz (o `voice.state`).
    pub fn voz_de(&mut self, canal: &str, lista: &Value) {
        let Some(ci) = self.canal_idx(canal) else { return };
        for p in self.pessoas.iter_mut() {
            if p.room == Some(ci) {
                p.room = None;
                p.cam = false;
                p.screen = false;
                p.muted = false;
            }
        }
        for pt in lista.as_array().into_iter().flatten() {
            let Some(id) = pt["profileId"].as_i64() else { continue };
            let i = self.indice(id);
            let p = &mut self.pessoas[i];
            p.room = Some(ci);
            p.online = true;
            p.muted = !pt["mic"].as_bool().unwrap_or(false);
            p.cam = pt["cam"].as_bool().unwrap_or(false);
            p.screen = pt["screen"].as_bool().unwrap_or(false);
        }
    }

    pub fn mensagem_de(&mut self, v: &Value) -> Option<(String, Mensagem)> {
        let canal = v["channel"].as_str()?.to_owned();
        let author = self.pessoa_de(&v["author"])?;
        Some((
            canal,
            Mensagem {
                mid: v["mid"].as_str().unwrap_or("").to_owned(),
                author,
                ts: v["ts"].as_i64().unwrap_or(0),
                text: v["text"].as_str().unwrap_or("").to_owned(),
                nonce: v["nonce"].as_str().map(str::to_owned),
            },
        ))
    }
}

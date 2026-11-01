//! O gateway da Call (`/api/diskenzy/v2/gateway`, contrato em `docs/DISKENZY-CONTRATO.md` do site): o WebSocket que diz
//! o que mudou nas salas, nos canais, na voz e nas conversas. Roda numa thread à parte, com a sessão no cabeçalho
//! `Cookie`, faz o `hello` (retomando a sessão quando dá), manda o `ping` a cada 25 s e reconecta sozinho.
//!
//! Os eventos vão direto para o armazém (`dados.rs`); o que pede uma decisão da tela (fui movido, fui derrubado) vai pela
//! fila `rx`.

use crate::dados::{self, Mensagem};
use crate::net;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};

pub enum Aviso {
    Pronto,
    Caiu,
    /// `voice.moved`: o servidor já mandou o token do canal novo.
    Movido { para: String, por: String, url: String, token: String },
    /// `voice.kicked`: tirado da call (por = "Sistema" quando perdeu o acesso).
    Derrubado { canal: String, por: String },
}

pub struct Gateway {
    tx: Sender<String>,
    pub rx: Receiver<Aviso>,
    parar: Arc<AtomicBool>,
}

impl Drop for Gateway {
    fn drop(&mut self) {
        self.parar.store(true, Ordering::Relaxed);
    }
}

impl Gateway {
    pub fn mandar(&self, t: &str, d: Value) {
        let _ = self.tx.send(json!({ "t": t, "d": d }).to_string());
    }
}

pub fn ligar(ctx: &eframe::egui::Context) -> Gateway {
    let (tx, rx_out) = channel::<String>();
    let (tx_in, rx) = channel::<Aviso>();
    let parar = Arc::new(AtomicBool::new(false));
    let p2 = parar.clone();
    let ctx = ctx.clone();
    std::thread::Builder::new()
        .name("call-gateway".into())
        .spawn(move || {
            let mut sessao: Option<(String, i64)> = None;
            let mut espera = 1u64;
            while !p2.load(Ordering::Relaxed) {
                match rodar(&rx_out, &tx_in, &p2, &ctx, &mut sessao) {
                    Ok(()) => espera = 1,
                    Err(e) => eprintln!("[call] gateway: {e}"),
                }
                if p2.load(Ordering::Relaxed) {
                    break;
                }
                let _ = tx_in.send(Aviso::Caiu);
                ctx.request_repaint();
                std::thread::sleep(Duration::from_secs(espera));
                espera = (espera * 2).min(30);
            }
        })
        .expect("thread do gateway");
    Gateway { tx, rx, parar }
}

fn rodar(rx_out: &Receiver<String>, tx_in: &Sender<Aviso>, parar: &AtomicBool, ctx: &eframe::egui::Context, sessao: &mut Option<(String, i64)>) -> Result<(), String> {
    use tungstenite::client::IntoClientRequest;
    use tungstenite::stream::MaybeTlsStream;
    use tungstenite::Message;
    let mut req = net::ws_url("/api/diskenzy/v2/gateway").into_client_request().map_err(|e| e.to_string())?;
    if let Some(c) = net::cookie() {
        req.headers_mut().insert("Cookie", c.parse().map_err(|_| "cookie".to_string())?);
    }
    let (mut ws, _) = tungstenite::connect(req).map_err(|e| e.to_string())?;
    let t = Some(Duration::from_millis(50));
    match ws.get_mut() {
        MaybeTlsStream::Plain(s) => s.set_read_timeout(t).map_err(|e| e.to_string())?,
        MaybeTlsStream::NativeTls(s) => s.get_mut().set_read_timeout(t).map_err(|e| e.to_string())?,
        _ => {}
    }
    let hello = |s: &Option<(String, i64)>| match s {
        Some((id, seq)) => json!({ "t": "hello", "d": { "sessionId": id, "lastSeq": seq } }),
        None => json!({ "t": "hello", "d": {} }),
    };
    ws.send(Message::text(hello(sessao).to_string())).map_err(|e| e.to_string())?;
    let mut ping = Instant::now();
    loop {
        if parar.load(Ordering::Relaxed) {
            let _ = ws.close(None);
            return Ok(());
        }
        while let Ok(s) = rx_out.try_recv() {
            ws.send(Message::text(s)).map_err(|e| e.to_string())?;
        }
        if ping.elapsed() > Duration::from_secs(25) {
            ping = Instant::now();
            ws.send(Message::text(json!({ "t": "ping", "d": {} }).to_string())).map_err(|e| e.to_string())?;
        }
        let texto = match ws.read() {
            Ok(Message::Text(s)) => s.to_string(),
            Ok(Message::Close(f)) => return Err(format!("fechado ({f:?})")),
            Ok(_) => continue,
            Err(tungstenite::Error::Io(e)) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => continue,
            Err(e) => return Err(e.to_string()),
        };
        let Ok(q) = serde_json::from_str::<Value>(&texto) else { continue };
        let (s, tipo, d) = (q["s"].as_i64().unwrap_or(0), q["t"].as_str().unwrap_or(""), &q["d"]);
        if s > 0 {
            if let Some(ss) = sessao.as_mut() {
                ss.1 = s;
            }
        }
        match tipo {
            "ready" => {
                *sessao = d["sessionId"].as_str().map(|id| (id.to_owned(), 0));
                pronto(d);
                let _ = tx_in.send(Aviso::Pronto);
                // os membros de cada sala de que se é membro (a coluna de pessoas), fora do laço do WebSocket
                let salas: Vec<String> = dados::com(|d| d.grupos.iter().filter(|g| g.membro).map(|g| g.id.clone()).collect());
                let c2 = ctx.clone();
                std::thread::spawn(move || {
                    for s in salas {
                        buscar_membros(&s);
                    }
                    c2.request_repaint();
                });
            }
            "resync" => {
                *sessao = None;
                ws.send(Message::text(hello(sessao).to_string())).map_err(|e| e.to_string())?;
            }
            "voice.moved" => {
                let _ = tx_in.send(Aviso::Movido {
                    para: d["to"].as_str().unwrap_or("").to_owned(),
                    por: d["by"]["name"].as_str().unwrap_or("").to_owned(),
                    url: d["url"].as_str().unwrap_or("").to_owned(),
                    token: d["token"].as_str().unwrap_or("").to_owned(),
                });
            }
            "voice.kicked" => {
                let _ = tx_in.send(Aviso::Derrubado { canal: d["channel"].as_str().unwrap_or("").to_owned(), por: d["by"]["name"].as_str().unwrap_or("").to_owned() });
            }
            outro => aplicar(outro, d, ctx),
        }
        ctx.request_repaint();
    }
}

/// O `ready`: tudo do zero.
fn pronto(d: &Value) {
    dados::mudar(|st| {
        let antigos = std::mem::take(&mut st.mensagens);
        *st = dados::Dados::default();
        st.mensagens = antigos;
        st.perms = d["perms"].as_array().into_iter().flatten().filter_map(|p| p.as_str().map(str::to_owned)).collect();
        st.admin = d["me"]["isAdmin"].as_bool().unwrap_or(false);
        for r in d["rooms"].as_array().into_iter().flatten() {
            st.sala_de(r);
        }
        let mut chs: Vec<&Value> = d["channels"].as_array().into_iter().flatten().collect();
        chs.sort_by_key(|c| c["position"].as_i64().unwrap_or(0));
        for c in chs {
            st.canal_de(c);
        }
        let mut me = d["me"].clone();
        me["roles"] = d["roles"].clone();
        st.eu = st.pessoa_de(&me);
        if let Some(i) = st.eu {
            st.pessoas[i].online = true;
        }
        if let Some(pr) = d["presence"].as_object() {
            for (id, estado) in pr {
                if let Ok(id) = id.parse::<i64>() {
                    let i = st.indice(id);
                    st.pessoas[i].online = estado.as_str() != Some("offline");
                }
            }
        }
        if let Some(v) = d["voice"].as_object() {
            for (canal, lista) in v {
                st.voz_de(canal, lista);
            }
        }
        // as mensagens guardadas de canais que sumiram não valem mais
        let ids: Vec<String> = st.canais.iter().map(|c| c.id.clone()).collect();
        st.mensagens.retain(|k, _| ids.contains(k));
    });
}

pub fn buscar_membros(sala: &str) {
    let Ok(v) = net::pedir_ja("GET", &format!("/api/diskenzy/v2/rooms/{sala}"), None) else { return };
    dados::mudar(|st| {
        if let Some(r) = v.get("room") {
            st.sala_de(r);
        }
        let mut lista = Vec::new();
        for m in v["members"].as_array().into_iter().flatten() {
            if let Some(i) = st.pessoa_de(m) {
                lista.push(i);
            }
        }
        st.membros.insert(sala.to_owned(), lista);
        for c in v["channels"].as_array().into_iter().flatten() {
            st.canal_de(c);
        }
    });
}

/// A conversa mais recente de um canal de texto (ou a página antes de `antes`).
pub fn buscar_mensagens(canal: &str, antes: Option<&str>) -> Result<(), net::Erro> {
    let path = match antes {
        Some(m) => format!("/api/diskenzy/v2/channels/{canal}/messages?limit=50&before={m}"),
        None => format!("/api/diskenzy/v2/channels/{canal}/messages?limit=50"),
    };
    let v = net::pedir_ja("GET", &path, None)?;
    dados::mudar(|st| {
        let mut novas: Vec<Mensagem> = Vec::new();
        for m in v["messages"].as_array().into_iter().flatten() {
            let mut m = m.clone();
            m["channel"] = json!(canal);
            if let Some((_, msg)) = st.mensagem_de(&m) {
                novas.push(msg);
            }
        }
        let lista = st.mensagens.entry(canal.to_owned()).or_default();
        for m in novas {
            if !lista.iter().any(|x| x.mid == m.mid) {
                lista.push(m);
            }
        }
        lista.sort_by_key(|m| m.ts);
        st.mais_antigas.insert(canal.to_owned(), v["hasMore"].as_bool().unwrap_or(false));
    });
    Ok(())
}

fn aplicar(tipo: &str, d: &Value, ctx: &eframe::egui::Context) {
    let t = ctx.input(|i| i.time);
    dados::mudar(|st| match tipo {
        "msg.create" => {
            let mut m = d["msg"].clone();
            m["channel"] = d["channel"].clone();
            if let Some((canal, msg)) = st.mensagem_de(&m) {
                let lista = st.mensagens.entry(canal.clone()).or_default();
                // a que eu mandei já está na tela (pelo nonce): troca pela de verdade
                if let Some(pos) = lista.iter().position(|x| x.mid == msg.mid || (msg.nonce.is_some() && x.nonce == msg.nonce)) {
                    lista[pos] = msg.clone();
                } else {
                    lista.push(msg.clone());
                }
                if let Some(dg) = st.digitando.get_mut(&canal) {
                    dg.retain(|(p, _)| *p != msg.author);
                }
            }
        }
        "msg.delete" => {
            if let (Some(c), Some(mid)) = (d["channel"].as_str(), d["mid"].as_str()) {
                if let Some(l) = st.mensagens.get_mut(c) {
                    l.retain(|m| m.mid != mid);
                }
            }
        }
        "channel.create" | "channel.update" => st.canal_de(&d["channel"]),
        "channel.delete" => {
            if let Some(i) = d["channel"]["id"].as_str().and_then(|id| st.canal_idx(id)) {
                // o índice continua existindo (as telas falam por índice); some de todas as listas
                st.canais[i].grupo = usize::MAX;
            }
        }
        "room.update" => st.sala_de(&d["room"]),
        "room.delete" => {
            if let Some(i) = d["room"].as_str().and_then(|id| st.grupo_idx(id)) {
                st.grupos[i].membro = false;
                st.grupos[i].name.clear();
            }
        }
        "member.join" => {
            if let (Some(sala), Some(i)) = (d["room"].as_str().map(str::to_owned), st.pessoa_de(&d["profile"])) {
                let l = st.membros.entry(sala).or_default();
                if !l.contains(&i) {
                    l.push(i);
                }
            }
        }
        "member.leave" => {
            if let (Some(sala), Some(id)) = (d["room"].as_str(), d["profile"]["profileId"].as_i64()) {
                if let (Some(&i), Some(l)) = (st.por_perfil.get(&id), st.membros.get_mut(sala)) {
                    l.retain(|x| *x != i);
                }
            }
        }
        "presence.update" => {
            if let Some(id) = d["profileId"].as_i64() {
                let i = st.indice(id);
                st.pessoas[i].online = d["state"].as_str() != Some("offline");
            }
        }
        "presidential.state" => {
            if let Some(sala) = d["room"].as_str() {
                let l: Vec<usize> = d["members"].as_array().into_iter().flatten().filter_map(|m| st.pessoa_de(m)).collect();
                st.presidencial.insert(sala.to_owned(), l);
            }
        }
        "voice.state" => {
            if let Some(c) = d["channel"].as_str() {
                st.voz_de(c, &d["participants"]);
            }
        }
        "typing" => {
            if let (Some(c), Some(id)) = (d["channel"].as_str(), d["profileId"].as_i64()) {
                let i = st.indice(id);
                let l = st.digitando.entry(c.to_owned()).or_default();
                l.retain(|(p, _)| *p != i);
                l.push((i, t + 8.0));
            }
        }
        "roles.update" => {
            if let Some(id) = d["profileId"].as_i64() {
                let i = st.indice(id);
                let mut roles: Vec<dados::Role> = d["roles"].as_array().into_iter().flatten().map(dados::role_de).collect();
                roles.sort_by_key(|r| r.posicao);
                st.pessoas[i].roles = roles;
            }
        }
        "me.update" => {
            st.perms = d["perms"].as_array().into_iter().flatten().filter_map(|p| p.as_str().map(str::to_owned)).collect();
            if let Some(i) = st.eu {
                let mut roles: Vec<dados::Role> = d["roles"].as_array().into_iter().flatten().map(dados::role_de).collect();
                roles.sort_by_key(|r| r.posicao);
                st.pessoas[i].roles = roles;
            }
        }
        _ => {}
    });
}

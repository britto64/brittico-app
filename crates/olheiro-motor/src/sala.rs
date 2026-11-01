//! A sala de voz de uma partida: pedir a entrada ao porteiro, entrar no LiveKit, publicar o microfone, receber a voz de cada um
//! e trocar as posições pelo canal de dados (sem garantia de entrega, 5 vezes por segundo).
//!
//! Quem decide o volume é o motor (`motor.rs`), com a fusão das posições; aqui só se liga o som de cada pessoa à mistura.

use crate::som::{Controles, MisturaRef, QUADRO, TAXA};
use futures_util::StreamExt;
use livekit::options::TrackPublishOptions;
use livekit::prelude::*;
use livekit::webrtc::audio_frame::AudioFrame;
use livekit::webrtc::audio_source::native::NativeAudioSource;
use livekit::webrtc::audio_source::{AudioSourceOptions, RtcAudioSource};
use livekit::webrtc::audio_stream::native::NativeAudioStream;
use olheiro_voz::{decodificar, Fusao, Pacote};
use serde_json::json;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc::UnboundedReceiver;

/// Tópico das posições no canal de dados. O mesmo do `entrar` e do teste T4.
pub const TOPICO: &str = "pos";

#[derive(Clone, Debug, PartialEq, Default)]
pub enum Conexao {
    /// Sem partida, ou sem servidor de voz.
    #[default]
    Fora,
    Entrando,
    Dentro,
    /// Não deu (o porteiro recusou, a internet caiu…): a frase diz o quê. O motor tenta de novo.
    Falhou(String),
}

/// O que a tela mostra da sala.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct InfoSala {
    pub conexao: Conexao,
    /// Chave → nome (o Riot ID como o jogo escreve) de quem está na sala, menos você.
    pub pessoas: BTreeMap<String, String>,
    /// Quem está com o microfone fechado (pelo próprio app).
    pub mudos: Vec<String>,
}

/// O que a sala e o motor dividem.
pub struct Comum {
    pub mistura: MisturaRef,
    pub ctl: Arc<Controles>,
    pub fusao: Mutex<Fusao>,
    pub info: Mutex<InfoSala>,
    /// Chamado quando algo que a tela mostra mudou (o Brittico redesenha a janela).
    pub acordar: Box<dyn Fn() + Send + Sync>,
}

impl Comum {
    fn info(&self, f: impl FnOnce(&mut InfoSala)) {
        if let Ok(mut i) = self.info.lock() {
            f(&mut i);
        }
        (self.acordar)();
    }
}

pub enum ParaSala {
    Posicoes(Pacote),
    Microfone { aberto: bool },
    Sair,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Pedido {
    pub porteiro: String,
    pub riot_id: String,
    pub partida: String,
    pub time: String,
    pub personalizada: bool,
    pub grupo: Option<String>,
}

pub struct Entrada {
    pub url: String,
    pub token: String,
    pub identidade: String,
}

/// Pede a entrada ao porteiro. Erros já em português, para a tela.
pub async fn pedir(p: &Pedido) -> Result<Entrada, String> {
    let url = format!("{}/entrar", p.porteiro.trim().trim_end_matches('/'));
    let mut corpo = json!({ "riot_id": p.riot_id, "partida": p.partida, "time": p.time, "modo": if p.personalizada { "personalizada" } else { "flex" } });
    if let Some(g) = p.grupo.as_deref().filter(|g| !g.trim().is_empty()) {
        corpo["codigo_grupo"] = json!(g.trim());
    }
    let cliente = reqwest::Client::builder().timeout(std::time::Duration::from_secs(10)).build().map_err(|e| e.to_string())?;
    let resp = cliente.post(&url).json(&corpo).send().await.map_err(|_| "Não consegui falar com o servidor de voz. Confira a internet.".to_string())?;
    let status = resp.status();
    let valor: serde_json::Value = resp.json().await.unwrap_or_default();
    if !status.is_success() {
        let erro = valor["erro"].as_str().unwrap_or("sem explicação");
        return Err(format!("O servidor de voz recusou a entrada: {erro}."));
    }
    let campo = |k: &str| valor[k].as_str().unwrap_or("").to_string();
    let e = Entrada { url: campo("url"), token: campo("token"), identidade: campo("identidade") };
    if e.url.is_empty() || e.token.is_empty() {
        return Err("O servidor de voz respondeu sem a permissão da sala.".into());
    }
    Ok(e)
}

/// Entra, fica e sai. Volta quando a partida acaba (`ParaSala::Sair`) ou a sala cai; o motor decide se tenta de novo.
pub async fn rodar(pedido: Pedido, mut rx: UnboundedReceiver<ParaSala>, comum: Arc<Comum>, microfone_aberto: bool) {
    comum.info(|i| {
        i.conexao = Conexao::Entrando;
        i.pessoas.clear();
        i.mudos.clear();
    });
    let entrada = match pedir(&pedido).await {
        Ok(e) => e,
        Err(e) => return comum.info(|i| i.conexao = Conexao::Falhou(e)),
    };
    let mut opcoes = RoomOptions::default();
    opcoes.auto_subscribe = true;
    let (room, mut eventos) = match Room::connect(&entrada.url, &entrada.token, opcoes).await {
        Ok(r) => r,
        Err(e) => return comum.info(|i| i.conexao = Conexao::Falhou(format!("Não entrei na sala de voz ({e})."))),
    };

    // o microfone: a limpeza já foi feita no `som.rs`, então a fonte não liga a dela
    let fonte = NativeAudioSource::new(AudioSourceOptions { echo_cancellation: false, noise_suppression: false, auto_gain_control: false }, TAXA, 1, 0);
    let faixa = LocalAudioTrack::create_audio_track("microfone", RtcAudioSource::Native(fonte.clone()));
    let publicar = TrackPublishOptions { source: TrackSource::Microphone, dtx: true, ..Default::default() };
    if let Err(e) = room.local_participant().publish_track(LocalTrack::Audio(faixa.clone()), publicar).await {
        let _ = room.close().await;
        return comum.info(|i| i.conexao = Conexao::Falhou(format!("Não consegui ligar o seu microfone na sala ({e}).")));
    }
    if !microfone_aberto {
        faixa.mute();
    }
    let rt = tokio::runtime::Handle::current();
    if let Ok(mut d) = comum.ctl.destino.lock() {
        let fonte = fonte.clone();
        *d = Some(Box::new(move |q: &[i16]| {
            let quadro = AudioFrame { data: q.to_vec().into(), sample_rate: TAXA, num_channels: 1, samples_per_channel: QUADRO as u32 };
            let _ = rt.block_on(fonte.capture_frame(&quadro));
        }));
    }
    comum.info(|i| i.conexao = Conexao::Dentro);
    for (_, p) in room.remote_participants() {
        let (k, n) = (p.identity().to_string(), p.name().to_string());
        comum.info(|i| {
            i.pessoas.insert(k, if n.is_empty() { p.identity().to_string() } else { n });
        });
    }

    let mut seq_ok = true;
    loop {
        tokio::select! {
            ev = eventos.recv() => {
                let Some(ev) = ev else { break };
                match ev {
                    RoomEvent::ParticipantConnected(p) => {
                        let (k, n) = (p.identity().to_string(), p.name().to_string());
                        comum.info(|i| { i.pessoas.insert(k.clone(), if n.is_empty() { k } else { n }); });
                    }
                    RoomEvent::ParticipantDisconnected(p) => {
                        let k = p.identity().to_string();
                        if let Ok(mut m) = comum.mistura.lock() { m.saiu(&k); }
                        comum.info(|i| { i.pessoas.remove(&k); i.mudos.retain(|x| *x != k); });
                    }
                    RoomEvent::TrackSubscribed { track: RemoteTrack::Audio(a), participant, .. } => {
                        let k = participant.identity().to_string();
                        let mistura = comum.mistura.clone();
                        tokio::spawn(async move {
                            let mut fluxo = NativeAudioStream::new(a.rtc_track(), TAXA as i32, 1);
                            while let Some(q) = fluxo.next().await {
                                if let Ok(mut m) = mistura.lock() { m.chegou(&k, &q.data); }
                            }
                        });
                    }
                    RoomEvent::TrackMuted { participant, .. } => {
                        let k = participant.identity().to_string();
                        comum.info(|i| if !i.mudos.contains(&k) { i.mudos.push(k) });
                    }
                    RoomEvent::TrackUnmuted { participant, .. } => {
                        let k = participant.identity().to_string();
                        comum.info(|i| i.mudos.retain(|x| *x != k));
                    }
                    RoomEvent::DataReceived { payload, topic, .. } if topic.as_deref() == Some(TOPICO) => {
                        if let Some(p) = decodificar(&payload) {
                            let agora = crate::agora_ms();
                            if let Ok(mut f) = comum.fusao.lock() {
                                for ponto in p.pos { f.observar(agora, ponto, p.t); }
                            }
                        }
                    }
                    RoomEvent::Disconnected { reason } => {
                        comum.info(|i| i.conexao = Conexao::Falhou(format!("A sala de voz caiu ({reason:?}). Tentando de novo…")));
                        seq_ok = false;
                        break;
                    }
                    RoomEvent::Reconnecting => comum.info(|i| i.conexao = Conexao::Entrando),
                    RoomEvent::Reconnected => comum.info(|i| i.conexao = Conexao::Dentro),
                    _ => {}
                }
            }
            cmd = rx.recv() => {
                match cmd {
                    Some(ParaSala::Posicoes(p)) => {
                        let pacote = DataPacket { payload: olheiro_voz::codificar(&p), topic: Some(TOPICO.into()), reliable: false, destination_identities: Vec::new() };
                        let _ = room.local_participant().publish_data(pacote).await;
                    }
                    Some(ParaSala::Microfone { aberto }) => if aberto { faixa.unmute() } else { faixa.mute() },
                    Some(ParaSala::Sair) | None => break,
                }
            }
        }
    }
    if let Ok(mut d) = comum.ctl.destino.lock() {
        *d = None;
    }
    if let Ok(mut m) = comum.mistura.lock() {
        m.limpar();
    }
    let _ = room.close().await;
    if seq_ok {
        comum.info(|i| {
            i.conexao = Conexao::Fora;
            i.pessoas.clear();
            i.mudos.clear();
        });
    }
    let _ = entrada.identidade;
}

//! A voz da Call: o canal de voz é uma sala do LiveKit (`canal:<id>`), e quem dá a entrada é o bot
//! (`POST /api/diskenzy/v2/voice/:canal/token`, a mesma rota do site). O som do PC é o do Olheiro (`olheiro_motor::som`):
//! o microfone limpo pelo WebRTC e a mistura de quem fala, com volume por pessoa. O vídeo que chega (a tela e a câmera de
//! cada um) vira quadros RGBA que a tela sobe para textura; a própria tela sai pelo capturador do libwebrtc.
//!
//! Tudo roda num runtime do tokio numa thread própria; a tela manda `Cmd` e lê o `Estado`.

use livekit::options::{DegradationPreference, TrackPublishOptions, VideoCodec, VideoEncoding};
use livekit::prelude::*;
use livekit::webrtc::audio_frame::AudioFrame;
use livekit::webrtc::audio_source::native::NativeAudioSource;
use livekit::webrtc::audio_source::{AudioSourceOptions, RtcAudioSource};
use livekit::webrtc::audio_stream::native::NativeAudioStream;
use livekit::webrtc::video_frame::{I420Buffer, VideoFrame, VideoRotation};
use livekit::webrtc::video_source::native::NativeVideoSource;
use livekit::webrtc::video_source::{RtcVideoSource, VideoResolution};
use livekit::webrtc::video_stream::native::NativeVideoStream;
use olheiro_motor::som::{Controles, Mistura, MisturaRef, Som, QUADRO, TAXA};
use futures_util::StreamExt;
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

#[derive(Clone, Debug, PartialEq, Default)]
pub enum Conexao {
    #[default]
    Entrando,
    Dentro,
    Falhou(String),
    Saiu,
}

/// Um quadro de vídeo que chegou: largura, altura, RGBA e um número que sobe a cada quadro novo.
pub struct Quadro {
    pub w: usize,
    pub h: usize,
    pub rgba: Vec<u8>,
    pub n: u64,
}

#[derive(Default)]
pub struct Estado {
    pub conexao: Conexao,
    /// O vídeo de cada um: (profileId, é a tela?) → o último quadro.
    pub videos: HashMap<(i64, bool), Quadro>,
    /// Mostrando a própria tela agora.
    pub tela: bool,
    pub aviso: Option<String>,
}

/// Como a tela sai: `Qualidade` segura a imagem em 1080p com bitrate alto e deixa os quadros caírem quando a rede ou o
/// computador não dão conta; `Fluidez` segura os 60 quadros por segundo e, se faltar, baixa o bitrate e a resolução.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Modo {
    Qualidade,
    /// O padrão, como no site.
    #[default]
    Fluidez,
}

impl Modo {
    /// Quadros por segundo pedidos ao capturador e ao codificador.
    fn fps(self) -> f64 {
        match self {
            Modo::Qualidade => 30.0,
            Modo::Fluidez => 60.0,
        }
    }
    /// O teto de bitrate (bits por segundo).
    fn bitrate(self) -> u64 {
        match self {
            // 8 Mbps é o teto do plano do servidor (o mesmo do site, `voice-presets.js`)
            Modo::Qualidade => 8_000_000,
            Modo::Fluidez => 8_000_000,
        }
    }
    fn degradar(self) -> DegradationPreference {
        match self {
            Modo::Qualidade => DegradationPreference::MaintainResolution,
            Modo::Fluidez => DegradationPreference::MaintainFramerate,
        }
    }
}

/// O que a pessoa escolheu no seletor do app: uma tela (monitor) ou uma janela, pelo id do capturador do libwebrtc.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Escolha {
    pub janela: bool,
    pub id: u64,
}

/// Uma tela ou janela que dá para mostrar, com o nome e uma miniatura (largura, altura, RGBA).
pub struct Fonte {
    pub escolha: Escolha,
    pub nome: String,
    pub miniatura: Option<(usize, usize, Vec<u8>)>,
}

/// No Wayland quem escolhe é o seletor do sistema (o portal); nos outros (Windows, Linux em X11) é o do app.
pub fn seletor_do_sistema() -> bool {
    cfg!(target_os = "linux") && std::env::var("WAYLAND_DISPLAY").is_ok()
}

/// As telas e as janelas abertas, cada uma com uma miniatura (um quadro capturado e reduzido). Demora um pouco: rodar
/// fora da tela.
pub fn fontes() -> Vec<Fonte> {
    use livekit::webrtc::desktop_capturer::{DesktopCaptureSourceType, DesktopCapturer, DesktopCapturerOptions};
    let mut out = Vec::new();
    for janela in [false, true] {
        let tipo = if janela { DesktopCaptureSourceType::Window } else { DesktopCaptureSourceType::Screen };
        let opcoes = || {
            let mut o = DesktopCapturerOptions::new(tipo);
            o.set_include_cursor(false);
            o
        };
        let Some(lista) = DesktopCapturer::new(opcoes()) else { continue };
        for (k, src) in lista.get_source_list().into_iter().enumerate() {
            let id = src.id();
            let titulo = src.title();
            if janela && titulo.trim().is_empty() {
                continue;
            }
            let nome = if janela { titulo } else { format!("Tela {}", k + 1) };
            // um capturador por fonte, só para tirar a miniatura
            let miniatura = DesktopCapturer::new(opcoes()).and_then(|mut cap| {
                let guardado: Arc<Mutex<Option<(usize, usize, Vec<u8>)>>> = Arc::new(Mutex::new(None));
                let g2 = guardado.clone();
                cap.start_capture(Some(src), move |r| {
                    let Ok(q) = r else { return };
                    if let Ok(mut g) = g2.lock() {
                        *g = miniatura_de(q.data(), q.width().max(0) as usize, q.height().max(0) as usize, q.stride() as usize);
                    }
                });
                // o primeiro quadro às vezes vem vazio (a captura ainda acordando): tenta algumas vezes
                for _ in 0..5 {
                    cap.capture_frame();
                    if guardado.lock().is_ok_and(|g| g.is_some()) {
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(40));
                }
                let r = guardado.lock().ok().and_then(|mut g| g.take());
                r
            });
            if janela && miniatura.is_none() {
                // janela minimizada ou que não deixa capturar: some da lista, como no Discord
                continue;
            }
            out.push(Fonte { escolha: Escolha { janela, id }, nome, miniatura });
        }
    }
    out
}

/// Um quadro BGRA reduzido para no máximo 320 de largura, em RGBA.
fn miniatura_de(bgra: &[u8], w: usize, h: usize, stride: usize) -> Option<(usize, usize, Vec<u8>)> {
    if w < 2 || h < 2 || bgra.len() < stride * h {
        return None;
    }
    let passo = (w as f32 / 320.0).max(1.0);
    let (mw, mh) = (((w as f32 / passo) as usize).max(1), ((h as f32 / passo) as usize).max(1));
    let mut out = Vec::with_capacity(mw * mh * 4);
    let mut tudo_preto = true;
    for y in 0..mh {
        let sy = ((y as f32 * passo) as usize).min(h - 1);
        for x in 0..mw {
            let sx = ((x as f32 * passo) as usize).min(w - 1);
            let i = sy * stride + sx * 4;
            let (b, g, r) = (bgra[i], bgra[i + 1], bgra[i + 2]);
            if r | g | b > 8 {
                tudo_preto = false;
            }
            out.extend_from_slice(&[r, g, b, 255]);
        }
    }
    (!tudo_preto).then_some((mw, mh, out))
}

/// A altura máxima da tela que sai: maior que isso (um monitor 1440p ou 4K) é reduzido para 1080p.
const ALTURA_MAX: u32 = 1080;

pub enum Cmd {
    Mic(bool),
    Surdo(bool),
    /// Mostrar a tela no modo dado (e a tela ou janela escolhida no seletor do app; `None` = o seletor do sistema no
    /// Wayland), ou parar (`None`).
    Tela(Option<(Modo, Option<Escolha>)>),
    Volume(i64, f32),
    Sair,
}

pub struct Voz {
    tx: UnboundedSender<Cmd>,
    pub estado: Arc<Mutex<Estado>>,
    pub mistura: MisturaRef,
    pub ctl: Arc<Controles>,
    /// O canal (id público) e quando entrou.
    pub canal: String,
}

impl Drop for Voz {
    fn drop(&mut self) {
        let _ = self.tx.send(Cmd::Sair);
    }
}

impl Voz {
    pub fn mandar(&self, c: Cmd) {
        let _ = self.tx.send(c);
    }

    pub fn conexao(&self) -> Conexao {
        self.estado.lock().map(|e| e.conexao.clone()).unwrap_or_default()
    }

    /// Quem está falando agora (pelo som que chega de cada um, e o seu pelo microfone).
    pub fn falando(&self, eu: i64, mic_aberto: bool) -> Vec<i64> {
        let mut v: Vec<i64> = self.mistura.lock().map(|m| m.quadro().into_iter().filter(|(_, nivel, _)| *nivel > 0.02).filter_map(|(k, _, _)| k.parse().ok()).collect()).unwrap_or_default();
        if mic_aberto && self.ctl.nivel_mic.get() > 0.03 {
            v.push(eu);
        }
        v
    }
}

/// Entra no canal de voz `canal`. `entrada`: o url e o token já dados (o `voice.moved`); sem ele, pede ao bot.
/// `token_path`: a rota que dá a entrada (`/api/diskenzy/v2/voice/<canal>/token`, ou a da Call Presidencial).
pub fn entrar(ctx: &eframe::egui::Context, canal: String, token_path: String, entrada: Option<(String, String)>, mic: String, saida: String, mic_aberto: bool) -> Voz {
    let (tx, rx) = unbounded_channel();
    let estado = Arc::new(Mutex::new(Estado::default()));
    let mistura = Mistura::nova();
    let ctl = Controles::novos();
    let (e2, m2, c2, ctx2, canal2) = (estado.clone(), mistura.clone(), ctl.clone(), ctx.clone(), canal.clone());
    std::thread::Builder::new()
        .name("call-voz".into())
        .spawn(move || {
            let rt = match tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build() {
                Ok(rt) => rt,
                Err(e) => {
                    if let Ok(mut s) = e2.lock() {
                        s.conexao = Conexao::Falhou(e.to_string());
                    }
                    return;
                }
            };
            rt.block_on(rodar(token_path, entrada, mic, saida, mic_aberto, rx, e2, m2, c2, ctx2));
            let _ = canal2;
        })
        .expect("thread da voz");
    Voz { tx, estado, mistura, ctl, canal }
}

#[allow(clippy::too_many_arguments)]
async fn rodar(token_path: String, entrada: Option<(String, String)>, mic: String, saida: String, mic_aberto: bool, mut rx: UnboundedReceiver<Cmd>, estado: Arc<Mutex<Estado>>, mistura: MisturaRef, ctl: Arc<Controles>, ctx: eframe::egui::Context) {
    let mudar = |f: &dyn Fn(&mut Estado)| {
        if let Ok(mut e) = estado.lock() {
            f(&mut e);
        }
        ctx.request_repaint();
    };
    // a entrada: o token do bot (a lotação e o acesso são decididos lá)
    let (url, token) = match entrada {
        Some(e) => e,
        None => {
            let c = token_path.clone();
            match tokio::task::spawn_blocking(move || crate::net::pedir_ja("POST", &c, None)).await {
                Ok(Ok(v)) => (v["url"].as_str().unwrap_or("").to_owned(), v["token"].as_str().unwrap_or("").to_owned()),
                Ok(Err(e)) => return mudar(&|s| s.conexao = Conexao::Falhou(e.msg.clone())),
                Err(e) => return mudar(&|s| s.conexao = Conexao::Falhou(e.to_string())),
            }
        }
    };
    let mut opcoes = RoomOptions::default();
    opcoes.auto_subscribe = true;
    let (room, mut eventos) = match Room::connect(&url, &token, opcoes).await {
        Ok(r) => r,
        Err(e) => return mudar(&|s| s.conexao = Conexao::Falhou(format!("Não entrei na sala de voz ({e})."))),
    };

    // o som do PC: sem o som não há call (mas a sala fica, para ver as telas)
    let som = Som::abrir(&mic, &saida, Some(mistura.clone()), ctl.clone());
    if let Err(e) = &som {
        let e = e.clone();
        mudar(&|s| s.aviso = Some(format!("O som do computador não abriu: {e}")));
    }
    let fonte = NativeAudioSource::new(AudioSourceOptions { echo_cancellation: false, noise_suppression: false, auto_gain_control: false }, TAXA, 1, 0);
    let faixa = LocalAudioTrack::create_audio_track("microfone", RtcAudioSource::Native(fonte.clone()));
    let publicar = TrackPublishOptions { source: TrackSource::Microphone, dtx: true, ..Default::default() };
    if let Err(e) = room.local_participant().publish_track(LocalTrack::Audio(faixa.clone()), publicar).await {
        mudar(&|s| s.aviso = Some(format!("Não consegui ligar o seu microfone na call ({e}).")));
    }
    if !mic_aberto {
        faixa.mute();
    }
    let rt = tokio::runtime::Handle::current();
    if std::env::var("BRITTICO_VOZ_LOG").is_ok() {
        // teste: o que a sala tem e o que chega, uma vez por segundo
        let (m, c, est) = (mistura.clone(), ctl.clone(), estado.clone());
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                let q = m.lock().map(|m| m.quadro()).unwrap_or_default();
                let vids: Vec<_> = est.lock().map(|e| e.videos.iter().map(|(k, q)| (*k, q.w, q.h, q.n)).collect()).unwrap_or_default();
                eprintln!("[voz] mic={:.3} chegando={:?} videos={:?}", c.nivel_mic.get(), q, vids);
            }
        });
    }
    if let Ok(mut d) = ctl.destino.lock() {
        let fonte = fonte.clone();
        *d = Some(Box::new(move |q: &[i16]| {
            let quadro = AudioFrame { data: q.to_vec().into(), sample_rate: TAXA, num_channels: 1, samples_per_channel: QUADRO as u32 };
            let _ = rt.block_on(fonte.capture_frame(&quadro));
        }));
    }
    mudar(&|s| s.conexao = Conexao::Dentro);

    let mut tela: Option<(LocalVideoTrack, std::sync::mpsc::SyncSender<()>)> = None;
    if std::env::var("BRITTICO_VOZ_TELA").is_ok() {
        let modo = if std::env::var("BRITTICO_VOZ_TELA").is_ok_and(|v| v == "fluidez") { Modo::Fluidez } else { Modo::Qualidade };
        match mostrar_tela(&room, modo, None).await {
            Ok(t) => { tela = Some(t); mudar(&|s| s.tela = true); }
            Err(e) => eprintln!("[voz] tela: {e}"),
        }
    }
    loop {
        tokio::select! {
            ev = eventos.recv() => {
                let Some(ev) = ev else { break };
                match ev {
                    RoomEvent::TrackSubscribed { track, participant, publication } => {
                        let k = participant.identity().to_string();
                        match track {
                            RemoteTrack::Audio(a) => {
                                // o áudio da tela de alguém entra na mistura junto com a voz dele
                                let chave = if publication.source() == TrackSource::ScreenshareAudio { format!("{k}:tela") } else { k };
                                let m = mistura.clone();
                                tokio::spawn(async move {
                                    let mut fluxo = NativeAudioStream::new(a.rtc_track(), TAXA as i32, 1);
                                    while let Some(q) = fluxo.next().await {
                                        if let Ok(mut m) = m.lock() { m.chegou(&chave, &q.data); }
                                    }
                                });
                            }
                            RemoteTrack::Video(v) => {
                                let id: i64 = k.parse().unwrap_or(0);
                                let e_tela = publication.source() == TrackSource::Screenshare;
                                let (est, c) = (estado.clone(), ctx.clone());
                                tokio::spawn(async move {
                                    let mut fluxo = NativeVideoStream::new(v.rtc_track());
                                    let mut n = 0u64;
                                    let mut ultimo = std::time::Instant::now() - std::time::Duration::from_secs(1);
                                    while let Some(f) = fluxo.next().await {
                                        // até ~60 quadros por segundo na tela (a tela de quem escolheu "Fluidez")
                                        if ultimo.elapsed() < std::time::Duration::from_millis(15) { continue; }
                                        ultimo = std::time::Instant::now();
                                        let (w, h) = (f.buffer.width() as usize, f.buffer.height() as usize);
                                        if w == 0 || h == 0 { continue; }
                                        let mut rgba = vec![0u8; w * h * 4];
                                        f.buffer.to_argb(livekit::webrtc::video_frame::VideoFormatType::ABGR, &mut rgba, (w * 4) as u32, w as i32, h as i32);
                                        n += 1;
                                        if let Ok(mut e) = est.lock() { e.videos.insert((id, e_tela), Quadro { w, h, rgba, n }); }
                                        c.request_repaint();
                                    }
                                    if let Ok(mut e) = est.lock() { e.videos.remove(&(id, e_tela)); }
                                    c.request_repaint();
                                });
                            }
                        }
                    }
                    RoomEvent::TrackUnsubscribed { participant, publication, .. } => {
                        let id: i64 = participant.identity().to_string().parse().unwrap_or(0);
                        if publication.kind() == TrackKind::Video {
                            let e_tela = publication.source() == TrackSource::Screenshare;
                            mudar(&|s| { s.videos.remove(&(id, e_tela)); });
                        }
                    }
                    RoomEvent::ParticipantDisconnected(p) => {
                        let k = p.identity().to_string();
                        if let Ok(mut m) = mistura.lock() { m.saiu(&k); m.saiu(&format!("{k}:tela")); }
                        let id: i64 = k.parse().unwrap_or(0);
                        mudar(&|s| s.videos.retain(|(i, _), _| *i != id));
                    }
                    RoomEvent::LocalTrackUnpublished { publication, .. } if publication.source() == TrackSource::Microphone => {
                        mudar(&|s| s.aviso = Some("Um moderador fechou o seu microfone nesta call.".into()));
                    }
                    RoomEvent::Disconnected { reason } => {
                        mudar(&|s| s.conexao = Conexao::Falhou(format!("A call caiu ({reason:?}).")));
                        break;
                    }
                    RoomEvent::Reconnecting => mudar(&|s| s.conexao = Conexao::Entrando),
                    RoomEvent::Reconnected => mudar(&|s| s.conexao = Conexao::Dentro),
                    _ => {}
                }
            }
            cmd = rx.recv() => {
                match cmd {
                    Some(Cmd::Mic(aberto)) => if aberto { faixa.unmute() } else { faixa.mute() },
                    Some(Cmd::Surdo(s)) => if let Ok(mut m) = mistura.lock() { m.surdo = s },
                    Some(Cmd::Volume(id, v)) => if let Ok(mut m) = mistura.lock() { m.alvo(&id.to_string(), v); m.alvo(&format!("{id}:tela"), v) },
                    Some(Cmd::Tela(Some((modo, escolha)))) if tela.is_none() => {
                        match mostrar_tela(&room, modo, escolha).await {
                            Ok(t) => { tela = Some(t); mudar(&|s| s.tela = true); }
                            Err(e) => mudar(&|s| { s.tela = false; s.aviso = Some(e.clone()); }),
                        }
                    }
                    Some(Cmd::Tela(None)) => {
                        if let Some((faixa_tela, parar)) = tela.take() {
                            drop(parar);
                            let _ = room.local_participant().unpublish_track(&faixa_tela.sid()).await;
                        }
                        mudar(&|s| s.tela = false);
                    }
                    Some(Cmd::Tela(Some(_))) => {}
                    Some(Cmd::Sair) | None => break,
                }
            }
        }
    }
    if let Ok(mut d) = ctl.destino.lock() {
        *d = None;
    }
    drop(som);
    let _ = room.close().await;
    mudar(&|s| {
        if !matches!(s.conexao, Conexao::Falhou(_)) {
            s.conexao = Conexao::Saiu;
        }
        s.videos.clear();
    });
}

/// Mostra a tela: o capturador do libwebrtc (no Windows a tela inteira; no Linux com Wayland o seletor do sistema)
/// manda quadros BGRA, que viram I420 (no máximo 1080p) e saem por uma faixa de vídeo `Screenshare` no ritmo, no bitrate
/// e com a preferência do `modo`.
async fn mostrar_tela(room: &Room, modo: Modo, escolha: Option<Escolha>) -> Result<(LocalVideoTrack, std::sync::mpsc::SyncSender<()>), String> {
    use livekit::webrtc::desktop_capturer::{DesktopCaptureSourceType, DesktopCapturer, DesktopCapturerOptions};
    #[cfg(target_os = "linux")]
    let generico = if seletor_do_sistema() { DesktopCaptureSourceType::Generic } else { DesktopCaptureSourceType::Screen };
    #[cfg(not(target_os = "linux"))]
    let generico = DesktopCaptureSourceType::Screen;
    let tipo = match escolha {
        Some(e) if e.janela => DesktopCaptureSourceType::Window,
        Some(_) => DesktopCaptureSourceType::Screen,
        None => generico,
    };
    let mut opcoes = DesktopCapturerOptions::new(tipo);
    opcoes.set_include_cursor(true);
    let fonte = NativeVideoSource::new(VideoResolution { width: 1920, height: 1080 }, true);
    let faixa = LocalVideoTrack::create_video_track("tela", RtcVideoSource::Native(fonte.clone()));
    let (parar_tx, parar_rx) = std::sync::mpsc::sync_channel::<()>(0);
    let (pronto_tx, pronto_rx) = tokio::sync::oneshot::channel::<Result<(), String>>();
    // 0 = esperando, 1 = chegou quadro, 2 = a captura acabou (no Wayland: a pessoa cancelou o seletor do sistema)
    let situacao = Arc::new(std::sync::atomic::AtomicU8::new(0));
    let sit = situacao.clone();
    let pelo_sistema = escolha.is_none() && seletor_do_sistema();
    // o capturador não pode mudar de thread: vive numa só, pedindo um quadro a cada 1/fps até a faixa sair
    let intervalo = std::time::Duration::from_secs_f64(1.0 / modo.fps());
    std::thread::Builder::new()
        .name("call-tela".into())
        .spawn(move || {
            let Some(mut cap) = DesktopCapturer::new(opcoes) else {
                let _ = pronto_tx.send(Err("Este computador não deixou capturar a tela.".into()));
                return;
            };
            let lista = cap.get_source_list();
            let fonte_escolhida = match escolha {
                Some(e) => match lista.into_iter().find(|f| f.id() == e.id) {
                    Some(f) => Some(f),
                    None => {
                        let _ = pronto_tx.send(Err(if e.janela { "Essa janela foi fechada." } else { "Essa tela não está mais ligada." }.into()));
                        return;
                    }
                },
                None => lista.into_iter().next(),
            };
            cap.start_capture(fonte_escolhida, move |r| {
                let q = match r {
                    Ok(q) => q,
                    Err(livekit::webrtc::desktop_capturer::CaptureError::Permanent) => {
                        if sit.swap(2, Ordering::Relaxed) != 2 && std::env::var("BRITTICO_VOZ_LOG").is_ok() {
                            eprintln!("[voz] tela: a captura acabou");
                        }
                        return;
                    }
                    Err(_) => return,
                };
                let (w, h) = (q.width() as u32 & !1, q.height() as u32 & !1);
                if w == 0 || h == 0 {
                    return;
                }
                let mut buf = I420Buffer::new(w, h);
                let (sy, su, sv) = buf.strides();
                let (dy, du, dv) = buf.data_mut();
                livekit::webrtc::native::yuv_helper::argb_to_i420(q.data(), q.stride(), dy, sy, du, su, dv, sv, w as i32, h as i32);
                if h > ALTURA_MAX {
                    let nw = ((w as u64 * ALTURA_MAX as u64 / h as u64) as i32) & !1;
                    buf = buf.scale(nw, ALTURA_MAX as i32);
                }
                let quadro = VideoFrame { rotation: VideoRotation::VideoRotation0, timestamp_us: 0, frame_metadata: None, buffer: buf };
                fonte.capture_frame(&quadro);
                if sit.compare_exchange(0, 1, Ordering::Relaxed, Ordering::Relaxed).is_ok() && std::env::var("BRITTICO_VOZ_LOG").is_ok() {
                    eprintln!("[voz] tela: primeiro quadro {w}x{h}");
                }
            });
            // só diz que começou quando o primeiro quadro chega: no Wayland isso é depois de a pessoa escolher no
            // seletor do sistema (que pode levar o tempo dela); se ela cancelar, a tela nem entra na call
            let mut pronto_tx = Some(pronto_tx);
            let prazo = std::time::Instant::now() + std::time::Duration::from_secs(if pelo_sistema { 180 } else { 20 });
            // o relógio marca a hora de cada quadro; se um atrasar, o próximo não tenta compensar em rajada
            let mut proximo = std::time::Instant::now();
            loop {
                proximo += intervalo;
                let agora = std::time::Instant::now();
                if proximo < agora {
                    proximo = agora;
                }
                match parar_rx.recv_timeout(proximo - agora) {
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => cap.capture_frame(),
                    _ => break,
                }
                if pronto_tx.is_some() {
                    let erro = match situacao.load(Ordering::Relaxed) {
                        1 => None,
                        2 => Some(if pelo_sistema { "A tela não foi escolhida." } else { "Não deu para capturar essa tela." }),
                        _ if std::time::Instant::now() > prazo => Some("A captura da tela não começou."),
                        _ => continue,
                    };
                    let falhou = erro.is_some();
                    let _ = pronto_tx.take().unwrap().send(erro.map_or(Ok(()), |e| Err(e.to_owned())));
                    if falhou {
                        break;
                    }
                }
            }
        })
        .map_err(|e| e.to_string())?;
    match pronto_rx.await {
        Ok(Ok(())) => {}
        Ok(Err(e)) => return Err(e),
        Err(_) => return Err("A captura da tela não começou.".into()),
    }
    let publicar = TrackPublishOptions {
        source: TrackSource::Screenshare,
        video_codec: VideoCodec::VP8,
        simulcast: false,
        video_encoding: Some(VideoEncoding { max_bitrate: modo.bitrate(), max_framerate: modo.fps() }),
        degradation_preference: Some(modo.degradar()),
        ..Default::default()
    };
    room.local_participant().publish_track(LocalTrack::Video(faixa.clone()), publicar).await.map_err(|e| format!("Não consegui mostrar a tela ({e})."))?;
    Ok((faixa, parar_tx))
}

/// Liga o volume de cada pessoa ao que a tela escolheu e diz se o microfone fica aberto (para os testes: `BRITTICO_VOZ=nao`
/// entra na call sem abrir aparelho nenhum).
pub fn sem_som() -> bool {
    std::env::var("OLHEIRO_SOM").is_ok_and(|v| v == "nao")
}

#[allow(dead_code)]
fn _usado(c: &Controles) -> bool {
    c.mudo.load(Ordering::Relaxed)
}

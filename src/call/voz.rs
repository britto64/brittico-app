//! A voz da Call: o canal de voz é uma sala do LiveKit (`canal:<id>`), e quem dá a entrada é o bot
//! (`POST /api/diskenzy/v2/voice/:canal/token`, a mesma rota do site). O som do PC é o do Olheiro (`olheiro_motor::som`):
//! o microfone limpo pelo WebRTC e a mistura de quem fala, com volume por pessoa. O vídeo que chega (a tela e a câmera de
//! cada um) vira quadros RGBA que a tela sobe para textura; a própria tela sai pelo capturador do libwebrtc.
//! No Windows, se houver um MFT H.264 de hardware, a tela é codificada na GPU e publicada já em H.264.
//!
//! Tudo roda num runtime do tokio numa thread própria; a tela manda `Cmd` e lê o `Estado`.

use livekit::options::{DegradationPreference, TrackPublishOptions, VideoCodec, VideoEncoding, VideoPreset};
use livekit::prelude::*;
use livekit::track::VideoQuality;
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

/// Um quadro de vídeo que chegou: largura, altura, RGBA e um número que sobe a cada quadro novo. `borrado` é o print
/// parado de uma tela que não está selecionada (pequeno e desfocado; a tela sobe a textura esticada).
pub struct Quadro {
    pub w: usize,
    pub h: usize,
    pub rgba: Vec<u8>,
    pub n: u64,
    pub borrado: bool,
}

/// A largura do print borrado (a altura segue a proporção). Pequeno de propósito: esticado no quadro, já fica desfocado.
const BORRADO_W: usize = 96;
/// De quanto em quanto tempo o print de uma tela parada é tirado de novo, e quanto se espera por ele.
const PRINT_DE_NOVO: std::time::Duration = std::time::Duration::from_secs(60);
const PRINT_ESPERA: std::time::Duration = std::time::Duration::from_secs(5);

/// O quadro é (quase) todo preto: amostra uma grade de pontos.
fn quase_preto(rgba: &[u8]) -> bool {
    let n = rgba.len() / 4;
    let passo = (n / 257).max(1);
    (0..n).step_by(passo).all(|i| rgba[i * 4] < 12 && rgba[i * 4 + 1] < 12 && rgba[i * 4 + 2] < 12)
}

/// O print borrado de um quadro RGBA: média em blocos até `BORRADO_W` de largura e duas passadas de desfoque 3x3.
fn borrar(w: usize, h: usize, rgba: &[u8]) -> (usize, usize, Vec<u8>) {
    if w == 0 || h == 0 || rgba.len() < w * h * 4 {
        return (w, h, rgba.to_vec());
    }
    let bw = BORRADO_W.min(w).max(1);
    let bh = (h * bw / w).max(1);
    let mut out = vec![0u8; bw * bh * 4];
    for by in 0..bh {
        let (y0, y1) = (by * h / bh, ((by + 1) * h / bh).max(by * h / bh + 1));
        for bx in 0..bw {
            let (x0, x1) = (bx * w / bw, ((bx + 1) * w / bw).max(bx * w / bw + 1));
            let mut soma = [0u64; 4];
            // amostra em passos de 2 (o resultado é desfocado de qualquer jeito; metade do trabalho)
            let mut n = 0u64;
            for y in (y0..y1).step_by(2) {
                for x in (x0..x1).step_by(2) {
                    let i = (y * w + x) * 4;
                    for c in 0..4 {
                        soma[c] += rgba[i + c] as u64;
                    }
                    n += 1;
                }
            }
            let o = (by * bw + bx) * 4;
            for c in 0..4 {
                out[o + c] = (soma[c] / n.max(1)) as u8;
            }
            out[o + 3] = 255;
        }
    }
    for _ in 0..2 {
        let antes = out.clone();
        for y in 0..bh {
            for x in 0..bw {
                let mut soma = [0u32; 3];
                let mut n = 0u32;
                for yy in y.saturating_sub(1)..(y + 2).min(bh) {
                    for xx in x.saturating_sub(1)..(x + 2).min(bw) {
                        let i = (yy * bw + xx) * 4;
                        for c in 0..3 {
                            soma[c] += antes[i + c] as u32;
                        }
                        n += 1;
                    }
                }
                let o = (y * bw + x) * 4;
                for c in 0..3 {
                    out[o + c] = (soma[c] / n) as u8;
                }
            }
        }
    }
    (bw, bh, out)
}

#[derive(Default)]
pub struct Estado {
    pub conexao: Conexao,
    /// O vídeo de cada um: (profileId, é a tela?) → o último quadro.
    pub videos: HashMap<(i64, bool), Quadro>,
    /// Mostrando a própria tela agora.
    pub tela: bool,
    /// Como cada tela que chega deve vir: 2 = em tempo real (a selecionada, na camada cheia); 0 = parada, um print
    /// borrado tirado de minuto em minuto, sem vídeo chegando (o mesmo que o site faz).
    pub qualidade: HashMap<(i64, bool), u8>,
    /// As telas paradas que estão esperando o quadro do print. Fora disso, quadro de tela parada é descartado: com o
    /// vídeo desligado o libwebrtc ainda entrega uns quadros pretos (até de 8x8), que apagariam o print.
    pub esperando_print: std::collections::HashSet<(i64, bool)>,
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
            // 30 (2026-11-01, pedido dele: um modo só, o Fluidez a 30 em vez de 60)
            Modo::Fluidez => 30.0,
        }
    }
    /// O teto de bitrate (bits por segundo): 12 Mbps (pedido dele, 2026-11-01). Na conta da porta de 200 Mbps da
    /// Contabo, 5 pessoas com 3 telas usam ~144 Mbps; 7 com 4 telas passariam (ver a memória do app).
    fn bitrate(self) -> u64 {
        12_000_000
    }
    /// O que ceder quando os bits não bastam. Atenção: os nomes seguem o WebRTC (`MaintainFramerate` = segura os
    /// quadros e baixa a resolução), ao contrário do que dizem os comentários da crate libwebrtc.
    /// - Fluidez (o LoL, movimento o tempo todo): segura os quadros E a resolução; só a nitidez cede até caber no teto.
    /// - Qualidade: segura a resolução; os quadros por segundo cedem.
    fn degradar(self) -> DegradationPreference {
        match std::env::var("BRITTICO_TELA_DEGRADAR").as_deref() {
            Ok("res") => return DegradationPreference::MaintainFramerate,
            Ok("fps") => return DegradationPreference::MaintainResolution,
            Ok("equilibrio") => return DegradationPreference::Balanced,
            Ok("nada") => return DegradationPreference::MaintainFramerateAndResolution,
            _ => {}
        }
        match self {
            Modo::Qualidade => DegradationPreference::MaintainResolution,
            Modo::Fluidez => DegradationPreference::MaintainFramerateAndResolution,
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

/// A camada extra da tela (a cheia é a da faixa), a mesma do site: a média de 540p a 30, o degrau para onde a tela
/// selecionada desce quando a banda aperta. A miniatura de 144p saiu (2026-11-03): a tela não selecionada não recebe
/// vídeo, fica num print borrado tirado de minuto em minuto nesta camada.
fn camadas(_modo: Modo, _fullhd: bool) -> Vec<VideoPreset> {
    vec![VideoPreset { width: 960, height: 540, encoding: VideoEncoding { max_bitrate: 1_500_000, max_framerate: 30.0 } }]
}

/// A altura da tela que sai: HD (720p, o padrão) ou Full HD (1080p). Maior que isso é reduzido.
fn altura(fullhd: bool) -> u32 {
    if fullhd {
        1080
    } else {
        720
    }
}

pub enum Cmd {
    Mic(bool),
    Surdo(bool),
    /// Mostrar a tela no modo dado (e a tela ou janela escolhida no seletor do app; `None` = o seletor do sistema no
    /// Wayland), ou parar (`None`).
    /// O `bool` é o Full HD (1080p; sem ele, HD 720p).
    Tela(Option<(Modo, Option<Escolha>, bool)>),
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

    /// Tempo real (a camada cheia) só para a tela selecionada; as outras ficam paradas num print borrado.
    pub fn pedir_qualidade(&self, chave: (i64, bool), tempo_real: bool) {
        let q = if tempo_real { 2 } else { 0 };
        if let Ok(mut e) = self.estado.lock() {
            e.qualidade.insert(chave, q);
        }
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
        let fullhd = std::env::var("BRITTICO_TELA_FULLHD").is_ok();
        match mostrar_tela(&room, modo, None, fullhd, &estado, &ctx).await {
            Ok(t) => { tela = Some(t); mudar(&|s| s.tela = true); }
            Err(e) => eprintln!("[voz] tela: {e}"),
        }
    }
    // as publicações de vídeo que chegam, para pedir a camada certa de cada uma
    let mut publicacoes: HashMap<(i64, bool), RemoteTrackPublication> = HashMap::new();
    let mut aplicada: HashMap<(i64, bool), u8> = HashMap::new();
    // as telas paradas: quando foi o último print e, enquanto um print novo é tirado, desde quando e o número do quadro
    // que havia antes (o print chegou quando o número sobe)
    let mut prints: HashMap<(i64, bool), std::time::Instant> = HashMap::new();
    let mut tirando: HashMap<(i64, bool), (std::time::Instant, u64)> = HashMap::new();
    let mut relogio = tokio::time::interval(std::time::Duration::from_millis(250));
    loop {
        tokio::select! {
            _ = relogio.tick() => {
                let pedidas: Vec<((i64, bool), u8)> = estado.lock().map(|e| e.qualidade.iter().map(|(k, v)| (*k, *v)).collect()).unwrap_or_default();
                let agora = std::time::Instant::now();
                for (k, q) in pedidas {
                    let Some(p) = publicacoes.get(&k) else { continue };
                    if !p.is_subscribed() {
                        continue;
                    }
                    if q != 0 {
                        // tempo real: o vídeo volta, na camada cheia
                        if aplicada.get(&k) != Some(&q) {
                            if std::env::var("BRITTICO_VOZ_LOG").is_ok() {
                                eprintln!("[voz] tela de {}: tempo real", k.0);
                            }
                            if let Ok(mut e) = estado.lock() {
                                e.esperando_print.remove(&k);
                            }
                            p.set_enabled(true);
                            if p.simulcasted() {
                                p.set_video_quality(VideoQuality::High);
                            }
                            aplicada.insert(k, q);
                            tirando.remove(&k);
                        }
                        continue;
                    }
                    let n_agora = estado.lock().ok().and_then(|e| e.videos.get(&k).map(|v| v.n)).unwrap_or(0);
                    if aplicada.get(&k) != Some(&0) {
                        // acabou de sair do tempo real: a última imagem vira o print, sem pedir vídeo de novo
                        let tem = estado.lock().ok().is_some_and(|mut e| match e.videos.get_mut(&k) {
                            Some(v) if v.borrado => true,
                            Some(v) if v.w >= 64 && v.h >= 36 && !quase_preto(&v.rgba) => {
                                let (w, h, rgba) = borrar(v.w, v.h, &v.rgba);
                                *v = Quadro { w, h, rgba, n: v.n + 1, borrado: true };
                                true
                            }
                            _ => false,
                        });
                        aplicada.insert(k, 0);
                        if std::env::var("BRITTICO_VOZ_LOG").is_ok() {
                            eprintln!("[voz] tela de {}: parada ({})", k.0, if tem { "a última imagem virou o print" } else { "sem imagem ainda" });
                        }
                        if tem {
                            p.set_enabled(false);
                            prints.insert(k, agora);
                            tirando.remove(&k);
                        }
                        ctx.request_repaint();
                    }
                    match tirando.get(&k).copied() {
                        Some((desde, n0)) => {
                            // o print chegou (ou passou o tempo): o vídeo para de novo
                            if n_agora > n0 || agora - desde > PRINT_ESPERA {
                                if std::env::var("BRITTICO_VOZ_LOG").is_ok() {
                                    eprintln!("[voz] tela de {}: {} (vídeo desligado)", k.0, if n_agora > n0 { "print novo" } else { "print não chegou a tempo" });
                                }
                                if let Ok(mut e) = estado.lock() {
                                    e.esperando_print.remove(&k);
                                }
                                p.set_enabled(false);
                                prints.insert(k, agora);
                                tirando.remove(&k);
                            }
                        }
                        None => {
                            if prints.get(&k).map_or(true, |t| agora - *t >= PRINT_DE_NOVO) {
                                // tirar um print: liga o vídeo na camada menor (540p) até chegar um quadro
                                if let Ok(mut e) = estado.lock() {
                                    e.esperando_print.insert(k);
                                }
                                p.set_enabled(true);
                                if p.simulcasted() {
                                    p.set_video_quality(VideoQuality::Low);
                                }
                                tirando.insert(k, (agora, n_agora));
                            }
                        }
                    }
                }
            }
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
                                publicacoes.insert((id, e_tela), publication.clone());
                                aplicada.remove(&(id, e_tela));
                                prints.remove(&(id, e_tela));
                                tirando.remove(&(id, e_tela));
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
                                        // a tela parada (não selecionada) só aceita o quadro do print, que vira o borrado;
                                        // quadro preto ou minúsculo (o que o libwebrtc entrega com o vídeo desligado) não conta
                                        let (parada, esperando) = est.lock().map(|e| (e_tela && e.qualidade.get(&(id, e_tela)) == Some(&0), e.esperando_print.contains(&(id, e_tela)))).unwrap_or((false, false));
                                        if parada && (!esperando || w < 64 || h < 36 || quase_preto(&rgba)) { continue; }
                                        let (w, h, rgba) = if parada { borrar(w, h, &rgba) } else { (w, h, rgba) };
                                        if let Ok(mut e) = est.lock() {
                                            if parada { e.esperando_print.remove(&(id, e_tela)); }
                                            n = n.max(e.videos.get(&(id, e_tela)).map_or(0, |q| q.n)) + 1;
                                            e.videos.insert((id, e_tela), Quadro { w, h, rgba, n, borrado: parada });
                                        }
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
                            publicacoes.remove(&(id, e_tela));
                            aplicada.remove(&(id, e_tela));
                            prints.remove(&(id, e_tela));
                            tirando.remove(&(id, e_tela));
                            mudar(&|s| { s.videos.remove(&(id, e_tela)); s.qualidade.remove(&(id, e_tela)); });
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
                    Some(Cmd::Tela(Some((modo, escolha, fullhd)))) if tela.is_none() => {
                        match mostrar_tela(&room, modo, escolha, fullhd, &estado, &ctx).await {
                            Ok(t) => { tela = Some(t); mudar(&|s| s.tela = true); }
                            Err(e) => mudar(&|s| { s.tela = false; s.aviso = Some(e.clone()); }),
                        }
                    }
                    Some(Cmd::Tela(None)) => {
                        if let Some((faixa_tela, parar)) = tela.take() {
                            drop(parar);
                            let _ = room.local_participant().unpublish_track(&faixa_tela.sid()).await;
                        }
                        let eu: i64 = room.local_participant().identity().to_string().parse().unwrap_or(0);
                        mudar(&|s| { s.tela = false; s.videos.remove(&(eu, true)); });
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

/// BGRA do capturador → I420, com o cursor (Windows), a escala e a prévia de quem mostra.
struct Montagem {
    alt: u32,
    eu: i64,
    log: bool,
    contados: u64,
    desde: std::time::Instant,
    ultima_previa: std::time::Instant,
    estado: Arc<Mutex<Estado>>,
    ctx: eframe::egui::Context,
    #[cfg(windows)]
    cursor: super::cursor::Cursor,
    #[cfg(windows)]
    janela: Option<u64>,
}

impl Montagem {
    fn nova(alt: u32, eu: i64, log: bool, estado: Arc<Mutex<Estado>>, ctx: eframe::egui::Context, janela: Option<u64>) -> Self {
        #[cfg(not(windows))]
        let _ = janela;
        Self {
            alt,
            eu,
            log,
            contados: 0,
            desde: std::time::Instant::now(),
            ultima_previa: std::time::Instant::now() - std::time::Duration::from_secs(1),
            estado,
            ctx,
            #[cfg(windows)]
            cursor: super::cursor::Cursor::default(),
            #[cfg(windows)]
            janela,
        }
    }

    fn i420(&mut self, dados: &[u8], stride: u32, w: u32, h: u32, esquerda: i32, topo: i32) -> Option<I420Buffer> {
        let (w, h) = (w & !1, h & !1);
        if w == 0 || h == 0 {
            return None;
        }
        let mut buf = I420Buffer::new(w, h);
        let (sy, su, sv) = buf.strides();
        let (dy, du, dv) = buf.data_mut();
        livekit::webrtc::native::yuv_helper::argb_to_i420(dados, stride, dy, sy, du, su, dv, sv, w as i32, h as i32);
        #[cfg(windows)]
        if let Some((x0, y0, pw, ph, px)) = self.cursor.retalho(dados, stride as usize, w as i32, h as i32, super::cursor::origem(self.janela, (esquerda, topo))) {
            // o retalho com o cursor, em I420, por cima do quadro
            let mut rb = I420Buffer::new(pw as u32, ph as u32);
            let (rsy, rsu, rsv) = rb.strides();
            let (ry, ru, rv) = rb.data_mut();
            livekit::webrtc::native::yuv_helper::argb_to_i420(&px, (pw * 4) as u32, ry, rsy, ru, rsu, rv, rsv, pw, ph);
            let (sy, su, sv) = buf.strides();
            let (dy, du, dv) = buf.data_mut();
            let (x0, y0, pw, ph) = (x0 as usize, y0 as usize, pw as usize, ph as usize);
            for y in 0..ph {
                dy[(y0 + y) * sy as usize + x0..(y0 + y) * sy as usize + x0 + pw].copy_from_slice(&ry[y * rsy as usize..y * rsy as usize + pw]);
            }
            for y in 0..ph / 2 {
                let (d, o) = ((y0 / 2 + y) * su as usize + x0 / 2, y * rsu as usize);
                du[d..d + pw / 2].copy_from_slice(&ru[o..o + pw / 2]);
                let (d, o) = ((y0 / 2 + y) * sv as usize + x0 / 2, y * rsv as usize);
                dv[d..d + pw / 2].copy_from_slice(&rv[o..o + pw / 2]);
            }
        }
        #[cfg(not(windows))]
        let _ = (esquerda, topo);
        if h > self.alt {
            let nw = ((w as u64 * self.alt as u64 / h as u64) as i32) & !1;
            buf = buf.scale(nw, self.alt as i32);
        }
        if self.ultima_previa.elapsed() >= std::time::Duration::from_millis(100) {
            self.ultima_previa = std::time::Instant::now();
            use livekit::webrtc::video_frame::native::VideoFrameBufferExt;
            use livekit::webrtc::video_frame::VideoBuffer;
            let (bw, bh) = (buf.width() as i32, buf.height() as i32);
            let pw = bw.min(960) & !1;
            let ph = ((bh as i64 * pw as i64 / bw.max(1) as i64) as i32) & !1;
            if pw > 0 && ph > 0 {
                let menor = buf.scale(pw, ph);
                let (w2, h2) = (pw as usize, ph as usize);
                let mut rgba = vec![0u8; w2 * h2 * 4];
                menor.to_argb(livekit::webrtc::video_frame::VideoFormatType::ABGR, &mut rgba, (w2 * 4) as u32, pw, ph);
                if let Ok(mut e) = self.estado.lock() {
                    let n = e.videos.get(&(self.eu, true)).map_or(1, |q| q.n + 1);
                    e.videos.insert((self.eu, true), Quadro { w: w2, h: h2, rgba, n, borrado: false });
                }
                self.ctx.request_repaint();
            }
        }
        if self.log {
            self.contados += 1;
            if self.desde.elapsed() >= std::time::Duration::from_secs(2) {
                eprintln!("[voz] tela: {:.1} quadros/s capturados ({w}x{h})", self.contados as f64 / self.desde.elapsed().as_secs_f64());
                self.contados = 0;
                self.desde = std::time::Instant::now();
            }
        }
        Some(buf)
    }
}

/// Codifica o I420 na GPU e, depois que a faixa foi publicada, manda o H.264 para o LiveKit.
#[cfg(windows)]
struct EstadoHw {
    fonte: NativeVideoSource,
    cod: Option<tela_hw::Codificador>,
    publicado: Arc<std::sync::atomic::AtomicBool>,
    fps_mili: Arc<std::sync::atomic::AtomicU32>,
    fps: u32,
    bitrate: u64,
    teto: u64,
    log: bool,
    viu_chave: bool,
    encaminhar: bool,
    sem_saida: u32,
    anunciado: bool,
    /// Depois de publicada, uma falha da placa não troca de codificador: fecha, espera e reabre. Seguidas demais,
    /// desiste e avisa (a tela fica parada até a pessoa mostrar de novo).
    falhas: u32,
    reabrir_em: Option<std::time::Instant>,
    desistiu: bool,
    estado: Arc<Mutex<Estado>>,
}

/// Quantas reaberturas seguidas da placa, com a tela já na call, antes de desistir.
#[cfg(windows)]
const FALHAS_MAX: u32 = 5;

#[cfg(windows)]
impl EstadoHw {
    #[allow(clippy::too_many_arguments)]
    fn novo(fonte: NativeVideoSource, fps: u32, bitrate: u64, publicado: Arc<std::sync::atomic::AtomicBool>, fps_mili: Arc<std::sync::atomic::AtomicU32>, log: bool, estado: Arc<Mutex<Estado>>) -> Self {
        Self {
            fonte,
            cod: None,
            publicado,
            fps_mili,
            fps: fps.max(1),
            bitrate,
            teto: bitrate,
            log,
            viu_chave: false,
            encaminhar: false,
            sem_saida: 0,
            anunciado: false,
            falhas: 0,
            reabrir_em: None,
            desistiu: false,
            estado,
        }
    }

    /// Como `tratar`, mas com a tela já na call uma falha não volta como erro: o codificador fecha e reabre depois
    /// de uma espera (1 s, 2 s, ... até 5 s), sem segurar a captura. Antes de publicar o erro sobe, e a Call
    /// tenta o VP8 de software.
    fn quadro(&mut self, buf: &I420Buffer) -> Result<bool, String> {
        if !self.publicado.load(Ordering::Acquire) {
            return self.tratar(buf);
        }
        if self.desistiu || self.reabrir_em.is_some_and(|t| std::time::Instant::now() < t) {
            return Ok(true);
        }
        self.reabrir_em = None;
        match self.tratar(buf) {
            Ok(v) => {
                if self.encaminhar {
                    self.falhas = 0;
                }
                Ok(v)
            }
            Err(e) => {
                self.cod = None;
                self.encaminhar = false;
                self.sem_saida = 0;
                self.falhas += 1;
                eprintln!("[voz] tela: a placa de vídeo falhou ({e}); tentativa {} de {FALHAS_MAX}", self.falhas);
                if self.falhas >= FALHAS_MAX {
                    self.desistiu = true;
                    if let Ok(mut est) = self.estado.lock() {
                        est.aviso = Some("A placa de vídeo parou de codificar a tela. Pare e mostre de novo.".into());
                    }
                } else {
                    self.reabrir_em = Some(std::time::Instant::now() + std::time::Duration::from_secs(self.falhas as u64));
                }
                Ok(true)
            }
        }
    }

    /// `Ok(true)` quando já saiu um quadro chave (a captura pode ser publicada).
    fn tratar(&mut self, buf: &I420Buffer) -> Result<bool, String> {
        use livekit::webrtc::video_frame::VideoBuffer;
        let (w, h) = (buf.width(), buf.height());
        if w < 2 || h < 2 {
            return Ok(self.viu_chave);
        }
        if self.cod.as_ref().map_or(true, |c| c.largura() != w || c.altura() != h) {
            // o teto que a rede pediu por último continua valendo no codificador novo
            let alvo = self.cod.as_ref().map_or(self.bitrate, |c| c.bitrate());
            self.cod = None;
            let media = tela_hw::media_para(h, self.teto);
            let cod = tela_hw::Codificador::abrir(w, h, self.fps, media, alvo, self.teto)?;
            if self.log && !self.anunciado {
                let gpu = tela_hw::fornecedor_gpu().unwrap_or("GPU");
                eprintln!(
                    "[voz] tela: H.264 por hardware ({}, {gpu}), {w}x{h}, taxa variável: média {} Mbps, teto {} Mbps, sem simulcast",
                    cod.nome(),
                    media / 1_000_000,
                    self.teto / 1_000_000
                );
                self.anunciado = true;
            }
            self.cod = Some(cod);
            self.encaminhar = false;
        }
        let nv = i420_nv12(buf);
        let mut forcar = self.publicado.load(Ordering::Acquire) && !self.encaminhar;
        let mut novo_bitrate = None;
        if self.publicado.load(Ordering::Acquire) {
            if self.fonte.take_keyframe_request() {
                forcar = true;
            }
            if let Some(rc) = self.fonte.take_rate_control_request() {
                if rc.target_bitrate_bps > 0 {
                    novo_bitrate = Some(rc.target_bitrate_bps);
                    self.bitrate = tela_hw::bitrate_alvo(rc.target_bitrate_bps, self.teto);
                }
                // não sobe dos fps do modo; o teto de bitrate fica no codificador
                if rc.framerate_fps.is_finite() && rc.framerate_fps >= 5.0 {
                    let mili = (rc.framerate_fps.min(self.fps as f64) * 1000.0) as u32;
                    self.fps_mili.store(mili.max(5_000), Ordering::Relaxed);
                }
            }
        }
        let quadros = {
            let cod = self.cod.as_mut().unwrap();
            if forcar {
                cod.pedir_chave();
            }
            if let Some(b) = novo_bitrate {
                cod.definir_bitrate(b);
            }
            cod.codificar(&nv)?
        };
        if quadros.is_empty() {
            self.sem_saida += 1;
            if self.sem_saida > self.fps.saturating_mul(3).max(30) {
                return Err("não entregou um quadro a tempo".into());
            }
            return Ok(self.viu_chave);
        }
        self.sem_saida = 0;
        if quadros.iter().any(|q| q.chave) {
            self.viu_chave = true;
        }
        if !self.publicado.load(Ordering::Acquire) {
            return Ok(self.viu_chave);
        }
        for q in &quadros {
            if !self.encaminhar && !q.chave {
                continue;
            }
            if !enviar_h264(&self.fonte, q, w, h) {
                self.encaminhar = false;
                if let Some(cod) = self.cod.as_mut() {
                    cod.pedir_chave();
                }
                break;
            }
            if q.chave {
                self.encaminhar = true;
            }
        }
        Ok(true)
    }
}

#[cfg(windows)]
fn i420_nv12(buf: &I420Buffer) -> Vec<u8> {
    use livekit::webrtc::video_frame::VideoBuffer;
    let (w, h) = (buf.width(), buf.height());
    let (sy, su, sv) = buf.strides();
    let (y, u, v) = buf.data();
    let mut nv = vec![0u8; (w as usize) * (h as usize) * 3 / 2];
    let (dy, duv) = nv.split_at_mut((w as usize) * (h as usize));
    livekit::webrtc::native::yuv_helper::i420_to_nv12(y, sy, u, su, v, sv, dy, w, duv, w, w as i32, h as i32);
    nv
}

#[cfg(windows)]
fn enviar_h264(fonte: &NativeVideoSource, acesso: &tela_hw::Acesso, w: u32, h: u32) -> bool {
    use livekit::webrtc::video_frame::{EncodedFrameType, EncodedVideoCodec, EncodedVideoFrame};
    fonte.capture_encoded_frame(&EncodedVideoFrame {
        codec: EncodedVideoCodec::H264,
        payload: &acesso.bytes,
        timestamp_us: 0,
        frame_type: if acesso.chave { EncodedFrameType::Key } else { EncodedFrameType::Delta },
        resolution: VideoResolution { width: w, height: h },
        frame_metadata: None,
    })
}

/// Mostra a tela: o capturador do libwebrtc (no Windows a tela inteira; no Linux com Wayland o seletor do sistema)
/// manda quadros BGRA, que viram I420 (no máximo 1080p) e saem por uma faixa de vídeo `Screenshare` no ritmo, no bitrate
/// e com a preferência do `modo`. No Windows, com um MFT H.264 de hardware, o mesmo quadro é codificado na GPU e
/// publicado já pronto (`PreEncoded`); se o hardware não existir ou falhar antes de publicar, volta ao VP8 de software.
async fn mostrar_tela(room: &Room, modo: Modo, escolha: Option<Escolha>, fullhd: bool, estado: &Arc<Mutex<Estado>>, ctx: &eframe::egui::Context) -> Result<(LocalVideoTrack, std::sync::mpsc::SyncSender<()>), String> {
    #[cfg(windows)]
    if deixar_hardware() {
        if tela_hw::tem_hardware() {
            match publicar_tela(room, modo, escolha, fullhd, estado, ctx, true).await {
                Ok(v) => return Ok(v),
                Err(e) if e.starts_with("hw:") => {
                    eprintln!("[voz] tela: H.264 por hardware indisponível ({}); seguindo no codificador de software", e.trim_start_matches("hw:").trim());
                }
                Err(e) => return Err(e),
            }
        } else if std::env::var("BRITTICO_VOZ_LOG").is_ok() {
            eprintln!("[voz] tela: sem codificador H.264 de hardware; VP8 de software");
        }
    }
    publicar_tela(room, modo, escolha, fullhd, estado, ctx, false).await
}

/// A placa de vídeo só codifica a tela com a chave das Configurações → O app ligada (desligada por padrão, até ser
/// testada em Windows de verdade) ou com `BRITTICO_TELA_HW=1`. `BRITTICO_TELA_HW=0` (ou `nao`/`off`/`software`)
/// força o VP8 mesmo com a chave. `BRITTICO_TELA_CODEC=vp8|vp9|av1` também: o hardware só fala H.264. O padrão
/// sintético (`BRITTICO_TELA_TESTE`) continua medindo o codificador de software.
#[cfg(windows)]
fn deixar_hardware() -> bool {
    if std::env::var("BRITTICO_TELA_TESTE").is_ok() {
        return false;
    }
    let pedido = match std::env::var("BRITTICO_TELA_HW").map(|v| v.to_ascii_lowercase()) {
        Ok(v) if matches!(v.as_str(), "0" | "nao" | "não" | "off" | "software") => return false,
        Ok(v) if matches!(v.as_str(), "1" | "sim" | "on" | "placa") => true,
        _ => super::tela_placa(),
    };
    pedido && !matches!(std::env::var("BRITTICO_TELA_CODEC").as_deref(), Ok("vp8" | "vp9" | "av1"))
}

async fn publicar_tela(room: &Room, modo: Modo, escolha: Option<Escolha>, fullhd: bool, estado: &Arc<Mutex<Estado>>, ctx: &eframe::egui::Context, hardware: bool) -> Result<(LocalVideoTrack, std::sync::mpsc::SyncSender<()>), String> {
    // a prévia da própria tela (como no site): uma cópia pequena, ~10 por segundo, no quadro de quem mostra
    let eu: i64 = room.local_participant().identity().to_string().parse().unwrap_or(0);
    let (est_previa, ctx_previa) = (estado.clone(), ctx.clone());
    let log = std::env::var("BRITTICO_VOZ_LOG").is_ok();
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
    // no Windows quem desenha o cursor é o app (call/cursor.rs): pedir que a captura o embuta poderia dar dois cursores
    opcoes.set_include_cursor(!cfg!(windows));
    // o modo "conteúdo de tela" do libwebrtc fica DESLIGADO: com movimento rápido ele descarta quadros de propósito e
    // nem usa a banda que tem (medido: 2 a 4 fps com ~3 Mbps de 8). BRITTICO_TELA_SCREENCAST=1 e
    // BRITTICO_TELA_CODEC=vp8|h264|vp9|av1 servem para medir. No Windows, sem codec (ou com h264) e com MFT de
    // hardware, quem codifica é a GPU; vp8/vp9/av1 continuam no software, e h264 sem hardware também.
    let screencast = std::env::var("BRITTICO_TELA_SCREENCAST").is_ok_and(|v| v == "1");
    let alt = altura(fullhd);
    let resolucao = VideoResolution { width: alt * 16 / 9, height: alt };
    // o hardware publica unidade de acesso pronta: sem o quadro preto de keepalive, que acordaria um codificador de verdade
    let fonte = if hardware { NativeVideoSource::new_encoded(resolucao) } else { NativeVideoSource::new(resolucao, screencast) };
    let fps_mili = Arc::new(std::sync::atomic::AtomicU32::new((modo.fps() * 1000.0) as u32));
    let publicado = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let erro_hw: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let faixa = LocalVideoTrack::create_video_track("tela", RtcVideoSource::Native(fonte.clone()));
    let (parar_tx, parar_rx) = std::sync::mpsc::sync_channel::<()>(0);
    let (pronto_tx, pronto_rx) = tokio::sync::oneshot::channel::<Result<(), String>>();
    // 0 = esperando, 1 = chegou quadro, 2 = a captura acabou (no Wayland: a pessoa cancelou o seletor do sistema)
    let situacao = Arc::new(std::sync::atomic::AtomicU8::new(0));
    let sit = situacao.clone();
    let pelo_sistema = escolha.is_none() && seletor_do_sistema();
    // o capturador não pode mudar de thread: vive numa só, pedindo um quadro a cada 1/fps até a faixa sair
    let intervalo = std::time::Duration::from_secs_f64(1.0 / modo.fps());
    // no erro do hardware a thread tem que terminar antes da tentativa em software, para soltar a captura DXGI
    let captura = if std::env::var("BRITTICO_TELA_TESTE").is_ok() {
        // BRITTICO_TELA_TESTE: no lugar da tela, uma de mentira com muito movimento (uma "janela" de textura arrastada
        // pela tela inteira), para medir o codificador sem depender do seletor nem do que está aberto
        tela_de_teste(fonte.clone(), parar_rx, pronto_tx, intervalo, alt);
        None
    } else {
    #[cfg(windows)]
    let publicado_thr = publicado.clone();
    #[cfg(windows)]
    let estado_hw = estado.clone();
    let viu_cru = Arc::new(std::sync::atomic::AtomicBool::new(false));
    Some(std::thread::Builder::new()
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
            let mut montagem = Montagem::nova(alt, eu, log, est_previa, ctx_previa, escolha.filter(|e| e.janela).map(|e| e.id));
            #[cfg(windows)]
            let mut hw = hardware.then(|| EstadoHw::novo(fonte.clone(), modo.fps() as u32, modo.bitrate(), publicado_thr, fps_mili.clone(), log, estado_hw));
            #[cfg(windows)]
            let erro_cb = erro_hw.clone();
            let viu = viu_cru.clone();
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
                let Some(buf) = montagem.i420(q.data(), q.stride(), q.width() as u32, q.height() as u32, q.left(), q.top()) else { return };
                viu.store(true, Ordering::Relaxed);
                #[cfg(windows)]
                if let Some(hw) = hw.as_mut() {
                    match hw.quadro(&buf) {
                        Ok(true) => {
                            if sit.compare_exchange(0, 1, Ordering::Relaxed, Ordering::Relaxed).is_ok() && log {
                                eprintln!("[voz] tela: primeiro quadro H.264 {w}x{h}");
                            }
                        }
                        Ok(false) => {}
                        Err(e) => {
                            if let Ok(mut g) = erro_cb.lock() {
                                *g = Some(format!("hw:{e}"));
                            }
                            sit.store(3, Ordering::Relaxed);
                        }
                    }
                    return;
                }
                let quadro = VideoFrame { rotation: VideoRotation::VideoRotation0, timestamp_us: 0, frame_metadata: None, buffer: buf };
                fonte.capture_frame(&quadro);
                if sit.compare_exchange(0, 1, Ordering::Relaxed, Ordering::Relaxed).is_ok() && log {
                    eprintln!("[voz] tela: primeiro quadro {w}x{h}");
                }
            });
            // só diz que começou quando o primeiro quadro chega: no Wayland isso é depois de a pessoa escolher no
            // seletor do sistema (que pode levar o tempo dela); se ela cancelar, a tela nem entra na call.
            // no hardware, "chegou quadro" é o primeiro H.264 chave (3 = o codificador falhou, e a Call volta ao VP8)
            let mut pronto_tx = Some(pronto_tx);
            let prazo = std::time::Instant::now() + std::time::Duration::from_secs(if pelo_sistema { 180 } else { 20 });
            // o relógio marca a hora de cada quadro; se um atrasar, o próximo não tenta compensar em rajada.
            // o hardware pode baixar o ritmo quando o LiveKit pede (sem passar dos fps do modo)
            let mut proximo = std::time::Instant::now();
            loop {
                let mili = fps_mili.load(Ordering::Relaxed).max(1_000);
                proximo += std::time::Duration::from_secs_f64(1000.0 / mili as f64);
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
                        3 => Some(erro_hw.lock().ok().and_then(|g| g.clone()).unwrap_or_else(|| "hw:codificador".into())),
                        2 => Some(if pelo_sistema { "A tela não foi escolhida." } else { "Não deu para capturar essa tela." }.to_owned()),
                        // sem quadro nenhum, a captura não começou (não adianta tentar de novo no outro codificador).
                        // com quadro cru e sem H.264, o hardware falhou e a Call volta ao VP8
                        _ if std::time::Instant::now() > prazo => Some(if hardware && viu_cru.load(Ordering::Relaxed) { "hw:o codificador não entregou um quadro a tempo" } else { "A captura da tela não começou." }.to_owned()),
                        _ => continue,
                    };
                    let falhou = erro.is_some();
                    let _ = pronto_tx.take().unwrap().send(erro.map_or(Ok(()), Err));
                    if falhou {
                        break;
                    }
                }
            }
        })
        .map_err(|e| e.to_string())?)
    };
    match pronto_rx.await {
        Ok(Ok(())) => {}
        Ok(Err(e)) => {
            if let Some(h) = captura {
                let _ = h.join();
            }
            return Err(e);
        }
        Err(_) => {
            if let Some(h) = captura {
                let _ = h.join();
            }
            return Err("A captura da tela não começou.".into());
        }
    }
    // no hardware o teto continua o do modo (12 Mbps). O pré-codificado é uma camada só: o SDK não fabrica a
    // miniatura nem a média a partir de um H.264 já pronto, então não há simulcast nessa faixa.
    let publicar = if hardware {
        TrackPublishOptions {
            source: TrackSource::Screenshare,
            video_codec: VideoCodec::H264,
            video_encoder: livekit::webrtc::rtp_sender::VideoEncoderBackend::PreEncoded,
            simulcast: false,
            simulcast_layers: None,
            video_encoding: Some(VideoEncoding { max_bitrate: modo.bitrate(), max_framerate: modo.fps() }),
            degradation_preference: Some(modo.degradar()),
            ..Default::default()
        }
    } else {
        TrackPublishOptions {
            source: TrackSource::Screenshare,
            video_codec: match std::env::var("BRITTICO_TELA_CODEC").as_deref() {
                Ok("h264") => VideoCodec::H264,
                Ok("vp9") => VideoCodec::VP9,
                Ok("av1") => VideoCodec::AV1,
                _ => VideoCodec::VP8,
            },
            // três camadas, como o site (`voice-presets.js`): a miniatura e a média saem junto com a cheia, e o servidor
            // manda para cada pessoa a que cabe no quadro dela; na conta da porta da Contabo é o que faz caber 7 pessoas
            // com 4 telas
            simulcast: std::env::var("BRITTICO_TELA_SIMULCAST").map_or(true, |v| v != "0"),
            simulcast_layers: Some(camadas(modo, fullhd)),
            video_encoding: Some(VideoEncoding { max_bitrate: modo.bitrate(), max_framerate: modo.fps() }),
            degradation_preference: Some(modo.degradar()),
            ..Default::default()
        }
    };
    room.local_participant().publish_track(LocalTrack::Video(faixa.clone()), publicar).await.map_err(|e| format!("Não consegui mostrar a tela ({e})."))?;
    // só a partir daqui os quadros H.264 vão para a faixa: o primeiro que sai depois disso é chave
    publicado.store(true, Ordering::Release);
    if log {
        // o que o codificador fez e o que saiu pela rede, a cada 2 s, enquanto a faixa existir
        let f2 = faixa.clone();
        let est = estado.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                if !est.lock().is_ok_and(|e| e.tela) {
                    break;
                }
                match f2.get_stats().await {
                    Ok(v) => {
                        use livekit::webrtc::stats::RtcStats;
                        // por camada: tamanho, fps, o alvo que o controle de banda deu e o motivo do limite
                        let mut camadas_txt = Vec::new();
                        let (mut alvo, mut kb) = (0f64, 0u64);
                        let mut ms = 0f64;
                        let mut enc = String::new();
                        let (mut rtt, mut perda) = (0f64, 0f64);
                        let (mut estimativa, mut rtt_par) = (0f64, 0f64);
                        for st in &v {
                            match st {
                                RtcStats::OutboundRtp(o) => {
                                    camadas_txt.push(format!(
                                        "{}:{}x{}@{:.0} {:?}",
                                        if o.outbound.rid.is_empty() { "f" } else { o.outbound.rid.as_str() },
                                        o.outbound.frame_width,
                                        o.outbound.frame_height,
                                        o.outbound.frames_per_second,
                                        o.outbound.quality_limitation_reason
                                    ));
                                    alvo += o.outbound.target_bitrate;
                                    kb += o.sent.bytes_sent / 1024;
                                    ms = ms.max(o.outbound.total_encode_time * 1000.0 / o.outbound.frames_encoded.max(1) as f64);
                                    enc = o.outbound.encoder_implementation.clone();
                                }
                                RtcStats::RemoteInboundRtp(r) => {
                                    rtt = rtt.max(r.remote_inbound.round_trip_time);
                                    perda = perda.max(r.remote_inbound.fraction_lost);
                                }
                                RtcStats::CandidatePair(c) if c.candidate_pair.nominated => {
                                    estimativa = estimativa.max(c.candidate_pair.available_outgoing_bitrate);
                                    rtt_par = rtt_par.max(c.candidate_pair.current_round_trip_time);
                                }
                                _ => {}
                            }
                        }
                        camadas_txt.sort();
                        eprintln!(
                            "[voz] tela enviada: {} | alvo {:.1} Mbps, banda estimada {:.1} Mbps, ida e volta {:.0} ms (par {:.0} ms), perda {:.1}%, {} KB, {:.1} ms por quadro, {}",
                            camadas_txt.join(" "),
                            alvo / 1e6,
                            estimativa / 1e6,
                            rtt * 1000.0,
                            rtt_par * 1000.0,
                            perda * 100.0,
                            kb,
                            ms,
                            enc
                        );
                    }
                    Err(e) => eprintln!("[voz] tela: sem estatística ({e})"),
                }
            }
        });
    }
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

/// A tela de mentira do `BRITTICO_TELA_TESTE`: um fundo com "texto" e uma janela de 900x600 de textura fina que corre
/// pela tela a ~1500 px/s, em I420 direto (o custo fica todo no codificador).
fn tela_de_teste(fonte: NativeVideoSource, parar_rx: std::sync::mpsc::Receiver<()>, pronto_tx: tokio::sync::oneshot::Sender<Result<(), String>>, intervalo: std::time::Duration, alt: u32) {
    // BRITTICO_TELA_TESTE=ffmpeg:<argumentos de entrada>: os quadros vêm de um ffmpeg (um vídeo, ou uma câmera
    // passeando por uma imagem do LoL), em 1920x1080 I420
    if let Some(args) = std::env::var("BRITTICO_TELA_TESTE").ok().and_then(|v| v.strip_prefix("ffmpeg:").map(str::to_owned)) {
        std::thread::spawn(move || {
            use std::io::Read;
            let mut filho = match std::process::Command::new("ffmpeg")
                .args(["-v", "error"])
                .args(args.split_whitespace())
                .args(["-f", "rawvideo", "-pix_fmt", "yuv420p", "-s", "1920x1080", "-"])
                .stdout(std::process::Stdio::piped())
                .spawn()
            {
                Ok(f) => f,
                Err(e) => {
                    let _ = pronto_tx.send(Err(format!("ffmpeg: {e}")));
                    return;
                }
            };
            let mut saida = filho.stdout.take().unwrap();
            let _ = pronto_tx.send(Ok(()));
            let (w, h) = (1920usize, 1080usize);
            let mut cru = vec![0u8; w * h * 3 / 2];
            while saida.read_exact(&mut cru).is_ok() {
                if !matches!(parar_rx.try_recv(), Err(std::sync::mpsc::TryRecvError::Empty)) {
                    break;
                }
                let mut buf = I420Buffer::new(w as u32, h as u32);
                let (sy, su, sv) = buf.strides();
                let (dy, du, dv) = buf.data_mut();
                for y in 0..h {
                    dy[y * sy as usize..y * sy as usize + w].copy_from_slice(&cru[y * w..y * w + w]);
                }
                let (cw, ch) = (w / 2, h / 2);
                for y in 0..ch {
                    du[y * su as usize..y * su as usize + cw].copy_from_slice(&cru[w * h + y * cw..w * h + y * cw + cw]);
                    dv[y * sv as usize..y * sv as usize + cw].copy_from_slice(&cru[w * h * 5 / 4 + y * cw..w * h * 5 / 4 + y * cw + cw]);
                }
                if alt < h as u32 {
                    buf = buf.scale((alt * 16 / 9) as i32 & !1, alt as i32);
                }
                fonte.capture_frame(&VideoFrame { rotation: VideoRotation::VideoRotation0, timestamp_us: 0, frame_metadata: None, buffer: buf });
            }
            let _ = filho.kill();
        });
        let _ = intervalo;
        return;
    }
    std::thread::spawn(move || {
        let (w, h) = (1920usize, 1080usize);
        let mut semente = 0x1234_5678u32;
        let mut aleat = move || {
            semente ^= semente << 13;
            semente ^= semente >> 17;
            semente ^= semente << 5;
            semente
        };
        // fundo: linhas de "texto" (traços claros e escuros)
        let mut fundo = vec![235u8; w * h];
        for y in 0..h {
            if (y / 18) % 2 == 0 && y % 18 < 12 {
                for x in 0..w {
                    if aleat() % 3 == 0 {
                        fundo[y * w + x] = 40;
                    }
                }
            }
        }
        let (jw, jh) = (900usize, 600usize);
        // a janela: barra de título, linhas de texto e um degradê (parecida com uma janela de verdade); o fundo ganha
        // um degradê suave com um pouco de grão, como um papel de parede
        let mut janela = vec![250u8; jw * jh];
        for y in 0..jh {
            for x in 0..jw {
                let v = if y < 32 {
                    60
                } else if x > jw * 2 / 3 {
                    (80 + (x * 120 / jw) as u32 + (y * 50 / jh) as u32) as u8
                } else if (y / 16) % 2 == 0 && y % 16 < 10 && aleat() % 3 == 0 {
                    30
                } else {
                    250
                };
                janela[y * jw + x] = v;
            }
        }
        if std::env::var("BRITTICO_TELA_TESTE").is_ok_and(|v| v == "ruido") {
            // o pior caso: a janela de ruído puro, que não comprime
            for v in janela.iter_mut() {
                *v = (aleat() % 256) as u8;
            }
        }
        for y in 0..h {
            for x in 0..w {
                if fundo[y * w + x] == 235 {
                    fundo[y * w + x] = (60 + (x * 90 / w) + (y * 60 / h)) as u8 + (aleat() % 6) as u8;
                }
            }
        }
        let _ = pronto_tx.send(Ok(()));
        let mut t = 0f64;
        let mut proximo = std::time::Instant::now();
        loop {
            proximo += intervalo;
            let agora = std::time::Instant::now();
            if proximo < agora {
                proximo = agora;
            }
            match parar_rx.recv_timeout(proximo - agora) {
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                _ => break,
            }
            t += intervalo.as_secs_f64();
            let x0 = ((t * 1.7).sin() * 0.5 + 0.5) * (w - jw) as f64;
            let y0 = ((t * 2.3).cos() * 0.5 + 0.5) * (h - jh) as f64;
            let (x0, y0) = (x0 as usize, y0 as usize);
            let mut buf = I420Buffer::new(w as u32, h as u32);
            let (sy, su, sv) = buf.strides();
            let (dy, du, dv) = buf.data_mut();
            for y in 0..h {
                dy[y * sy as usize..y * sy as usize + w].copy_from_slice(&fundo[y * w..y * w + w]);
            }
            for y in 0..jh {
                let o = (y0 + y) * sy as usize + x0;
                dy[o..o + jw].copy_from_slice(&janela[y * jw..y * jw + jw]);
            }
            du.fill(128);
            dv.fill(128);
            let _ = (su, sv);
            if alt < h as u32 {
                buf = buf.scale((alt * 16 / 9) as i32 & !1, alt as i32);
            }
            fonte.capture_frame(&VideoFrame { rotation: VideoRotation::VideoRotation0, timestamp_us: 0, frame_metadata: None, buffer: buf });
        }
    });
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn print_borrado_fica_pequeno_e_na_proporcao() {
        // 1280x720 com metade esquerda branca e metade direita preta
        let (w, h) = (1280usize, 720usize);
        let mut rgba = vec![0u8; w * h * 4];
        for y in 0..h {
            for x in 0..w / 2 {
                let i = (y * w + x) * 4;
                rgba[i..i + 4].copy_from_slice(&[255, 255, 255, 255]);
            }
        }
        let (bw, bh, out) = borrar(w, h, &rgba);
        assert_eq!((bw, bh), (BORRADO_W, 54));
        assert_eq!(out.len(), bw * bh * 4);
        let px = |x: usize, y: usize| out[(y * bw + x) * 4];
        // longe da divisa as cores ficam; na divisa, o desfoque mistura
        assert_eq!(px(5, 27), 255);
        assert_eq!(px(90, 27), 0);
        let meio = px(bw / 2, 27);
        assert!(meio > 40 && meio < 215, "na divisa devia misturar, deu {meio}");
        assert!(out.chunks(4).all(|c| c[3] == 255));
    }
}

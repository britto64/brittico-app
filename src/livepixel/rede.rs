//! O WebSocket do LivePixel (`/api/livepixel/ws`, o mesmo do site): numa thread à parte, com a sessão no cabeçalho
//! `Cookie`. O que chega vira JSON numa fila que a tela esvazia a cada quadro; o que a tela manda entra noutra fila. Se a
//! conexão cai, a thread tenta de novo sozinha (e avisa, para a tela pedir de novo a parte do mapa que está vendo).

use serde_json::Value;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender, TryRecvError};
use std::sync::Arc;
use std::time::Duration;

pub enum Chegou {
    /// Conectou (de novo): a tela reenvia o `view`.
    Conectou,
    Caiu,
    Msg(Value),
}

pub struct Rede {
    tx: Sender<String>,
    pub rx: Receiver<Chegou>,
    parar: Arc<AtomicBool>,
}

impl Drop for Rede {
    fn drop(&mut self) {
        self.parar.store(true, Ordering::Relaxed);
    }
}

impl Rede {
    pub fn mandar(&self, v: Value) {
        let _ = self.tx.send(v.to_string());
    }
}

pub fn ligar(ctx: &eframe::egui::Context) -> Rede {
    let (tx, rx_out) = channel::<String>();
    let (tx_in, rx) = channel::<Chegou>();
    let parar = Arc::new(AtomicBool::new(false));
    let p2 = parar.clone();
    let ctx = ctx.clone();
    std::thread::Builder::new()
        .name("livepixel-ws".into())
        .spawn(move || {
            let mut espera = 1u64;
            while !p2.load(Ordering::Relaxed) {
                match sessao(&rx_out, &tx_in, &p2, &ctx) {
                    Ok(()) => espera = 1,
                    Err(e) => eprintln!("[livepixel] {e}"),
                }
                if p2.load(Ordering::Relaxed) {
                    break;
                }
                let _ = tx_in.send(Chegou::Caiu);
                ctx.request_repaint();
                std::thread::sleep(Duration::from_secs(espera));
                espera = (espera * 2).min(15);
                // o que a tela pediu enquanto estava fora não vale mais (ela pede de novo ao reconectar)
                while rx_out.try_recv().is_ok() {}
            }
        })
        .expect("thread do livepixel");
    Rede { tx, rx, parar }
}

fn sessao(rx_out: &Receiver<String>, tx_in: &Sender<Chegou>, parar: &AtomicBool, ctx: &eframe::egui::Context) -> Result<(), String> {
    use tungstenite::client::IntoClientRequest;
    use tungstenite::stream::MaybeTlsStream;
    use tungstenite::Message;
    let mut req = crate::net::ws_url("/api/livepixel/ws").into_client_request().map_err(|e| e.to_string())?;
    if let Some(c) = crate::net::cookie() {
        req.headers_mut().insert("Cookie", c.parse().map_err(|_| "cookie".to_string())?);
    }
    let (mut ws, _) = tungstenite::connect(req).map_err(|e| e.to_string())?;
    let t = Some(Duration::from_millis(40));
    match ws.get_mut() {
        MaybeTlsStream::Plain(s) => s.set_read_timeout(t).map_err(|e| e.to_string())?,
        MaybeTlsStream::NativeTls(s) => s.get_mut().set_read_timeout(t).map_err(|e| e.to_string())?,
        _ => {}
    }
    let _ = tx_in.send(Chegou::Conectou);
    ctx.request_repaint();
    let mut ultimo_ping = std::time::Instant::now();
    loop {
        if parar.load(Ordering::Relaxed) {
            let _ = ws.close(None);
            return Ok(());
        }
        loop {
            match rx_out.try_recv() {
                Ok(s) => ws.send(Message::text(s)).map_err(|e| e.to_string())?,
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return Ok(()),
            }
        }
        if ultimo_ping.elapsed() > Duration::from_secs(25) {
            ultimo_ping = std::time::Instant::now();
            ws.send(Message::Ping(Default::default())).map_err(|e| e.to_string())?;
        }
        match ws.read() {
            Ok(Message::Text(s)) => {
                if let Ok(v) = serde_json::from_str::<Value>(&s) {
                    let _ = tx_in.send(Chegou::Msg(v));
                    ctx.request_repaint();
                }
            }
            Ok(Message::Close(_)) => return Err("o servidor fechou".into()),
            Ok(_) => {}
            Err(tungstenite::Error::Io(e)) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => {}
            Err(e) => return Err(e.to_string()),
        }
    }
}

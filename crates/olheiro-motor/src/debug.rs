//! O pacote de depuração: quadros reais do minimapa, para eu ajustar a leitura sem ter o jogo aqui.
//!
//! Desligado por padrão e só a pedido da pessoa. Durante uma partida grava, a cada 2 s e por no máximo 4 minutos, o quadro capturado
//! (um quadrado em volta do minimapa, sem o resto da tela) e o que o Olheiro achou nele, e guarda os ícones usados. A pasta fica no
//! computador; a pessoa a compacta e manda a quem for ajudar. Nada sai sozinho. Os quadros podem incluir o nome dos aliados.

use olheiro_core::tracker::{How, Placement};
use olheiro_core::{Frame, Rect};
use serde_json::json;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

/// De quanto em quanto tempo um quadro é guardado, e quantos no máximo (4 minutos).
pub const EVERY_MS: f64 = 2000.0;
pub const MAX_FRAMES: usize = 120;

pub struct Meta {
    pub version: String,
    pub game_id: String,
    pub map: i64,
    pub window: Rect,
    pub capture: Rect,
    pub region: Rect,
    pub scale: f64,
    /// `(chave do aliado, alias do campeão)` na ordem em que o olheiro os numera.
    pub allies: Vec<(String, String)>,
}

pub struct Bundle {
    dir: PathBuf,
    n: usize,
    last: f64,
    log: fs::File,
}

fn rect(r: &Rect) -> serde_json::Value {
    json!({ "x": r.x, "y": r.y, "w": r.w, "h": r.h })
}

impl Bundle {
    /// Cria `<base>/debug/<instante>-<partida>/` com `meta.json` e os ícones. `None` se não deu para escrever (a depuração não pode derrubar nada).
    pub fn start(base: &Path, meta: &Meta, unix_secs: u64) -> Option<Bundle> {
        let dir = base.join("debug").join(format!("{unix_secs}-{}", meta.game_id));
        fs::create_dir_all(dir.join("frames")).ok()?;
        let meta_json = json!({
            "version": meta.version, "gameId": meta.game_id, "map": meta.map, "startedUnix": unix_secs, "scale": meta.scale,
            "window": rect(&meta.window), "capture": rect(&meta.capture), "region": rect(&meta.region),
            "allies": meta.allies.iter().map(|(k, a)| json!({ "key": k, "alias": a })).collect::<Vec<_>>(),
        });
        fs::write(dir.join("meta.json"), serde_json::to_string_pretty(&meta_json).ok()?).ok()?;
        // os ícones que a partida usou, para o pacote ser autossuficiente
        let _ = fs::create_dir_all(dir.join("icons"));
        for (_, alias) in &meta.allies {
            let _ = fs::copy(base.join("icons").join(format!("{alias}.png")), dir.join("icons").join(format!("{alias}.png")));
        }
        let log = fs::File::create(dir.join("frames.jsonl")).ok()?;
        Some(Bundle { dir, n: 0, last: f64::NEG_INFINITY, log })
    }

    pub fn is_full(&self) -> bool {
        self.n >= MAX_FRAMES
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn path(&self) -> &Path {
        &self.dir
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn frames(&self) -> usize {
        self.n
    }

    /// Guarda este quadro se já passou o intervalo. Silencioso em qualquer falha.
    pub fn frame(&mut self, now_ms: f64, frame: &Frame, rings: usize, places: &[Placement], keys: &[String]) {
        if self.is_full() || now_ms - self.last < EVERY_MS {
            return;
        }
        self.last = now_ms;
        let name = format!("frames/{:04}.png", self.n);
        let Some(img) = image::RgbaImage::from_raw(frame.width as u32, frame.height as u32, frame.data.clone()) else { return };
        if img.save(self.dir.join(&name)).is_err() {
            return;
        }
        let pos: Vec<_> = places
            .iter()
            .map(|p| {
                let how = match p.how {
                    How::Seen => "seen",
                    How::Under => "under",
                    How::Inferred => "inferred",
                    How::Held => "held",
                };
                json!({ "id": p.id, "key": keys.get(p.id), "x": (p.x * 10.0).round() / 10.0, "y": (p.y * 10.0).round() / 10.0, "how": how })
            })
            .collect();
        let line = json!({ "n": self.n, "file": name, "t": (now_ms / 100.0).round() / 10.0, "rings": rings, "pos": pos });
        let _ = writeln!(self.log, "{line}");
        self.n += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meta() -> Meta {
        Meta {
            version: "0.1.0".into(),
            game_id: "abc123".into(),
            map: 11,
            window: Rect::new(0, 0, 1920, 1080),
            capture: Rect::new(1488, 648, 432, 432),
            region: Rect::new(91, 91, 341, 341),
            scale: 1.92,
            allies: vec![("kennzy#br1".into(), "ahri".into()), ("mylon#br1".into(), "garen".into())],
        }
    }

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("olheiro-debug-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn grava_o_meta_os_icones_e_um_quadro_por_intervalo() {
        let base = tmp("a");
        fs::create_dir_all(base.join("icons")).unwrap();
        fs::write(base.join("icons").join("ahri.png"), b"png").unwrap();
        let mut b = Bundle::start(&base, &meta(), 1_700_000_000).unwrap();
        let f = Frame::new(32, 32);
        let p = [Placement { id: 0, x: 10.04, y: 20.0, how: How::Seen }];
        let keys = vec!["kennzy#br1".to_string(), "mylon#br1".to_string()];
        b.frame(0.0, &f, 5, &p, &keys);
        b.frame(500.0, &f, 5, &p, &keys); // cedo demais
        b.frame(2100.0, &f, 4, &p, &keys);
        assert_eq!(b.frames(), 2);
        let meta: serde_json::Value = serde_json::from_str(&fs::read_to_string(b.path().join("meta.json")).unwrap()).unwrap();
        assert_eq!(meta["allies"][1]["alias"], "garen");
        assert_eq!(meta["region"]["w"], 341);
        assert!(b.path().join("icons/ahri.png").exists(), "o ícone vem junto");
        assert!(b.path().join("frames/0001.png").exists());
        let log = fs::read_to_string(b.path().join("frames.jsonl")).unwrap();
        let first: serde_json::Value = serde_json::from_str(log.lines().next().unwrap()).unwrap();
        assert_eq!((first["rings"].as_u64(), first["pos"][0]["key"].as_str(), first["pos"][0]["how"].as_str()), (Some(5), Some("kennzy#br1"), Some("seen")));
        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn para_no_limite_de_quadros() {
        let base = tmp("b");
        let mut b = Bundle::start(&base, &meta(), 1).unwrap();
        let f = Frame::new(8, 8);
        for i in 0..(MAX_FRAMES + 20) {
            b.frame(i as f64 * 2500.0, &f, 0, &[], &[]);
        }
        assert!(b.is_full() && b.frames() == MAX_FRAMES);
        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn pasta_impossivel_nao_derruba_nada() {
        // um arquivo no lugar da pasta base: não dá para criar `debug/`
        let base = tmp("c");
        fs::write(&base, b"arquivo").unwrap();
        assert!(Bundle::start(&base, &meta(), 1).is_none());
        let _ = fs::remove_file(base);
    }
}

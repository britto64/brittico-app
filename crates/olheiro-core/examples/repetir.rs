//! Repete um pacote de depuração (a pasta com `meta.json`, `frames/` e `icons/`) em cada sensibilidade e conta como cada aliado
//! foi achado. Uso: `cargo run --release -p olheiro-core --example repetir -- <pasta do pacote>`.

use olheiro_core::templates::{icon_templates, TemplateSet};
use olheiro_core::{Frame, How, Rect, Scout, ScoutPlayer};
use std::path::Path;

fn rect(v: &serde_json::Value) -> Rect {
    Rect::new(v["x"].as_i64().unwrap_or(0) as i32, v["y"].as_i64().unwrap_or(0) as i32, v["w"].as_i64().unwrap_or(0) as i32, v["h"].as_i64().unwrap_or(0) as i32)
}

fn main() {
    let dir = std::env::args().nth(1).expect("pasta do pacote");
    let dir = Path::new(&dir);
    let meta: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(dir.join("meta.json")).unwrap()).unwrap();
    let region = rect(&meta["region"]);
    let allies: Vec<(String, String)> =
        meta["allies"].as_array().unwrap().iter().map(|a| (a["key"].as_str().unwrap().to_string(), a["alias"].as_str().unwrap().to_string())).collect();
    let players: Vec<ScoutPlayer> = allies
        .iter()
        .enumerate()
        .filter_map(|(id, (_, alias))| {
            let img = image::open(dir.join("icons").join(format!("{alias}.png"))).ok()?.to_rgba8();
            let vecs = icon_templates(img.as_raw(), img.width() as usize, img.height() as usize);
            Some(ScoutPlayer { id, templates: TemplateSet::new(id, vecs) })
        })
        .collect();
    let mut files: Vec<_> = std::fs::read_dir(dir.join("frames")).unwrap().filter_map(|e| e.ok()).map(|e| e.path()).collect();
    files.sort();
    let frames: Vec<Frame> = files
        .iter()
        .map(|f| {
            let img = image::open(f).unwrap().to_rgba8();
            let mut fr = Frame::new(img.width() as usize, img.height() as usize);
            fr.data.copy_from_slice(img.as_raw());
            fr
        })
        .collect();
    println!("{} quadros, {} aliados com ícone", frames.len(), players.len());
    for s in [0.0f32, 0.25, 0.5, 0.75, 1.0] {
        let mut scout = Scout::new();
        scout.set_sensitivity(s);
        scout.set_players(players.clone());
        let mut c = [[0usize; 4]; 16];
        let mut rings = 0usize;
        for (n, fr) in frames.iter().enumerate() {
            let now = n as f64 * 2000.0;
            scout.on_frame(fr, &region, now);
            rings += scout.rings_seen();
            for p in scout.placements(now) {
                let k = match p.how {
                    How::Seen => 0,
                    How::Under => 1,
                    How::Inferred => 2,
                    How::Held => 3,
                };
                c[p.id][k] += 1;
            }
        }
        print!("sens {s:.2} · anéis/quadro {:.1} · ", rings as f64 / frames.len() as f64);
        for (id, (key, _)) in allies.iter().enumerate() {
            print!("{}: visto {} debaixo {} elim {} retido {} | ", key.split('#').next().unwrap(), c[id][0], c[id][1], c[id][2], c[id][3]);
        }
        println!();
    }
}

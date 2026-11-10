//! Conta os anéis de aliado que o Olheiro acha em quadros de um VOD (uma pasta de PNGs já cortados como a captura do app), para cada
//! tamanho de minimapa e sensibilidade. Uso: `aneis <pasta> <x> <y> <lado>...` (o lado do minimapa dentro do corte; vários lados são testados).

use olheiro_core::{Frame, Rect, Scout};

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let dir = &a[0];
    let (x, y): (i32, i32) = (a[1].parse().unwrap(), a[2].parse().unwrap());
    let lados: Vec<i32> = a[3..].iter().map(|s| s.parse().unwrap()).collect();
    let mut files: Vec<_> = std::fs::read_dir(dir).unwrap().filter_map(|e| e.ok()).map(|e| e.path()).collect();
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
    if std::env::var("GRADE").is_ok() {
        // procura o melhor encaixe (x, y, lado) pelo número de anéis achados com a sensibilidade máxima
        let mut best: Vec<(f64, i32, i32, i32)> = Vec::new();
        let few: Vec<&Frame> = frames.iter().step_by(3).collect();
        for lado in (240..=300).step_by(6) {
            for yy in (140..=190).step_by(5) {
                for xx in (140..=190).step_by(5) {
                    let region = Rect::new(xx, yy, lado, lado);
                    if !olheiro_core::geometry::region_fits(&region, &Rect::new(0, 0, 432, 432)) {
                        continue;
                    }
                    let mut scout = Scout::new();
                    scout.set_sensitivity(1.0);
                    let mut tot = 0;
                    for (n, fr) in few.iter().enumerate() {
                        scout.on_frame(fr, &region, n as f64 * 4000.0);
                        tot += scout.rings_seen();
                    }
                    best.push((tot as f64 / few.len() as f64, xx, yy, lado));
                }
            }
        }
        best.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
        for b in best.iter().take(6) {
            println!("anéis/quadro {:.2} em x {} y {} lado {}", b.0, b.1, b.2, b.3);
        }
        return;
    }
    for lado in lados {
        let region = Rect::new(x, y, lado, lado);
        for s in [0.0f32, 0.5, 1.0] {
            let mut scout = Scout::new();
            scout.set_sensitivity(s);
            let mut hist = [0usize; 9];
            for (n, fr) in frames.iter().enumerate() {
                scout.on_frame(fr, &region, n as f64 * 4000.0);
                hist[scout.rings_seen().min(8)] += 1;
            }
            let media = hist.iter().enumerate().map(|(k, c)| k * c).sum::<usize>() as f64 / frames.len() as f64;
            println!("lado {lado} sens {s:.1}: anéis/quadro {media:.2}  histograma 0..8 = {hist:?}");
        }
    }
}

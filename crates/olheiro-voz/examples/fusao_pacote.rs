//! Passa as posições de um pacote de depuração (`frames.jsonl`, em pixels do minimapa) pela `Fusao`, a 5 quadros por segundo
//! (interpolando entre os quadros de 2 s), e mostra o que a fusão guardou para dois jogadores.
//! Uso: `fusao_pacote <pasta do pacote> <chave de quem ouve> <chave do outro>`

use olheiro_voz::{distancia, Fusao, Ponto};

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let dir = &a[0];
    let meta: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(format!("{dir}/meta.json")).unwrap()).unwrap();
    let w = meta["region"]["w"].as_f64().unwrap();
    let (mw, mh) = (14870.0, 14980.0);
    let to_game = |x: f64, y: f64| (x / w * mw, mh - y / w * mh);
    let rows: Vec<serde_json::Value> = std::fs::read_to_string(format!("{dir}/frames.jsonl")).unwrap().lines().map(|l| serde_json::from_str(l).unwrap()).collect();
    let pos_de = |r: &serde_json::Value, k: &str| -> Option<(f64, f64, String)> {
        r["pos"].as_array()?.iter().find(|p| p["key"].as_str().is_some_and(|s| s.starts_with(k))).map(|p| (p["x"].as_f64().unwrap(), p["y"].as_f64().unwrap(), p["how"].as_str().unwrap().to_string()))
    };
    let mut f = Fusao::nova();
    let (eu, outro) = (a[1].clone(), a[2].clone());
    let mut mudancas = 0;
    let mut sem_posicao = 0;
    for n in 0..rows.len() - 1 {
        for sub in 0..10 {
            let t_ms = ((n * 10 + sub) * 200) as u64 + 1_000;
            for k in [&eu, &outro] {
                // o quadro de 2 s vale até o próximo; a posição só muda quando o próximo traz outra (degrau: o pior caso para o filtro)
                let r = if sub >= 9 { &rows[n + 1] } else { &rows[n] };
                if let Some((x, y, how)) = pos_de(r, k) {
                    let (gx, gy) = to_game(x, y);
                    let c = match how.as_str() { "seen" => 0.9, "held" => 0.3, _ => 0.6 };
                    f.observar(t_ms, Ponto { k: format!("{k}#x"), x: gx, y: gy, c, s: how }, t_ms);
                }
            }
            let (p1, p2) = (f.posicao(t_ms, &format!("{eu}#x")), f.posicao(t_ms, &format!("{outro}#x")));
            match (p1, p2) {
                (Some(a1), Some(b1)) => {
                    let d = distancia(a1.0, a1.1, b1.0, b1.1);
                    if sub == 0 && (n % 1 == 0) && (n >= 100 && n <= 110) {
                        println!("quadro {n}.{sub}: distância na fusão {:.0} u", d);
                    }
                }
                _ => sem_posicao += 1,
            }
            mudancas += 1;
        }
    }
    println!("{mudancas} instantes; sem posição de algum dos dois: {sem_posicao}");
}

//! A casa dos jogos (`public/jogos/`: `jogos.js` desenha, `jogos.css` mede).
//!
//! A faixa de cima (`.jg-palco`) mostra o jogo escolhido: um retalho da capa, o nome, a pílula do estado ao vivo e o
//! botão. Embaixo, a parede de capas (`.jg-grade`); passar o mouse numa capa já troca quem está na faixa, como no site.
//! Os jogos "Quadros da live" ficam numa prateleira própria, com o rótulo em cima.
//!
//! As capas são as do site, extraídas em camadas por `tools/extrair-jogos.cjs` (o que fica parado e cada grupo que
//! flutua); aqui cada camada flutua como a animação de CSS dela. O estado ao vivo vem de `assets/mock/jogos.json`.

use crate::theme::*;
use crate::ui::{self, mix, text, text_width};
use crate::Section;
use eframe::egui::{self, text::LayoutJob, Align2, Color32, ColorImage, CornerRadius, CursorIcon, ImageSource, Pos2, Rect, Sense, Stroke, StrokeKind, TextFormat, TextureHandle, TextureOptions, UiBuilder, Vec2};
use std::collections::HashMap;

/// `.jg .main__inner`: até 1240 px, com 32 px em cima e 24 nos lados.
const INNER_MAX: f32 = 1240.0;
const PAD_X: f32 = 24.0;
const PAD_TOP: f32 = 32.0;
const PALCO_H: f32 = 74.0;
/// `.jg-grade`: `repeat(auto-fill, minmax(212px, 1fr))`, 14 px entre as capas.
const CARD_MIN: f32 = 212.0;
const CARD_GAP: f32 = 14.0;
/// O verde de "acontecendo agora" no tema escuro (`--jg-vivo`).
/// `--jg-vivo`: o verde de "acontecendo agora", mais claro no escuro.
#[allow(non_snake_case)]
fn VIVO() -> Color32 {
    if DARK() {
        Color32::from_rgb(0x6e, 0xc7, 0x4f)
    } else {
        Color32::from_rgb(0x3f, 0x8f, 0x28)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tom {
    Vivo,
    Espera,
    Pronto,
    Parado,
}

/// Uma camada da capa e como ela flutua: o deslocamento máximo (em unidades da capa, 300×200), o tempo de ida e o atraso.
struct Camada {
    src: ImageSource<'static>,
    /// A mesma camada com as cores do tema claro.
    claro: ImageSource<'static>,
    vai: Vec2,
    periodo: f64,
    atraso: f64,
}

struct Jogo {
    id: &'static str,
    nome: &'static str,
    resumo: &'static str,
    /// A cor própria do jogo (`hue` do catálogo): pinta o fundo da capa.
    hue: f32,
    cta: &'static str,
    /// Qual sonda preenche a pílula (as do `SONDAS` do jogos.js); sem sonda, sem pílula.
    sonda: Option<&'static str>,
    /// Mora na prateleira "Quadros da live".
    quadros: bool,
    camadas: Vec<Camada>,
}

/// A camada no tema escuro e no claro (`<nome>.svg` e `<nome>-claro.svg`): as cores das capas vêm do CSS do site, que
/// muda com o tema.
macro_rules! capa {
    ($n:literal) => {
        (
            ImageSource::Bytes {
                uri: std::borrow::Cow::Borrowed(concat!("bytes://jogos/", $n, ".svg")),
                bytes: egui::load::Bytes::Static(include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/jogos/", $n, ".svg"))),
            },
            ImageSource::Bytes {
                uri: std::borrow::Cow::Borrowed(concat!("bytes://jogos/", $n, "-claro.svg")),
                bytes: egui::load::Bytes::Static(include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/jogos/", $n, "-claro.svg"))),
            },
        )
    };
}
type Par = (ImageSource<'static>, ImageSource<'static>);

/// As animações do jogos.css: `jg-flutua` vai a (9, −11), `jg-flutua2` a (−11, 7), ida e volta.
fn fl((src, claro): Par) -> Camada {
    Camada { src, claro, vai: Vec2::new(9.0, -11.0), periodo: 9.0, atraso: 0.0 }
}
fn fl2((src, claro): Par) -> Camada {
    Camada { src, claro, vai: Vec2::new(-11.0, 7.0), periodo: 11.0, atraso: 0.0 }
}
fn fl3((src, claro): Par) -> Camada {
    Camada { src, claro, vai: Vec2::new(9.0, -11.0), periodo: 13.0, atraso: -4.0 }
}
fn base((src, claro): Par) -> Camada {
    Camada { src, claro, vai: Vec2::ZERO, periodo: 1.0, atraso: 0.0 }
}

/// O `JOGOS` do jogos.js, na mesma ordem. O nome, o resumo e o botão são conteúdo do site, copiados daqui de lá.
fn catalogo() -> Vec<Jogo> {
    vec![
        Jogo {
            id: "bolotas",
            nome: "Bolotas",
            resumo: "A arena sempre aberta: cada pessoa dirige a própria bola, com o mouse e o WASD.",
            hue: 32.0,
            cta: "Abrir a arena",
            sonda: Some("bolotas"),
            quadros: false,
            camadas: vec![base(capa!("bolotas-base")), fl(capa!("bolotas-fl")), fl2(capa!("bolotas-fl2")), fl3(capa!("bolotas-fl3"))],
        },
        Jogo {
            id: "cocowork",
            nome: "Co-co-work",
            resumo: "Um escritório na fazenda. Senta numa mesa e fala com quem está perto.",
            hue: 96.0,
            cta: "Bater o ponto",
            sonda: None,
            quadros: false,
            camadas: vec![base(capa!("cocowork-base")), fl(capa!("cocowork-fl")), fl2(capa!("cocowork-fl2")), fl3(capa!("cocowork-fl3"))],
        },
        Jogo {
            id: "quemdisse",
            nome: "Quem disse isso?",
            resumo: "Uma mensagem do chat, três suspeitos. Adivinha quem falou.",
            hue: 8.0,
            cta: "Abrir o caso",
            sonda: Some("quemdisse"),
            quadros: false,
            camadas: vec![base(capa!("quemdisse-base")), fl(capa!("quemdisse-fl"))],
        },
        Jogo {
            id: "carona",
            nome: "Carona com Marcos",
            resumo: "Pega carona com o Marcos. Ou dirige e leva o chat.",
            hue: 160.0,
            cta: "Pedir carona",
            sonda: None,
            quadros: true,
            camadas: vec![base(capa!("carona-base")), fl(capa!("carona-fl")), fl2(capa!("carona-fl2"))],
        },
        Jogo {
            id: "livepixel",
            nome: "LivePixel",
            resumo: "Canvas infinito de pixels. Representa um streamer e conquiste território.",
            hue: 210.0,
            cta: "Abrir o canvas",
            sonda: Some("livepixel"),
            quadros: false,
            camadas: vec![base(capa!("livepixel-base")), fl(capa!("livepixel-fl")), fl2(capa!("livepixel-fl2"))],
        },
    ]
}

/// O que cada sonda diria com as respostas de `assets/mock/jogos.json` (o `ler` de cada uma no jogos.js).
fn sondas() -> HashMap<&'static str, (Tom, String)> {
    let v: serde_json::Value = serde_json::from_str(include_str!("../assets/mock/jogos.json")).expect("assets/mock/jogos.json");
    let mut out = HashMap::new();
    let b = &v["bolotas"];
    out.insert(
        "bolotas",
        if b["aberto"].as_bool() == Some(true) {
            let n = b["jogadores"].as_u64().unwrap_or(0) + b["bots"].as_u64().unwrap_or(0);
            (Tom::Vivo, if n == 1 { "1 na arena".into() } else { format!("{n} na arena") })
        } else {
            (Tom::Parado, "arena fechada".into())
        },
    );
    let q = &v["quemdisse"];
    out.insert(
        "quemdisse",
        if q["lendo"].as_bool() == Some(true) {
            (Tom::Espera, "abrindo o arquivo".into())
        } else if q["pronto"].as_bool() != Some(true) {
            (Tom::Parado, "arquivo vazio".into())
        } else {
            (Tom::Pronto, format!("{} suspeitos", q["suspeitos"].as_u64().unwrap_or(0)))
        },
    );
    out.insert(
        "livepixel",
        match v["livepixel"]["entries"].as_array() {
            None => (Tom::Parado, "offline".into()),
            Some(e) if e.is_empty() => (Tom::Vivo, "canvas vazio".into()),
            Some(e) => (Tom::Vivo, format!("{} streamer{}", e.len(), if e.len() == 1 { "" } else { "s" })),
        },
    );
    out
}

pub struct JogosState {
    jogos: Vec<Jogo>,
    estado: HashMap<&'static str, (Tom, String)>,
    /// Quem está na faixa: abre no primeiro que tem porta, como no site.
    sel: usize,
    /// Os fundos das capas já desenhados, por jogo, lugar e tamanho em pixels.
    fundos: HashMap<(usize, bool, u32, u32, [u8; 4]), TextureHandle>,
    pub toast: ui::Toast,
}

impl Default for JogosState {
    fn default() -> Self {
        Self { jogos: catalogo(), estado: sondas(), sel: 0, fundos: HashMap::new(), toast: ui::Toast::default() }
    }
}

/* ================= a capa ================= */

/// `ease-in-out` com `alternate`: vai e volta, devagar nas pontas.
fn flutua(c: &Camada, t: f64) -> Vec2 {
    if c.vai == Vec2::ZERO {
        return Vec2::ZERO;
    }
    let x = ((t - c.atraso) / c.periodo).rem_euclid(2.0);
    let ida = if x < 1.0 { x } else { 2.0 - x };
    let k = 0.5 - 0.5 * (std::f64::consts::PI * ida).cos();
    c.vai * k as f32
}

/// O fundo da capa: `--cream-2` com uma mancha da cor do jogo vindo de cima à esquerda (o `radial-gradient` do CSS),
/// e os cantos arredondados. Desenhado pixel a pixel uma vez por tamanho e guardado.
///
/// Na parede é `radial-gradient(120% 130% at 20% 10%, …, transparent 64%)`; na faixa, `130% 180% at 18% 12%`, até 66%.
fn fundo(ui: &egui::Ui, st: &mut JogosState, j: usize, palco: bool, rect: Rect, cantos: CornerRadius) -> TextureHandle {
    let ppp = ui.ctx().pixels_per_point();
    let (w, h) = ((rect.width() * ppp).round().max(1.0) as u32, (rect.height() * ppp).round().max(1.0) as u32);
    // a cor de fundo entra na chave: trocar de tema ou de paleta pede outro desenho
    let key = (j, palco, w, h, CREAM_2().to_array());
    if let Some(t) = st.fundos.get(&key) {
        return t.clone();
    }
    let (cx, cy, rx, ry, fim) = if palco { (0.18, 0.12, 1.3, 1.8, 0.66) } else { (0.20, 0.10, 1.2, 1.3, 0.64) };
    // `--jg-l` e `--jg-wash` mudam com o tema: a arte fica sobre quase-preto num e sobre claro no outro
    let (l, wash) = if DARK() { (0.56, 0.24) } else { (0.58, 0.16) };
    let tinta = ui::hsl(st.jogos[j].hue, 0.70, l);
    let r = |c: u8| c as f32 * ppp;
    let (nw, ne, sw, se) = (r(cantos.nw), r(cantos.ne), r(cantos.sw), r(cantos.se));
    let mut img = ColorImage::new([w as usize, h as usize], Color32::TRANSPARENT);
    for py in 0..h {
        for px in 0..w {
            let (x, y) = (px as f32 + 0.5, py as f32 + 0.5);
            let d = (((x / w as f32 - cx) / rx).powi(2) + ((y / h as f32 - cy) / ry).powi(2)).sqrt();
            let a = wash * (1.0 - d / fim).max(0.0);
            let c = mix(tinta, CREAM_2(), a);
            // quanto do pixel fica dentro do canto arredondado
            let canto = |rad: f32, ox: f32, oy: f32| -> f32 {
                if rad <= 0.0 {
                    return 1.0;
                }
                let dist = ((x - ox).powi(2) + (y - oy).powi(2)).sqrt();
                (rad - dist + 0.5).clamp(0.0, 1.0)
            };
            let (wf, hf) = (w as f32, h as f32);
            let cobre = if x < nw && y < nw {
                canto(nw, nw, nw)
            } else if x > wf - ne && y < ne {
                canto(ne, wf - ne, ne)
            } else if x < sw && y > hf - sw {
                canto(sw, sw, hf - sw)
            } else if x > wf - se && y > hf - se {
                canto(se, wf - se, hf - se)
            } else {
                1.0
            };
            img.pixels[(py * w + px) as usize] = Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), (cobre * 255.0).round() as u8);
        }
    }
    let tex = ui.ctx().load_texture(format!("fundo-jogo-{j}-{palco}-{w}x{h}"), img, TextureOptions::LINEAR);
    st.fundos.insert(key, tex.clone());
    tex
}

/// A capa inteira dentro de `rect`: o fundo e as camadas, que preenchem a caixa como o `slice` do SVG (a capa é 3:2;
/// numa caixa de outro formato ela sai cortada, não espremida).
fn arte(ui: &mut egui::Ui, st: &mut JogosState, j: usize, palco: bool, rect: Rect, cantos: CornerRadius, t: f64) {
    let tex = fundo(ui, st, j, palco, rect, cantos);
    ui.painter().image(tex.id(), rect, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
    let s = (rect.width() / 300.0).max(rect.height() / 200.0);
    let origem = rect.center() - Vec2::new(150.0, 100.0) * s;
    let mut dentro = ui.new_child(UiBuilder::new().max_rect(rect));
    dentro.set_clip_rect(rect.intersect(ui.clip_rect()));
    let jogo = &st.jogos[j];
    for c in &jogo.camadas {
        let at = Rect::from_min_size(origem + flutua(c, t) * s, Vec2::new(300.0, 200.0) * s);
        ui::image(&dentro, if DARK() { c.src.clone() } else { c.claro.clone() }, at, 0.0);
    }
    // o "?" do balão do Quem disse isso?: um <text> no site, com a fonte do site; aqui, a Patua (ver extrair-jogos.cjs)
    if jogo.id == "quemdisse" {
        let d = flutua(&jogo.camadas[1], t);
        let base = origem + (Vec2::new(205.0, 106.0) + d) * s;
        ui::bold(dentro.painter(), base - Vec2::new(0.0, 54.0 * 0.36 * s), Align2::CENTER_CENTER, "?", patua(54.0 * s), CREAM_0(), 0.0);
    }
}

/// A pílula do estado: na faixa (`.jg-pill`) ou na capa (`.jg-card__badge`). O ponto pisca no que está acontecendo.
fn pilula(ui: &egui::Ui, st: &JogosState, sonda: &str, at: Pos2, na_capa: bool, t: f64) -> Rect {
    let Some((tom, txt)) = st.estado.get(sonda) else { return Rect::NOTHING };
    let p = ui.painter();
    let txt = txt.to_uppercase();
    let (font, pad, h, dot) = if na_capa { (f8(10.0), 9.0, 24.0, 6.0) } else { (f8(11.0), 11.0, 25.0, 7.0) };
    let sp = font.size * 0.05;
    let w = pad + dot + if na_capa { 6.0 } else { 7.0 } + text_width(p, &txt, font.clone(), sp) + pad;
    let r = Rect::from_min_size(at, Vec2::new(w, h));
    let (bg, fg, ponto) = match (tom, na_capa) {
        (Tom::Vivo, false) => (mix(VIVO(), CREAM_0(), if DARK() { 0.16 } else { 0.14 }), VIVO(), VIVO()),
        // no claro a letra da pílula âmbar é a `--on-secondary`; o ponto continua âmbar
        (Tom::Espera | Tom::Pronto, false) => (AMBER_SOFT_SOBRE(), if DARK() { SECONDARY() } else { ON_SECONDARY() }, SECONDARY()),
        (Tom::Parado, false) => (CREAM_3(), FAINT(), FAINT()),
        (Tom::Vivo, true) => (VIVO(), Color32::WHITE, Color32::WHITE),
        (Tom::Espera | Tom::Pronto, true) => (SECONDARY(), ON_SECONDARY(), ON_SECONDARY()),
        (Tom::Parado, true) => (CREAM_0(), FAINT(), FAINT()),
    };
    p.rect_filled(r, h / 2.0, bg);
    // `@keyframes pulse`: de 1 a 0,35 e de volta, em 1,5 s
    let pisca = matches!(tom, Tom::Vivo | Tom::Espera);
    let a = if pisca { 0.675 + 0.325 * (t * std::f64::consts::TAU / 1.5).cos() as f32 } else { 1.0 };
    p.circle_filled(Pos2::new(r.left() + pad + dot / 2.0, r.center().y), dot / 2.0, mix(ponto, bg, a));
    text(p, Pos2::new(r.left() + pad + dot + if na_capa { 6.0 } else { 7.0 }, r.center().y + 0.5), Align2::LEFT_CENTER, &txt, font, fg, sp);
    r
}

/* ================= a página ================= */

/// Desenha a página. Devolve para onde ir quando alguém abre um jogo que o app já tem (hoje, o LivePixel).
pub fn show(ui: &mut egui::Ui, st: &mut JogosState, body: Rect, t: f64) -> Option<Section> {
    let mut ir = None;
    let mut abrir: Option<usize> = None;
    let mut child = ui.new_child(UiBuilder::new().max_rect(body));
    egui::ScrollArea::vertical().id_salt("jogos").auto_shrink(false).show(&mut child, |ui| {
        let inner = INNER_MAX.min(body.width());
        let x0 = body.center().x - inner / 2.0 + PAD_X;
        let cw = inner - 2.0 * PAD_X;
        let cols = (((cw + CARD_GAP) / (CARD_MIN + CARD_GAP)).floor() as usize).max(1);
        let card_w = (cw - CARD_GAP * (cols - 1) as f32) / cols as f32;
        let capa_h = (card_w - 2.0) * 2.0 / 3.0;
        let card_h = 1.0 + capa_h + 1.0 + 10.0 + 18.0 + 3.0 + 32.4 + 13.0;

        let soltos: Vec<usize> = (0..st.jogos.len()).filter(|&i| !st.jogos[i].quadros).collect();
        let quadros: Vec<usize> = (0..st.jogos.len()).filter(|&i| st.jogos[i].quadros).collect();
        let linhas = |n: usize| (n + cols - 1) / cols;
        let mut altura = PAD_TOP + PALCO_H + 24.0 + linhas(soltos.len()) as f32 * (card_h + CARD_GAP) - CARD_GAP;
        if !quadros.is_empty() {
            altura += CARD_GAP + 20.0 + 16.0 + CARD_GAP + linhas(quadros.len()) as f32 * (card_h + CARD_GAP) - CARD_GAP;
        }
        altura += 90.0;
        let (area, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), altura), Sense::hover());
        let top = area.top();

        if let Some(j) = palco(ui, st, Rect::from_min_size(Pos2::new(x0, top + PAD_TOP), Vec2::new(cw, PALCO_H)), t) {
            abrir = Some(j);
        }

        let mut y = top + PAD_TOP + PALCO_H + 24.0;
        let mut parede = |ui: &mut egui::Ui, st: &mut JogosState, lista: &[usize], y: &mut f32| {
            for (k, &j) in lista.iter().enumerate() {
                let (c, l) = (k % cols, k / cols);
                let r = Rect::from_min_size(Pos2::new(x0 + c as f32 * (card_w + CARD_GAP), *y + l as f32 * (card_h + CARD_GAP)), Vec2::new(card_w, card_h));
                if card(ui, st, j, r, capa_h, t) {
                    abrir = Some(j);
                }
            }
            *y += linhas(lista.len()) as f32 * (card_h + CARD_GAP) - CARD_GAP;
        };
        parede(ui, st, &soltos, &mut y);
        if !quadros.is_empty() {
            // o rótulo da prateleira: `.jg-categoria`
            y += CARD_GAP + 20.0;
            ui::bold(ui.painter(), Pos2::new(x0, y + 8.0), Align2::LEFT_CENTER, "QUADROS DA LIVE", patua(13.0), FAINT(), 1.56);
            y += 16.0 + CARD_GAP;
            parede(ui, st, &quadros, &mut y);
        }
    });
    if let Some(j) = abrir {
        match st.jogos[j].id {
            "livepixel" => ir = Some(Section::LivePixel),
            // os outros jogos são páginas do site (canvas e WebSocket do navegador): abrem nele
            id => {
                ui.ctx().open_url(egui::OpenUrl::new_tab(format!("https://brittico.xyz/jogos/{id}/")));
                let msg = format!("{} abriu no navegador.", st.jogos[j].nome);
                st.toast.say(&msg, t);
            }
        }
    }
    st.toast.show(ui, t);
    ir
}

/// A faixa do jogo escolhido. Devolve o jogo se o botão foi clicado.
fn palco(ui: &mut egui::Ui, st: &mut JogosState, r: Rect, t: f64) -> Option<usize> {
    let j = st.sel;
    let p = ui.painter().clone();
    p.rect_filled(r, R_MD(), CREAM_0());
    // o retalho da capa: clamp(104px, 13vw, 168px)
    let vw = ui.ctx().screen_rect().width();
    let aw = (vw * 0.13).clamp(104.0, 168.0);
    let a = Rect::from_min_size(r.min + Vec2::splat(1.0), Vec2::new(aw, r.height() - 2.0));
    arte(ui, st, j, true, a, CornerRadius { nw: 13, sw: 13, ne: 0, se: 0 }, t);

    let jogo = &st.jogos[j];
    let cy = r.center().y;
    let size = (vw * 0.021).clamp(19.0, 25.0);
    let nome = ui::bold(&p, Pos2::new(a.right() + 16.0, cy), Align2::LEFT_CENTER, jogo.nome, patua(size), INK(), -0.4);
    if let Some(s) = jogo.sonda {
        pilula(ui, st, s, Pos2::new(nome.right() + 12.0, cy - 12.5), false, t);
    }

    // o botão, encostado à direita
    let jogo = &st.jogos[j];
    let bw = 15.0 + 15.0 + 8.0 + text_width(&p, jogo.cta, patua(13.5), 0.0) + 0.3 + 15.0;
    let b = Rect::from_min_size(Pos2::new(r.right() - 15.0 - bw, cy - 17.0), Vec2::new(bw, 34.0));
    let resp = ui.interact(b, ui.id().with("jogar"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
    // `:hover`: um pouco mais claro e um pixel para cima
    let (b, bg) = if resp.hovered() { (b.translate(Vec2::new(0.0, -1.0)), mix(Color32::WHITE, SECONDARY(), 0.06)) } else { (b, SECONDARY()) };
    p.rect_filled(b, R_SM(), bg);
    // o ▶ do IC_PLAY: um triângulo cheio de (7,4) a (19,12) a (7,20) num quadrado de 24
    let k = 15.0 / 24.0;
    let o = Pos2::new(b.left() + 15.0, b.center().y - 7.5);
    p.add(egui::Shape::convex_polygon(vec![o + Vec2::new(7.0, 4.0) * k, o + Vec2::new(19.0, 12.0) * k, o + Vec2::new(7.0, 20.0) * k], ON_SECONDARY(), Stroke::NONE));
    ui::bold(&p, Pos2::new(b.left() + 15.0 + 15.0 + 8.0, b.center().y + 0.5), Align2::LEFT_CENTER, jogo.cta, patua(13.5), ON_SECONDARY(), 0.0);
    resp.clicked().then_some(j)
}

/// Uma capa da parede (`.jg-card`). Passar o mouse põe o jogo na faixa; clicar abre. Devolve se foi clicada.
fn card(ui: &mut egui::Ui, st: &mut JogosState, j: usize, r: Rect, capa_h: f32, t: f64) -> bool {
    let id = ui.id().with(("jogo", j));
    let resp = ui.interact(r, id, Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
    if resp.hovered() {
        st.sel = j;
    }
    // `:hover`: sobe 3 px em 0,16 s
    let sobe = ui.ctx().animate_bool_with_time(id, resp.hovered(), 0.16);
    let r = r.translate(Vec2::new(0.0, -3.0 * sobe));
    let p = ui.painter().clone();
    p.rect_filled(r, R_MD(), CREAM_0());

    let capa = Rect::from_min_size(r.min + Vec2::splat(1.0), Vec2::new(r.width() - 2.0, capa_h));
    arte(ui, st, j, false, capa, CornerRadius { nw: 13, ne: 13, sw: 0, se: 0 }, t);
    let jogo = &st.jogos[j];
    if let Some(s) = jogo.sonda {
        pilula(ui, st, s, capa.min + Vec2::splat(8.0), true, t);
    }

    // o nome e o resumo, cortado em duas linhas
    let jogo = &st.jogos[j];
    let x = r.left() + 13.0;
    let w = r.width() - 26.0;
    let ny = capa.bottom() + 1.0 + 10.0;
    ui::bold(&p, Pos2::new(x, ny + 9.0), Align2::LEFT_CENTER, jogo.nome, patua(15.0), INK(), 0.0);
    let mut job = LayoutJob::single_section(
        jogo.resumo.to_owned(),
        TextFormat { font_id: f6(12.0), color: FAINT(), line_height: Some(16.2), valign: egui::Align::Center, ..Default::default() },
    );
    job.wrap = egui::text::TextWrapping { max_width: w, max_rows: 2, break_anywhere: false, overflow_character: Some('…') };
    let g = p.layout_job(job);
    p.galley(Pos2::new(x, ny + 18.0 + 3.0), g, FAINT());

    // `.is-sel`: a borda âmbar no que está na faixa
    if st.sel == j {
        p.rect_stroke(r, R_MD(), Stroke::new(1.0_f32, SECONDARY()), StrokeKind::Inside);
    }
    resp.clicked()
}

/// `--amber-soft` pintado sobre `--cream-0` (ele é transparente no Preto).
#[allow(non_snake_case)]
fn AMBER_SOFT_SOBRE() -> Color32 {
    let a = AMBER_SOFT();
    if a.a() == 255 {
        a
    } else {
        mix(SECONDARY(), CREAM_0(), a.a() as f32 / 255.0)
    }
}

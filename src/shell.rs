//! A casca do site (`public/assets/js/shell.js` + `shell.css`): o topo com a marca, a busca e a conta, e a trilha de ícones à esquerda.

use crate::theme::*;
use crate::ui::{self, mix, text};
use crate::{ic, png, App, Section};
use eframe::egui::{self, Align2, Color32, CursorIcon, ImageSource, Pos2, Rect, Sense, Vec2};

/// `--nav-h` e `--rail-w`.
pub const NAV_H: f32 = 56.0;
pub const RAIL_W: f32 = 76.0;

/// O nome, a migalha do topo e o ícone de cada seção, as três de cima e os atalhos.
fn about(s: Section) -> (&'static str, &'static str, ImageSource<'static>) {
    match s {
        Section::Home => ("Início", "INÍCIO", ic!("rail-home")),
        Section::Perfil => ("Meu perfil", "PERFIL", ic!("cw-user")),
        Section::Jogos => ("Jogos", "JOGOS", ic!("rail-jogos")),
        Section::LivePixel => ("LivePixel", "LIVEPIXEL", ic!("rail-livepixel")),
        Section::Call => ("Call", "CALL", ic!("rail-call")),
        Section::Config => ("Configurações", "CONFIGURAÇÕES", ic!("gear")),
        Section::Proximidade => ("LoL de proximidade", "LOL DE PROXIMIDADE", ic!("rail-lol")),
    }
}

pub fn topbar(ui: &mut egui::Ui, app: &mut App, full: Rect) {
    let bar = Rect::from_min_size(full.min, Vec2::new(full.width(), NAV_H));
    let p = ui.painter().clone();
    p.rect_filled(bar, 0.0, CREAM_1());
    let o = bar.min;

    // a marca: o ícone e "brittico" em Avara
    ui::image(ui, png!("brittico-icon"), Rect::from_min_size(o + Vec2::new(24.0, 13.0), Vec2::splat(30.0)), R_SM());
    text(&p, o + Vec2::new(64.0, 29.0), Align2::LEFT_CENTER, "brittico", avara(19.0), INK(), 0.5);
    // a migalha: "› CALL"
    ui::icon(ui, ic!("chevron-right"), o + Vec2::new(177.0, 28.0), 12.0, FAINT());
    let crumb = match app.section {
        Section::Call => crate::dados::grupos().get(app.call.group).map(|g| g.name.to_uppercase()).unwrap_or_else(|| "CALL".into()),
        s => about(s).1.to_owned(),
    };
    text(&p, o + Vec2::new(193.0, 28.5), Align2::LEFT_CENTER, &crumb, patua(15.0), mix(INK(), CREAM_1(), 0.5), 0.5);

    // a marca leva ao Início, como o link do site
    let marca = Rect::from_min_size(o + Vec2::new(20.0, 10.0), Vec2::new(150.0, 36.0));
    if ui.interact(marca, ui.id().with("marca"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand).clicked() {
        app.section = Section::Home;
    }

    // a busca, no meio
    let search = Rect::from_min_size(Pos2::new(bar.center().x - 268.0, o.y + 9.0), Vec2::new(520.0, 38.0));
    busca(ui, app, search);

    // o botão do tema e a conta
    let theme_btn = Rect::from_min_size(Pos2::new(bar.right() - 255.0, o.y + 10.0), Vec2::splat(36.0));
    atualizacao(ui, app, Pos2::new(theme_btn.left() - 10.0, theme_btn.center().y));
    let r = ui.interact(theme_btn, ui.id().with("tema"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand).on_hover_text("Alternar tema claro/escuro");
    if r.clicked() {
        // o botão do topo alterna entre os dois, deixando de seguir o sistema (como o do site)
        app.perfil.aj.tema = if DARK() { crate::perfil::ajustes::Tema::Claro } else { crate::perfil::ajustes::Tema::Escuro };
    }
    p.circle_filled(theme_btn.center(), 18.0, if r.hovered() { SECONDARY() } else { CREAM_2() });
    ui::icon(ui, if DARK() { ic!("sun") } else { ic!("moon") }, theme_btn.center(), 16.0, if r.hovered() { ON_SECONDARY() } else { SOFT() });

    // a conta: clicar no nome abre o perfil; o ícone ao lado sai. Sem login, o "Conectar conta" do site.
    let Some(me) = crate::conta::eu() else {
        if !app.conta.pronto {
            return;
        }
        let w = 168.0;
        let b = Rect::from_min_size(Pos2::new(bar.right() - 24.0 - w, o.y + 10.0), Vec2::new(w, 36.0));
        let r = ui.interact(b, ui.id().with("entrar"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
        p.rect_filled(b, 18.0, if r.hovered() { ui::mix(SECONDARY(), Color32::WHITE, 0.08) } else { SECONDARY() });
        ui::icon(ui, ic!("login"), b.left_center() + Vec2::new(22.0, 0.0), 16.0, ON_SECONDARY());
        text(&p, b.left_center() + Vec2::new(38.0, 0.0), Align2::LEFT_CENTER, "Conectar conta", f7(13.0), ON_SECONDARY(), 0.0);
        if r.clicked() {
            app.conta.pedir_login();
        }
        return;
    };
    let user = Rect::from_min_size(Pos2::new(bar.right() - 203.0, o.y + 8.0), Vec2::new(179.0, 40.0));
    let hit = Rect::from_min_max(user.min, Pos2::new(user.right() - 40.0, user.bottom()));
    let r = ui.interact(hit, ui.id().with("conta"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand).on_hover_text("Meu perfil");
    if r.clicked() {
        app.section = Section::Perfil;
    }
    p.rect_filled(user, 20.0, if r.hovered() || app.section == Section::Perfil { CREAM_4() } else { CREAM_2() });
    let nome = me.nome().to_owned();
    ui::avatar(ui, &nome, crate::net::imagem(&me.user.avatar), user.left_center() + Vec2::new(20.0, 0.0), 30.0, false, None);
    let max_nome = if me.is_mod { 80.0 } else { 120.0 };
    let name = ui::clip(&p, user.left_center() + Vec2::new(43.0, 0.0), Align2::LEFT_CENTER, &nome, f7(13.0), INK(), max_nome);
    if me.is_mod {
        let badge = Rect::from_min_size(Pos2::new(name.right() + 8.0, user.center().y - 8.5), Vec2::new(38.0, 17.0));
        p.rect_filled(badge, 8.5, ACCENT());
        text(&p, badge.center(), Align2::CENTER_CENTER, "MOD", f8(9.0), Color32::WHITE, 0.6);
    }
    let sair = Rect::from_center_size(Pos2::new(user.right() - 21.0, user.center().y), Vec2::splat(30.0));
    let sr = ui.interact(sair, ui.id().with("sair"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand).on_hover_text("Sair");
    ui::icon(ui, ic!("logout"), sair.center(), 16.0, if sr.hovered() { INK() } else { SOFT() });
    if sr.clicked() {
        app.conta.sair(ui.ctx());
        app.section = Section::Home;
    }
}

/// Para onde um resultado da busca leva.
#[derive(Clone)]
enum Destino {
    Secao(Section),
    Pessoa(usize),
    Canal(usize),
    Site(String),
}

/// Os resultados da busca, em grupos (o `/api/search` do site agrupa Perfis, Produtos, Campeões, Itens e Comandos;
/// loja e LoL não estão no app, então aqui os grupos são o que o app tem).
fn resultados(q: &str) -> Vec<(&'static str, Vec<(String, String, Destino)>)> {
    let q = q.to_lowercase();
    let tem = |s: &str| s.to_lowercase().contains(&q);
    let mut grupos = vec![];
    let perfis: Vec<_> = crate::dados::pessoas().iter().enumerate().filter(|(_, p)| !p.name.starts_with("Perfil ") && (tem(&p.name) || tem(&p.login))).take(4).map(|(i, p)| {
        let sub = p.roles.first().map(|r| r.name().to_owned()).unwrap_or_else(|| if p.online { "online".into() } else { "offline".into() });
        (p.name.to_owned(), sub, Destino::Pessoa(i))
    }).collect();
    if !perfis.is_empty() {
        grupos.push(("Perfis", perfis));
    }
    let grupos_call = crate::dados::grupos();
    let canais: Vec<_> = crate::dados::canais().iter().enumerate().filter(|(_, c)| c.grupo < grupos_call.len() && tem(&c.name)).take(4).map(|(i, c)| {
        let tipo = match c.kind { crate::dados::Kind::Voice { .. } => "canal de voz", crate::dados::Kind::News => "avisos", _ => "canal de texto" };
        (crate::nome::simples(&c.name), format!("{tipo} · {}", crate::nome::simples(&grupos_call[c.grupo].name)), Destino::Canal(i))
    }).collect();
    if !canais.is_empty() {
        grupos.push(("Canais da Call", canais));
    }
    let jogos: Vec<_> = [("Bolotas", "bolotas"), ("Co-co-work", "cocowork"), ("Quem disse isso?", "quemdisse"), ("LivePixel", "livepixel"), ("Carona com Marcos", "carona")]
        .into_iter()
        .filter(|(n, _)| tem(n))
        .map(|(n, id)| (n.to_owned(), "jogo".to_owned(), if id == "livepixel" { Destino::Secao(Section::LivePixel) } else { Destino::Site(format!("https://brittico.xyz/jogos/{id}/")) }))
        .collect();
    if !jogos.is_empty() {
        grupos.push(("Jogos", jogos));
    }
    let telas: Vec<_> = [Section::Home, Section::Jogos, Section::LivePixel, Section::Proximidade, Section::Perfil, Section::Config]
        .into_iter()
        .filter(|s| tem(about(*s).0))
        .map(|s| (about(s).0.to_owned(), "tela do app".to_owned(), Destino::Secao(s)))
        .collect();
    if !telas.is_empty() {
        grupos.push(("Telas", telas));
    }
    grupos
}

/// A barra de busca (`.kz-search`): a prévia agrupada a partir de 2 letras, setas e Enter, Esc fecha.
fn busca(ui: &mut egui::Ui, app: &mut App, search: Rect) {
    let p = ui.painter().clone();
    let id = ui.id().with("busca");
    let foco = ui.memory(|m| m.has_focus(id));
    p.rect_filled(search, search.height() / 2.0, CREAM_2());
    if foco {
        p.rect_stroke(search, search.height() / 2.0, egui::Stroke::new(1.0_f32, SECONDARY()), egui::StrokeKind::Inside);
    }
    ui::icon(ui, ic!("search"), Pos2::new(search.left() + 23.0, search.center().y), 14.0, FAINT());
    let campo = Rect::from_min_max(Pos2::new(search.left() + 41.0, search.top() + 9.0), Pos2::new(search.right() - 18.0, search.bottom() - 9.0));
    let mut fui = ui.new_child(egui::UiBuilder::new().max_rect(campo));
    let out = fui.add(
        egui::TextEdit::singleline(&mut app.busca)
            .id(id)
            .frame(false)
            .hint_text(egui::RichText::new("Buscar no site...").color(FAINT()).font(f6(13.0)))
            .font(f6(13.0))
            .text_color(INK())
            .vertical_align(egui::Align::Center)
            .margin(egui::Margin::ZERO)
            .desired_width(campo.width()),
    );
    let termo = app.busca.trim().to_owned();
    if termo.chars().count() < 2 || !(foco || out.lost_focus()) {
        app.busca_cursor = None;
        return;
    }
    let grupos = resultados(&termo);
    let todos: Vec<(String, String, Destino)> = grupos.iter().flat_map(|(_, g)| g.iter().cloned()).collect();
    let n = todos.len() + 1;
    let (cima, baixo, enter, esc) = ui.input(|i| (i.key_pressed(egui::Key::ArrowUp), i.key_pressed(egui::Key::ArrowDown), i.key_pressed(egui::Key::Enter), i.key_pressed(egui::Key::Escape)));
    if baixo {
        app.busca_cursor = Some(app.busca_cursor.map(|c| (c + 1) % n).unwrap_or(0));
    }
    if cima {
        app.busca_cursor = Some(app.busca_cursor.map(|c| (c + n - 1) % n).unwrap_or(n - 1));
    }
    let site = format!("https://brittico.xyz/busca/?q={}", termo.replace(' ', "+"));
    let mut ir: Option<Destino> = None;
    if esc {
        app.busca.clear();
        out.surrender_focus();
        return;
    }
    if enter {
        // com algo destacado, o Enter é "abrir isto"; sem nada, é "ver todos"
        ir = Some(match app.busca_cursor {
            Some(c) if c < todos.len() => todos[c].2.clone(),
            _ => Destino::Site(site.clone()),
        });
    }

    // o painel, embaixo da barra
    let mut h = 4.0;
    for (_, g) in &grupos {
        h += 24.0 + g.len() as f32 * 44.0 + 4.0;
    }
    if grupos.is_empty() {
        h += 44.0;
    }
    h += 38.0 + 4.0;
    let painel = Rect::from_min_size(Pos2::new(search.left(), search.bottom() + 8.0), Vec2::new(search.width(), h));
    let clicado = egui::Area::new(ui.id().with("busca-painel")).order(egui::Order::Foreground).fixed_pos(painel.min).constrain(false).show(ui.ctx(), |ui| {
        ui.allocate_exact_size(painel.size(), Sense::click());
        let p = ui.painter();
        p.rect_filled(painel.translate(Vec2::new(0.0, 8.0)).expand(4.0), R_MD() + 4.0, Color32::from_black_alpha(60));
        p.rect_filled(painel, R_MD(), CREAM_0());
        p.rect_stroke(painel, R_MD(), egui::Stroke::new(1.0_f32, HAIRLINE_2()), egui::StrokeKind::Inside);
        let mut y = painel.top() + 4.0;
        let mut k = 0;
        let mut clicado = None;
        if grupos.is_empty() {
            text(p, Pos2::new(painel.center().x, y + 22.0), Align2::CENTER_CENTER, "Nada encontrado.", f6(13.0), FAINT(), 0.0);
            y += 44.0;
        }
        for (gi, (nome, itens)) in grupos.iter().enumerate() {
            if gi > 0 {
                p.rect_filled(Rect::from_min_size(Pos2::new(painel.left() + 4.0, y), Vec2::new(painel.width() - 8.0, 1.0)), 0.0, HAIRLINE_2());
            }
            text(p, Pos2::new(painel.left() + 14.0, y + 14.0), Align2::LEFT_CENTER, &nome.to_uppercase(), f8(10.5), FAINT(), 0.63);
            y += 24.0;
            for (titulo, sub, dest) in itens {
                let it = Rect::from_min_size(Pos2::new(painel.left() + 4.0, y), Vec2::new(painel.width() - 8.0, 44.0));
                let resp = ui.interact(it, ui.id().with(("busca-it", k)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
                if resp.hovered() || app.busca_cursor == Some(k) {
                    p.rect_filled(it, R_SM(), AMBER_SOFT());
                }
                let art = Rect::from_min_size(Pos2::new(it.left() + 10.0, it.center().y - 15.0), Vec2::splat(30.0));
                match dest {
                    Destino::Pessoa(i) => {
                        let pp = crate::dados::pessoa(*i);
                        ui::avatar(ui, &pp.name, pp.photo(), art.center(), 30.0, false, None)
                    }
                    _ => {
                        p.rect_filled(art, R_XS(), CREAM_2());
                        ui::bold(p, art.center(), Align2::CENTER_CENTER, &titulo.chars().next().unwrap_or('?').to_uppercase().to_string(), patua(13.0), FAINT(), 0.0);
                    }
                }
                text(p, Pos2::new(art.right() + 10.0, it.center().y - 8.0), Align2::LEFT_CENTER, titulo, f6(13.5), INK(), 0.0);
                text(p, Pos2::new(art.right() + 10.0, it.center().y + 9.0), Align2::LEFT_CENTER, sub, f6(11.5), FAINT(), 0.0);
                if resp.clicked() {
                    clicado = Some(dest.clone());
                }
                y += 44.0;
                k += 1;
            }
            y += 4.0;
        }
        let all = Rect::from_min_size(Pos2::new(painel.left() + 4.0, y), Vec2::new(painel.width() - 8.0, 38.0));
        p.rect_filled(Rect::from_min_size(all.min, Vec2::new(all.width(), 1.0)), 0.0, HAIRLINE_2());
        let ar = ui.interact(all, ui.id().with("busca-todos"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
        let quente = ar.hovered() || app.busca_cursor == Some(todos.len());
        if quente {
            p.rect_filled(all, R_SM(), AMBER_SOFT());
        }
        text(p, all.center(), Align2::CENTER_CENTER, "Ver todos os resultados", f7(12.5), if quente { INK() } else { SOFT() }, 0.0);
        if ar.clicked() {
            clicado = Some(Destino::Site(site.clone()));
        }
        clicado
    });
    let ir = ir.or(clicado.inner);
    if let Some(d) = ir {
        match d {
            Destino::Secao(s) => app.section = s,
            Destino::Pessoa(i) => {
                if i == crate::dados::eu() {
                    app.section = Section::Perfil;
                } else {
                    app.section = Section::Call;
                    app.call.abrir_cartao(i);
                }
            }
            Destino::Canal(i) => {
                app.section = Section::Call;
                if let Some(c) = crate::dados::canal(i) {
                    app.call.group = c.grupo;
                }
                app.call.view = crate::call::View::Channel(i);
            }
            Destino::Site(url) => ui.ctx().open_url(egui::OpenUrl::new_tab(url)),
        }
        app.busca.clear();
        app.busca_cursor = None;
        out.surrender_focus();
    } else if !foco && ui.input(|i| i.pointer.any_pressed()) && !ui.input(|i| i.pointer.interact_pos()).is_some_and(|q| painel.contains(q)) {
        app.busca_cursor = None;
    }
}

/// O lado de cada ícone da trilha (`.rail__btn`): as áreas, o LivePixel e os grupos da Call têm todos o mesmo.
const BTN: f32 = 48.0;

/// A trilha: o Início; os Jogos, o LivePixel e o LoL de proximidade (não é o hub de LoL do site: é a voz por
/// proximidade, que ainda vai chegar); os grupos da Call (os "servidores", cada um como um app da trilha, que
/// abre a Call nele) e o "+" de criar ou entrar num grupo; e as configurações do app presas no pé. Um divisor entre
/// um bloco e o outro. A loja, a música, o LoL, os VODs e o que é da moderação ficam só no site.
pub fn rail(ui: &mut egui::Ui, app: &mut App, full: Rect) {
    let col = Rect::from_min_max(Pos2::new(full.left(), full.top() + NAV_H), Pos2::new(full.left() + RAIL_W, full.bottom()));
    ui.painter().rect_filled(col, 0.0, CREAM_1());
    let cx = col.center().x;
    let mut y = col.top() + 16.0;
    let next = |y: &mut f32| {
        let r = Rect::from_min_size(Pos2::new(cx - BTN / 2.0, *y), Vec2::splat(BTN));
        *y += BTN + 10.0;
        r
    };

    // o Início, com a cara da marca: fundo de tinta e o ícone em creme
    let r = next(&mut y);
    if button(ui, r, "inicio", "Início", app.section == Section::Home, |ui, r, _| {
        ui.painter().rect_filled(r, R_LG(), INK());
        ui::icon(ui, ic!("rail-home"), r.center(), 26.0, CREAM_1());
    }) {
        app.section = Section::Home;
    }

    y += 4.0;
    divider(ui, cx, y - 4.0);
    y += 10.0;
    // o LoL de proximidade abre a janela do Olheiro: aceso enquanto ela está aberta, com a bolinha do estado dele
    let (ol_aberta, ol_estado) = app.olheiro.lock().map(|j| (j.aberta, Some(j.motor.estado()))).unwrap_or((false, None));
    let ol_dica = match &ol_estado {
        Some(e) => format!("LoL de proximidade · {}", crate::olheiro::resumo(e).0),
        None => "LoL de proximidade".to_owned(),
    };
    for (sec, key, label, icon) in [(Section::Jogos, "jogos", "Jogos".to_owned(), ic!("rail-jogos")), (Section::LivePixel, "livepixel", "LivePixel".to_owned(), ic!("rail-livepixel")), (Section::Proximidade, "proximidade", ol_dica, ic!("rail-lol"))] {
        let r = next(&mut y);
        let on = if sec == Section::Proximidade { ol_aberta } else { app.section == sec };
        if button(ui, r, key, &label, on, |ui, r, hot| {
            plate(ui, r, hot);
            ui::icon(ui, icon, r.center(), 22.0, if hot { ON_SECONDARY() } else { SOFT() });
        }) {
            app.section = sec;
        }
        if sec == Section::Proximidade {
            if let Some(e) = ol_estado.as_ref().filter(|e| e.ligado) {
                let d = r.right_bottom() - Vec2::splat(6.0);
                ui.painter().circle_filled(d, 6.5, CREAM_1());
                ui.painter().circle_filled(d, 4.5, crate::olheiro::resumo(e).1);
            }
        }
    }

    y += 4.0;
    divider(ui, cx, y - 4.0);
    y += 10.0;
    for (i, g) in crate::dados::grupos().into_iter().enumerate() {
        // as salas de que se é membro (as públicas que ainda não entrou ficam para o convite)
        if !g.membro || g.name.is_empty() {
            continue;
        }
        let r = next(&mut y);
        let on = app.section == Section::Call && app.call.group == i;
        let name = crate::nome::simples(&g.name);
        if button(ui, r, &format!("grupo-{}", g.id), &name, on, |ui, r, hot| {
            plate(ui, r, hot);
            // o `roomIcon` do site: foto (endereço ou caminho da API), emoji curto, ou as iniciais sem os códigos de posição
            let icone = g.icon.trim();
            let foto = (icone.starts_with("http") || icone.starts_with('/')).then(|| crate::net::imagem(icone)).flatten();
            let emoji = (!icone.is_empty() && foto.is_none() && icone.chars().count() <= 8)
                .then(|| crate::nome::partes(icone).into_iter().find_map(|p| if let crate::nome::Parte::Emoji(c) = p { Some(c) } else { None }))
                .flatten();
            let cor = if hot { ON_SECONDARY() } else { SOFT() };
            if let Some(src) = foto {
                ui::image(ui, src, r.shrink(4.0), R_MD());
            } else if let Some(c) = emoji {
                ui::image(ui, crate::nome::emoji(&c), Rect::from_center_size(r.center(), Vec2::splat(24.0)), 0.0);
            } else {
                let letras = crate::nome::sem_icones(&g.name);
                match (letras.is_empty(), crate::nome::primeira_pos(&g.name)) {
                    (true, Some(p)) => crate::nome::icone_pos(ui, p, r.center(), 20.0, cor),
                    _ => {
                        let base = if letras.is_empty() { g.name.clone() } else { letras };
                        text(ui.painter(), r.center() + Vec2::new(0.0, 1.0), Align2::CENTER_CENTER, &ui::initials(&base), patua(15.0), cor, 0.0);
                    }
                }
            }
        }) {
            app.section = Section::Call;
            app.call.abrir_grupo(i);
        }
    }
    // o "+" que morava no pé da coluna de grupos da Call
    let r = next(&mut y);
    if button(ui, r, "grupo+", "Criar ou entrar num grupo", false, |ui, r, hot| {
        if hot {
            ui.painter().rect_filled(r, R_LG(), AMBER_SOFT());
        } else {
            ui.painter().rect_stroke(r, R_LG(), egui::Stroke::new(1.0_f32, HAIRLINE_2()), egui::StrokeKind::Inside);
        }
        ui::icon(ui, ic!("plus"), r.center(), 18.0, if hot { SECONDARY() } else { FAINT() });
    }) {
        app.section = Section::Call;
        app.call.pedir_convite();
    }

    // as configurações do app, presas no pé
    let gear = Rect::from_min_size(Pos2::new(cx - BTN / 2.0, (col.bottom() - 16.0 - BTN).max(y)), Vec2::splat(BTN));
    if button(ui, gear, "config", "Configurações do app", app.section == Section::Config, |ui, r, hot| {
        plate(ui, r, hot);
        ui::icon(ui, ic!("gear"), r.center(), 22.0, if hot { ON_SECONDARY() } else { SOFT() });
    }) {
        app.section = Section::Config;
    }
}

/// O fundo de um ícone da trilha: `--cream-0` parado, âmbar e mais quadrado com o mouse em cima ou quando está aberto.
fn plate(ui: &egui::Ui, r: Rect, hot: bool) {
    ui.painter().rect_filled(r, if hot { R_MD() } else { R_LG() }, if hot { SECONDARY() } else { CREAM_0() });
}

/// Um ícone da trilha: o desenho dele (que sabe se está quente) e a barrinha à esquerda, cheia no aberto e meia no
/// mouse em cima. Devolve se foi clicado.
fn button(ui: &mut egui::Ui, rect: Rect, key: &str, tip: &str, on: bool, draw: impl FnOnce(&egui::Ui, Rect, bool)) -> bool {
    let resp = ui.interact(rect, ui.id().with(("rail", key)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand).on_hover_text(tip);
    let hot = on || resp.hovered();
    draw(ui, rect, hot);
    if hot {
        let h = if on { 30.0 } else { 18.0 };
        let c = if on { ACCENT() } else { ACCENT().gamma_multiply(0.45) };
        let pill = Rect::from_min_size(Pos2::new(ui.max_rect().left(), rect.center().y - h / 2.0), Vec2::new(4.0, h));
        ui.painter().rect_filled(pill, egui::CornerRadius { nw: 0, sw: 0, ne: 4, se: 4 }, c);
    }
    resp.clicked()
}

/// O divisor entre um bloco da trilha e o outro. O do site (`--hairline-2`, 8% de tinta) some sobre o preto; aqui ele é
/// `--cream-4`, o cinza das bordas do tema escuro, para os blocos se lerem separados.
fn divider(ui: &egui::Ui, cx: f32, y: f32) {
    ui.painter().rect_filled(Rect::from_center_size(Pos2::new(cx, y + 1.0), Vec2::new(32.0, 2.0)), 1.0, CREAM_4());
}

/// O que ainda não existe no app: o `.dz-state` do site (o quadrado âmbar com o ícone, o título e uma frase).
pub fn soon(ui: &mut egui::Ui, body: Rect, s: Section) {
    let (label, _, icon) = about(s);
    let p = ui.painter();
    let c = body.center() - Vec2::new(0.0, 40.0);
    let sq = Rect::from_center_size(c, Vec2::splat(64.0));
    p.rect_filled(sq, R_LG(), SECONDARY());
    ui::icon(ui, icon, c, 28.0, Color32::WHITE);
    let msg = match s {
        Section::Config => "As configurações do app ainda estão por fazer.",
        Section::Proximidade => "Novidades em breve.",
        _ => "Esta parte do site ainda não chegou ao app.",
    };
    text(p, c + Vec2::new(0.0, 62.0), Align2::CENTER_CENTER, label, patua(20.0), INK(), 0.0);
    text(p, c + Vec2::new(0.0, 92.0), Align2::CENTER_CENTER, msg, f6(13.5), SOFT(), 0.0);
}

/// O aviso de versão nova, à esquerda do botão do tema (o do Discord): "Atualizar para 1.0.1", a barra do download, o
/// "Reiniciando…" e, se falhar, "Tentar de novo" com o motivo no mouse.
fn atualizacao(ui: &mut egui::Ui, app: &mut App, direita: Pos2) {
    use crate::atualizar::Fase;
    use std::sync::atomic::Ordering;
    let v = |tag: &str| tag.trim_start_matches('v').to_owned();
    let (rotulo, icone, progresso, dica): (String, egui::ImageSource<'static>, Option<f32>, Option<String>) = match &app.atualizador.fase {
        Fase::Nada => return,
        Fase::Disponivel(tag) if app.atualizador.tipo.sozinho() => (format!("Atualizar para {}", v(tag)), ic!("download"), None, None),
        Fase::Disponivel(tag) => (format!("Versão {} disponível", v(tag)), ic!("download"), None, None),
        Fase::Baixando { tag, feito, total } => {
            let (f, t) = (feito.load(Ordering::Relaxed), total.load(Ordering::Relaxed));
            let pct = (t > 0).then(|| (f as f32 / t as f32).clamp(0.0, 1.0));
            let txt = match pct {
                Some(p) => format!("Baixando {} · {}%", v(tag), (p * 100.0) as u32),
                None => format!("Baixando {} · {} MB", v(tag), f / 1_048_576),
            };
            (txt, ic!("download"), Some(pct.unwrap_or(0.0)), None)
        }
        Fase::Trocando => ("Reiniciando…".to_owned(), ic!("refresh"), Some(1.0), None),
        Fase::Erro { msg, .. } => ("Atualização falhou · tentar de novo".to_owned(), ic!("alert"), None, Some(msg.clone())),
    };
    let p = ui.painter().clone();
    let w = 16.0 + 16.0 + 8.0 + ui::text_width(&p, &rotulo, f7(12.5), 0.0) + 16.0;
    let r = Rect::from_min_size(Pos2::new(direita.x - w, direita.y - 18.0), Vec2::new(w, 36.0));
    let ocupado = progresso.is_some();
    let mut resp = ui.interact(r, ui.id().with("atualizar"), if ocupado { Sense::hover() } else { Sense::click() });
    if !ocupado {
        resp = resp.on_hover_cursor(CursorIcon::PointingHand);
    }
    if let Some(d) = &dica {
        resp = resp.on_hover_text(d);
    }
    let erro = dica.is_some();
    let base = if erro { Color32::from_rgb(200, 70, 60) } else { SECONDARY() };
    match progresso {
        Some(f) => {
            p.rect_filled(r, 18.0, mix(base, CREAM_2(), 0.25));
            if f > 0.0 {
                let cheio = Rect::from_min_max(r.min, Pos2::new(r.left() + r.width() * f, r.bottom()));
                p.with_clip_rect(cheio).rect_filled(r, 18.0, base);
            }
        }
        None => {
            p.rect_filled(r, 18.0, if resp.hovered() { mix(base, Color32::WHITE, 0.9) } else { base });
        }
    }
    let cor = if erro { Color32::WHITE } else { ON_SECONDARY() };
    ui::icon(ui, icone, r.left_center() + Vec2::new(16.0 + 8.0, 0.0), 16.0, cor);
    text(&p, r.left_center() + Vec2::new(16.0 + 16.0 + 8.0, 0.0), Align2::LEFT_CENTER, &rotulo, f7(12.5), cor, 0.0);
    if resp.clicked() {
        app.atualizador.atualizar(ui.ctx());
    }
}

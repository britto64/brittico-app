//! O que abre por cima da Call (`call/js/ui-overlay.js`, `ui-chat.js`, `ui-voice.js` do site): os emotes da conversa e o
//! seletor deles, o cartão de uma pessoa, o menu do grupo, o da conta, o das ferramentas de LoL, a janela de convite,
//! a Call Presidencial aberta e os atalhos de teclado de dentro da call.

use super::*;
use eframe::egui::{Key, Order};

/// Os emotes do modo de teste do site (`EMOTES` do mock.js), com a imagem de cada um.
pub(super) fn emote(nome: &str) -> Option<ImageSource<'static>> {
    macro_rules! e {
        ($n:literal) => {
            ImageSource::Bytes { uri: std::borrow::Cow::Borrowed(concat!("bytes://emote/", $n, ".png")), bytes: egui::load::Bytes::Static(include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/emotes/", $n, ".png"))) }
        };
    }
    Some(match nome {
        "bobo" => e!("bobo"),
        "kennzyO7" => e!("kennzyO7"),
        "playKz" => e!("play"),
        "sonlas" => e!("sonlas"),
        _ => return None,
    })
}
pub(super) const EMOTES: [&str; 4] = ["bobo", "kennzyO7", "playKz", "sonlas"];

/// Uma linha da conversa: as palavras que são emote viram a imagem (28 px), os links ficam âmbar e abrem no navegador.
pub(super) fn linha(ui: &egui::Ui, x: f32, y: f32, line: &str) {
    let p = ui.painter();
    let mut cx = x;
    let space = text_width(p, " ", f6(13.5), 0.0);
    for (k, word) in line.split(' ').enumerate() {
        if k > 0 {
            cx += space;
        }
        if let Some(src) = emote(word) {
            let r = Rect::from_min_size(Pos2::new(cx, y - 14.0), Vec2::splat(28.0));
            ui::image(ui, src, r, 0.0);
            ui.interact(r, ui.id().with(("emote", x as i32, y as i32, k)), Sense::hover()).on_hover_text(word);
            cx += 28.0;
        } else if word.starts_with("https://") || word.starts_with("http://") {
            let r = text(p, Pos2::new(cx, y), Align2::LEFT_CENTER, word, f6(13.5), SECONDARY(), 0.0);
            let resp = ui.interact(r, ui.id().with(("link", word, y as i32)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand).on_hover_text("Abrir no navegador");
            if resp.hovered() {
                p.line_segment([Pos2::new(r.left(), r.bottom()), r.right_bottom()], Stroke::new(1.0_f32, SECONDARY()));
            }
            if resp.clicked() {
                ui.ctx().open_url(egui::OpenUrl::new_tab(word));
            }
            cx = r.right();
        } else {
            // emoji e código de posição viram imagem, como nos nomes
            cx = crate::nome::desenhar(ui, Pos2::new(cx, y), Align2::LEFT_CENTER, word, f6(13.5), INK(), 0.0, 0.0).right();
        }
    }
}

/// Um menu pequeno (`.dz-menu`): as linhas com ícone e texto, numa camada por cima. Devolve a linha clicada e se clicou fora.
fn menu(ui: &egui::Ui, id: &str, at: Pos2, w: f32, itens: &[(ImageSource<'static>, &str, bool)], titulo: Option<&str>) -> (Option<usize>, bool) {
    let topo = if titulo.is_some() { 28.0 } else { 0.0 };
    let h = 8.0 + topo + itens.len() as f32 * 36.0 + 8.0;
    let r = Rect::from_min_size(at, Vec2::new(w, h));
    let out = egui::Area::new(ui.id().with(("dz-menu", id))).order(Order::Foreground).fixed_pos(r.min).constrain(false).show(ui.ctx(), |ui| {
        ui.allocate_exact_size(r.size(), Sense::click());
        let p = ui.painter();
        p.rect_filled(r.translate(Vec2::new(0.0, 8.0)).expand(4.0), R_MD() + 4.0, Color32::from_black_alpha(60));
        p.rect_filled(r, R_MD(), CREAM_0());
        p.rect_stroke(r, R_MD(), Stroke::new(1.0_f32, HAIRLINE_2()), StrokeKind::Inside);
        if let Some(t) = titulo {
            text(p, Pos2::new(r.left() + 16.0, r.top() + 8.0 + 14.0), Align2::LEFT_CENTER, &t.to_uppercase(), f8(10.0), FAINT(), 0.8);
        }
        let mut esc = None;
        for (i, (icon, label, perigo)) in itens.iter().enumerate() {
            let it = Rect::from_min_size(Pos2::new(r.left() + 6.0, r.top() + 8.0 + topo + i as f32 * 36.0), Vec2::new(w - 12.0, 34.0));
            let resp = ui.interact(it, ui.id().with(("dz-menu-it", id, i)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
            if resp.hovered() {
                p.rect_filled(it, R_SM(), if *perigo { Color32::from_rgba_unmultiplied(229, 53, 43, 30) } else { AMBER_SOFT() });
            }
            let c = if *perigo { DIS_OFF } else if resp.hovered() { INK() } else { SOFT() };
            ui::icon(ui, icon.clone(), Pos2::new(it.left() + 12.0 + 7.5, it.center().y), 15.0, if *perigo { DIS_OFF } else { SECONDARY() });
            text(p, Pos2::new(it.left() + 40.0, it.center().y), Align2::LEFT_CENTER, label, f6(13.0), c, 0.0);
            if resp.clicked() {
                esc = Some(i);
            }
        }
        esc
    });
    let fora = ui.input(|i| i.pointer.any_pressed()) && !ui.input(|i| i.pointer.interact_pos()).is_some_and(|q| r.contains(q));
    (out.inner, fora)
}

/// O cartão de uma pessoa (`.dz-card`): foto, nome, @, cargos, o estado e "Ver perfil".
pub(super) fn cartao(ui: &egui::Ui, st: &mut CallState, body: Rect) {
    let Some((id, at)) = st.card else { return };
    let person = dados::pessoa(id);
    // quem modera vê, para quem está na call em que ele também está, silenciar e tirar da call
    let canal_dela = st.room_of(id);
    let pode_mutar = id != dados::eu() && canal_dela.is_some() && dados::tem_perm("diskenzy.voice.mute");
    let pode_tirar = id != dados::eu() && canal_dela.is_some() && dados::tem_perm("diskenzy.voice.disconnect");
    let extras = pode_mutar as usize + pode_tirar as usize;
    let w = 260.0;
    let h = 16.0 + 40.0 + 10.0 + if person.roles.is_empty() { 0.0 } else { 30.0 } + 12.0 + 30.0 + 12.0 + 40.0 + 10.0 + extras as f32 * 36.0;
    let r = Rect::from_min_size(Pos2::new(at.x.clamp(body.left() + 8.0, body.right() - w - 8.0), at.y.clamp(body.top() + 8.0, body.bottom() - h - 8.0)), Vec2::new(w, h));
    let res = egui::Area::new(ui.id().with("dz-card")).order(Order::Foreground).fixed_pos(r.min).constrain(false).show(ui.ctx(), |ui| {
        ui.allocate_exact_size(r.size(), Sense::click());
        let p = ui.painter();
        p.rect_filled(r.translate(Vec2::new(0.0, 8.0)).expand(4.0), R_MD() + 4.0, Color32::from_black_alpha(60));
        p.rect_filled(r, R_MD(), CREAM_0());
        p.rect_stroke(r, R_MD(), Stroke::new(1.0_f32, HAIRLINE_2()), StrokeKind::Inside);
        let x = r.left() + 16.0;
        let mut y = r.top() + 16.0;
        let dot = if !person.online { None } else if st.room_of(id).is_some() { Some((SECONDARY(), CREAM_0())) } else { Some((DIS_OK, CREAM_0())) };
        ui::avatar(ui, &person.name, person.photo(), Pos2::new(x + 20.0, y + 20.0), 40.0, false, dot);
        let nome = if id == dados::eu() { format!("{} (você)", person.name) } else { person.name.to_owned() };
        text(p, Pos2::new(x + 52.0, y + 12.0), Align2::LEFT_CENTER, &nome, patua(16.0), INK(), 0.0);
        let login = if person.login.is_empty() { person.name.to_lowercase().replace(' ', "") } else { person.login.clone() };
        text(p, Pos2::new(x + 52.0, y + 30.0), Align2::LEFT_CENTER, &format!("@{login}"), f6(11.5), FAINT(), 0.0);
        y += 40.0 + 10.0;
        if !person.roles.is_empty() {
            let mut rx = x;
            for role in &person.roles {
                let label = role.name();
                let tw = 22.0 + 6.0 + text_width(p, label, f7(11.0), 0.0) + 10.0;
                let tag = Rect::from_min_size(Pos2::new(rx, y), Vec2::new(tw, 24.0));
                p.rect_filled(tag, 12.0, role.color());
                ui::role_chip(ui, role, Pos2::new(tag.left() + 12.0, tag.center().y), 20.0);
                text(p, Pos2::new(tag.left() + 26.0, tag.center().y), Align2::LEFT_CENTER, label, f7(11.0), Color32::WHITE, 0.0);
                rx = tag.right() + 6.0;
            }
            y += 30.0;
        }
        p.rect_filled(Rect::from_min_size(Pos2::new(x, y + 5.0), Vec2::new(w - 32.0, 1.0)), 0.0, HAIRLINE_2());
        y += 12.0;
        let estado = match st.room_of(id) {
            Some(room) => format!("Na call · {}", dados::canal_ou(room).name),
            None if person.online => "Online".into(),
            None => "Offline".into(),
        };
        crate::nome::desenhar(ui, Pos2::new(x, y + 15.0), Align2::LEFT_CENTER, &estado, f6(12.5), SOFT(), 0.0, w - 32.0);
        y += 30.0;
        p.rect_filled(Rect::from_min_size(Pos2::new(x, y + 5.0), Vec2::new(w - 32.0, 1.0)), 0.0, HAIRLINE_2());
        y += 12.0;
        let b = Rect::from_min_size(Pos2::new(x - 6.0, y), Vec2::new(w - 20.0, 34.0));
        let resp = ui.interact(b, ui.id().with("dz-card-perfil"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
        if resp.hovered() {
            p.rect_filled(b, R_SM(), AMBER_SOFT());
        }
        ui::icon(ui, ic!("users"), Pos2::new(b.left() + 12.0 + 7.5, b.center().y), 15.0, SECONDARY());
        text(p, Pos2::new(b.left() + 40.0, b.center().y), Align2::LEFT_CENTER, "Ver perfil", f6(13.0), if resp.hovered() { INK() } else { SOFT() }, 0.0);
        let mut acao = resp.clicked().then_some(0);
        let mut by = b.bottom() + 2.0;
        for (k, (ok, icone, label)) in [(pode_mutar, ic!("mic-off"), "Silenciar o microfone"), (pode_tirar, ic!("hangup"), "Tirar da call")].into_iter().enumerate() {
            if !ok {
                continue;
            }
            let b2 = Rect::from_min_size(Pos2::new(x - 6.0, by), Vec2::new(w - 20.0, 34.0));
            let r2 = ui.interact(b2, ui.id().with(("dz-card-mod", k)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
            if r2.hovered() {
                p.rect_filled(b2, R_SM(), Color32::from_rgba_unmultiplied(229, 53, 43, 30));
            }
            ui::icon(ui, icone, Pos2::new(b2.left() + 12.0 + 7.5, b2.center().y), 15.0, DIS_OFF);
            text(p, Pos2::new(b2.left() + 40.0, b2.center().y), Align2::LEFT_CENTER, label, f6(13.0), DIS_OFF, 0.0);
            if r2.clicked() {
                acao = Some(k + 1);
            }
            by += 36.0;
        }
        acao
    });
    if let Some(a) = res.inner {
        st.card = None;
        if a == 0 {
            if id == dados::eu() {
                st.abrir_perfil = true;
            } else {
                st.abrir_outro = Some(person.profile_id);
            }
        } else if let Some(c) = canal_dela {
            let canal = dados::canal_ou(c).id;
            let pid = person.profile_id;
            let nome = person.name.clone();
            st.tarefas.push(net::rodar(ui.ctx(), move || {
                if a == 1 {
                    net::pedir_ja("POST", &format!("/api/diskenzy/v2/voice/{canal}/mute"), Some(&serde_json::json!({ "profileId": pid, "track": "mic", "muted": true })))?;
                    Ok(format!("O microfone de {nome} foi silenciado."))
                } else {
                    net::pedir_ja("POST", &format!("/api/diskenzy/v2/voice/{canal}/disconnect"), Some(&serde_json::json!({ "profileId": pid })))?;
                    Ok(format!("{nome} saiu da call."))
                }
            }));
        }
    } else if ui.input(|i| i.pointer.any_pressed()) && !ui.input(|i| i.pointer.interact_pos()).is_some_and(|q| r.contains(q)) {
        st.card = None;
    }
}

/// Os menus: o do grupo (pelo nome ou pela engrenagem), o da conta (engrenagem da barra "eu") e o das ferramentas de LoL.
pub(super) fn menus(ui: &egui::Ui, st: &mut CallState, t: f64) {
    if let Some(at) = st.grupo_menu {
        let itens = [
            (ic!("user-plus"), "Convidar pessoas", false),
            (ic!("plus"), "Criar canal", false),
            (ic!("move"), "Organizar canais", false),
            (ic!("gear"), "Configurar a sala", false),
            (ic!("link"), "Entrar com convite", false),
        ];
        let (esc, fora) = menu(ui, "grupo", at, 220.0, &itens, None);
        match esc {
            Some(0) => st.abrir_convite(ui.ctx()),
            Some(4) => st.convite = Some(Convite::Entrar(String::new())),
            Some(_) => {
                // criar, organizar e configurar canais moram no painel da Call do site
                net::abrir_no_navegador(&format!("{}/call/", net::SITE));
                st.toast.say("Os ajustes da sala abrem no site, no navegador.", t);
            }
            None => {}
        }
        if esc.is_some() || fora {
            st.grupo_menu = None;
        }
    }
    if let Some(at) = st.conta_menu {
        let itens = [(ic!("cw-user"), "Meu perfil", false), (ic!("link"), "Entrar com convite", false)];
        let (esc, fora) = menu(ui, "conta", at, 220.0, &itens, None);
        match esc {
            Some(0) => st.abrir_perfil = true,
            Some(_) => st.convite = Some(Convite::Entrar(String::new())),
            None => {}
        }
        if esc.is_some() || fora {
            st.conta_menu = None;
        }
    }
}

/// A janela de convite: o link para chamar gente, ou o campo para entrar numa sala com o código de alguém.
#[derive(Clone)]
pub(super) enum Convite {
    /// O link criado (vazio enquanto o servidor não respondeu).
    Convidar,
    Entrar(String),
}

impl CallState {
    /// "Convidar pessoas": cria um convite de membro de 7 dias na sala aberta.
    pub(super) fn abrir_convite(&mut self, ctx: &egui::Context) {
        self.convite = Some(Convite::Convidar);
        self.link_convite = None;
        let sala = self.sala_id();
        let (tx, rx) = std::sync::mpsc::channel();
        self.link_rx = Some(rx);
        let c = ctx.clone();
        std::thread::spawn(move || {
            let r = net::pedir_ja("POST", &format!("/api/diskenzy/guilds/{sala}/invites"), Some(&serde_json::json!({ "ttl": "7d", "kind": "member", "maxUses": 0 })));
            let _ = tx.send(r.map(|v| format!("{}/call/?convite={}", net::SITE, v["invite"]["token"].as_str().unwrap_or(""))));
            c.request_repaint();
        });
    }
}

/// O token de um convite colado (`/call/?convite=abc`, o do Diskenzy ou o token puro).
fn token_do_convite(s: &str) -> String {
    let s = s.trim();
    let t = s.split_once("convite=").map(|(_, t)| t.split(['&', '#']).next().unwrap_or("")).unwrap_or(s);
    if t.len() >= 6 && t.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
        t.to_owned()
    } else {
        String::new()
    }
}

pub(super) fn convite(ui: &egui::Ui, st: &mut CallState, body: Rect, t: f64) {
    let Some(c) = st.convite.clone() else { return };
    if let Some(r) = st.link_rx.as_ref().and_then(|rx| rx.try_recv().ok()) {
        st.link_rx = None;
        match r {
            Ok(l) => st.link_convite = Some(l),
            Err(e) => {
                st.toast.say(&e.msg, t);
                st.convite = None;
                return;
            }
        }
    }
    let link = st.link_convite.clone().unwrap_or_else(|| "criando o link…".to_owned());
    let nome_sala = dados::grupos().get(st.group).map(|g| g.name.clone()).unwrap_or_default();
    let w = 440.0;
    let h = 210.0;
    let r = Rect::from_center_size(body.center(), Vec2::new(w, h));
    let mut fechar = false;
    egui::Area::new(ui.id().with("dz-convite")).order(Order::Foreground).fixed_pos(body.min).constrain(false).show(ui.ctx(), |ui| {
        let (_, fundo) = ui.allocate_exact_size(body.size(), Sense::click());
        let p = ui.painter().clone();
        let p = &p;
        p.rect_filled(body, 0.0, Color32::from_black_alpha(150));
        p.rect_filled(r, R_LG(), CREAM_0());
        let titulo = match c {
            Convite::Convidar => "Convidar pessoas",
            Convite::Entrar(_) => "Entrar com convite",
        };
        ui::bold(p, Pos2::new(r.left() + 24.0, r.top() + 34.0), Align2::LEFT_CENTER, titulo, patua(18.0), INK(), 0.0);
        let x = Rect::from_center_size(Pos2::new(r.right() - 30.0, r.top() + 34.0), Vec2::splat(30.0));
        let xr = ui.interact(x, ui.id().with("dz-convite-x"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand).on_hover_text("Fechar");
        if xr.hovered() {
            p.rect_filled(x, R_SM(), CREAM_2());
        }
        ui::icon(ui, ic!("x"), x.center(), 18.0, if xr.hovered() { INK() } else { FAINT() });
        let campo = Rect::from_min_size(Pos2::new(r.left() + 24.0, r.top() + 92.0), Vec2::new(w - 48.0 - 110.0, 42.0));
        let botao = Rect::from_min_size(Pos2::new(campo.right() + 10.0, campo.top()), Vec2::new(100.0, 42.0));
        p.rect_filled(campo, R_SM(), CREAM_2());
        let (dica, acao) = match &c {
            Convite::Convidar => (format!("Quem abrir este link entra na sala {nome_sala}. Ele vale por 7 dias."), "Copiar"),
            Convite::Entrar(_) => ("Cole o link ou o código que alguém mandou para você.".to_owned(), "Entrar"),
        };
        text(p, Pos2::new(r.left() + 24.0, r.top() + 70.0), Align2::LEFT_CENTER, &dica, f6(12.5), SOFT(), 0.0);
        match &mut st.convite {
            Some(Convite::Entrar(codigo)) => {
                let mut fui = ui.new_child(UiBuilder::new().max_rect(campo.shrink2(Vec2::new(12.0, 10.0))));
                fui.add(egui::TextEdit::singleline(codigo).frame(false).hint_text(egui::RichText::new("brittico.xyz/call/c/…").color(FAINT())).font(f6(13.5)).text_color(INK()).desired_width(campo.width() - 24.0));
            }
            _ => {
                ui::clip(p, Pos2::new(campo.left() + 12.0, campo.center().y), Align2::LEFT_CENTER, &link, f6(13.5), INK(), campo.width() - 24.0);
            }
        }
        let br = ui.interact(botao, ui.id().with("dz-convite-ok"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
        p.rect_filled(botao, R_SM(), if br.hovered() { mix(Color32::WHITE, SECONDARY(), 0.08) } else { SECONDARY() });
        text(p, botao.center(), Align2::CENTER_CENTER, acao, f7(13.5), ON_SECONDARY(), 0.0);
        if br.clicked() {
            match st.convite.clone() {
                Some(Convite::Convidar) => {
                    if let Some(l) = st.link_convite.clone() {
                        ui.ctx().copy_text(l);
                        st.toast.say("Link do convite copiado.", t);
                        fechar = true;
                    }
                }
                Some(Convite::Entrar(cod)) => {
                    let tok = token_do_convite(&cod);
                    if tok.is_empty() {
                        st.toast.say("Cole o link ou o código do convite.", t);
                    } else {
                        st.tarefas.push(net::rodar(ui.ctx(), move || {
                            let v = net::pedir_ja("POST", &format!("/api/diskenzy/v2/invites/{tok}/accept"), None)?;
                            let nome = v["room"]["name"].as_str().unwrap_or("").to_owned();
                            let id = v["room"]["id"].as_str().unwrap_or("").to_owned();
                            dados::mudar(|d| {
                                d.sala_de(&v["room"]);
                                for c in v["channels"].as_array().into_iter().flatten() {
                                    d.canal_de(c);
                                }
                            });
                            rede::buscar_membros(&id);
                            Ok(format!("Você entrou na sala {nome}."))
                        }));
                        fechar = true;
                    }
                }
                None => {}
            }
        }
        text(p, Pos2::new(r.left() + 24.0, r.bottom() - 34.0), Align2::LEFT_CENTER, "Convites dados por moderadores não expiram.", f6(11.5), FAINT(), 0.0);
        if xr.clicked() || (fundo.clicked() && !ui.input(|i| i.pointer.interact_pos()).is_some_and(|q| r.contains(q))) {
            fechar = true;
        }
    });
    if fechar || ui.input(|i| i.key_pressed(Key::Escape)) {
        st.convite = None;
    }
}

/// O seletor de emotes, acima do compositor: a busca e a grade. Clicar põe o nome do emote na mensagem.
pub(super) fn seletor_emotes(ui: &egui::Ui, st: &mut CallState, composer: Rect) {
    if !st.emotes_abertos {
        return;
    }
    let r = Rect::from_min_size(Pos2::new(composer.left() + 20.0, composer.top() - 8.0 - 104.0), Vec2::new(260.0, 104.0));
    let fora = egui::Area::new(ui.id().with("dz-emotes")).order(Order::Foreground).fixed_pos(r.min).constrain(false).show(ui.ctx(), |ui| {
        ui.allocate_exact_size(r.size(), Sense::click());
        let p = ui.painter().clone();
        let p = &p;
        p.rect_filled(r.translate(Vec2::new(0.0, 6.0)).expand(3.0), R_MD() + 3.0, Color32::from_black_alpha(50));
        p.rect_filled(r, R_MD(), CREAM_0());
        p.rect_stroke(r, R_MD(), Stroke::new(1.0_f32, HAIRLINE_2()), StrokeKind::Inside);
        let campo = Rect::from_min_size(r.min + Vec2::new(10.0, 10.0), Vec2::new(r.width() - 20.0, 32.0));
        p.rect_filled(campo, R_SM(), CREAM_2());
        p.rect_stroke(campo, R_SM(), Stroke::new(1.0_f32, SECONDARY()), StrokeKind::Inside);
        let mut fui = ui.new_child(UiBuilder::new().max_rect(campo.shrink2(Vec2::new(10.0, 7.0))));
        let resp = fui.add(egui::TextEdit::singleline(&mut st.emote_q).frame(false).hint_text(egui::RichText::new("procurar emote").color(FAINT())).font(f6(12.5)).text_color(INK()).desired_width(campo.width() - 20.0));
        if !resp.has_focus() && st.emote_q.is_empty() {
            resp.request_focus();
        }
        let q = st.emote_q.to_lowercase();
        let mut x = r.left() + 10.0;
        let mut escolhido = None;
        for nome in EMOTES.iter().filter(|n| n.to_lowercase().contains(&q)) {
            let b = Rect::from_min_size(Pos2::new(x, campo.bottom() + 10.0), Vec2::splat(40.0));
            let br = ui.interact(b, ui.id().with(("dz-emote", *nome)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand).on_hover_text(*nome);
            if br.hovered() {
                p.rect_filled(b, R_SM(), AMBER_SOFT());
            }
            ui::image(ui, emote(nome).unwrap(), b.shrink(5.0), 0.0);
            if br.clicked() {
                escolhido = Some(*nome);
            }
            x += 44.0;
        }
        if let Some(n) = escolhido {
            if !st.draft.is_empty() && !st.draft.ends_with(' ') {
                st.draft.push(' ');
            }
            st.draft.push_str(n);
            st.draft.push(' ');
            st.emotes_abertos = false;
            st.emote_q.clear();
        }
        ui.input(|i| i.pointer.any_pressed()) && !ui.input(|i| i.pointer.interact_pos()).is_some_and(|q| r.contains(q) || composer.contains(q))
    });
    if fora.inner || ui.input(|i| i.key_pressed(Key::Escape)) {
        st.emotes_abertos = false;
    }
}

/// A Call Presidencial aberta (o painel que sobe do pé da coluna de pessoas): o que ela é, quem está e o botão.
pub(super) fn presidencial(ui: &egui::Ui, st: &mut CallState, col: Rect, t: f64) -> f32 {
    if !st.presi_aberta {
        return 0.0;
    }
    let h = 150.0;
    let r = Rect::from_min_max(Pos2::new(col.left(), col.bottom() - PRESI_H - h), Pos2::new(col.right(), col.bottom() - PRESI_H));
    let p = ui.painter();
    p.rect_filled(r, 0.0, CREAM_0());
    p.line_segment([r.left_top(), r.right_top()], Stroke::new(1.0_f32, HAIRLINE_2()));
    let mut job = egui::text::LayoutJob::single_section(
        "Uma call só de voz para quem tem esta permissão. Quem está na call principal não ouve o que se fala aqui.".into(),
        egui::TextFormat { font_id: f6(11.5), color: SOFT(), line_height: Some(16.0), ..Default::default() },
    );
    job.wrap.max_width = r.width() - 32.0;
    let g = p.layout_job(job);
    let gh = g.size().y;
    p.galley(r.min + Vec2::new(16.0, 12.0), g, SOFT());
    let y = r.top() + 12.0 + gh + 10.0;
    let sala = st.sala_id();
    let quem: Vec<usize> = dados::com(|d| d.presidencial.get(&sala).cloned().unwrap_or_default());
    let mut x = r.left() + 16.0;
    if quem.is_empty() {
        text(p, Pos2::new(x, y + 12.0), Align2::LEFT_CENTER, "Ninguém na call agora.", f6(11.5), FAINT(), 0.0);
    }
    for id in &quem {
        let person = dados::pessoa(*id);
        if x > r.right() - 60.0 {
            break;
        }
        ui::avatar(ui, &person.name, person.photo(), Pos2::new(x + 12.0, y + 12.0), 24.0, st.presi.is_some() && dados::falando(person.profile_id), None);
        let n = text(p, Pos2::new(x + 30.0, y + 12.0), Align2::LEFT_CENTER, &person.name, f7(12.0), INK(), 0.0);
        x = n.right() + 14.0;
    }
    let _ = t;
    let b = Rect::from_min_size(Pos2::new(r.left() + 16.0, r.bottom() - 12.0 - 32.0), Vec2::new(r.width() - 32.0, 32.0));
    let br = ui.interact(b, ui.id().with("presi-entrar"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
    let (bg, fg, label) = if st.presi.is_some() { (DIS_OFF, Color32::WHITE, "Sair da call presidencial") } else { (SECONDARY(), ON_SECONDARY(), "Entrar na call presidencial") };
    p.rect_filled(b, R_SM(), if br.hovered() { mix(Color32::WHITE, bg, 0.08) } else { bg });
    ui::icon(ui, ic!("phone"), Pos2::new(b.left() + 14.0 + 7.0, b.center().y), 14.0, fg);
    text(p, Pos2::new(b.left() + 36.0, b.center().y), Align2::LEFT_CENTER, label, f7(12.5), fg, 0.0);
    if br.clicked() {
        st.presi_alternar(ui.ctx());
    }
    h
}

/// Os atalhos de dentro da call (os mesmos do site): M microfone, D ouvir, S tela, I estatísticas, Esc volta.
pub(super) fn atalhos(ui: &egui::Ui, st: &mut CallState) {
    if st.joined.is_none() || ui.ctx().wants_keyboard_input() {
        return;
    }
    let mut pedir_tela = false;
    ui.input(|i| {
        if i.key_pressed(Key::M) {
            if st.deafened {
                st.deafened = false;
                st.muted = false;
            } else {
                st.muted = !st.muted;
            }
            st.aplicar_som();
        }
        if i.key_pressed(Key::D) {
            st.deafened = !st.deafened;
            st.aplicar_som();
        }
        if i.key_pressed(Key::S) && st.seletor.is_none() {
            if st.sharing {
                if let Some(v) = &st.voz {
                    v.mandar(voz::Cmd::Tela(None));
                }
            } else {
                pedir_tela = true;
            }
        }
        if i.key_pressed(Key::I) {
            st.stats = !st.stats;
        }
        if i.key_pressed(Key::Escape) {
            if st.quadro_cheio.is_some() {
                st.quadro_cheio = None;
            } else if st.cheia {
                st.cheia = false;
            } else {
                st.foco = None;
                st.palco = false;
            }
        }
    });
    if pedir_tela {
        let modo = st.tela_modo;
        st.comecar_tela(ui.ctx(), modo);
    }
}

/* ================= o seletor de tela (Windows e Linux em X11) ================= */

/// O seletor de "Mostrar a tela" do app, como o do Discord: uma aba de telas e outra de janelas, cada uma com miniatura.
pub(crate) struct Seletor {
    tarefa: Option<net::Tarefa<Vec<voz::Fonte>>>,
    fontes: Vec<voz::Fonte>,
    /// As miniaturas já na placa de vídeo (na ordem de `fontes`).
    texturas: Vec<Option<egui::TextureHandle>>,
    janelas: bool,
    erro: Option<String>,
}

impl Seletor {
    pub(crate) fn abrir(ctx: &egui::Context) -> Self {
        Seletor { tarefa: Some(net::rodar(ctx, || Ok(voz::fontes()))), fontes: Vec::new(), texturas: Vec::new(), janelas: false, erro: None }
    }
}

pub(super) fn seletor_tela(ui: &egui::Ui, st: &mut CallState, body: Rect) {
    let Some(sel) = st.seletor.as_mut() else { return };
    if let Some(r) = sel.tarefa.as_ref().and_then(|t| t.pronto()) {
        sel.tarefa = None;
        match r {
            Ok(f) => {
                sel.texturas = f
                    .iter()
                    .enumerate()
                    .map(|(k, x)| {
                        x.miniatura.as_ref().map(|(w, h, rgba)| ui.ctx().load_texture(format!("tela-mini-{k}"), egui::ColorImage::from_rgba_unmultiplied([*w, *h], rgba), egui::TextureOptions::LINEAR))
                    })
                    .collect();
                sel.fontes = f;
                if sel.fontes.is_empty() {
                    sel.erro = Some("Não achei nenhuma tela para mostrar.".into());
                }
            }
            Err(e) => sel.erro = Some(e.msg),
        }
    }
    let w = 720.0_f32.min(body.width() - 40.0);
    let h = 520.0_f32.min(body.height() - 40.0);
    let r = Rect::from_center_size(body.center(), Vec2::new(w, h));
    let mut fechar = false;
    let mut escolhida: Option<voz::Escolha> = None;
    egui::Area::new(ui.id().with("dz-seletor-tela")).order(Order::Foreground).fixed_pos(body.min).constrain(false).show(ui.ctx(), |ui| {
        let (_, fundo) = ui.allocate_exact_size(body.size(), Sense::click());
        let p = ui.painter().clone();
        let p = &p;
        p.rect_filled(body, 0.0, Color32::from_black_alpha(150));
        p.rect_filled(r, R_LG(), CREAM_0());
        ui::bold(p, Pos2::new(r.left() + 24.0, r.top() + 34.0), Align2::LEFT_CENTER, "Mostrar a tela", patua(18.0), INK(), 0.0);
        let modo = if super::tela_fullhd() { "1080p · 30 fps" } else { "720p · 30 fps" };
        text(p, Pos2::new(r.left() + 24.0, r.top() + 58.0), Align2::LEFT_CENTER, modo, f6(12.0), FAINT(), 0.0);
        let x = Rect::from_center_size(Pos2::new(r.right() - 30.0, r.top() + 34.0), Vec2::splat(30.0));
        let xr = ui.interact(x, ui.id().with("dz-seletor-x"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
        if xr.hovered() {
            p.rect_filled(x, R_SM(), CREAM_2());
        }
        ui::icon(ui, ic!("x"), x.center(), 16.0, SOFT());
        // as abas
        let mut ax = r.left() + 24.0;
        for (janelas, nome) in [(false, "Telas"), (true, "Janelas")] {
            let n = sel.fontes.iter().filter(|f| f.escolha.janela == janelas).count();
            let rotulo = if sel.tarefa.is_none() { format!("{nome} · {n}") } else { nome.to_owned() };
            let tw = text_width(p, &rotulo, f7(13.0), 0.0) + 28.0;
            let ab = Rect::from_min_size(Pos2::new(ax, r.top() + 78.0), Vec2::new(tw, 32.0));
            let resp = ui.interact(ab, ui.id().with(("dz-seletor-aba", janelas)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
            let on = sel.janelas == janelas;
            p.rect_filled(ab, R_SM(), if on { SECONDARY() } else if resp.hovered() { CREAM_2() } else { CREAM_1() });
            text(p, ab.center(), Align2::CENTER_CENTER, &rotulo, f7(13.0), if on { ON_SECONDARY() } else { SOFT() }, 0.0);
            if resp.clicked() {
                sel.janelas = janelas;
            }
            ax += tw + 8.0;
        }
        let area = Rect::from_min_max(Pos2::new(r.left() + 16.0, r.top() + 122.0), Pos2::new(r.right() - 16.0, r.bottom() - 16.0));
        if sel.tarefa.is_some() {
            text(p, area.center(), Align2::CENTER_CENTER, "Procurando as telas e janelas…", f6(13.5), FAINT(), 0.0);
        } else if let Some(e) = &sel.erro {
            text(p, area.center(), Align2::CENTER_CENTER, e, f6(13.5), FAINT(), 0.0);
        } else {
            let idx: Vec<usize> = (0..sel.fontes.len()).filter(|&k| sel.fontes[k].escolha.janela == sel.janelas).collect();
            if idx.is_empty() {
                text(p, area.center(), Align2::CENTER_CENTER, if sel.janelas { "Nenhuma janela aberta." } else { "Nenhuma tela." }, f6(13.5), FAINT(), 0.0);
            }
            let mut filho = ui.new_child(egui::UiBuilder::new().max_rect(area));
            egui::ScrollArea::vertical().id_salt("dz-seletor-lista").auto_shrink([false, false]).show(&mut filho, |ui| {
                let cols = if sel.janelas { 3 } else { 2 };
                let gap = 12.0;
                let cw = ((area.width() - 10.0 - gap * (cols as f32 - 1.0)) / cols as f32).floor();
                let mini_h = (cw * 9.0 / 16.0).floor();
                let ch = mini_h + 16.0 + 26.0;
                for linha in idx.chunks(cols) {
                    let (lr, _) = ui.allocate_exact_size(Vec2::new(area.width() - 10.0, ch + gap), Sense::hover());
                    for (c, &k) in linha.iter().enumerate() {
                        let f = &sel.fontes[k];
                        let cart = Rect::from_min_size(Pos2::new(lr.left() + c as f32 * (cw + gap), lr.top()), Vec2::new(cw, ch));
                        let resp = ui.interact(cart, ui.id().with(("dz-seletor-fonte", k)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
                        let p = ui.painter();
                        p.rect_filled(cart, R_MD(), if resp.hovered() { CREAM_2() } else { CREAM_1() });
                        if resp.hovered() {
                            p.rect_stroke(cart, R_MD(), Stroke::new(2.0_f32, SECONDARY()), egui::StrokeKind::Inside);
                        }
                        let caixa = Rect::from_min_size(cart.min + Vec2::splat(8.0), Vec2::new(cw - 16.0, mini_h));
                        p.rect_filled(caixa, R_SM(), Color32::BLACK);
                        match sel.texturas.get(k).and_then(|t| t.as_ref()) {
                            Some(tex) => {
                                // encaixa sem esticar
                                let ts = tex.size_vec2();
                                let esc = (caixa.width() / ts.x).min(caixa.height() / ts.y);
                                let img = Rect::from_center_size(caixa.center(), ts * esc);
                                p.image(tex.id(), img, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
                            }
                            None => ui::icon(ui, if f.escolha.janela { ic!("screen") } else { ic!("screen") }, caixa.center(), 28.0, FAINT()),
                        }
                        ui::clip(p, Pos2::new(cart.left() + 10.0, cart.bottom() - 16.0), Align2::LEFT_CENTER, &f.nome, f7(12.5), INK(), cw - 20.0);
                        if resp.clicked() {
                            escolhida = Some(f.escolha);
                        }
                    }
                }
            });
        }
        if xr.clicked() || (fundo.clicked() && !ui.input(|i| i.pointer.interact_pos()).is_some_and(|q| r.contains(q))) || ui.input(|i| i.key_pressed(Key::Escape)) {
            fechar = true;
        }
    });
    if let Some(e) = escolhida {
        if let Some(v) = &st.voz {
            v.mandar(voz::Cmd::Tela(Some((st.tela_modo, Some(e), super::tela_fullhd()))));
        }
        fechar = true;
    }
    if fechar {
        st.seletor = None;
    }
}

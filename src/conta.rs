//! Quem está usando o app: a sessão do site (`net`), o `/api/me` e o cartão de "Conectar conta" (o `/entrar/` do site:
//! Twitch, Kick em breve, YouTube). Entrar abre o login no navegador e o app espera a volta; sair apaga a sessão daqui.

use crate::net::{self, Me, Tarefa};
use crate::theme::*;
use crate::ui::{self, text};
use crate::ic;
use eframe::egui::{self, Align2, Color32, CursorIcon, Order, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};
use std::sync::RwLock;

/// O `/api/me` mais recente, para qualquer tela perguntar quem é você (`eu()`).
static EU: RwLock<Option<Me>> = RwLock::new(None);

pub fn eu() -> Option<Me> {
    EU.read().unwrap().clone()
}

/// O id do perfil de quem está usando (0 sem login).
pub fn perfil_id() -> i64 {
    EU.read().unwrap().as_ref().map(|m| m.profile_id).unwrap_or(0)
}

pub fn tem_perm(p: &str) -> bool {
    EU.read().unwrap().as_ref().is_some_and(|m| m.is_mod || m.perms.iter().any(|x| x == p))
}

#[derive(Default)]
pub struct Conta {
    me: Option<Tarefa<Me>>,
    login: Option<net::Login>,
    /// O cartão de "Conectar conta" aberto.
    pub cartao: bool,
    erro: Option<String>,
    /// Sobe 1 a cada vez que quem está usando muda (entrou, saiu): as telas recarregam o que é da pessoa.
    pub geracao: u64,
    /// Já perguntou ao servidor quem é (mesmo que a resposta seja "ninguém").
    pub pronto: bool,
}

impl Conta {
    pub fn iniciar(&mut self, ctx: &egui::Context) {
        net::carregar_sessao();
        self.recarregar(ctx);
    }

    pub fn recarregar(&mut self, ctx: &egui::Context) {
        if net::tem_sessao() {
            self.me = Some(net::get(ctx, "/api/me"));
        } else {
            self.definir(None);
            self.pronto = true;
        }
    }

    fn definir(&mut self, me: Option<Me>) {
        let antes = perfil_id();
        let agora = me.as_ref().map(|m| m.profile_id).unwrap_or(0);
        *EU.write().unwrap() = me;
        if antes != agora || !self.pronto {
            self.geracao += 1;
        }
    }

    pub fn logado(&self) -> bool {
        perfil_id() > 0
    }

    /// Olha as respostas que chegaram (uma vez por quadro).
    pub fn passo(&mut self, ctx: &egui::Context) {
        if let Some(r) = self.me.as_ref().and_then(|t| t.pronto()) {
            self.me = None;
            self.pronto = true;
            match r {
                Ok(m) if m.logged_in => self.definir(Some(m)),
                Ok(_) => {
                    // a sessão guardada venceu ou foi recusada
                    net::guardar_sessao(None);
                    self.definir(None);
                }
                Err(e) => {
                    // sem internet: segue com o que tinha, e tenta de novo daqui a pouco
                    eprintln!("/api/me: {e}");
                    if e.status == 0 {
                        let c = ctx.clone();
                        std::thread::spawn(move || {
                            std::thread::sleep(std::time::Duration::from_secs(5));
                            c.request_repaint();
                        });
                        self.pronto = false;
                    }
                }
            }
        }
        if !self.pronto && self.me.is_none() && net::tem_sessao() {
            self.me = Some(net::get(ctx, "/api/me"));
        }
        if let Some(r) = self.login.as_ref().and_then(|l| l.tarefa.pronto()) {
            self.login = None;
            match r {
                Ok(_) => {
                    self.cartao = false;
                    self.erro = None;
                    self.recarregar(ctx);
                }
                Err(e) => self.erro = Some(e.msg),
            }
        }
    }

    pub fn sair(&mut self, ctx: &egui::Context) {
        // o servidor só apaga o cookie (a sessão é assinada, não fica guardada lá); aqui some o arquivo
        let _: Tarefa<serde_json::Value> = net::pedir(ctx, "POST", "/api/auth/logout", None);
        net::guardar_sessao(None);
        self.definir(None);
    }

    pub fn pedir_login(&mut self) {
        self.cartao = true;
        self.erro = None;
    }

    /// O cartão do `/entrar/`, no meio da janela por cima de tudo.
    pub fn mostrar(&mut self, ctx: &egui::Context) {
        if !self.cartao {
            return;
        }
        let screen = ctx.screen_rect();
        let fundo = egui::Area::new(egui::Id::new("entrar-fundo")).order(Order::Foreground).fixed_pos(screen.min).show(ctx, |ui| {
            let r = ui.allocate_rect(screen, Sense::click());
            ui.painter().rect_filled(screen, 0.0, Color32::from_black_alpha(140));
            r.clicked()
        });
        let card_w = 440.0;
        let card_h = if self.login.is_some() { 230.0 } else { 372.0 };
        let card = Rect::from_center_size(screen.center(), Vec2::new(card_w, card_h));
        let mut fechar = fundo.inner && self.login.is_none();
        egui::Area::new(egui::Id::new("entrar-cartao")).order(Order::Tooltip).fixed_pos(card.min).show(ctx, |ui| {
            ui.set_min_size(card.size());
            ui.allocate_rect(card, Sense::click());
            let p = ui.painter().clone();
            p.rect_filled(card, R_LG(), CREAM_1());
            p.rect_stroke(card, R_LG(), Stroke::new(1.0_f32, HAIRLINE_2()), StrokeKind::Inside);
            let x = card.left() + 28.0;
            let mut y = card.top() + 30.0;
            if let Some(l) = &self.login {
                let nome = match l.plataforma {
                    "youtube" => "YouTube",
                    _ => "Twitch",
                };
                text(&p, Pos2::new(x, y + 12.0), Align2::LEFT_CENTER, "Conectar conta", patua(26.0), INK(), 0.0);
                y += 50.0;
                text(&p, Pos2::new(x, y), Align2::LEFT_TOP, &format!("Termine de entrar com a {nome} no navegador."), f6(14.0), SOFT(), 0.0);
                y += 46.0;
                let ab = Rect::from_min_size(Pos2::new(x, y), Vec2::new(180.0, 40.0));
                if botao(ui, ab, "Abrir de novo", true) {
                    net::abrir_no_navegador(&l.url);
                }
                let cancel = Rect::from_min_size(Pos2::new(x + 192.0, y), Vec2::new(120.0, 40.0));
                if botao(ui, cancel, "Cancelar", false) {
                    self.login = None;
                }
                let t = ui.input(|i| i.time);
                let dots = ".".repeat(1 + (t * 2.0) as usize % 3);
                text(&p, Pos2::new(x, card.bottom() - 30.0), Align2::LEFT_CENTER, &format!("Esperando a volta{dots}"), f6(12.0), FAINT(), 0.0);
                ui.ctx().request_repaint_after(std::time::Duration::from_millis(400));
                return;
            }
            text(&p, Pos2::new(x, y + 12.0), Align2::LEFT_CENTER, "Conectar conta", patua(26.0), INK(), 0.0);
            y += 40.0;
            let sub = "Escolha a plataforma que você quer conectar. Você pode conectar as outras redes à sua conta depois, no seu perfil.";
            let g = p.layout(sub.into(), f6(13.5), SOFT(), card_w - 56.0);
            p.galley(Pos2::new(x, y), g.clone(), SOFT());
            y += g.size().y + 16.0;
            if let Some(e) = &self.erro {
                let g = p.layout(e.clone(), f6(12.5), Color32::from_rgb(0xe5, 0x35, 0x2b), card_w - 56.0);
                p.galley(Pos2::new(x, y), g.clone(), Color32::WHITE);
                y += g.size().y + 10.0;
            }
            for (id, label, desc, cor, pronto, icone) in [
                ("twitch", "Twitch", "Chat, inscrição e moderação do canal", Color32::from_rgb(0x91, 0x46, 0xff), true, ic!("plat-twitch")),
                ("kick", "Kick", "Para quem acompanha as lives por lá", Color32::from_rgb(0x53, 0xfc, 0x18), false, ic!("plat-kick")),
                ("youtube", "YouTube", "Entre com o seu canal do YouTube", Color32::from_rgb(0xff, 0, 0), true, ic!("plat-youtube")),
            ] {
                let r = Rect::from_min_size(Pos2::new(x, y), Vec2::new(card_w - 56.0, 58.0));
                let resp = ui.interact(r, ui.id().with(("plat", id)), if pronto { Sense::click() } else { Sense::hover() });
                let resp = if pronto { resp.on_hover_cursor(CursorIcon::PointingHand) } else { resp };
                p.rect_filled(r, R_MD(), if resp.hovered() && pronto { CREAM_3() } else { CREAM_2() });
                let ic_r = Rect::from_min_size(r.min + Vec2::new(11.0, 11.0), Vec2::splat(36.0));
                p.rect_filled(ic_r, R_SM(), if pronto { cor } else { ui::mix(cor, CREAM_2(), 0.6) });
                ui::icon(ui, icone, ic_r.center(), 20.0, if id == "kick" { Color32::BLACK } else { Color32::WHITE });
                let nm = text(&p, Pos2::new(ic_r.right() + 12.0, r.top() + 20.0), Align2::LEFT_CENTER, label, f7(14.0), if pronto { INK() } else { FAINT() }, 0.0);
                if !pronto {
                    let b = Rect::from_min_size(Pos2::new(nm.right() + 8.0, nm.center().y - 8.0), Vec2::new(62.0, 16.0));
                    p.rect_filled(b, 8.0, CREAM_4());
                    text(&p, b.center(), Align2::CENTER_CENTER, "em breve", f7(10.0), SOFT(), 0.0);
                }
                text(&p, Pos2::new(ic_r.right() + 12.0, r.top() + 39.0), Align2::LEFT_CENTER, desc, f6(12.0), FAINT(), 0.0);
                if pronto {
                    ui::icon(ui, ic!("chevron-right"), Pos2::new(r.right() - 20.0, r.center().y), 14.0, SOFT());
                }
                if resp.clicked() {
                    match net::entrar(ui.ctx(), if id == "youtube" { "youtube" } else { "twitch" }) {
                        Ok(l) => {
                            self.login = Some(l);
                            self.erro = None;
                        }
                        Err(e) => self.erro = Some(e.msg),
                    }
                }
                y += 66.0;
            }
            let legal = "Só lemos o nome, a foto e o identificador da sua conta. Ao entrar, você concorda com os termos de serviço e com a política de privacidade.";
            let g = p.layout(legal.into(), f6(11.5), FAINT(), card_w - 56.0);
            p.galley(Pos2::new(x, y + 6.0), g, FAINT());
        });
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) && self.login.is_none() {
            fechar = true;
        }
        if fechar {
            self.cartao = false;
        }
    }
}

fn botao(ui: &mut egui::Ui, r: Rect, label: &str, primario: bool) -> bool {
    let resp = ui.interact(r, ui.id().with(("btn", label)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
    let p = ui.painter();
    if primario {
        p.rect_filled(r, R_MD(), if resp.hovered() { ui::mix(SECONDARY(), Color32::WHITE, 0.08) } else { SECONDARY() });
        text(p, r.center(), Align2::CENTER_CENTER, label, f7(13.5), ON_SECONDARY(), 0.0);
    } else {
        p.rect_filled(r, R_MD(), if resp.hovered() { CREAM_2() } else { Color32::TRANSPARENT });
        p.rect_stroke(r, R_MD(), Stroke::new(1.0_f32, HAIRLINE_2()), StrokeKind::Inside);
        text(p, r.center(), Align2::CENTER_CENTER, label, f7(13.5), SOFT(), 0.0);
    }
    resp.clicked()
}

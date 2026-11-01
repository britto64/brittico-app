//! Brittico App: o site do Brittico como programa. A meta é ficar igual ao site, tela por tela.
//!
//! Protótipo só de interface: as pessoas, os canais e a conversa são os do modo de teste da Call do site (`mock.rs`)
//! e nada fala com o servidor. A casca é a do site (o topo e a trilha, `shell.rs`); a Call (`call.rs`) é a página
//! /call/ com as quatro colunas; os Jogos (`jogos.rs`) são a
//! parede de capas do /jogos/; o resto ainda é a tela de "em breve".
//! O resto do site (loja, música, LoL, VODs, LivePixel, painel, overlays, thumbs, mídia kit, IRL) fica fora do app de propósito.
//!
//! Medidas e cores vêm do site medido no navegador (tema Preto escuro, janela de 1600×900). Leve de propósito:
//! só redesenha quando algo muda ou quando há alguém falando na tela.

// no Windows, um programa de janela: sem o console preto aberto junto
#![cfg_attr(windows, windows_subsystem = "windows")]

mod call;
mod config;
mod conta;
mod home;
mod jogos;
mod livepixel;
mod dados;

mod net;
mod nome;
mod olheiro;
mod perfil;
mod shell;
mod theme;
mod ui;

use eframe::egui;
use perfil::ajustes::{Paleta, Tema};
use std::time::Duration;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Section {
    Home,
    /// O perfil de quem está usando (o /u/ do site); abre pelo nome no topo e pelos links de perfil do Início.
    Perfil,
    Jogos,
    LivePixel,
    /// O LoL de proximidade (a voz que muda com a distância no mapa). Não é uma tela: pedir esta seção abre a janela do
    /// Olheiro (`olheiro/`), à parte, e a janela grande fica onde estava.
    Proximidade,
    Call,
    /// As configurações do próprio app, presas no pé da trilha.
    Config,
}

fn main() -> eframe::Result {
    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([1600.0, 900.0])
        .with_min_inner_size([1100.0, 620.0])
        .with_title("Brittico")
        // para as capturas: outro nome de janela, que o Hyprland manda para uma tela virtual
        .with_app_id(std::env::var("BRITTICO_APP_ID").unwrap_or_else(|_| "brittico".into()));
    if let Ok(img) = image::load_from_memory(include_bytes!("../assets/brittico-icon.png")) {
        let img = img.to_rgba8();
        viewport = viewport.with_icon(std::sync::Arc::new(egui::IconData { width: img.width(), height: img.height(), rgba: img.into_raw() }));
    }
    // vsync desligado: no Wayland, com ele ligado, cada quadro espera o "pode desenhar" do compositor; com a janela numa
    // área de trabalho escondida esse aviso nunca vem, o app trava ali e o Hyprland diz que ele "não está respondendo".
    // O ritmo fica com os pedidos de redesenho e com o teto de `segurar_ritmo`.
    let options = eframe::NativeOptions { viewport, vsync: false, ..Default::default() };
    eframe::run_native("Brittico", options, Box::new(|cc| Ok(Box::new(App::new(cc)))))
}

pub struct App {
    pub section: Section,
    pub call: call::CallState,
    pub home: home::HomeState,
    pub jogos: jogos::JogosState,
    pub perfil: perfil::PerfilState,
    pub lp: livepixel::LpState,
    pub config: config::ConfigState,
    /// A aceleração da roda do mouse (ver `raw_input_hook`).
    roda_acel: f32,
    roda_t: f64,
    /// Quando começou o último quadro (o teto de quadros sem vsync, ver `segurar_ritmo`).
    ultimo_quadro: std::time::Instant,
    /// `--minimizado` (o atalho "iniciar com o Windows" do instalador): no 1º quadro a janela vai para a barra de tarefas.
    minimizar_ao_abrir: bool,
    /// Quem está usando (a sessão do site) e o cartão de entrar.
    pub conta: conta::Conta,
    /// O que está escrito na busca do topo e o resultado destacado pelas setas.
    pub busca: String,
    pub busca_cursor: Option<usize>,
    /// Os roteiros de teste: o da janela grande (`BRITTICO_ROTEIRO`) e o da janela do Olheiro (`BRITTICO_OLHEIRO_ROTEIRO`).
    roteiro: Roteiro,
    roteiro_ol: Roteiro,
    /// O tema que está aplicado agora (tema, paleta, cor de destaque, sistema escuro?), para só trocar quando mudar.
    tema_aplicado: Option<(Tema, Paleta, Option<egui::Color32>, bool)>,
    /// A janela do LoL de proximidade (o Olheiro), que mora fora da janela grande.
    pub olheiro: olheiro::JanelaRef,
    /// A última seção de verdade (pedir o LoL de proximidade abre a janela dele e volta para ela).
    secao_antes: Section,
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        theme::install(&cc.egui_ctx);
        egui_extras::install_image_loaders(&cc.egui_ctx);
        // abre no Início, como o site
        let mut app = Self { section: Section::Home, call: call::CallState::default(), home: home::HomeState::default(), jogos: jogos::JogosState::default(), perfil: perfil::PerfilState::default(), lp: livepixel::LpState::default(), config: config::ConfigState::default(), roda_acel: 1.0, roda_t: -10.0, ultimo_quadro: std::time::Instant::now(), minimizar_ao_abrir: std::env::args().any(|a| a == "--minimizado"), conta: conta::Conta::default(), busca: String::new(), busca_cursor: None, roteiro: Roteiro::default(), roteiro_ol: Roteiro::default(), tema_aplicado: None, olheiro: olheiro::Janela::nova(&cc.egui_ctx), secao_antes: Section::Home };
        perfil::aparencia_local(&mut app.perfil.aj);
        app.conta.iniciar(&cc.egui_ctx);
        // para as capturas: BRITTICO_TELA=inicio-teste | jogos | livepixel | perfil | perfil-baixo | perfil-aba1..5 | geral | sala | dentro (sem nada, o Início)
        match std::env::var("BRITTICO_TELA").as_deref() {
            Ok("geral") => app.section = Section::Call,
            Ok("inicio-teste") => app.home.demo(),
            Ok("jogos") => app.section = Section::Jogos,
            Ok("livepixel") => app.section = Section::LivePixel,
            Ok(t) if t.starts_with("config") => {
                // config, config1 … config4: as abas das configurações do app
                app.section = Section::Config;
                app.config.abrir(t["config".len()..].parse().unwrap_or(0));
            }
            Ok(t) if t.starts_with("livepixel-") => {
                // livepixel-menu | config | premium | notif | roleta | imagem | streamer | denuncia | info | mod | territorios
                app.section = Section::LivePixel;
                app.lp.demo(&t["livepixel-".len()..]);
            }
            Ok("perfil") => app.section = Section::Perfil,
            Ok("perfil-baixo") => {
                app.section = Section::Perfil;
                app.perfil.rolar = Some(800.0);
            }
            Ok(t) if t.starts_with("perfil-aba") => {
                // perfil-aba1 … perfil-aba5: as abas de configuração (Editar Conta, Preferências, Segurança, Privacidade, Conexões)
                app.section = Section::Perfil;
                app.perfil.aba = t["perfil-aba".len()..].parse().unwrap_or(1);
            }
            Ok("sala") => app.section = Section::Call,
            Ok("dentro") => app.section = Section::Call,
            _ => {}
        }
        // BRITTICO_ROTEIRO="c 700 300 | w 10 | k Escape": eventos de mouse e teclado de mentira, para testar os cliques
        // numa janela da tela virtual sem mexer no mouse de ninguém (ver `roteiro_passo`)
        if let Ok(r) = std::env::var("BRITTICO_ROTEIRO") {
            app.roteiro = Roteiro::de(&r);
        }
        if let Ok(r) = std::env::var("BRITTICO_OLHEIRO_ROTEIRO") {
            app.roteiro_ol = Roteiro::de(&r);
        }
        // BRITTICO_TEMA=claro | escuro, e a paleta junto se quiser: "claro agressivo", "escuro arredondado"
        if let Ok(t) = std::env::var("BRITTICO_TEMA") {
            for w in t.split_whitespace() {
                match w {
                    "claro" => app.perfil.aj.tema = Tema::Claro,
                    "escuro" => app.perfil.aj.tema = Tema::Escuro,
                    "agressivo" => app.perfil.aj.paleta = Paleta::Agressivo,
                    "arredondado" => app.perfil.aj.paleta = Paleta::Arredondado,
                    _ => {}
                }
            }
        }
        app
    }
}

impl App {
    /// As preferências do perfil mandam no tema do app inteiro: escuro, claro ou o do sistema; a paleta; a cor de destaque.
    fn aplicar_tema(&mut self, ctx: &egui::Context) {
        let aj = &self.perfil.aj;
        let sys_dark = ctx.system_theme().is_none_or(|t| t == egui::Theme::Dark);
        let want = (aj.tema, aj.paleta, aj.destaque, sys_dark);
        if self.tema_aplicado == Some(want) {
            return;
        }
        self.tema_aplicado = Some(want);
        let dark = match aj.tema {
            Tema::Escuro => true,
            Tema::Claro => false,
            Tema::Sistema => sys_dark,
        };
        use theme::palettes::*;
        let p = match (aj.paleta, dark) {
            (Paleta::Preto, true) => PRETO_ESCURO,
            (Paleta::Preto, false) => PRETO_CLARO,
            (Paleta::Agressivo, true) => AGRESSIVO_ESCURO,
            (Paleta::Agressivo, false) => AGRESSIVO_CLARO,
            (Paleta::Arredondado, true) => ARREDONDADO_ESCURO,
            (Paleta::Arredondado, false) => ARREDONDADO_CLARO,
        };
        theme::set(ctx, p, aj.destaque);
    }
}

/// Um roteiro de teste: os passos que faltam, quantos quadros esperar antes do próximo e o botão que ficou apertado no
/// quadro anterior (o clique solta no quadro seguinte).
#[derive(Default)]
struct Roteiro {
    fila: std::collections::VecDeque<String>,
    espera: u32,
    soltar: Option<(egui::Pos2, egui::PointerButton)>,
}

impl Roteiro {
    fn de(s: &str) -> Roteiro {
        Roteiro { fila: s.split('|').map(|s| s.trim().to_owned()).filter(|s| !s.is_empty()).collect(), ..Default::default() }
    }

    fn ativo(&self) -> bool {
        !self.fila.is_empty() || self.espera > 0 || self.soltar.is_some()
    }
}

impl Roteiro {
    /// Um passo do roteiro de teste por quadro. Os comandos (coordenadas da janela):
    /// `m x y` move · `c x y` clica · `r x y` clique direito · `p x y` aperta · `u x y` solta · `k Tecla` (nome do
    /// egui::Key, com `shift+` ou `alt+` na frente se quiser) · `t texto` digita · `f caminho` solta um arquivo · `s dy` rola · `w n` espera n quadros.
    fn roteiro_passo(&mut self, raw: &mut egui::RawInput) {
        use egui::{Event, PointerButton, Pos2};
        if let Some((pos, button)) = self.soltar.take() {
            raw.events.push(Event::PointerButton { pos, button, pressed: false, modifiers: Default::default() });
            return;
        }
        if self.espera > 0 {
            self.espera -= 1;
            return;
        }
        let Some(cmd) = self.fila.pop_front() else { return };
        let mut it = cmd.splitn(2, ' ');
        let op = it.next().unwrap_or("");
        let rest = it.next().unwrap_or("").trim();
        let xy = || {
            let v: Vec<f32> = rest.split_whitespace().filter_map(|n| n.parse().ok()).collect();
            Pos2::new(*v.first().unwrap_or(&0.0), *v.get(1).unwrap_or(&0.0))
        };
        let press = |raw: &mut egui::RawInput, pos, button, pressed| raw.events.push(Event::PointerButton { pos, button, pressed, modifiers: Default::default() });
        match op {
            "m" => raw.events.push(Event::PointerMoved(xy())),
            "c" | "r" => {
                let pos = xy();
                let b = if op == "c" { PointerButton::Primary } else { PointerButton::Secondary };
                raw.events.push(Event::PointerMoved(pos));
                press(raw, pos, b, true);
                self.soltar = Some((pos, b));
            }
            "p" | "u" => {
                let pos = xy();
                raw.events.push(Event::PointerMoved(pos));
                press(raw, pos, PointerButton::Primary, op == "p");
            }
            "k" => {
                let mut mods = egui::Modifiers::default();
                let mut name = rest;
                if let Some(n) = name.strip_prefix("shift+") {
                    mods.shift = true;
                    name = n;
                }
                if let Some(n) = name.strip_prefix("alt+") {
                    mods.alt = true;
                    name = n;
                }
                if let Some(key) = egui::Key::from_name(name) {
                    raw.events.push(Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: mods });
                    raw.events.push(Event::Key { key, physical_key: None, pressed: false, repeat: false, modifiers: mods });
                }
            }
            "t" => raw.events.push(Event::Text(rest.to_owned())),
            "f" => raw.dropped_files.push(egui::DroppedFile { path: Some(rest.into()), ..Default::default() }),
            "s" => raw.events.push(Event::MouseWheel { unit: egui::MouseWheelUnit::Point, delta: egui::vec2(0.0, rest.parse().unwrap_or(0.0)), modifiers: Default::default() }),
            "w" => self.espera = rest.parse().unwrap_or(1),
            _ => eprintln!("roteiro: não entendi \"{cmd}\""),
        }
    }
}

impl App {
    /// Alguém pediu o LoL de proximidade (a trilha, a busca, o menu de LoL da Call): abre a janela dele, ou a traz para a
    /// frente, e a janela grande continua na seção em que estava.
    fn pedir_olheiro(&mut self, ctx: &egui::Context) {
        if self.section != Section::Proximidade {
            self.secao_antes = self.section;
            return;
        }
        self.section = self.secao_antes;
        if let Ok(mut j) = self.olheiro.lock() {
            if j.aberta {
                ctx.send_viewport_cmd_to(olheiro::vp(), egui::ViewportCommand::Focus);
            } else {
                j.abrir();
            }
        }
        ctx.request_repaint();
    }

    /// A janela do Olheiro, se está aberta; e o X da janela grande, que com ela aberta só esconde o Brittico (como a Steam
    /// com a lista de amigos aberta).
    fn janela_do_olheiro(&mut self, ctx: &egui::Context) {
        let (aberta, ligado) = self.olheiro.lock().map(|j| (j.aberta, j.motor.estado().ligado)).unwrap_or((false, false));
        if aberta {
            let j = self.olheiro.clone();
            ctx.show_viewport_deferred(olheiro::vp(), olheiro::builder(), move |ctx, _| olheiro::mostrar(ctx, &j));
            if ctx.input(|i| i.viewport().close_requested()) {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                // o Wayland não deixa um programa esconder a própria janela: lá ela só minimiza
                if cfg!(target_os = "linux") {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                } else {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
                }
                if let Ok(mut j) = self.olheiro.lock() {
                    j.principal_oculta = true;
                }
            }
        }
        // a bolinha do "L" na trilha acompanha o Olheiro (uma olhada por segundo basta)
        if ligado {
            ctx.request_repaint_after(Duration::from_secs(1));
        }
    }
}

impl eframe::App for App {
    fn raw_input_hook(&mut self, ctx: &egui::Context, raw: &mut egui::RawInput) {
        // a roda do mouse: mais rápida que o padrão do egui (40 px por clique) e com aceleração, como num navegador:
        // cliques seguidos em menos de 0,2 s vão somando velocidade (até 4×); parou, volta ao normal
        let agora = raw.time.unwrap_or(0.0);
        for ev in raw.events.iter_mut() {
            if let egui::Event::MouseWheel { unit, delta, modifiers } = ev {
                if modifiers.ctrl {
                    continue;
                }
                match unit {
                    egui::MouseWheelUnit::Line => {
                        self.roda_acel = if agora - self.roda_t < 0.2 { (self.roda_acel + 0.4).min(4.0) } else { 1.0 };
                        self.roda_t = agora;
                        *delta *= 2.5 * self.roda_acel;
                    }
                    egui::MouseWheelUnit::Point => *delta *= 1.5,
                    egui::MouseWheelUnit::Page => {}
                }
            }
        }
        // o eframe passa por aqui a entrada de cada janela: cada uma tem o seu roteiro
        let (r, vp) = if raw.viewport_id == olheiro::vp() { (&mut self.roteiro_ol, olheiro::vp()) } else { (&mut self.roteiro, egui::ViewportId::ROOT) };
        if r.ativo() {
            r.roteiro_passo(raw);
            ctx.request_repaint_of(vp);
        }
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        segurar_ritmo(&mut self.ultimo_quadro);
        if std::mem::take(&mut self.minimizar_ao_abrir) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
        }
        let _t0 = std::time::Instant::now();
        let _med = scopeguard_tempo(_t0);
        self.aplicar_tema(ctx);
        self.conta.passo(ctx);
        let t_agora = ctx.input(|i| i.time);
        self.perfil.passo_prefs(ctx, self.conta.geracao, t_agora);
        // a Call vive fora da tela dela: os servidores da trilha, a voz e os avisos chegam em qualquer seção
        self.call.passo(ctx, self.conta.geracao, t_agora);
        egui::CentralPanel::default().frame(egui::Frame::new().fill(theme::CREAM_1())).show(ctx, |ui| {
            let full = ui.max_rect();
            let t = ctx.input(|i| i.time);
            shell::topbar(ui, self, full);
            shell::rail(ui, self, full);
            self.pedir_olheiro(ctx);
            let body = egui::Rect::from_min_max(egui::pos2(full.left() + shell::RAIL_W, full.top() + shell::NAV_H), full.max);
            match self.section {
                Section::Home => {
                    home::show(ui, &mut self.home, body, t, self.conta.geracao);
                    if std::mem::take(&mut self.home.open_profile) {
                        self.perfil.alvo = None;
                        self.section = Section::Perfil;
                    }
                    if let Some(id) = self.home.open_other.take() {
                        self.perfil.abrir_outro(id);
                        self.section = Section::Perfil;
                    }
                    if std::mem::take(&mut self.home.pedir_login) {
                        self.conta.pedir_login();
                    }
                    if std::mem::take(&mut self.home.pedir_sair) {
                        self.conta.sair(ctx);
                    }
                    if std::mem::take(&mut self.home.alternar_tema) {
                        self.perfil.aj.tema = if theme::DARK() { Tema::Claro } else { Tema::Escuro };
                    }
                }
                Section::Perfil => {
                    perfil::show(ui, &mut self.perfil, body, t, self.conta.geracao);
                    if std::mem::take(&mut self.perfil.pedir_sair) {
                        self.conta.sair(ctx);
                        self.section = Section::Home;
                    }
                }
                Section::LivePixel => livepixel::show(ui, &mut self.lp, body, t, self.conta.geracao),
                Section::Config => config::show(ui, &mut self.config, &mut self.perfil, body, t),
                Section::Call => {
                    call::show(ui, &mut self.call, body, t, self.conta.geracao);
                    if let Some(id) = self.call.abrir_outro.take() {
                        self.perfil.abrir_outro(id);
                        self.section = Section::Perfil;
                    }
                    if std::mem::take(&mut self.call.abrir_perfil) {
                        self.section = Section::Perfil;
                    }
                    if std::mem::take(&mut self.call.abrir_proximidade) {
                        self.section = Section::Proximidade;
                        self.pedir_olheiro(ctx);
                    }
                }
                Section::Jogos => {
                    // sem animações (Configurações > O app), as capas ficam paradas
                    let tj = if self.config.animacoes() { t } else { 0.0 };
                    if let Some(s) = jogos::show(ui, &mut self.jogos, body, tj) {
                        self.section = s;
                    }
                }
                s => shell::soon(ui, body, s),
            }
        });
        self.conta.mostrar(ctx);
        self.janela_do_olheiro(ctx);
        // quem fala muda a cada ~0,4 s: só vale redesenhar sozinho quando a Call está na tela
        if self.section == Section::Call {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
        // as capas dos jogos flutuam e o ponto do "ao vivo" pisca: 30 quadros por segundo bastam para um movimento tão lento
        if self.section == Section::Jogos && self.config.animacoes() {
            ctx.request_repaint_after(Duration::from_millis(33));
        }
    }
}

/// Sem vsync, uma animação que pede redesenho sem parar (um aviso aparecendo, por exemplo) rodaria a centenas de quadros
/// por segundo. Este teto segura em ~165 por segundo: espera o resto do intervalo antes de começar o próximo quadro.
fn segurar_ritmo(ultimo: &mut std::time::Instant) {
    const MIN: Duration = Duration::from_micros(6_000);
    let passou = ultimo.elapsed();
    if passou < MIN {
        std::thread::sleep(MIN - passou);
    }
    *ultimo = std::time::Instant::now();
}

/// Para medir (BRITTICO_TEMPO=1): quanto leva cada quadro de `update`, impresso de tempos em tempos.
fn scopeguard_tempo(t0: std::time::Instant) -> impl Drop {
    struct G(std::time::Instant);
    impl Drop for G {
        fn drop(&mut self) {
            if std::env::var("BRITTICO_TEMPO").is_ok() {
                eprintln!("[quadro] {:.1} ms", self.0.elapsed().as_secs_f64() * 1000.0);
            }
        }
    }
    G(t0)
}

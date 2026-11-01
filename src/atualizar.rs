//! Atualização do app, no jeito do Discord: ao abrir e depois de 6 em 6 horas pergunta ao GitHub qual é o último
//! lançamento; se for mais novo, o topo mostra "Atualizar para X", e o clique abre a página de download do site
//! (brittico.xyz/app/). A troca sozinha abaixo está DESLIGADA (sem assinatura digital o Windows bloqueia o instalador;
//! ver `Tipo::sozinho`). Quando ligada, o clique baixa o arquivo certo para o jeito como o
//! app foi instalado, confere o SHA-256 com o `SHA256SUMS.txt` do lançamento e troca:
//!
//! - Windows instalado (há um `unins000.exe` ao lado): roda o instalador novo em silêncio (`/VERYSILENT`), que fecha
//!   este app, instala por cima e abre o novo (o `/ATUALIZAR=1` liga o "abrir" do `brittico.iss`).
//! - Windows portátil: o .exe em uso vira `.antigo` (o Windows deixa renomear um .exe aberto), o novo toma o lugar, abre.
//! - Linux pelo AppImage (`$APPIMAGE`): o arquivo novo substitui o antigo no mesmo caminho e abre.
//! - .deb, .rpm, pacote do Arch e o que mais for: quem atualiza é o gerenciador de pacotes; o botão abre a página.
//!
//! Nada é trocado se o download ou a conferência falhar. `BRITTICO_ATUALIZAR=<url>` troca a origem (testes): lá
//! `latest.json` responde como a API do GitHub e `download/<tag>/<arquivo>` são os arquivos.

use crate::net::{self, Tarefa};
use std::io::Read;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

const REPO: &str = "britto64/brittico-app";
pub const VERSAO: &str = env!("CARGO_PKG_VERSION");

fn origem() -> Option<String> {
    std::env::var("BRITTICO_ATUALIZAR").ok().map(|s| s.trim_end_matches('/').to_owned())
}

fn url_ultimo() -> String {
    match origem() {
        Some(o) => format!("{o}/latest.json"),
        None => format!("https://api.github.com/repos/{REPO}/releases/latest"),
    }
}

fn url_arquivo(tag: &str, nome: &str) -> String {
    match origem() {
        Some(o) => format!("{o}/download/{tag}/{nome}"),
        None => format!("https://github.com/{REPO}/releases/download/{tag}/{nome}"),
    }
}

/// A página de download do site, para onde o aviso leva.
pub fn pagina() -> String {
    "https://brittico.xyz/app/".to_owned()
}

/// Como este app está instalado (decide o que baixar e como trocar).
#[derive(Clone, Debug, PartialEq)]
pub enum Tipo {
    WinInstalado,
    WinPortatil(PathBuf),
    AppImage(PathBuf),
    /// Pacote do sistema ou compilado à mão: só avisa e abre a página.
    Outro,
}

pub fn tipo() -> Tipo {
    let exe = std::env::current_exe().unwrap_or_default();
    if cfg!(windows) {
        if exe.parent().is_some_and(|d| d.join("unins000.exe").exists()) {
            return Tipo::WinInstalado;
        }
        return Tipo::WinPortatil(exe);
    }
    match std::env::var_os("APPIMAGE").map(PathBuf::from) {
        Some(p) if p.is_file() => Tipo::AppImage(p),
        _ => Tipo::Outro,
    }
}

impl Tipo {
    fn arquivo(&self) -> Option<&'static str> {
        match self {
            Tipo::WinInstalado => Some("Brittico-Setup-x64.exe"),
            Tipo::WinPortatil(_) => Some("Brittico-portable-x64.exe"),
            Tipo::AppImage(_) => Some("Brittico-x86_64.AppImage"),
            Tipo::Outro => None,
        }
    }
    /// Dá para trocar sozinho (senão o botão só abre a página). Desligado em 2026-11-01, a pedido dele: sem assinatura
    /// digital, o Windows bloqueia o instalador que o app baixa e roda ("não foi possível confirmar quem assinou").
    /// O aviso leva à página de download do site. A troca sozinha continua aqui para quando houver assinatura
    /// (`BRITTICO_ATUALIZAR_SOZINHO=1` liga, para testes).
    pub fn sozinho(&self) -> bool {
        self.arquivo().is_some() && std::env::var("BRITTICO_ATUALIZAR_SOZINHO").is_ok_and(|v| v == "1")
    }
}

/// "1.2.3" (com ou sem "v") em números, para comparar.
fn numeros(v: &str) -> Vec<u64> {
    v.trim().trim_start_matches('v').split(['.', '-', '+']).take(3).map(|p| p.parse().unwrap_or(0)).collect()
}

pub fn mais_nova(outra: &str, atual: &str) -> bool {
    let (a, b) = (numeros(outra), numeros(atual));
    (0..3).map(|i| (a.get(i).copied().unwrap_or(0), b.get(i).copied().unwrap_or(0))).find(|(x, y)| x != y).is_some_and(|(x, y)| x > y)
}

fn agente(timeout: Option<Duration>) -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(timeout)
        .timeout_connect(Some(Duration::from_secs(15)))
        .tls_config(ureq::tls::TlsConfig::builder().provider(ureq::tls::TlsProvider::NativeTls).root_certs(ureq::tls::RootCerts::PlatformVerifier).build())
        .user_agent(concat!("BritticoApp/", env!("CARGO_PKG_VERSION")))
        .build()
        .into()
}

fn erro(msg: impl Into<String>) -> net::Erro {
    net::Erro { status: 0, msg: msg.into() }
}

/// A tag do último lançamento (`v1.0.1`), se for mais nova que esta.
fn checar() -> Result<Option<String>, net::Erro> {
    let mut r = agente(Some(Duration::from_secs(20))).get(&url_ultimo()).header("Accept", "application/vnd.github+json").call().map_err(|e| erro(e.to_string()))?;
    let v: serde_json::Value = r.body_mut().read_json().map_err(|e| erro(e.to_string()))?;
    let tag = v["tag_name"].as_str().unwrap_or("").to_owned();
    if v["draft"].as_bool() == Some(true) || v["prerelease"].as_bool() == Some(true) {
        return Ok(None);
    }
    Ok((!tag.is_empty() && mais_nova(&tag, VERSAO)).then_some(tag))
}

pub enum Fase {
    Nada,
    Disponivel(String),
    Baixando { tag: String, feito: Arc<AtomicU64>, total: Arc<AtomicU64> },
    /// Baixou e conferiu: fechando para trocar.
    Trocando,
    Erro { tag: String, msg: String },
}

pub struct Atualizador {
    pub fase: Fase,
    pub tipo: Tipo,
    checagem: Option<Tarefa<Option<String>>>,
    proxima: Instant,
    baixa: Option<Tarefa<()>>,
}

impl Default for Atualizador {
    fn default() -> Self {
        limpar_sobras();
        Atualizador { fase: Fase::Nada, tipo: tipo(), checagem: None, proxima: Instant::now() + Duration::from_secs(3), baixa: None }
    }
}

impl Atualizador {
    /// Uma vez por quadro.
    pub fn passo(&mut self, ctx: &eframe::egui::Context) {
        if self.checagem.is_none() && self.baixa.is_none() && Instant::now() >= self.proxima && matches!(self.fase, Fase::Nada | Fase::Disponivel(_)) {
            self.proxima = Instant::now() + Duration::from_secs(6 * 3600);
            self.checagem = Some(net::rodar(ctx, checar));
            // acorda quando for a hora da próxima
            let c = ctx.clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_secs(6 * 3600 + 1));
                c.request_repaint();
            });
        }
        if let Some(r) = self.checagem.as_ref().and_then(|t| t.pronto()) {
            self.checagem = None;
            match r {
                Ok(Some(tag)) => self.fase = Fase::Disponivel(tag),
                Ok(None) => {}
                // sem internet ou o GitHub recusou: tenta de novo daqui a meia hora, sem incomodar
                Err(e) => {
                    eprintln!("[atualizar] {}", e.msg);
                    self.proxima = Instant::now() + Duration::from_secs(1800);
                }
            }
        }
        if let Some(r) = self.baixa.as_ref().and_then(|t| t.pronto()) {
            self.baixa = None;
            let tag = match &self.fase {
                Fase::Baixando { tag, .. } => tag.clone(),
                _ => String::new(),
            };
            match r {
                Ok(()) => {
                    self.fase = Fase::Trocando;
                    // o novo já foi aberto (ou o instalador já está rodando): este sai
                    std::process::exit(0);
                }
                Err(e) => self.fase = Fase::Erro { tag, msg: e.msg },
            }
        }
        if matches!(self.fase, Fase::Baixando { .. }) {
            ctx.request_repaint_after(Duration::from_millis(200));
        }
    }

    /// O clique no aviso.
    pub fn atualizar(&mut self, ctx: &eframe::egui::Context) {
        let tag = match &self.fase {
            Fase::Disponivel(t) | Fase::Erro { tag: t, .. } => t.clone(),
            _ => return,
        };
        if !self.tipo.sozinho() {
            net::abrir_no_navegador(&pagina());
            return;
        }
        let (feito, total) = (Arc::new(AtomicU64::new(0)), Arc::new(AtomicU64::new(0)));
        let (f2, t2, tipo, tg) = (feito.clone(), total.clone(), self.tipo.clone(), tag.clone());
        self.baixa = Some(net::rodar(ctx, move || baixar_e_trocar(&tg, &tipo, &f2, &t2)));
        self.fase = Fase::Baixando { tag, feito, total };
    }
}

/// O `.antigo` que a troca do portátil deixa para trás (só some depois que o antigo fechou).
fn limpar_sobras() {
    if let Ok(exe) = std::env::current_exe() {
        let _ = std::fs::remove_file(exe.with_extension("exe.antigo"));
    }
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn baixar_e_trocar(tag: &str, tipo: &Tipo, feito: &AtomicU64, total: &AtomicU64) -> Result<(), net::Erro> {
    use sha2::{Digest, Sha256};
    let nome = tipo.arquivo().ok_or_else(|| erro("Este jeito de instalar não se atualiza sozinho."))?;
    // a soma esperada, do mesmo lançamento
    let mut r = agente(Some(Duration::from_secs(30))).get(&url_arquivo(tag, "SHA256SUMS.txt")).call().map_err(|e| erro(format!("Não deu para baixar a conferência ({e}).")))?;
    let somas = r.body_mut().read_to_string().map_err(|e| erro(e.to_string()))?;
    let esperada = somas
        .lines()
        .find_map(|l| {
            let mut it = l.split_whitespace();
            let (s, n) = (it.next()?, it.next()?);
            (n.trim_start_matches('*') == nome).then(|| s.to_lowercase())
        })
        .ok_or_else(|| erro("Este lançamento não tem o arquivo para este computador."))?;

    // onde o arquivo novo fica enquanto baixa: no Windows instalado, a pasta temporária; nos outros, ao lado do atual
    // (a troca no fim é um "renomear", que só é instantâneo dentro do mesmo disco)
    let destino_final = match tipo {
        Tipo::WinInstalado => std::env::temp_dir().join(format!("Brittico-Setup-{tag}.exe")),
        Tipo::WinPortatil(exe) => exe.with_extension("exe.novo"),
        Tipo::AppImage(p) => p.with_file_name(format!(".Brittico-{tag}.AppImage.novo")),
        Tipo::Outro => unreachable!(),
    };
    let mut resp = agente(None).get(&url_arquivo(tag, nome)).call().map_err(|e| erro(format!("Não deu para baixar a versão nova ({e}).")))?;
    if resp.status() != 200 {
        return Err(erro(format!("Não deu para baixar a versão nova (HTTP {}).", resp.status().as_u16())));
    }
    if let Some(n) = resp.headers().get("content-length").and_then(|v| v.to_str().ok()).and_then(|v| v.parse().ok()) {
        total.store(n, Ordering::Relaxed);
    }
    let mut leitor = resp.body_mut().with_config().limit(1 << 30).reader();
    let mut arq = std::fs::File::create(&destino_final).map_err(|e| erro(format!("Não deu para gravar a versão nova ({e}).")))?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 256 * 1024];
    loop {
        let n = leitor.read(&mut buf).map_err(|e| erro(format!("O download caiu ({e}).")))?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
        std::io::Write::write_all(&mut arq, &buf[..n]).map_err(|e| erro(format!("Não deu para gravar a versão nova ({e}).")))?;
        feito.fetch_add(n as u64, Ordering::Relaxed);
    }
    drop(arq);
    if hex(&h.finalize()) != esperada {
        let _ = std::fs::remove_file(&destino_final);
        return Err(erro("O arquivo baixado não bate com a conferência. Nada foi trocado."));
    }

    let args: Vec<String> = std::env::args().skip(1).filter(|a| a != "--minimizado").collect();
    match tipo {
        Tipo::WinInstalado => {
            std::process::Command::new(&destino_final)
                .args(["/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART", "/CLOSEAPPLICATIONS", "/ATUALIZAR=1"])
                .spawn()
                .map_err(|e| erro(format!("Não deu para abrir o instalador ({e}).")))?;
        }
        Tipo::WinPortatil(exe) => {
            let antigo = exe.with_extension("exe.antigo");
            let _ = std::fs::remove_file(&antigo);
            std::fs::rename(exe, &antigo).map_err(|e| erro(format!("Não deu para trocar o arquivo ({e}).")))?;
            if let Err(e) = std::fs::rename(&destino_final, exe) {
                let _ = std::fs::rename(&antigo, exe);
                return Err(erro(format!("Não deu para trocar o arquivo ({e}).")));
            }
            std::process::Command::new(exe).args(&args).spawn().map_err(|e| erro(format!("Trocou, mas não abriu a versão nova ({e}). Abra o Brittico de novo.")))?;
        }
        Tipo::AppImage(p) => {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(&destino_final, std::fs::Permissions::from_mode(0o755));
            }
            std::fs::rename(&destino_final, p).map_err(|e| erro(format!("Não deu para trocar o AppImage ({e}).")))?;
            // o novo não herda o ambiente do AppImage antigo (a pasta montada some quando este sair)
            let mut c = std::process::Command::new(p);
            c.args(&args);
            for k in ["APPIMAGE", "APPDIR", "ARGV0", "OWD", "LD_LIBRARY_PATH"] {
                c.env_remove(k);
            }
            // o AppImageLauncher (comum no Arch) pergunta "integrar ou rodar uma vez?" a cada AppImage que abre; a
            // atualização não é hora disso
            c.env("APPIMAGELAUNCHER_DISABLE", "1");
            c.spawn().map_err(|e| erro(format!("Trocou, mas não abriu a versão nova ({e}). Abra o Brittico de novo.")))?;
        }
        Tipo::Outro => {}
    }
    Ok(())
}

#[cfg(test)]
mod testes {
    use super::*;
    #[test]
    fn versoes() {
        assert!(mais_nova("v1.0.1", "1.0.0"));
        assert!(mais_nova("1.1.0", "1.0.9"));
        assert!(mais_nova("v2.0.0", "1.9.9"));
        assert!(!mais_nova("v1.0.0", "1.0.0"));
        assert!(!mais_nova("v0.9.9", "1.0.0"));
        assert!(!mais_nova("", "1.0.0"));
    }
}

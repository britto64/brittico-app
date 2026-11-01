//! A conversa com o servidor do Brittico (a mesma API do site, `api.brittico.xyz`).
//!
//! A sessão é a mesma do site: o cookie `kz_sess`, que o app guarda num arquivo da pasta do Brittico e manda no
//! cabeçalho `Cookie` como um navegador faria. Ele chega pelo login no navegador da pessoa (`entrar`): o app escuta numa
//! porta do próprio PC, abre o login do site com a volta apontando para `/api/auth/app`, e o servidor manda o navegador
//! de volta para essa porta com a sessão.
//!
//! Cada pedido roda numa thread à parte e devolve o resultado por um canal (`Tarefa`); quem pediu olha a cada quadro.
//! `BRITTICO_API` troca o servidor (o de teste do bot: `http://127.0.0.1:3098`).

use serde::de::DeserializeOwned;
use serde_json::Value;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, TryRecvError};
use std::sync::{OnceLock, RwLock};
use std::time::Duration;

pub const API_PADRAO: &str = "https://api.brittico.xyz";
pub const SITE: &str = "https://brittico.xyz";

pub fn api() -> &'static str {
    static API: OnceLock<String> = OnceLock::new();
    API.get_or_init(|| std::env::var("BRITTICO_API").map(|s| s.trim_end_matches('/').to_owned()).unwrap_or_else(|_| API_PADRAO.into()))
}

/// Endereço completo de um caminho da API, ou o próprio endereço se já é completo (`https://…`).
pub fn url(path: &str) -> String {
    if path.starts_with("http://") || path.starts_with("https://") {
        path.to_owned()
    } else {
        format!("{}{}", api(), path)
    }
}

/// O endereço WebSocket da API (`wss://…` ou `ws://…`).
pub fn ws_url(path: &str) -> String {
    let a = api();
    let base = a.strip_prefix("https://").map(|r| format!("wss://{r}")).or_else(|| a.strip_prefix("http://").map(|r| format!("ws://{r}"))).unwrap_or_else(|| a.to_owned());
    format!("{base}{path}")
}

/// A pasta do Brittico na máquina (a mesma família da do Olheiro).
pub fn dir() -> PathBuf {
    if let Ok(d) = std::env::var("BRITTICO_DIR") {
        return PathBuf::from(d);
    }
    #[cfg(windows)]
    {
        if let Ok(a) = std::env::var("APPDATA") {
            return PathBuf::from(a).join("Brittico");
        }
    }
    if let Ok(x) = std::env::var("XDG_CONFIG_HOME") {
        return PathBuf::from(x).join("brittico");
    }
    PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".into())).join(".config").join("brittico")
}

fn arquivo_sessao() -> PathBuf {
    // uma sessão por servidor: a de teste não pisa na de verdade
    if api() == API_PADRAO {
        dir().join("sessao")
    } else {
        dir().join("sessao-teste")
    }
}

static SESSAO: RwLock<Option<String>> = RwLock::new(None);

pub fn carregar_sessao() {
    let s = std::fs::read_to_string(arquivo_sessao()).ok().map(|s| s.trim().to_owned()).filter(|s| !s.is_empty());
    *SESSAO.write().unwrap() = s;
}

pub fn tem_sessao() -> bool {
    SESSAO.read().unwrap().is_some()
}

pub fn guardar_sessao(tok: Option<&str>) {
    *SESSAO.write().unwrap() = tok.map(str::to_owned);
    let f = arquivo_sessao();
    match tok {
        Some(t) => {
            let _ = std::fs::create_dir_all(dir());
            let _ = std::fs::write(&f, t);
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(&f, std::fs::Permissions::from_mode(0o600));
            }
        }
        None => {
            let _ = std::fs::remove_file(&f);
        }
    }
}

/// O cabeçalho `Cookie` dos pedidos (vazio sem sessão).
pub fn cookie() -> Option<String> {
    SESSAO.read().unwrap().as_ref().map(|t| format!("kz_sess={t}"))
}

fn agente() -> &'static ureq::Agent {
    static A: OnceLock<ureq::Agent> = OnceLock::new();
    A.get_or_init(|| {
        ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(20)))
            .tls_config(ureq::tls::TlsConfig::builder().provider(ureq::tls::TlsProvider::NativeTls).build())
            .http_status_as_error(false)
            .user_agent(concat!("BritticoApp/", env!("CARGO_PKG_VERSION")))
            .build()
            .into()
    })
}

/// O que deu errado num pedido, já em palavras para mostrar.
#[derive(Clone, Debug)]
pub struct Erro {
    pub status: u16,
    pub msg: String,
}

impl std::fmt::Display for Erro {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.msg)
    }
}

/// Faz um pedido agora (na thread de quem chamou). `corpo` vazio = sem corpo.
pub fn pedir_ja(metodo: &str, path: &str, corpo: Option<&Value>) -> Result<Value, Erro> {
    let u = url(path);
    let a = agente();
    let ck = cookie();
    macro_rules! hdr {
        ($b:expr) => {{
            let b = $b;
            match &ck {
                Some(c) => b.header("Cookie", c),
                None => b,
            }
        }};
    }
    let r = match (metodo, corpo) {
        ("GET", _) => hdr!(a.get(&u)).call(),
        ("DELETE", None) => hdr!(a.delete(&u)).call(),
        ("DELETE", Some(c)) => hdr!(a.delete(&u)).force_send_body().send_json(c),
        ("POST", c) => hdr!(a.post(&u)).send_json(c.cloned().unwrap_or(Value::Object(Default::default()))),
        ("PUT", c) => hdr!(a.put(&u)).send_json(c.cloned().unwrap_or(Value::Object(Default::default()))),
        ("PATCH", c) => hdr!(a.patch(&u)).send_json(c.cloned().unwrap_or(Value::Object(Default::default()))),
        _ => return Err(Erro { status: 0, msg: format!("método {metodo}") }),
    };
    let mut resp = r.map_err(|e| Erro { status: 0, msg: sem_rede(&e.to_string()) })?;
    let status = resp.status().as_u16();
    // o servidor pode trocar a sessão (duas contas que viraram uma): guarda a nova
    for v in resp.headers().get_all("set-cookie") {
        if let Ok(s) = v.to_str() {
            if let Some(rest) = s.strip_prefix("kz_sess=") {
                let tok = rest.split(';').next().unwrap_or("");
                let tok = percent_decode(tok);
                guardar_sessao(if tok.is_empty() { None } else { Some(&tok) });
            }
        }
    }
    let texto = resp.body_mut().read_to_string().unwrap_or_default();
    let v: Value = serde_json::from_str(&texto).unwrap_or(Value::Null);
    if status >= 400 {
        let msg = v.get("error").and_then(Value::as_str).map(str::to_owned).unwrap_or_else(|| format!("O servidor respondeu {status}."));
        return Err(Erro { status, msg });
    }
    Ok(v)
}

fn sem_rede(e: &str) -> String {
    format!("Sem conexão com o Brittico ({})", e.chars().take(80).collect::<String>())
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 3 <= b.len() {
            if let Ok(v) = u8::from_str_radix(std::str::from_utf8(&b[i + 1..i + 3]).unwrap_or(""), 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Codifica um pedaço de URL (`encodeURIComponent`).
pub fn enc(s: &str) -> String {
    let mut o = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&b) {
            o.push(b as char);
        } else {
            o.push_str(&format!("%{b:02X}"));
        }
    }
    o
}

/// Um pedido em andamento.
pub struct Tarefa<T>(Receiver<Result<T, Erro>>);

impl<T> Tarefa<T> {
    /// O resultado, se já chegou (só uma vez).
    pub fn pronto(&self) -> Option<Result<T, Erro>> {
        match self.0.try_recv() {
            Ok(r) => Some(r),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(Err(Erro { status: 0, msg: "O pedido se perdeu.".into() })),
        }
    }
}

/// Roda `f` numa thread e devolve a tarefa; quando acabar, pede um quadro novo.
pub fn rodar<T: Send + 'static>(ctx: &eframe::egui::Context, f: impl FnOnce() -> Result<T, Erro> + Send + 'static) -> Tarefa<T> {
    let (tx, rx) = channel();
    let ctx = ctx.clone();
    std::thread::spawn(move || {
        let _ = tx.send(f());
        ctx.request_repaint();
    });
    Tarefa(rx)
}

/// Um pedido JSON em segundo plano, já convertido para `T`.
pub fn pedir<T: DeserializeOwned + Send + 'static>(ctx: &eframe::egui::Context, metodo: &'static str, path: impl Into<String>, corpo: Option<Value>) -> Tarefa<T> {
    let path = path.into();
    rodar(ctx, move || {
        let v = pedir_ja(metodo, &path, corpo.as_ref())?;
        serde_json::from_value(v).map_err(|e| Erro { status: 0, msg: format!("Resposta inesperada do servidor ({e}).") })
    })
}

pub fn get<T: DeserializeOwned + Send + 'static>(ctx: &eframe::egui::Context, path: impl Into<String>) -> Tarefa<T> {
    pedir(ctx, "GET", path, None)
}

/// Abre um endereço no navegador da pessoa.
pub fn abrir_no_navegador(u: &str) {
    // para os testes: outro "navegador" (ex.: `curl -sL -c /tmp/cj -b /tmp/cj -o /dev/null`), sem abrir o da pessoa
    if let Ok(cmd) = std::env::var("BRITTICO_NAVEGADOR") {
        let mut it = cmd.split_whitespace();
        if let Some(prog) = it.next() {
            let _ = std::process::Command::new(prog).args(it).arg(u).spawn();
            return;
        }
    }
    #[cfg(target_os = "windows")]
    let r = std::process::Command::new("rundll32").args(["url.dll,FileProtocolHandler", u]).spawn();
    #[cfg(target_os = "macos")]
    let r = std::process::Command::new("open").arg(u).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let r = std::process::Command::new("xdg-open").arg(u).spawn();
    if let Err(e) = r {
        eprintln!("não deu para abrir {u}: {e}");
    }
}

/* ================= entrar ================= */

/// Um login em andamento: a porta do PC em que o app espera a volta do navegador.
pub struct Login {
    pub plataforma: &'static str,
    pub tarefa: Tarefa<String>,
    pub url: String,
}

/// Começa o login: escuta numa porta livre do 127.0.0.1, abre o navegador no login da plataforma e espera a volta.
/// A tarefa termina com a sessão (já guardada) ou com o erro.
pub fn entrar(ctx: &eframe::egui::Context, plataforma: &'static str) -> Result<Login, Erro> {
    let ouvinte = std::net::TcpListener::bind("127.0.0.1:0").map_err(|e| Erro { status: 0, msg: format!("Não deu para abrir a porta do login ({e}).") })?;
    let port = ouvinte.local_addr().map(|a| a.port()).unwrap_or(0);
    let state = estado_aleatorio();
    let volta = format!("{}/api/auth/app?port={port}&state={state}", api());
    let u = if api() == API_PADRAO {
        format!("{}/api/auth/{plataforma}/login?return={}", api(), enc(&volta))
    } else {
        // o servidor de teste não fala com a Twitch: o "site" dele põe a sessão de uma pessoa de teste
        format!("{}/dev/entrar?return={}", api(), enc(&volta))
    };
    abrir_no_navegador(&u);
    let st = state.clone();
    let tarefa = rodar(ctx, move || esperar_volta(ouvinte, &st));
    Ok(Login { plataforma, tarefa, url: u })
}

fn estado_aleatorio() -> String {
    use std::hash::{BuildHasher, Hasher};
    let mut s = String::new();
    for i in 0..4u64 {
        let mut h = std::collections::hash_map::RandomState::new().build_hasher();
        h.write_u64(i ^ std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(0));
        h.write_u32(std::process::id());
        s.push_str(&format!("{:016x}", h.finish()));
    }
    s
}

fn esperar_volta(ouvinte: std::net::TcpListener, state: &str) -> Result<String, Erro> {
    // dez minutos para a pessoa terminar de entrar; depois disso a porta fecha
    let _ = ouvinte.set_nonblocking(true);
    let fim = std::time::Instant::now() + Duration::from_secs(600);
    loop {
        if std::time::Instant::now() > fim {
            return Err(Erro { status: 0, msg: "O login demorou demais. Tente de novo.".into() });
        }
        let (mut s, _) = match ouvinte.accept() {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(150));
                continue;
            }
            Err(e) => return Err(Erro { status: 0, msg: e.to_string() }),
        };
        let _ = s.set_nonblocking(false);
        let _ = s.set_read_timeout(Some(Duration::from_secs(5)));
        let mut buf = [0u8; 8192];
        let n = s.read(&mut buf).unwrap_or(0);
        let req = String::from_utf8_lossy(&buf[..n]);
        let linha = req.lines().next().unwrap_or("");
        let alvo = linha.split_whitespace().nth(1).unwrap_or("");
        if !alvo.starts_with("/entrou") {
            let _ = s.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
            continue;
        }
        let q: std::collections::HashMap<String, String> = alvo
            .split_once('?')
            .map(|(_, q)| q.split('&').filter_map(|kv| kv.split_once('=')).map(|(k, v)| (k.to_owned(), percent_decode(v))).collect())
            .unwrap_or_default();
        let ok = q.get("state").map(String::as_str) == Some(state) && q.contains_key("sessao");
        let pagina = |titulo: &str, txt: &str| {
            format!(
                "<!doctype html><meta charset=utf-8><title>Brittico</title><body style=\"font-family:system-ui,sans-serif;background:#0b0b0b;color:#eee;display:grid;place-items:center;height:100vh;margin:0\"><div style=\"text-align:center\"><h1 style=\"font-size:22px\">{titulo}</h1><p style=\"color:#aaa\">{txt}</p></div><script>setTimeout(function(){{window.close()}},800)</script>"
            )
        };
        let html = if ok { pagina("Pronto, você entrou.", "Pode fechar esta aba e voltar ao Brittico.") } else { pagina("Não deu para entrar.", "Volte ao Brittico e tente de novo.") };
        let _ = s.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{html}", html.len()).as_bytes());
        let _ = s.flush();
        if q.get("state").map(String::as_str) != Some(state) {
            continue;
        }
        return match q.get("sessao") {
            Some(tok) if !tok.is_empty() => {
                guardar_sessao(Some(tok));
                Ok(tok.clone())
            }
            _ => Err(Erro { status: 401, msg: "O navegador voltou sem a sessão. Tente de novo.".into() }),
        };
    }
}

/* ================= quem sou eu ================= */

/// O `/api/me`: quem está usando o app.
#[derive(Clone, Debug, Default, serde::Deserialize)]
#[serde(default)]
pub struct Me {
    #[serde(rename = "loggedIn")]
    pub logged_in: bool,
    pub user: MeUser,
    #[serde(rename = "profileId")]
    pub profile_id: i64,
    #[serde(rename = "livepixelId")]
    pub livepixel_id: Option<String>,
    #[serde(rename = "isModerator")]
    pub is_mod: bool,
    pub perms: Vec<String>,
    pub roles: Vec<MeRole>,
}

#[derive(Clone, Debug, Default, serde::Deserialize)]
#[serde(default)]
pub struct MeUser {
    pub id: String,
    pub login: String,
    pub display_name: String,
    pub avatar: String,
    pub platform: Option<String>,
}

#[derive(Clone, Debug, Default, serde::Deserialize)]
#[serde(default)]
pub struct MeRole {
    pub key: String,
    pub name: String,
    pub color: String,
}

impl Me {
    pub fn nome(&self) -> &str {
        if self.user.display_name.is_empty() {
            &self.user.login
        } else {
            &self.user.display_name
        }
    }
}

/// Uma imagem da internet (avatar, foto de post) como fonte de imagem do egui; `None` se não tem endereço.
pub fn imagem(u: &str) -> Option<eframe::egui::ImageSource<'static>> {
    let u = u.trim();
    if u.is_empty() {
        return None;
    }
    let full = if u.starts_with("http://") || u.starts_with("https://") {
        u.to_owned()
    } else if u.starts_with('/') {
        if u.starts_with("/api/") {
            url(u)
        } else {
            format!("{SITE}{u}")
        }
    } else {
        format!("{}/api/feed/images/{u}", api())
    };
    Some(eframe::egui::ImageSource::Uri(full.into()))
}

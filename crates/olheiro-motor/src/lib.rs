//! O Olheiro sem janela, para morar dentro de outro programa (o Brittico).
//!
//! Lê o LoL e o minimapa (o mesmo laço do app antigo, `session.rs`), entra sozinho na sala de voz da partida pelo porteiro,
//! grava e toca a voz e ajusta o volume de cada colega pela distância, calculado aqui mesmo (`olheiro-voz`). Não fala com bot,
//! site nem conta nenhuma: o Riot ID vem do jogo.
//!
//! Quem desenha a tela só faz duas coisas: lê o [`Estado`] e manda [`Comando`]s. O motor roda numa thread própria e chama
//! `acordar` quando algo que a tela mostra mudou.

pub mod ajustes;
mod capture;
mod debug;
pub mod demo;
mod icons;
mod logfile;
pub mod lol;
mod lolapi;
mod motor;
mod net;
pub mod sala;
pub mod session;
pub mod som;

pub use ajustes::{Ajustes, Lado};
pub use motor::{Comando, Estado, Fase, Jogador, Leitura, Motor, Ouvido, Partida};
pub use olheiro_core::sim::Art;
pub use olheiro_voz::{CHEIO_ATE, ZERO_EM};
pub use sala::Conexao;

pub const VERSAO: &str = env!("CARGO_PKG_VERSION");

/// Milissegundos desde a época Unix (o relógio dos pacotes de posição).
pub fn agora_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// Onde fica o registro (para o botão "abrir a pasta").
pub fn pasta() -> std::path::PathBuf {
    ajustes::dir()
}

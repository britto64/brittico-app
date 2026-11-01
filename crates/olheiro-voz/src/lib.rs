//! O que cada Olheiro calcula sozinho, para todos concordarem sem um bot no meio.
//!
//! Nada aqui abre conexão, toca áudio ou carrega segredo. O porteiro (no servidor) é quem assina a entrada
//! na sala. Este crate só diz como a posição viaja, como duas leituras do mesmo jogador viram uma, e qual
//! volume sai da distância.

mod fusao;
mod pacote;
mod volume;

pub use fusao::Fusao;
pub use pacote::{codificar, decodificar, Pacote, Ponto};
pub use volume::{distancia, ganho, CHEIO_ATE, ZERO_EM};

/// Leitura mais velha que isto é descartada. "Mais de 5 s": no instante exato ainda vale.
pub const VELHO_MS: u64 = 5_000;

/// Quantas posições por segundo o plano pede. O canal é sem garantia: perder uma não importa.
pub const HZ: u32 = 5;

/// A chave com que as posições se encontram. A mesma conta do Riot ID no `olheiro-core`: NFKC, sem espaços nas pontas, minúsculas.
pub fn chave(riot_id: &str) -> String {
    use unicode_normalization::UnicodeNormalization;
    riot_id.trim().nfkc().collect::<String>().to_lowercase()
}

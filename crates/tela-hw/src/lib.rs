//! H.264 da tela por hardware no Windows, pelos MFTs do Media Foundation (`MFTEnumEx` com o flag de
//! hardware: NVIDIA, AMD e Intel). Fora do Windows não há codificador: a Call segue no VP8 de software.
//!
//! A amostra sai em Annex B, com SPS e PPS em cada quadro chave, para o LiveKit publicar sem reencodar.

#[cfg_attr(not(windows), allow(dead_code))]
mod bitstream;

#[cfg(windows)]
mod mf;

pub use bitstream::{bitrate_alvo, media_para, Acesso};

#[cfg(windows)]
pub use mf::{fornecedor_gpu, tem_hardware, Codificador};

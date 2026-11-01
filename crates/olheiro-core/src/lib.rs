//! O núcleo do Olheiro.
//!
//! Recebe o desenho do minimapa do LoL (um quadro de pixels) e o elenco da partida, e diz onde cada aliado
//! está no mapa. Não conhece Windows, rede nem janela: tudo aqui roda e se testa em qualquer sistema, que é o
//! que permite desenvolver no Arch sem um Windows à mão.
//!
//! O método é o do Camera mode do LoLProxChat (referência estudada em `referencia/`): cada ícone é achado pela
//! cor do anel, nomeado por correlação com o ícone base do campeão, acompanhado de quadro em quadro, e o que
//! sobra se resolve por eliminação. A diferença é que aqui o alvo são TODOS os aliados, não "onde estou eu".

pub mod blobs;
pub mod frame;
pub mod geometry;
pub mod identify;
pub mod identity;
pub mod protocol;
pub mod rings;
pub mod sim;
pub mod templates;
pub mod tracker;

pub use frame::Frame;
pub use geometry::{MapKind, MinimapSide, Rect};
pub use tracker::{How, Placement, Scout, ScoutPlayer};

/// Um jogador do elenco, pelo número dele no elenco da partida (a ordem em que o jogo os lista).
pub type PlayerId = usize;

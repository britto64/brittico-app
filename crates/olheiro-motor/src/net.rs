//! Os dois clientes HTTP do app, que não podem se misturar:
//!
//!  - `local`: fala com o jogo (`https://127.0.0.1:2999`) e com o cliente do LoL. Os dois usam certificado próprio da Riot e só
//!    atendem em 127.0.0.1, então não há o que validar. NUNCA usar para o Brittico nem para o Community Dragon.
//!  - `web`: fala com o Brittico e com o Community Dragon, com a validação de certificado normal, feita pelo repositório de certificados
//!    do SISTEMA (no Windows, o do Windows). Não usar a lista de raízes embutida do Mozilla, que é o padrão do `ureq`: com o Schannel
//!    ela é a ÚNICA raiz aceita, e a cadeia da Cloudflare/Google (`GTS Root R4` cruzada pela `GlobalSign Root CA`) termina numa raiz
//!    que o Windows conhece e essa lista não tem. O resultado era "unable to find any user-specified roots in the final cert chain".

use std::time::Duration;
use ureq::tls::{RootCerts, TlsConfig, TlsProvider};

pub fn local(timeout: Duration) -> ureq::Agent {
    let tls = TlsConfig::builder().provider(TlsProvider::NativeTls).disable_verification(true).build();
    ureq::Agent::config_builder().tls_config(tls).timeout_global(Some(timeout)).build().into()
}

/// `status_as_error(false)`: o pareamento responde 428 ("ainda não confirmou") e 410 ("venceu") de propósito, e quem chama decide.
pub fn web(timeout: Duration) -> ureq::Agent {
    let tls = TlsConfig::builder().provider(TlsProvider::NativeTls).root_certs(RootCerts::PlatformVerifier).build();
    ureq::Agent::config_builder().tls_config(tls).timeout_global(Some(timeout)).http_status_as_error(false).build().into()
}

/// Corpo de uma resposta como texto, com um teto (uma resposta absurda não pode encher a memória).
pub fn read_text(mut resp: ureq::http::Response<ureq::Body>, max: u64) -> Option<String> {
    resp.body_mut().with_config().limit(max).read_to_string().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// O padrão do `ureq` (raízes do Mozilla embutidas) quebra o pareamento no Windows; ver o comentário do módulo.
    #[test]
    fn o_cliente_da_web_confia_nos_certificados_do_sistema() {
        let agent = web(Duration::from_secs(1));
        assert!(matches!(agent.config().tls_config().root_certs(), RootCerts::PlatformVerifier));
        assert!(!agent.config().tls_config().disable_verification());
    }

    #[test]
    fn o_cliente_local_nao_valida_porque_so_fala_com_o_proprio_pc() {
        let agent = local(Duration::from_secs(1));
        assert!(agent.config().tls_config().disable_verification());
    }
}

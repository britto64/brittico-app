//! Unidades de acesso H.264 em Annex B, que é o que o LiveKit espera num quadro pré-codificado.
//!
//! O MFT de hardware tanto pode devolver Annex B como NALs com o tamanho na frente (AVCC). O cabeçalho
//! de sequência (`MF_MT_MPEG_SEQUENCE_HEADER`) diz qual dos dois, e num quadro chave o SPS e o PPS vão
//! junto para quem entra no meio da call conseguir decodificar.

/// Um quadro já em Annex B.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Acesso {
    pub bytes: Vec<u8>,
    pub chave: bool,
}

/// Converte a amostra do codificador para uma unidade de acesso Annex B.
///
/// `cabecalho` é o blob `MF_MT_MPEG_SEQUENCE_HEADER` (Annex B ou AVCDecoderConfigurationRecord).
/// `limpo` é o `MFSampleExtension_CleanPoint` da amostra. Devolve `None` se a amostra não fecha
/// um quadro inteiro.
pub fn acesso_annex_b(amostra: &[u8], cabecalho: &[u8], limpo: bool) -> Option<Acesso> {
    let (sps_pps, comprimento) = interpretar_cabecalho(cabecalho);
    let mut bytes = para_annex_b(amostra, comprimento)?;
    if bytes.is_empty() {
        return None;
    }
    let idr = tem_tipo(&bytes, 5);
    let chave = idr || limpo;
    if chave && !tem_tipo(&bytes, 7) && !sps_pps.is_empty() {
        let mut com = sps_pps;
        com.append(&mut bytes);
        bytes = com;
    }
    Some(Acesso { bytes, chave })
}

/// SPS/PPS em Annex B e, se o cabeçalho for AVCC, quantos bytes cada NAL usa para o tamanho (1, 2 ou 4).
/// `0` quer dizer que a saída do codificador já é Annex B.
pub fn interpretar_cabecalho(cabecalho: &[u8]) -> (Vec<u8>, usize) {
    if cabecalho.is_empty() {
        return (Vec::new(), 0);
    }
    if parece_annex_b(cabecalho) {
        return (cabecalho.to_vec(), 0);
    }
    // AVCDecoderConfigurationRecord: versão 1, e o tamanho do NAL em `lengthSizeMinusOne`.
    if cabecalho.first() != Some(&1) || cabecalho.len() < 7 {
        return (Vec::new(), 0);
    }
    let comprimento = ((cabecalho[4] & 0x03) as usize) + 1;
    let mut out = Vec::new();
    let mut i = 6;
    let n_sps = (cabecalho[5] & 0x1f) as usize;
    for _ in 0..n_sps {
        if !empurrar_nal(cabecalho, &mut i, &mut out) {
            break;
        }
    }
    if i < cabecalho.len() {
        let n_pps = cabecalho[i] as usize;
        i += 1;
        for _ in 0..n_pps {
            if !empurrar_nal(cabecalho, &mut i, &mut out) {
                break;
            }
        }
    }
    (out, comprimento)
}

fn empurrar_nal(dados: &[u8], i: &mut usize, out: &mut Vec<u8>) -> bool {
    if *i + 2 > dados.len() {
        return false;
    }
    let len = u16::from_be_bytes([dados[*i], dados[*i + 1]]) as usize;
    *i += 2;
    if len == 0 || *i + len > dados.len() {
        return false;
    }
    out.extend_from_slice(&[0, 0, 0, 1]);
    out.extend_from_slice(&dados[*i..*i + len]);
    *i += len;
    true
}

fn para_annex_b(amostra: &[u8], comprimento: usize) -> Option<Vec<u8>> {
    if comprimento > 0 {
        if let Some(bytes) = avcc_se_consome(amostra, comprimento) {
            return Some(bytes);
        }
        // há MFT que anuncia AVCC e mesmo assim manda Annex B
        if parece_annex_b(amostra) {
            return Some(amostra.to_vec());
        }
        return None;
    }
    if parece_annex_b(amostra) {
        return Some(amostra.to_vec());
    }
    avcc_se_consome(amostra, 4)
}

/// AVCC só vale quando os prefixos de tamanho cobrem a amostra inteira e cada NAL tem o bit
/// proibido em zero. É o que separa um NAL curto (tamanho `00 00 01 xx`, que parece start code)
/// de um Annex B de verdade.
fn avcc_se_consome(amostra: &[u8], comprimento: usize) -> Option<Vec<u8>> {
    if !(1..=4).contains(&comprimento) || amostra.len() < comprimento + 1 {
        return None;
    }
    let mut i = 0;
    let mut out = Vec::with_capacity(amostra.len() + 4);
    let mut nals = 0u32;
    while i + comprimento <= amostra.len() {
        let mut len = 0usize;
        for b in &amostra[i..i + comprimento] {
            len = (len << 8) | *b as usize;
        }
        i += comprimento;
        if len == 0 || i + len > amostra.len() {
            return None;
        }
        if amostra[i] & 0x80 != 0 {
            return None;
        }
        out.extend_from_slice(&[0, 0, 0, 1]);
        out.extend_from_slice(&amostra[i..i + len]);
        i += len;
        nals += 1;
    }
    (i == amostra.len() && nals > 0).then_some(out)
}

pub fn parece_annex_b(dados: &[u8]) -> bool {
    dados.starts_with(&[0, 0, 0, 1]) || dados.starts_with(&[0, 0, 1])
}

fn tem_tipo(annex_b: &[u8], tipo: u8) -> bool {
    nals(annex_b).any(|nal| nal.first().is_some_and(|b| b & 0x1f == tipo))
}

/// Os NALs de um Annex B, sem o start code e sem o `00` extra de um start code de 4 bytes.
fn nals(annex_b: &[u8]) -> impl Iterator<Item = &[u8]> {
    let inicios = inicios_de_nal(annex_b);
    let mut k = 0;
    std::iter::from_fn(move || {
        let s = *inicios.get(k)?;
        let fim = match inicios.get(k + 1) {
            Some(&p) if p >= 4 && annex_b.get(p - 4..p) == Some(&[0, 0, 0, 1]) => p - 4,
            Some(&p) if p >= 3 && annex_b.get(p - 3..p) == Some(&[0, 0, 1]) => p - 3,
            Some(&p) => p,
            None => annex_b.len(),
        };
        k += 1;
        (s < fim).then_some(&annex_b[s..fim])
    })
}

fn inicios_de_nal(dados: &[u8]) -> Vec<usize> {
    let mut out = Vec::new();
    let mut i = 0;
    while i + 3 <= dados.len() {
        if i + 4 <= dados.len() && dados[i..i + 4] == [0, 0, 0, 1] {
            out.push(i + 4);
            i += 4;
            continue;
        }
        if dados[i..i + 3] == [0, 0, 1] {
            out.push(i + 3);
            i += 3;
            continue;
        }
        i += 1;
    }
    out
}

/// O alvo de bitrate nunca passa do teto (o da Call, 12 Mbps) e não desce a zero quando a rede pede.
pub fn bitrate_alvo(pedido: u64, teto: u64) -> u64 {
    let teto = teto.max(150_000);
    pedido.clamp(150_000, teto)
}

/// A média e o teto da taxa variável: o teto é o pedido da rede (sem passar do da Call) e a média nunca passa do teto.
pub fn taxas(media: u64, pedido: u64, teto: u64) -> (u64, u64) {
    let maximo = bitrate_alvo(pedido, teto);
    (media.clamp(150_000, maximo), maximo)
}

/// A média que uma imagem boa precisa em H.264 de placa de vídeo, pela altura da tela (720p: 6 Mbps; 1080p: 9 Mbps).
pub fn media_para(altura: u32, teto: u64) -> u64 {
    let m = if altura > 720 { 9_000_000 } else { 6_000_000 };
    m.min(teto)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cabecalho_avcc() -> Vec<u8> {
        // versão, profile, compat, level, lengthSizeMinusOne = 3 → 4 bytes; 1 SPS de 3 bytes; 1 PPS de 2
        vec![0x01, 0x42, 0x00, 0x1f, 0xff, 0xe1, 0x00, 0x03, 0x67, 0x64, 0x00, 0x01, 0x00, 0x02, 0x68, 0xce]
    }

    #[test]
    fn avcc_de_chave_ganha_sps_pps_em_annex_b() {
        // NAL de 300 bytes: o tamanho `00 00 01 2c` parece um start code de 3 bytes, e mesmo assim é AVCC
        let mut amostra = vec![0x00, 0x00, 0x01, 0x2c, 0x65];
        amostra.extend(std::iter::repeat(0x11).take(299));
        let acesso = acesso_annex_b(&amostra, &cabecalho_avcc(), false).unwrap();
        assert!(acesso.chave);
        assert!(acesso.bytes.starts_with(&[0, 0, 0, 1, 0x67, 0x64, 0x00, 0, 0, 0, 1, 0x68, 0xce, 0, 0, 0, 1, 0x65]));
        assert_eq!(acesso.bytes.len(), 4 + 3 + 4 + 2 + 4 + 300);
    }

    #[test]
    fn start_code_de_3_bytes_conta_como_idr() {
        let amostra = [0u8, 0, 1, 0x67, 0x42, 0x00, 0, 0, 1, 0x65, 0x88];
        let acesso = acesso_annex_b(&amostra, &[], false).unwrap();
        assert!(acesso.chave);
        assert_eq!(acesso.bytes, amostra);
    }

    #[test]
    fn annex_b_que_ja_tem_sps_nao_duplica() {
        let amostra = [0u8, 0, 0, 1, 0x67, 0x64, 0x00, 0, 0, 0, 1, 0x68, 0xce, 0, 0, 0, 1, 0x65, 0x88];
        let acesso = acesso_annex_b(&amostra, &amostra[..13], true).unwrap();
        assert_eq!(acesso.bytes, amostra);
        assert!(acesso.chave);
    }

    #[test]
    fn delta_avcc_nao_leva_sps() {
        let amostra = [0x00, 0x00, 0x00, 0x02, 0x41, 0x9a];
        let acesso = acesso_annex_b(&amostra, &cabecalho_avcc(), false).unwrap();
        assert!(!acesso.chave);
        assert_eq!(acesso.bytes, [0, 0, 0, 1, 0x41, 0x9a]);
    }

    #[test]
    fn ponto_limpo_sem_idr_ainda_e_chave_e_leva_sps() {
        let amostra = [0x00, 0x00, 0x00, 0x02, 0x41, 0x9a];
        let acesso = acesso_annex_b(&amostra, &cabecalho_avcc(), true).unwrap();
        assert!(acesso.chave);
        assert!(acesso.bytes.windows(5).any(|w| w == [0, 0, 0, 1, 0x67]));
    }

    #[test]
    fn taxa_variavel_media_abaixo_do_teto() {
        assert_eq!(taxas(6_000_000, 12_000_000, 12_000_000), (6_000_000, 12_000_000));
        // a rede pede 4 Mbps: o teto desce para 4 e a média também
        assert_eq!(taxas(6_000_000, 4_000_000, 12_000_000), (4_000_000, 4_000_000));
        // pedir mais que o teto da Call não passa dele
        assert_eq!(taxas(6_000_000, 30_000_000, 12_000_000), (6_000_000, 12_000_000));
        assert_eq!(media_para(720, 12_000_000), 6_000_000);
        assert_eq!(media_para(1080, 12_000_000), 9_000_000);
    }

    #[test]
    fn bitrate_respeita_o_teto_de_12_mbps() {
        assert_eq!(bitrate_alvo(20_000_000, 12_000_000), 12_000_000);
        assert_eq!(bitrate_alvo(5_000_000, 12_000_000), 5_000_000);
        assert_eq!(bitrate_alvo(0, 12_000_000), 150_000);
        assert_eq!(bitrate_alvo(12_000_000, 12_000_000), 12_000_000);
    }
}

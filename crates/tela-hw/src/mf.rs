//! Codificador H.264 síncrono por fora, em cima do MFT assíncrono de hardware.
//!
//! O MFT de hardware pede entrada (`METransformNeedInput`) e avisa saída (`METransformHaveOutput`).
//! Taxa variável com teto (`PeakConstrainedVBR`): a média é o que uma imagem boa precisa (6 Mbps em 720p) e o
//! teto é o da Call (12 Mbps), usado só quando a cena mexe muito. Quando a rede pede menos (o controle de banda do
//! LiveKit), o teto desce para o pedido e a média nunca passa dele.

use std::mem::ManuallyDrop;
use std::ptr;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use windows::core::{Interface, GUID};
use windows::Win32::Foundation::VARIANT_TRUE;
use windows::Win32::Graphics::Dxgi::{CreateDXGIFactory1, IDXGIFactory1};
use windows::Win32::Media::MediaFoundation::{
    eAVEncCommonRateControlMode_PeakConstrainedVBR, CODECAPI_AVEncCommonMaxBitRate, CODECAPI_AVEncCommonMeanBitRate,
    CODECAPI_AVEncCommonRateControlMode, CODECAPI_AVEncMPVDefaultBPictureCount, CODECAPI_AVEncMPVGOPSize,
    CODECAPI_AVEncVideoForceKeyFrame, CODECAPI_AVLowLatencyMode, ICodecAPI, IMFActivate, IMFMediaEventGenerator,
    IMFSample, IMFTransform, MEError, METransformHaveOutput, METransformNeedInput, MFCreateMediaType,
    MFCreateMemoryBuffer, MFCreateSample, MFMediaType_Video, MFSampleExtension_CleanPoint, MFStartup, MFTEnumEx,
    MFT_CATEGORY_VIDEO_ENCODER, MFT_ENUM_FLAG, MFT_ENUM_FLAG_ASYNCMFT, MFT_ENUM_FLAG_HARDWARE,
    MFT_ENUM_FLAG_SORTANDFILTER, MFT_ENUM_FLAG_SYNCMFT, MFT_FRIENDLY_NAME_Attribute, MFT_MESSAGE_COMMAND_FLUSH,
    MFT_MESSAGE_NOTIFY_BEGIN_STREAMING, MFT_MESSAGE_NOTIFY_END_OF_STREAM, MFT_MESSAGE_NOTIFY_END_STREAMING,
    MFT_MESSAGE_NOTIFY_START_OF_STREAM, MFT_OUTPUT_DATA_BUFFER, MFT_OUTPUT_STREAM_CAN_PROVIDE_SAMPLES,
    MFT_OUTPUT_STREAM_PROVIDES_SAMPLES, MFT_REGISTER_TYPE_INFO, MFVideoFormat_H264, MFVideoFormat_NV12,
    MFVideoInterlace_Progressive, MFSTARTUP_NOSOCKET, MF_EVENT_FLAG_NO_WAIT, MF_E_NO_EVENTS_AVAILABLE,
    MF_E_TRANSFORM_NEED_MORE_INPUT, MF_E_TRANSFORM_STREAM_CHANGE, MF_LOW_LATENCY, MF_MT_AVG_BITRATE,
    MF_MT_DEFAULT_STRIDE, MF_MT_FRAME_RATE, MF_MT_FRAME_SIZE, MF_MT_INTERLACE_MODE, MF_MT_MAJOR_TYPE,
    MF_MT_MPEG2_LEVEL, MF_MT_MPEG2_PROFILE, MF_MT_MPEG_SEQUENCE_HEADER, MF_MT_PIXEL_ASPECT_RATIO, MF_MT_SUBTYPE,
    MF_TRANSFORM_ASYNC, MF_TRANSFORM_ASYNC_UNLOCK, MF_VERSION,
};
use windows::Win32::System::Com::{CoInitializeEx, CoTaskMemFree, COINIT_MULTITHREADED};
use windows::Win32::System::Variant::{VARIANT, VARIANT_0_0, VARIANT_0_0_0, VT_BOOL, VT_UI4};

use crate::bitstream::{acesso_annex_b, bitrate_alvo, taxas, Acesso};

const ESPERA_ENTRADA: Duration = Duration::from_millis(800);
const FOLGA_SAIDA: Duration = Duration::from_millis(15);
const FOLGA_PRIMEIRO: Duration = Duration::from_millis(400);

/// Perfis na ordem em que a gente tenta: baseline com a restrição do WebRTC, baseline, main e high.
/// O high é o último recurso (nem todo assinante de software decodifica), mas ainda é hardware.
const PERFIS: [u32; 4] = [256, 66, 77, 100];

pub struct Codificador {
    nome: String,
    largura: u32,
    altura: u32,
    fps: u32,
    /// A média para uma imagem boa, o teto de agora (o pedido da rede, sem passar do teto da Call) e o da Call.
    media: u64,
    bitrate: u64,
    teto: u64,
    transform: IMFTransform,
    eventos: Option<IMFMediaEventGenerator>,
    codec_api: Option<ICodecAPI>,
    fornece_amostras: bool,
    tamanho_saida: u32,
    id_entrada: u32,
    id_saida: u32,
    creditos: u32,
    pedir_chave: bool,
    quadros: u64,
    ja_saiu: bool,
    cabecalho: Vec<u8>,
}

// Os ponteiros COM ficam nesta thread (a da captura). O MFT de hardware é free-threaded.
unsafe impl Send for Codificador {}

impl Codificador {
    /// Abre o primeiro MFT H.264 de hardware que aceitar NV12 neste tamanho.
    /// Em bits por segundo: `media` é a média da taxa variável, `bitrate` o teto pedido pela rede (sem passar de
    /// `teto`, o da Call).
    pub fn abrir(largura: u32, altura: u32, fps: u32, media: u64, bitrate: u64, teto: u64) -> Result<Self, String> {
        if largura < 2 || altura < 2 || largura % 2 != 0 || altura % 2 != 0 {
            return Err(format!("tamanho inválido para NV12 ({largura}x{altura})"));
        }
        if fps == 0 {
            return Err("fps zerado".into());
        }
        if !iniciar_mf() {
            return Err("o Media Foundation não iniciou".into());
        }
        let bitrate = bitrate_alvo(bitrate, teto);
        let mut ultimo = String::from("nenhum codificador H.264 de hardware aceitou o formato");
        for ativar in enumerar()? {
            let nome = nome_amigavel(&ativar).unwrap_or_else(|| "MFT H.264".into());
            match montar(&ativar, nome, largura, altura, fps, media, bitrate, teto.max(bitrate)) {
                Ok(c) => return Ok(c),
                Err(e) => ultimo = e,
            }
        }
        Err(ultimo)
    }

    pub fn nome(&self) -> &str {
        &self.nome
    }

    pub fn largura(&self) -> u32 {
        self.largura
    }

    pub fn altura(&self) -> u32 {
        self.altura
    }

    /// Troca a resolução reabrindo o MFT. O próximo quadro sai como chave.
    pub fn redimensionar(&mut self, largura: u32, altura: u32) -> Result<(), String> {
        if largura == self.largura && altura == self.altura {
            return Ok(());
        }
        let mut novo = Self::abrir(largura, altura, self.fps, self.media, self.bitrate, self.teto)?;
        novo.pedir_chave();
        *self = novo;
        Ok(())
    }

    /// O pedido do controle de banda, em bits por segundo: vira o teto de agora (sem passar do da Call), e a média
    /// desce junto se ficar acima dele.
    pub fn definir_bitrate(&mut self, pedido: u64) {
        let alvo = bitrate_alvo(pedido, self.teto);
        if alvo == self.bitrate {
            return;
        }
        self.bitrate = alvo;
        if let Some(api) = &self.codec_api {
            let (media, maximo) = taxas(self.media, alvo, self.teto);
            definir_u32(api, &CODECAPI_AVEncCommonMeanBitRate, media as u32);
            definir_u32(api, &CODECAPI_AVEncCommonMaxBitRate, maximo as u32);
        }
    }

    /// O teto de agora (o pedido da rede, sem passar do da Call), para reabrir com ele.
    pub fn bitrate(&self) -> u64 {
        self.bitrate
    }

    pub fn pedir_chave(&mut self) {
        self.pedir_chave = true;
    }

    /// Codifica um NV12 empacotado (Y e depois UV, stride = largura). Pode devolver vazio quando o
    /// MFT ainda está enchendo o pipeline; a saída atrasada sai na chamada seguinte.
    pub fn codificar(&mut self, nv12: &[u8]) -> Result<Vec<Acesso>, String> {
        let esperado = (self.largura as usize) * (self.altura as usize) * 3 / 2;
        if nv12.len() != esperado {
            return Err(format!("NV12 com {} bytes; esperava {esperado} ({}x{})", nv12.len(), self.largura, self.altura));
        }
        let amostra = self.amostra_entrada(nv12)?;
        if self.pedir_chave {
            if let Some(api) = &self.codec_api {
                definir_u32(api, &CODECAPI_AVEncVideoForceKeyFrame, 1);
            }
            self.pedir_chave = false;
        }
        let mut saida = Vec::new();
        let transform = self.transform.clone();
        if let Some(eventos) = self.eventos.clone() {
            self.codificar_async(&transform, &eventos, amostra, &mut saida)?;
        } else {
            self.codificar_sync(&transform, amostra, &mut saida)?;
        }
        if !saida.is_empty() {
            self.ja_saiu = true;
        }
        self.quadros += 1;
        Ok(saida)
    }

    fn amostra_entrada(&self, nv12: &[u8]) -> Result<IMFSample, String> {
        unsafe {
            let buffer = MFCreateMemoryBuffer(nv12.len() as u32).map_err(|e| format!("buffer de entrada: {e}"))?;
            let mut ptr = std::ptr::null_mut();
            buffer.Lock(&mut ptr, None, None).map_err(|e| format!("travar o buffer: {e}"))?;
            if !ptr.is_null() {
                ptr::copy_nonoverlapping(nv12.as_ptr(), ptr, nv12.len());
            }
            buffer.Unlock().map_err(|e| format!("soltar o buffer: {e}"))?;
            buffer.SetCurrentLength(nv12.len() as u32).map_err(|e| format!("tamanho do buffer: {e}"))?;
            let amostra = MFCreateSample().map_err(|e| format!("amostra: {e}"))?;
            amostra.AddBuffer(&buffer).map_err(|e| format!("anexar o buffer: {e}"))?;
            let fps = self.fps.max(1) as i64;
            let duracao = 10_000_000 / fps;
            amostra
                .SetSampleTime(self.quadros as i64 * duracao)
                .map_err(|e| format!("tempo da amostra: {e}"))?;
            amostra.SetSampleDuration(duracao).map_err(|e| format!("duração da amostra: {e}"))?;
            Ok(amostra)
        }
    }

    fn codificar_async(&mut self, transform: &IMFTransform, eventos: &IMFMediaEventGenerator, amostra: IMFSample, saida: &mut Vec<Acesso>) -> Result<(), String> {
        let prazo = Instant::now() + ESPERA_ENTRADA;
        loop {
            if self.creditos > 0 {
                self.creditos -= 1;
                unsafe {
                    transform
                        .ProcessInput(self.id_entrada, &amostra, 0)
                        .map_err(|e| format!("ProcessInput: {e}"))?;
                }
                break;
            }
            match self.puxar_evento(eventos, transform, saida)? {
                Puxado::Teve => {}
                Puxado::Vazio => {
                    if Instant::now() >= prazo {
                        return Err("o codificador não pediu outro quadro a tempo".into());
                    }
                    std::thread::sleep(Duration::from_millis(1));
                }
            }
        }
        let folga = if self.ja_saiu { FOLGA_SAIDA } else { FOLGA_PRIMEIRO };
        let prazo = Instant::now() + folga;
        let mut saiu = !saida.is_empty();
        loop {
            let antes = saida.len();
            match self.puxar_evento(eventos, transform, saida)? {
                Puxado::Teve => saiu |= saida.len() > antes,
                Puxado::Vazio => {
                    if saiu || Instant::now() >= prazo {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(1));
                }
            }
        }
        Ok(())
    }

    fn codificar_sync(&mut self, transform: &IMFTransform, amostra: IMFSample, saida: &mut Vec<Acesso>) -> Result<(), String> {
        unsafe {
            transform
                .ProcessInput(self.id_entrada, &amostra, 0)
                .map_err(|e| format!("ProcessInput: {e}"))?;
        }
        while self.ler_saida(transform, saida)? {}
        Ok(())
    }

    fn puxar_evento(&mut self, eventos: &IMFMediaEventGenerator, transform: &IMFTransform, saida: &mut Vec<Acesso>) -> Result<Puxado, String> {
        let evento = unsafe { eventos.GetEvent(MF_EVENT_FLAG_NO_WAIT) };
        let evento = match evento {
            Ok(ev) => ev,
            Err(e) if e.code() == MF_E_NO_EVENTS_AVAILABLE => return Ok(Puxado::Vazio),
            Err(e) => return Err(format!("evento do codificador: {e}")),
        };
        let tipo = unsafe { evento.GetType() }.map_err(|e| format!("tipo do evento: {e}"))?;
        if tipo == METransformNeedInput.0 as u32 {
            self.creditos += 1;
        } else if tipo == METransformHaveOutput.0 as u32 {
            self.ler_saida(transform, saida)?;
        } else if tipo == MEError.0 as u32 {
            return Err("o codificador de hardware sinalizou um erro".into());
        }
        Ok(Puxado::Teve)
    }

    fn ler_saida(&mut self, transform: &IMFTransform, saida: &mut Vec<Acesso>) -> Result<bool, String> {
        unsafe {
            loop {
                let fornecida = if self.fornece_amostras { None } else { Some(self.alocar_saida()?) };
                let mut buffers = [MFT_OUTPUT_DATA_BUFFER {
                    dwStreamID: self.id_saida,
                    pSample: ManuallyDrop::new(fornecida),
                    dwStatus: 0,
                    pEvents: ManuallyDrop::new(None),
                }];
                let mut status = 0u32;
                let resultado = transform.ProcessOutput(0, &mut buffers, &mut status);
                let amostra = ManuallyDrop::take(&mut buffers[0].pSample);
                let _eventos = ManuallyDrop::take(&mut buffers[0].pEvents);
                match resultado {
                    Ok(()) => {
                        if let Some(amostra) = amostra {
                            if let Some(acesso) = self.acesso_da_amostra(&amostra)? {
                                saida.push(acesso);
                            }
                        }
                        return Ok(true);
                    }
                    Err(e) if e.code() == MF_E_TRANSFORM_NEED_MORE_INPUT => return Ok(false),
                    Err(e) if e.code() == MF_E_TRANSFORM_STREAM_CHANGE => {
                        let novo = transform
                            .GetOutputAvailableType(self.id_saida, 0)
                            .map_err(|e| format!("tipo de saída novo: {e}"))?;
                        transform
                            .SetOutputType(self.id_saida, &novo, 0)
                            .map_err(|e| format!("aceitar o tipo de saída: {e}"))?;
                        if let Ok(info) = transform.GetOutputStreamInfo(self.id_saida) {
                            self.atualizar_info(&info);
                        }
                        self.cabecalho = ler_cabecalho(transform, self.id_saida);
                        continue;
                    }
                    Err(e) => return Err(format!("ProcessOutput: {e}")),
                }
            }
        }
    }

    fn atualizar_info(&mut self, info: &windows::Win32::Media::MediaFoundation::MFT_OUTPUT_STREAM_INFO) {
        self.fornece_amostras = info.dwFlags & (MFT_OUTPUT_STREAM_PROVIDES_SAMPLES.0 as u32 | MFT_OUTPUT_STREAM_CAN_PROVIDE_SAMPLES.0 as u32) != 0;
        self.tamanho_saida = info.cbSize;
    }

    fn alocar_saida(&self) -> Result<IMFSample, String> {
        unsafe {
            let tam = if self.tamanho_saida > 0 {
                self.tamanho_saida
            } else {
                self.largura * self.altura * 3 / 2
            };
            let buffer = MFCreateMemoryBuffer(tam).map_err(|e| format!("buffer de saída: {e}"))?;
            let amostra = MFCreateSample().map_err(|e| format!("amostra de saída: {e}"))?;
            amostra.AddBuffer(&buffer).map_err(|e| format!("anexar a saída: {e}"))?;
            Ok(amostra)
        }
    }

    fn acesso_da_amostra(&self, amostra: &IMFSample) -> Result<Option<Acesso>, String> {
        unsafe {
            let limpo = amostra.GetUINT32(&MFSampleExtension_CleanPoint).unwrap_or(0) != 0;
            let buffer = amostra.ConvertToContiguousBuffer().map_err(|e| format!("buffer contíguo: {e}"))?;
            let mut ptr = std::ptr::null_mut();
            let mut tam = 0u32;
            buffer.Lock(&mut ptr, None, Some(&mut tam)).map_err(|e| format!("ler a saída: {e}"))?;
            let acesso = if !ptr.is_null() && tam > 0 {
                let bytes = std::slice::from_raw_parts(ptr, tam as usize);
                acesso_annex_b(bytes, &self.cabecalho, limpo)
            } else {
                None
            };
            buffer.Unlock().map_err(|e| format!("soltar a saída: {e}"))?;
            Ok(acesso)
        }
    }

    fn fechar(&mut self) {
        unsafe {
            let _ = self.transform.ProcessMessage(MFT_MESSAGE_NOTIFY_END_OF_STREAM, self.id_entrada as usize);
            let _ = self.transform.ProcessMessage(MFT_MESSAGE_NOTIFY_END_STREAMING, 0);
            let _ = self.transform.ProcessMessage(MFT_MESSAGE_COMMAND_FLUSH, 0);
        }
        self.eventos = None;
        self.codec_api = None;
        self.creditos = 0;
    }
}

impl Drop for Codificador {
    fn drop(&mut self) {
        self.fechar();
    }
}

enum Puxado {
    Teve,
    Vazio,
}

/// Há um MFT H.264 marcado como hardware. É isto que decide se a tela tenta a GPU; o fornecedor
/// da placa (DXGI) só entra no registro.
pub fn tem_hardware() -> bool {
    static CACHE: OnceLock<bool> = OnceLock::new();
    *CACHE.get_or_init(|| {
        if !iniciar_mf() {
            return false;
        }
        enumerar().is_ok_and(|v| !v.is_empty())
    })
}

/// Fornecedor da primeira placa que não é adaptador de software, para o registro.
pub fn fornecedor_gpu() -> Option<&'static str> {
    if !iniciar_mf() {
        return None;
    }
    unsafe {
        let fabrica: IDXGIFactory1 = CreateDXGIFactory1().ok()?;
        let mut i = 0u32;
        let mut outro = false;
        while let Ok(adaptador) = fabrica.EnumAdapters1(i) {
            i += 1;
            let Ok(desc) = adaptador.GetDesc1() else { continue };
            match desc.VendorId {
                0x10DE => return Some("NVIDIA"),
                0x1002 | 0x1022 => return Some("AMD"),
                0x8086 => return Some("Intel"),
                _ => outro = true,
            }
        }
        outro.then_some("outro")
    }
}

fn iniciar_mf() -> bool {
    static OK: OnceLock<bool> = OnceLock::new();
    *OK.get_or_init(|| unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        MFStartup(MF_VERSION, MFSTARTUP_NOSOCKET).is_ok()
    })
}

fn enumerar() -> Result<Vec<IMFActivate>, String> {
    let mut lista = enumerar_com(MFT_ENUM_FLAG_HARDWARE.0 | MFT_ENUM_FLAG_SORTANDFILTER.0 | MFT_ENUM_FLAG_ASYNCMFT.0)?;
    if lista.is_empty() {
        lista = enumerar_com(MFT_ENUM_FLAG_HARDWARE.0 | MFT_ENUM_FLAG_SORTANDFILTER.0 | MFT_ENUM_FLAG_SYNCMFT.0)?;
    }
    Ok(lista)
}

fn enumerar_com(flags: i32) -> Result<Vec<IMFActivate>, String> {
    unsafe {
        let saida = MFT_REGISTER_TYPE_INFO { guidMajorType: MFMediaType_Video, guidSubtype: MFVideoFormat_H264 };
        let mut ativos: *mut Option<IMFActivate> = ptr::null_mut();
        let mut n = 0u32;
        MFTEnumEx(
            MFT_CATEGORY_VIDEO_ENCODER,
            MFT_ENUM_FLAG(flags),
            None,
            Some(&saida),
            &mut ativos,
            &mut n,
        )
        .map_err(|e| format!("MFTEnumEx: {e}"))?;
        let mut lista = Vec::with_capacity(n as usize);
        if !ativos.is_null() {
            for i in 0..n as usize {
                if let Some(ativar) = (*ativos.add(i)).take() {
                    lista.push(ativar);
                }
            }
            CoTaskMemFree(Some(ativos as *const _));
        }
        Ok(lista)
    }
}

fn nome_amigavel(ativar: &IMFActivate) -> Option<String> {
    unsafe {
        let mut valor = windows::core::PWSTR::null();
        let mut tam = 0u32;
        ativar.GetAllocatedString(&MFT_FRIENDLY_NAME_Attribute, &mut valor, &mut tam).ok()?;
        if valor.is_null() {
            return None;
        }
        let nome = valor.to_string().ok();
        CoTaskMemFree(Some(valor.as_ptr() as *const _));
        nome
    }
}

#[allow(clippy::too_many_arguments)]
fn montar(ativar: &IMFActivate, nome: String, largura: u32, altura: u32, fps: u32, media: u64, bitrate: u64, teto: u64) -> Result<Codificador, String> {
    let (media_ef, maximo) = taxas(media, bitrate, teto);
    unsafe {
        let transform: IMFTransform = ativar.ActivateObject().map_err(|e| format!("{nome}: não ativou ({e})"))?;
        let mut assincrono = false;
        if let Ok(attrs) = transform.GetAttributes() {
            assincrono = attrs.GetUINT32(&MF_TRANSFORM_ASYNC).unwrap_or(0) == 1;
            if assincrono {
                attrs.SetUINT32(&MF_TRANSFORM_ASYNC_UNLOCK, 1).map_err(|e| format!("{nome}: não destrancou ({e})"))?;
            }
            let _ = attrs.SetUINT32(&MF_LOW_LATENCY, 1);
        }
        let mut ids_e = [0u32; 1];
        let mut ids_s = [0u32; 1];
        let (id_entrada, id_saida) = match transform.GetStreamIDs(&mut ids_e, &mut ids_s) {
            Ok(()) => (ids_e[0], ids_s[0]),
            Err(_) => (0, 0),
        };
        let codec_api: Option<ICodecAPI> = transform.cast().ok();
        if let Some(api) = &codec_api {
            definir_u32(api, &CODECAPI_AVEncCommonRateControlMode, eAVEncCommonRateControlMode_PeakConstrainedVBR.0 as u32);
            definir_u32(api, &CODECAPI_AVEncCommonMeanBitRate, media_ef as u32);
            definir_u32(api, &CODECAPI_AVEncCommonMaxBitRate, maximo as u32);
            definir_bool(api, &CODECAPI_AVLowLatencyMode, true);
            definir_u32(api, &CODECAPI_AVEncMPVGOPSize, fps.saturating_mul(2).max(1));
            definir_u32(api, &CODECAPI_AVEncMPVDefaultBPictureCount, 0);
        }
        let mut erro = None;
        for perfil in PERFIS {
            match tipos(&transform, id_entrada, id_saida, largura, altura, fps, media_ef as u32, perfil) {
                Ok(()) => {
                    erro = None;
                    break;
                }
                Err(e) => erro = Some(e),
            }
        }
        if let Some(e) = erro {
            return Err(format!("{nome}: {e}"));
        }
        let info = transform.GetOutputStreamInfo(id_saida).map_err(|e| format!("{nome}: saída ({e})"))?;
        let fornece = info.dwFlags & (MFT_OUTPUT_STREAM_PROVIDES_SAMPLES.0 as u32 | MFT_OUTPUT_STREAM_CAN_PROVIDE_SAMPLES.0 as u32) != 0;
        let eventos = if assincrono {
            Some(transform.cast::<IMFMediaEventGenerator>().map_err(|e| format!("{nome}: sem eventos ({e})"))?)
        } else {
            None
        };
        transform
            .ProcessMessage(MFT_MESSAGE_NOTIFY_BEGIN_STREAMING, 0)
            .and_then(|()| transform.ProcessMessage(MFT_MESSAGE_NOTIFY_START_OF_STREAM, 0))
            .map_err(|e| format!("{nome}: não começou ({e})"))?;
        let cabecalho = ler_cabecalho(&transform, id_saida);
        Ok(Codificador {
            nome,
            largura,
            altura,
            fps,
            media,
            bitrate,
            teto,
            transform,
            eventos,
            codec_api,
            fornece_amostras: fornece,
            tamanho_saida: info.cbSize,
            id_entrada,
            id_saida,
            creditos: 0,
            pedir_chave: true,
            quadros: 0,
            ja_saiu: false,
            cabecalho,
        })
    }
}

fn tipos(transform: &IMFTransform, id_entrada: u32, id_saida: u32, largura: u32, altura: u32, fps: u32, bitrate: u32, perfil: u32) -> Result<(), String> {
    unsafe {
        let saida = MFCreateMediaType().map_err(|e| format!("tipo de saída: {e}"))?;
        saida
            .SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)
            .and_then(|()| saida.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_H264))
            .and_then(|()| saida.SetUINT32(&MF_MT_AVG_BITRATE, bitrate))
            .and_then(|()| saida.SetUINT64(&MF_MT_FRAME_SIZE, par_u32(largura, altura)))
            .and_then(|()| saida.SetUINT64(&MF_MT_FRAME_RATE, par_u32(fps, 1)))
            .and_then(|()| saida.SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, par_u32(1, 1)))
            .and_then(|()| saida.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32))
            .and_then(|()| saida.SetUINT32(&MF_MT_MPEG2_PROFILE, perfil))
            .and_then(|()| saida.SetUINT32(&MF_MT_MPEG2_LEVEL, nivel(largura, altura)))
            .map_err(|e| format!("montar H.264: {e}"))?;
        transform.SetOutputType(id_saida, &saida, 0).map_err(|e| format!("SetOutputType (perfil {perfil}): {e}"))?;
        let entrada = MFCreateMediaType().map_err(|e| format!("tipo de entrada: {e}"))?;
        entrada
            .SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)
            .and_then(|()| entrada.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_NV12))
            .and_then(|()| entrada.SetUINT64(&MF_MT_FRAME_SIZE, par_u32(largura, altura)))
            .and_then(|()| entrada.SetUINT64(&MF_MT_FRAME_RATE, par_u32(fps, 1)))
            .and_then(|()| entrada.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32))
            .and_then(|()| entrada.SetUINT32(&MF_MT_DEFAULT_STRIDE, largura))
            .map_err(|e| format!("montar NV12: {e}"))?;
        transform.SetInputType(id_entrada, &entrada, 0).map_err(|e| format!("SetInputType: {e}"))?;
        Ok(())
    }
}

fn ler_cabecalho(transform: &IMFTransform, id_saida: u32) -> Vec<u8> {
    unsafe {
        let Ok(tipo) = transform.GetOutputCurrentType(id_saida) else { return Vec::new() };
        let Ok(tam) = tipo.GetBlobSize(&MF_MT_MPEG_SEQUENCE_HEADER) else { return Vec::new() };
        if tam == 0 {
            return Vec::new();
        }
        let mut buf = vec![0u8; tam as usize];
        let mut escrito = 0u32;
        if tipo.GetBlob(&MF_MT_MPEG_SEQUENCE_HEADER, &mut buf, Some(&mut escrito)).is_err() {
            return Vec::new();
        }
        buf.truncate(escrito as usize);
        buf
    }
}

fn nivel(largura: u32, altura: u32) -> u32 {
    let mb = ((largura + 15) / 16) * ((altura + 15) / 16);
    if largura <= 1280 && altura <= 720 && mb <= 3600 {
        31
    } else if mb <= 8192 {
        42
    } else {
        51
    }
}

fn par_u32(alto: u32, baixo: u32) -> u64 {
    ((alto as u64) << 32) | baixo as u64
}

fn definir_u32(api: &ICodecAPI, guid: &GUID, valor: u32) {
    let mut var = VARIANT::default();
    var.Anonymous.Anonymous = ManuallyDrop::new(VARIANT_0_0 {
        vt: VT_UI4,
        wReserved1: 0,
        wReserved2: 0,
        wReserved3: 0,
        Anonymous: VARIANT_0_0_0 { ulVal: valor },
    });
    unsafe {
        let _ = api.SetValue(guid, &var);
    }
}

fn definir_bool(api: &ICodecAPI, guid: &GUID, valor: bool) {
    let mut var = VARIANT::default();
    var.Anonymous.Anonymous = ManuallyDrop::new(VARIANT_0_0 {
        vt: VT_BOOL,
        wReserved1: 0,
        wReserved2: 0,
        wReserved3: 0,
        Anonymous: VARIANT_0_0_0 {
            boolVal: if valor { VARIANT_TRUE } else { windows::Win32::Foundation::VARIANT_FALSE },
        },
    });
    unsafe {
        let _ = api.SetValue(guid, &var);
    }
}

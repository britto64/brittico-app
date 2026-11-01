//! O som do PC, sem o LiveKit tocar em nada do sistema:
//!
//!  - **microfone** (cpal) → mono a 48 kHz → limpeza do WebRTC (eco, ruído, ganho automático) em quadros de 10 ms → quem
//!    estiver ouvindo (a sala, ou ninguém no teste do microfone);
//!  - **saída**: a mistura de quem fala, cada um com o próprio volume (o da distância vezes o que a pessoa escolheu), que sobe e
//!    desce suave, → o fone (cpal). O que sai também volta para a limpeza, para ela tirar o eco de quem joga sem fone.
//!
//! O LiveKit não deixa mudar o volume de cada pessoa quando ele mesmo toca o som; por isso a mistura é nossa.

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

/// A taxa e o tamanho do quadro com que tudo anda por dentro (o que o WebRTC pede: 10 ms).
pub const TAXA: u32 = 48_000;
pub const QUADRO: usize = (TAXA / 100) as usize;

/// Antes de tocar alguém, junta isto (absorve o tremor da rede sem atrasar demais).
const PRE: usize = QUADRO * 6;
/// Mais que isto na fila de alguém e o começo é jogado fora: atraso acumulado é pior que um estalo.
const TETO: usize = QUADRO * 20;
/// O volume corre atrás do alvo com esta constante de tempo (a distância muda 5 vezes por segundo; o ouvido não pode sentir degrau).
const SUAVE_S: f32 = 0.25;

/// Um `f32` dividido entre threads (o nível do microfone, o ganho escolhido).
#[derive(Default)]
pub struct F32(AtomicU32);

impl F32 {
    pub fn new(v: f32) -> Self {
        F32(AtomicU32::new(v.to_bits()))
    }
    pub fn get(&self) -> f32 {
        f32::from_bits(self.0.load(Ordering::Relaxed))
    }
    pub fn set(&self, v: f32) {
        self.0.store(v.to_bits(), Ordering::Relaxed)
    }
}

/// Quem está falando na sala: a fila do que chegou e o volume que está valendo.
struct Fala {
    fila: VecDeque<f32>,
    tocando: bool,
    ganho: f32,
    alvo: f32,
    /// O quanto chegou de som por último (0 a 1), antes do volume: é o "está falando" da tela.
    nivel: f32,
}

impl Fala {
    fn nova() -> Fala {
        // nasce no volume 1: se a posição ainda não chegou, a falha não silencia (o alvo certo vem no próximo passo)
        Fala { fila: VecDeque::with_capacity(TETO), tocando: false, ganho: 1.0, alvo: 1.0, nivel: 0.0 }
    }
}

/// A mistura. Cada pessoa da sala tem uma `Fala`; a saída puxa 10 ms de cada vez.
#[derive(Default)]
pub struct Mistura {
    falas: HashMap<String, Fala>,
    pub surdo: bool,
    pub volume_geral: f32,
}

pub type MisturaRef = Arc<Mutex<Mistura>>;

impl Mistura {
    pub fn nova() -> MisturaRef {
        Arc::new(Mutex::new(Mistura { volume_geral: 1.0, ..Default::default() }))
    }

    /// Chegou som de `quem` (48 kHz, mono).
    pub fn chegou(&mut self, quem: &str, amostras: &[i16]) {
        let f = self.falas.entry(quem.to_string()).or_insert_with(Fala::nova);
        let mut soma = 0.0f32;
        for &s in amostras {
            let v = s as f32 / 32768.0;
            soma += v * v;
            f.fila.push_back(v);
        }
        if !amostras.is_empty() {
            let rms = (soma / amostras.len() as f32).sqrt();
            f.nivel = f.nivel.max(rms);
        }
        if f.fila.len() > TETO {
            let sobra = f.fila.len() - PRE;
            f.fila.drain(..sobra);
        }
    }

    pub fn saiu(&mut self, quem: &str) {
        self.falas.remove(quem);
    }

    pub fn limpar(&mut self) {
        self.falas.clear();
    }

    /// O volume que `quem` deve ter agora (0 a 2). Quem ainda não falou guarda o alvo para quando falar.
    pub fn alvo(&mut self, quem: &str, alvo: f32) {
        self.falas.entry(quem.to_string()).or_insert_with(Fala::nova).alvo = alvo.clamp(0.0, 2.0);
    }

    /// `(quem, nível do que chega, volume que está valendo)`.
    pub fn quadro(&self) -> Vec<(String, f32, f32)> {
        self.falas.iter().map(|(k, f)| (k.clone(), f.nivel, f.ganho)).collect()
    }

    /// 10 ms da mistura, a 48 kHz.
    pub fn misturar(&mut self, saida: &mut [f32; QUADRO]) {
        saida.fill(0.0);
        let passo = 1.0 - (-1.0 / (TAXA as f32 * SUAVE_S)).exp();
        let geral = if self.surdo { 0.0 } else { self.volume_geral };
        for f in self.falas.values_mut() {
            f.nivel *= 0.85;
            if !f.tocando {
                if f.fila.len() >= PRE {
                    f.tocando = true;
                } else {
                    // enquanto junta, o volume já vai indo para o alvo (senão o primeiro som sairia no volume velho)
                    f.ganho = f.alvo;
                    continue;
                }
            }
            if f.fila.len() < QUADRO {
                // acabou o que tinha: para e junta de novo (um buraco curto em vez de picotar por muito tempo)
                f.tocando = false;
                continue;
            }
            for s in saida.iter_mut() {
                f.ganho += (f.alvo - f.ganho) * passo;
                *s += f.fila.pop_front().unwrap_or(0.0) * f.ganho * geral;
            }
        }
        for s in saida.iter_mut() {
            // sem estourar quando dois falam alto ao mesmo tempo
            *s = s.tanh();
        }
    }
}

/// Os nomes dos microfones e das saídas que o sistema tem.
pub fn aparelhos() -> (Vec<String>, Vec<String>) {
    let host = cpal::default_host();
    let ent = host.input_devices().map(|d| d.filter_map(|d| d.name().ok()).collect()).unwrap_or_default();
    let sai = host.output_devices().map(|d| d.filter_map(|d| d.name().ok()).collect()).unwrap_or_default();
    (ent, sai)
}

fn achar(entrada: bool, nome: &str) -> Option<cpal::Device> {
    let host = cpal::default_host();
    if !nome.is_empty() {
        let lista = if entrada { host.input_devices().ok() } else { host.output_devices().ok() };
        if let Some(d) = lista.and_then(|mut l| l.find(|d| d.name().is_ok_and(|n| n == nome))) {
            return Some(d);
        }
    }
    if entrada { host.default_input_device() } else { host.default_output_device() }
}

/// Para onde vai cada quadro limpo do microfone (10 ms, 48 kHz, mono). `None`: ninguém (só o medidor anda).
pub type Destino = Arc<Mutex<Option<Box<dyn FnMut(&[i16]) + Send>>>>;

/// O que a tela lê e controla do som sem esperar ninguém.
pub struct Controles {
    pub nivel_mic: F32,
    pub ganho_mic: F32,
    pub mudo: AtomicBool,
    pub destino: Destino,
}

impl Controles {
    pub fn novos() -> Arc<Controles> {
        Arc::new(Controles { nivel_mic: F32::new(0.0), ganho_mic: F32::new(1.0), mudo: AtomicBool::new(false), destino: Arc::new(Mutex::new(None)) })
    }
}

/// O som ligado: as duas pontas abertas numa thread própria (o `Stream` do cpal não pode mudar de thread). Soltar fecha tudo.
pub struct Som {
    parar: Option<SyncSender<()>>,
    fio: Option<std::thread::JoinHandle<()>>,
    pub entrada: String,
    pub saida: String,
}

impl Drop for Som {
    fn drop(&mut self) {
        self.parar.take();
        if let Some(f) = self.fio.take() {
            let _ = f.join();
        }
    }
}

impl Som {
    /// Abre o microfone e (com `mistura`) a saída. Devolve os nomes de verdade dos aparelhos abertos, ou o motivo da falha.
    pub fn abrir(microfone: &str, saida: &str, mistura: Option<MisturaRef>, ctl: Arc<Controles>) -> Result<Som, String> {
        // para testar e capturar sem tocar no som de ninguém: `OLHEIRO_SOM=nao` (nenhum aparelho) ou `tom` (o "microfone" é um tom)
        match std::env::var("OLHEIRO_SOM").as_deref() {
            Ok("nao") => return Ok(Som::falso(false, mistura, ctl)),
            Ok("tom") => return Ok(Som::falso(true, mistura, ctl)),
            _ => {}
        }
        let (pronto_tx, pronto_rx) = sync_channel::<Result<(String, String), String>>(1);
        let (parar_tx, parar_rx) = sync_channel::<()>(1);
        let (mic, sai) = (microfone.to_string(), saida.to_string());
        let fio = std::thread::Builder::new()
            .name("olheiro-som".into())
            .spawn(move || {
                // o eco que sai pelo fone volta para a limpeza do microfone por aqui
                let (eco_tx, eco_rx) = sync_channel::<Vec<i16>>(64);
                let saida = match &mistura {
                    Some(m) => match abrir_saida(&sai, m.clone(), eco_tx) {
                        Ok(s) => Some(s),
                        Err(e) => {
                            let _ = pronto_tx.send(Err(e));
                            return;
                        }
                    },
                    None => None,
                };
                let entrada = match abrir_microfone(&mic, ctl, eco_rx) {
                    Ok(s) => s,
                    Err(e) => {
                        let _ = pronto_tx.send(Err(e));
                        return;
                    }
                };
                let nomes = (entrada.1.clone(), saida.as_ref().map(|s| s.1.clone()).unwrap_or_default());
                let _ = pronto_tx.send(Ok(nomes));
                // fica aqui até alguém soltar o `Som`
                let _ = parar_rx.recv();
                drop(entrada);
                drop(saida);
            })
            .map_err(|e| e.to_string())?;
        match pronto_rx.recv_timeout(Duration::from_secs(10)) {
            Ok(Ok((entrada, saida))) => Ok(Som { parar: Some(parar_tx), fio: Some(fio), entrada, saida }),
            Ok(Err(e)) => {
                let _ = fio.join();
                Err(e)
            }
            Err(_) => Err("O som do computador não respondeu.".into()),
        }
    }
}

impl Som {
    /// Sem aparelho nenhum: um relógio de 10 ms puxa a mistura (para os volumes e o "está falando" andarem) e, com `tom`, fala um
    /// tom de 440 Hz um segundo sim, um segundo não.
    fn falso(tom: bool, mistura: Option<MisturaRef>, ctl: Arc<Controles>) -> Som {
        let (parar_tx, parar_rx) = sync_channel::<()>(1);
        let fio = std::thread::spawn(move || {
            let mut bloco = [0.0f32; QUADRO];
            let mut n = 0usize;
            while let Err(std::sync::mpsc::RecvTimeoutError::Timeout) = parar_rx.recv_timeout(Duration::from_millis(10)) {
                if let Some(m) = &mistura {
                    if let Ok(mut m) = m.lock() {
                        m.misturar(&mut bloco);
                    }
                }
                if !tom {
                    continue;
                }
                let falando = (n / TAXA as usize) % 2 == 0;
                let quadro: Vec<i16> = (0..QUADRO).map(|_| {
                    n += 1;
                    if falando { ((n as f32 * std::f32::consts::TAU * 440.0 / TAXA as f32).sin() * 9000.0) as i16 } else { 0 }
                }).collect();
                ctl.nivel_mic.set(if falando { 0.2 } else { ctl.nivel_mic.get() * 0.85 });
                if ctl.mudo.load(Ordering::Relaxed) {
                    continue;
                }
                if let Some(d) = ctl.destino.lock().ok().as_mut().and_then(|g| g.as_mut()) {
                    d(&quadro);
                }
            }
        });
        Som { parar: Some(parar_tx), fio: Some(fio), entrada: if tom { "tom de teste".into() } else { String::new() }, saida: String::new() }
    }
}

/// O microfone: o fio do cpal só converte e passa adiante; a limpeza roda numa thread nossa.
fn abrir_microfone(nome: &str, ctl: Arc<Controles>, eco_rx: Receiver<Vec<i16>>) -> Result<(cpal::Stream, String), String> {
    let dev = achar(true, nome).ok_or("Não achei nenhum microfone.")?;
    let nome_real = dev.name().unwrap_or_default();
    let cfg = dev.default_input_config().map_err(|e| format!("O microfone não abriu: {e}"))?;
    let canais = cfg.channels() as usize;
    let taxa = cfg.sample_rate().0;
    let (tx, rx) = sync_channel::<Vec<f32>>(64);
    let erro = |e| eprintln!("[som] microfone: {e}");
    let conf: cpal::StreamConfig = cfg.clone().into();
    let stream = match cfg.sample_format() {
        cpal::SampleFormat::F32 => dev.build_input_stream(&conf, move |d: &[f32], _| { let _ = tx.try_send(mono(d, canais, |s| s)); }, erro, None),
        cpal::SampleFormat::I16 => dev.build_input_stream(&conf, move |d: &[i16], _| { let _ = tx.try_send(mono(d, canais, |s| s as f32 / 32768.0)); }, erro, None),
        cpal::SampleFormat::U16 => dev.build_input_stream(&conf, move |d: &[u16], _| { let _ = tx.try_send(mono(d, canais, |s| (s as f32 - 32768.0) / 32768.0)); }, erro, None),
        f => return Err(format!("O microfone usa um formato que o Olheiro ainda não lê ({f:?}).")),
    }
    .map_err(|e| format!("O microfone não abriu: {e}"))?;
    stream.play().map_err(|e| format!("O microfone não começou: {e}"))?;
    std::thread::Builder::new().name("olheiro-mic".into()).spawn(move || limpar_microfone(rx, eco_rx, taxa, ctl)).map_err(|e| e.to_string())?;
    Ok((stream, nome_real))
}

fn mono<T: Copy>(d: &[T], canais: usize, f: impl Fn(T) -> f32) -> Vec<f32> {
    d.chunks(canais.max(1)).map(|c| c.iter().map(|&s| f(s)).sum::<f32>() / c.len() as f32).collect()
}

/// Troca de taxa por interpolação linear, guardando o meio do caminho entre uma chamada e outra.
pub struct Reamostra {
    passo: f64,
    pos: f64,
    ultimo: f32,
}

impl Reamostra {
    pub fn new(de: u32, para: u32) -> Self {
        Reamostra { passo: de as f64 / para as f64, pos: 0.0, ultimo: 0.0 }
    }

    pub fn rodar(&mut self, entrada: &[f32], saida: &mut Vec<f32>) {
        if (self.passo - 1.0).abs() < 1e-9 {
            saida.extend_from_slice(entrada);
            return;
        }
        // `pos` conta a partir da última amostra do bloco anterior (`ultimo`): em `pos` o som está entre a amostra `pos - 1` e `pos`
        let n = entrada.len() as f64;
        let ultimo = self.ultimo;
        let em = |i: isize| if i < 0 { ultimo } else { entrada[i as usize] };
        while self.pos < n {
            let i = self.pos.floor() as isize;
            let fr = (self.pos - self.pos.floor()) as f32;
            let (a, b) = (em(i - 1), em(i));
            saida.push(a + (b - a) * fr);
            self.pos += self.passo;
        }
        self.pos -= n;
        if let Some(&u) = entrada.last() {
            self.ultimo = u;
        }
    }
}

fn limpar_microfone(rx: Receiver<Vec<f32>>, eco_rx: Receiver<Vec<i16>>, taxa: u32, ctl: Arc<Controles>) {
    use livekit::webrtc::native::apm::AudioProcessingModule;
    let mut apm = AudioProcessingModule::new(true, true, true, true);
    let mut rs = Reamostra::new(taxa, TAXA);
    let mut acumulado: Vec<f32> = Vec::with_capacity(QUADRO * 4);
    let mut quadro = vec![0i16; QUADRO];
    while let Ok(bloco) = rx.recv() {
        rs.rodar(&bloco, &mut acumulado);
        while acumulado.len() >= QUADRO {
            // primeiro o que tocou no fone (o eco), depois o microfone: é a ordem que a limpeza pede
            loop {
                match eco_rx.try_recv() {
                    Ok(mut e) => {
                        let _ = apm.process_reverse_stream(&mut e, TAXA as i32, 1);
                    }
                    Err(TryRecvError::Empty | TryRecvError::Disconnected) => break,
                }
            }
            let ganho = ctl.ganho_mic.get();
            for (q, s) in quadro.iter_mut().zip(acumulado.drain(..QUADRO)) {
                *q = ((s * ganho).clamp(-1.0, 1.0) * 32767.0) as i16;
            }
            let _ = apm.process_stream(&mut quadro, TAXA as i32, 1);
            let rms = (quadro.iter().map(|&s| (s as f32 / 32768.0).powi(2)).sum::<f32>() / QUADRO as f32).sqrt();
            ctl.nivel_mic.set(rms.max(ctl.nivel_mic.get() * 0.85));
            if ctl.mudo.load(Ordering::Relaxed) {
                continue;
            }
            if let Some(d) = ctl.destino.lock().ok().as_mut().and_then(|g| g.as_mut()) {
                d(&quadro);
            }
        }
    }
}

fn abrir_saida(nome: &str, mistura: MisturaRef, eco_tx: SyncSender<Vec<i16>>) -> Result<(cpal::Stream, String), String> {
    let dev = achar(false, nome).ok_or("Não achei nenhuma saída de som (fone ou caixa).")?;
    let nome_real = dev.name().unwrap_or_default();
    let cfg = dev.default_output_config().map_err(|e| format!("A saída de som não abriu: {e}"))?;
    let canais = cfg.channels() as usize;
    let taxa = cfg.sample_rate().0;
    let conf: cpal::StreamConfig = cfg.clone().into();
    let mut gerador = Gerador::new(mistura, taxa, eco_tx);
    let erro = |e| eprintln!("[som] saída: {e}");
    let stream = match cfg.sample_format() {
        cpal::SampleFormat::F32 => dev.build_output_stream(&conf, move |d: &mut [f32], _| gerador.encher(d, canais, |v| v), erro, None),
        cpal::SampleFormat::I16 => dev.build_output_stream(&conf, move |d: &mut [i16], _| gerador.encher(d, canais, |v| (v * 32767.0) as i16), erro, None),
        cpal::SampleFormat::U16 => dev.build_output_stream(&conf, move |d: &mut [u16], _| gerador.encher(d, canais, |v| (v * 32767.0 + 32768.0) as u16), erro, None),
        f => return Err(format!("A saída de som usa um formato que o Olheiro ainda não toca ({f:?}).")),
    }
    .map_err(|e| format!("A saída de som não abriu: {e}"))?;
    stream.play().map_err(|e| format!("A saída de som não começou: {e}"))?;
    Ok((stream, nome_real))
}

/// Puxa a mistura a 48 kHz e entrega na taxa da placa.
struct Gerador {
    mistura: MisturaRef,
    fila: VecDeque<f32>,
    passo: f64,
    pos: f64,
    eco_tx: SyncSender<Vec<i16>>,
    bloco: [f32; QUADRO],
}

impl Gerador {
    fn new(mistura: MisturaRef, taxa: u32, eco_tx: SyncSender<Vec<i16>>) -> Self {
        Gerador { mistura, fila: VecDeque::with_capacity(QUADRO * 3), passo: TAXA as f64 / taxa as f64, pos: 0.0, eco_tx, bloco: [0.0; QUADRO] }
    }

    fn encher<T>(&mut self, d: &mut [T], canais: usize, conv: impl Fn(f32) -> T)
    where
        T: Copy,
    {
        for quadro in d.chunks_mut(canais.max(1)) {
            while self.fila.len() < 2 {
                if let Ok(mut m) = self.mistura.lock() {
                    m.misturar(&mut self.bloco);
                } else {
                    self.bloco.fill(0.0);
                }
                self.fila.extend(self.bloco.iter().copied());
                let eco: Vec<i16> = self.bloco.iter().map(|&v| (v.clamp(-1.0, 1.0) * 32767.0) as i16).collect();
                let _ = self.eco_tx.try_send(eco);
            }
            let fr = self.pos as f32;
            let v = self.fila[0] + (self.fila[1] - self.fila[0]) * fr;
            let s = conv(v);
            for c in quadro.iter_mut() {
                *c = s;
            }
            self.pos += self.passo;
            while self.pos >= 1.0 {
                self.fila.pop_front();
                self.pos -= 1.0;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tom(n: usize, amp: f32) -> Vec<i16> {
        (0..n).map(|i| ((i as f32 * 0.05).sin() * amp * 32767.0) as i16).collect()
    }

    fn energia(b: &[f32]) -> f32 {
        (b.iter().map(|v| v * v).sum::<f32>() / b.len() as f32).sqrt()
    }

    #[test]
    fn so_toca_depois_de_juntar_um_pouco_e_para_quando_acaba() {
        let m = Mistura::nova();
        let mut m = m.lock().unwrap();
        let mut out = [0.0; QUADRO];
        m.chegou("ana#br1", &tom(QUADRO * 2, 0.5));
        m.misturar(&mut out);
        assert_eq!(energia(&out), 0.0, "ainda juntando");
        m.chegou("ana#br1", &tom(QUADRO * 6, 0.5));
        m.misturar(&mut out);
        assert!(energia(&out) > 0.1);
        for _ in 0..10 {
            m.misturar(&mut out);
        }
        assert_eq!(energia(&out), 0.0, "acabou o que tinha: silêncio, sem repetir");
    }

    #[test]
    fn o_volume_vai_ate_o_alvo_sem_degrau() {
        let m = Mistura::nova();
        let mut m = m.lock().unwrap();
        let mut out = [0.0; QUADRO];
        m.chegou("ana#br1", &tom(TAXA as usize * 2, 0.5));
        m.misturar(&mut out);
        let cheio = energia(&out);
        m.alvo("ana#br1", 0.0);
        m.misturar(&mut out);
        let logo_depois = energia(&out);
        assert!(logo_depois > cheio * 0.8, "10 ms depois ainda não sumiu: {logo_depois} de {cheio}");
        for _ in 0..100 {
            m.misturar(&mut out);
        }
        assert!(energia(&out) < cheio * 0.05, "1 s depois quase sumiu");
    }

    #[test]
    fn surdo_nao_toca_ninguem_e_cada_um_tem_o_seu_volume() {
        let m = Mistura::nova();
        let mut m = m.lock().unwrap();
        let mut out = [0.0; QUADRO];
        m.alvo("longe#br1", 0.0);
        m.chegou("longe#br1", &tom(QUADRO * 40, 0.5));
        for _ in 0..8 {
            m.misturar(&mut out);
        }
        assert!(energia(&out) < 1e-3, "quem nasce com alvo zero não aparece nem no começo");
        m.chegou("perto#br1", &tom(QUADRO * 40, 0.5));
        m.misturar(&mut out);
        assert!(energia(&out) > 0.1);
        m.surdo = true;
        m.misturar(&mut out);
        assert_eq!(energia(&out), 0.0);
    }

    #[test]
    fn fila_atrasada_demais_e_cortada() {
        let m = Mistura::nova();
        let mut m = m.lock().unwrap();
        m.chegou("ana#br1", &tom(TAXA as usize * 3, 0.5));
        assert!(m.falas["ana#br1"].fila.len() <= TETO);
    }

    #[test]
    fn a_troca_de_taxa_mantem_a_duracao() {
        for (de, para) in [(44_100, 48_000), (48_000, 44_100), (16_000, 48_000), (48_000, 48_000)] {
            let mut r = Reamostra::new(de, para);
            let mut out = Vec::new();
            for _ in 0..100 {
                r.rodar(&vec![0.25; de as usize / 100], &mut out);
            }
            let esperado = para as f64;
            assert!((out.len() as f64 - esperado).abs() <= 2.0, "{de}→{para}: {} de {esperado}", out.len());
            assert!(out.iter().skip(4).all(|v| (v - 0.25).abs() < 1e-6), "depois da primeira amostra, o sinal constante continua constante");
        }
    }
}

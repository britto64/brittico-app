//! O overlay no Windows: uma janela do sistema com transparência por pixel (`WS_EX_LAYERED` + `UpdateLayeredWindow`), sempre por
//! cima, que deixa o mouse passar (`WS_EX_TRANSPARENT`) e não pega o foco do jogo (`WS_EX_NOACTIVATE`). Um fio próprio lê o estado do
//! motor umas dez vezes por segundo, desenha os círculos (`overlay::rasterizar`) e só atualiza a janela quando algo muda.
//!
//! Não injeta nada no jogo: é uma janela comum do Brittico, no mesmo processo, desenhada por cima da dele.

use super::overlay::{lugar, quem_agora, quer, rasterizar, Imagem};
use super::JanelaRef;
use std::time::{Duration, Instant};
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, SIZE, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, GetDC, ReleaseDC, SelectObject, AC_SRC_ALPHA, AC_SRC_OVER, BITMAPINFO, BITMAPINFOHEADER, BI_RGB,
    BLENDFUNCTION, DIB_RGB_COLORS,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, PeekMessageW, RegisterClassW, SetWindowPos, ShowWindow, TranslateMessage, UpdateLayeredWindow, HWND_TOPMOST, MSG, PM_REMOVE,
    SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SW_HIDE, SW_SHOWNOACTIVATE, ULW_ALPHA, WNDCLASSW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
};

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

unsafe extern "system" fn proc_janela(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    DefWindowProcW(hwnd, msg, wp, lp)
}

struct Janela {
    hwnd: HWND,
    pos: (i32, i32),
    imagem: Option<Imagem>,
    visivel: bool,
    topo_em: Instant,
}

impl Janela {
    fn criar() -> Option<Janela> {
        unsafe {
            let inst = GetModuleHandleW(std::ptr::null());
            let classe = wide("BritticoOlheiroOverlay");
            let mut wc: WNDCLASSW = std::mem::zeroed();
            wc.lpfnWndProc = Some(proc_janela);
            wc.hInstance = inst;
            wc.lpszClassName = classe.as_ptr();
            // registrar de novo (já existe) devolve 0 e não atrapalha: a classe é a mesma
            RegisterClassW(&wc);
            let titulo = wide("Olheiro overlay");
            let hwnd = CreateWindowExW(
                WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOPMOST | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW,
                classe.as_ptr(),
                titulo.as_ptr(),
                WS_POPUP,
                0,
                0,
                1,
                1,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                inst,
                std::ptr::null(),
            );
            if hwnd.is_null() {
                return None;
            }
            Some(Janela { hwnd, pos: (i32::MIN, i32::MIN), imagem: None, visivel: false, topo_em: Instant::now() })
        }
    }

    fn esconder(&mut self) {
        if self.visivel {
            unsafe { ShowWindow(self.hwnd, SW_HIDE) };
            self.visivel = false;
            self.imagem = None;
        }
    }

    /// Põe esta imagem neste lugar da tela (só mexe no sistema se algo mudou).
    fn mostrar(&mut self, pos: (i32, i32), img: Imagem) {
        if self.visivel && self.pos == pos && self.imagem.as_ref() == Some(&img) {
            // de tempos em tempos, de volta ao topo: o jogo pode ter tomado a frente
            if self.topo_em.elapsed() > Duration::from_secs(2) {
                self.topo_em = Instant::now();
                unsafe { SetWindowPos(self.hwnd, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE) };
            }
            return;
        }
        unsafe {
            let tela = GetDC(std::ptr::null_mut());
            let mem = CreateCompatibleDC(tela);
            let mut bmi: BITMAPINFO = std::mem::zeroed();
            bmi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
            bmi.bmiHeader.biWidth = img.w as i32;
            bmi.bmiHeader.biHeight = -(img.h as i32); // de cima para baixo
            bmi.bmiHeader.biPlanes = 1;
            bmi.bmiHeader.biBitCount = 32;
            bmi.bmiHeader.biCompression = BI_RGB;
            let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
            let bmp = CreateDIBSection(mem, &bmi, DIB_RGB_COLORS, &mut bits, std::ptr::null_mut(), 0);
            if !bmp.is_null() && !bits.is_null() {
                std::ptr::copy_nonoverlapping(img.bgra.as_ptr(), bits as *mut u8, img.bgra.len());
                let antigo = SelectObject(mem, bmp);
                let destino = POINT { x: pos.0, y: pos.1 };
                let tamanho = SIZE { cx: img.w as i32, cy: img.h as i32 };
                let origem = POINT { x: 0, y: 0 };
                let mistura = BLENDFUNCTION { BlendOp: AC_SRC_OVER as u8, BlendFlags: 0, SourceConstantAlpha: 255, AlphaFormat: AC_SRC_ALPHA as u8 };
                UpdateLayeredWindow(self.hwnd, tela, &destino, &tamanho, mem, &origem, 0, &mistura, ULW_ALPHA);
                SelectObject(mem, antigo);
                DeleteObject(bmp);
                if !self.visivel {
                    ShowWindow(self.hwnd, SW_SHOWNOACTIVATE);
                    SetWindowPos(self.hwnd, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
                    self.visivel = true;
                }
            }
            DeleteDC(mem);
            ReleaseDC(std::ptr::null_mut(), tela);
        }
        self.pos = pos;
        self.imagem = Some(img);
        self.topo_em = Instant::now();
    }

    fn bombear(&self) {
        unsafe {
            let mut m: MSG = std::mem::zeroed();
            while PeekMessageW(&mut m, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                TranslateMessage(&m);
                DispatchMessageW(&m);
            }
        }
    }
}

impl Drop for Janela {
    fn drop(&mut self) {
        unsafe { DestroyWindow(self.hwnd) };
    }
}

/// Liga o fio do overlay. Ele vive enquanto o app vive e só tem uma janela quando há partida e a pessoa quer.
pub fn iniciar(j: JanelaRef) {
    let _ = std::thread::Builder::new().name("olheiro-overlay".into()).spawn(move || {
        let mut janela: Option<Janela> = None;
        let t0 = Instant::now();
        loop {
            std::thread::sleep(Duration::from_millis(90));
            let lido = match j.lock() {
                Ok(g) => {
                    let (est, itens) = quem_agora(&g, t0.elapsed().as_secs_f64());
                    let com_arte: Vec<_> = itens.into_iter().map(|i| { let a = g.motor.icone(&i.alias); (i, a) }).collect();
                    Some((quer(&est), com_arte))
                }
                Err(_) => None,
            };
            let Some((onde, itens)) = lido else { continue };
            match onde {
                Some(jan) if !itens.is_empty() => {
                    if janela.is_none() {
                        janela = Janela::criar();
                    }
                    if let Some(w) = janela.as_mut() {
                        let (x, y, escala) = lugar(jan);
                        w.mostrar((x, y), rasterizar(&itens, escala));
                        w.bombear();
                    }
                }
                _ => {
                    if let Some(w) = janela.as_mut() {
                        w.esconder();
                        w.bombear();
                    }
                }
            }
        }
    });
}

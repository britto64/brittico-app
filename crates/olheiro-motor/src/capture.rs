//! Achar a janela do jogo e fotografar o canto dela onde fica o minimapa.
//!
//! Portado da referência (`capture.rs` e `game_window.rs` do LoLProxChat): `FindWindowW` na classe `RiotWindowClass` e um `BitBlt` da
//! tela inteira (a "tela virtual", que inclui monitores à esquerda e acima do principal). É a mesma captura que o OBS faz: nada de gancho
//! no jogo, nada de memória. O que a tela mostra é o que se lê, então o minimapa precisa estar visível (modo Janela sem borda).
//!
//! Só existe de verdade no Windows. Em outros sistemas `ScreenCapture` recusa com uma frase clara, e o modo `--demo` usa o simulador.

use crate::session::Capture;
use olheiro_core::{Frame, Rect};

/// A classe da janela do JOGO. O cliente (loja, lobby) é outro processo e outra classe, então esta busca nunca o confunde.
#[cfg(windows)]
const GAME_WINDOW_CLASS: &str = "RiotWindowClass";

/// Uma janela menor que isto não é o LoL (diálogos e telas de abertura passam por aqui).
#[cfg_attr(not(windows), allow(dead_code))]
const MIN_GAME_W: i32 = 640;
#[cfg_attr(not(windows), allow(dead_code))]
const MIN_GAME_H: i32 = 480;

pub struct ScreenCapture;

/// A janela serve se tem o tamanho de um jogo.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn plausible(rect: &Rect) -> bool {
    rect.w >= MIN_GAME_W && rect.h >= MIN_GAME_H
}

#[cfg(windows)]
impl Capture for ScreenCapture {
    /// A ÁREA DO CLIENTE da janela, em coordenadas da tela: o minimapa ancora no canto dela, não no do monitor (e no modo janela a barra de
    /// título não conta). Minimizada (`-32000`) ou invisível: `None`.
    fn window(&mut self) -> Option<Rect> {
        use windows_sys::Win32::Foundation::{HWND, POINT, RECT};
        use windows_sys::Win32::Graphics::Gdi::MapWindowPoints;
        use windows_sys::Win32::UI::WindowsAndMessaging::{FindWindowW, GetClientRect, IsIconic, IsWindowVisible};
        let class: Vec<u16> = GAME_WINDOW_CLASS.encode_utf16().chain(std::iter::once(0)).collect();
        unsafe {
            let hwnd: HWND = FindWindowW(class.as_ptr(), std::ptr::null());
            if hwnd.is_null() || IsWindowVisible(hwnd) == 0 || IsIconic(hwnd) != 0 {
                return None;
            }
            let mut rc: RECT = std::mem::zeroed();
            if GetClientRect(hwnd, &mut rc) == 0 {
                return None;
            }
            let mut pts = [POINT { x: rc.left, y: rc.top }, POINT { x: rc.right, y: rc.bottom }];
            MapWindowPoints(hwnd, std::ptr::null_mut(), pts.as_mut_ptr(), 2);
            let r = Rect::new(pts[0].x, pts[0].y, pts[1].x - pts[0].x, pts[1].y - pts[0].y);
            if plausible(&r) {
                note_geometry(hwnd, &r);
            }
            plausible(&r).then_some(r)
        }
    }

    fn grab(&mut self, b: Rect) -> Result<Frame, String> {
        use windows_sys::Win32::Graphics::Gdi::{
            BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC, GetDIBits, ReleaseDC, SelectObject, BITMAPINFO,
            BI_RGB, DIB_RGB_COLORS, SRCCOPY,
        };
        use windows_sys::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN};
        if b.w <= 0 || b.h <= 0 {
            return Err("O quadrado a capturar não tem tamanho.".into());
        }
        unsafe {
            // fora da tela virtual o BitBlt entrega PRETO em vez de falhar, o que depois parece "o minimapa não está na tela": diz o motivo
            let virt = Rect::new(
                GetSystemMetrics(SM_XVIRTUALSCREEN),
                GetSystemMetrics(SM_YVIRTUALSCREEN),
                GetSystemMetrics(SM_CXVIRTUALSCREEN),
                GetSystemMetrics(SM_CYVIRTUALSCREEN),
            );
            if !b.intersects(&virt) {
                return Err(format!("A janela do LoL está fora da tela ({},{} {}x{}).", b.x, b.y, b.w, b.h));
            }
            // GetDC(NULL) é o DC da tela VIRTUAL; o da área de trabalho só alcança o monitor principal
            let screen = GetDC(std::ptr::null_mut());
            if screen.is_null() {
                return Err("GetDC falhou.".into());
            }
            let mem = CreateCompatibleDC(screen);
            let bmp = CreateCompatibleBitmap(screen, b.w, b.h);
            if mem.is_null() || bmp.is_null() {
                if !bmp.is_null() {
                    DeleteObject(bmp);
                }
                if !mem.is_null() {
                    DeleteDC(mem);
                }
                ReleaseDC(std::ptr::null_mut(), screen);
                return Err("Não consegui reservar memória para a captura.".into());
            }
            let old = SelectObject(mem, bmp);
            let copied = BitBlt(mem, 0, 0, b.w, b.h, screen, b.x, b.y, SRCCOPY);
            let mut bmi: BITMAPINFO = std::mem::zeroed();
            bmi.bmiHeader.biSize = std::mem::size_of::<windows_sys::Win32::Graphics::Gdi::BITMAPINFOHEADER>() as u32;
            bmi.bmiHeader.biWidth = b.w;
            bmi.bmiHeader.biHeight = -b.h; // negativo: de cima para baixo
            bmi.bmiHeader.biPlanes = 1;
            bmi.bmiHeader.biBitCount = 32;
            bmi.bmiHeader.biCompression = BI_RGB;
            let mut px = vec![0u8; b.w as usize * b.h as usize * 4];
            let lines = if copied != 0 { GetDIBits(mem, bmp, 0, b.h as u32, px.as_mut_ptr().cast(), &mut bmi, DIB_RGB_COLORS) } else { 0 };
            SelectObject(mem, old);
            DeleteObject(bmp);
            DeleteDC(mem);
            ReleaseDC(std::ptr::null_mut(), screen);
            if copied == 0 {
                return Err("O BitBlt falhou.".into());
            }
            if lines == 0 {
                return Err("O GetDIBits falhou.".into());
            }
            // O GDI entrega BGRA com o 4º byte indefinido: troca para RGBA UMA vez, aqui (trocar dos dois lados desfaz).
            Ok(Frame::from_bgra(b.w as usize, b.h as usize, px))
        }
    }
}

/// Escreve no `olheiro.log` onde o app achou a janela do LoL e em que escala o Windows está, UMA vez e de novo se mudar: foi o que faltou para achar o
/// defeito da escala de 125% (o app media a janela em pixels encolhidos e fotografava o lugar errado). Quem lê o log vê na hora se `consciência` não é 2
/// (por monitor) ou se `dpi` da janela passa de 96 sem que a janela tenha o tamanho físico da tela.
#[cfg(windows)]
fn note_geometry(hwnd: windows_sys::Win32::Foundation::HWND, r: &Rect) {
    use std::sync::Mutex;
    use windows_sys::Win32::UI::HiDpi::{GetAwarenessFromDpiAwarenessContext, GetDpiForWindow, GetThreadDpiAwarenessContext};
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN};
    static LAST: Mutex<String> = Mutex::new(String::new());
    let line = unsafe {
        let awareness = GetAwarenessFromDpiAwarenessContext(GetThreadDpiAwarenessContext());
        format!(
            "Janela do LoL: {}x{} em {},{} · tela virtual {}x{} · DPI da janela {} · o Olheiro enxerga em modo {} (2 = por monitor, o certo)",
            r.w, r.h, r.x, r.y, GetSystemMetrics(SM_CXVIRTUALSCREEN), GetSystemMetrics(SM_CYVIRTUALSCREEN), GetDpiForWindow(hwnd), awareness
        )
    };
    if let Ok(mut last) = LAST.lock() {
        if *last != line {
            crate::logfile::write(&line);
            *last = line;
        }
    }
}

#[cfg(not(windows))]
impl Capture for ScreenCapture {
    fn window(&mut self) -> Option<Rect> {
        None
    }

    fn grab(&mut self, _: Rect) -> Result<Frame, String> {
        Err("A captura do minimapa só existe no Windows. Para ver o app funcionar aqui, use a partida simulada.".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn so_serve_janela_com_tamanho_de_jogo() {
        assert!(plausible(&Rect::new(0, 0, 1920, 1080)));
        assert!(plausible(&Rect::new(-1920, -400, 1366, 768)));
        assert!(!plausible(&Rect::new(0, 0, 400, 300)), "diálogo");
        assert!(!plausible(&Rect::new(-32000, -32000, 0, 0)), "minimizada");
    }

    #[cfg(not(windows))]
    #[test]
    fn fora_do_windows_recusa_com_uma_frase_clara() {
        let mut c = ScreenCapture;
        assert!(c.window().is_none());
        assert!(c.grab(Rect::new(0, 0, 100, 100)).unwrap_err().contains("simulada"));
    }
}

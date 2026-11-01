//! O cursor do mouse na tela mostrada, no Windows. A captura do libwebrtc que o LiveKit usa lá (DXGI) não põe o cursor
//! na imagem (o Chrome o desenha por conta própria; o LiveKit não), então o app desenha: a cada quadro lê a posição
//! (`GetCursorInfo`), e só quando o cursor muda de forma lê a imagem (`GetIconInfo` + `GetDIBits`). O desenho é feito
//! num retalho pequeno em volta do cursor, convertido para I420 e copiado por cima do quadro, sem copiar a tela inteira.

use windows_sys::Win32::Foundation::{POINT, RECT};
use windows_sys::Win32::Graphics::Gdi::{
    DeleteObject, GetDC, GetDIBits, GetObjectW, ReleaseDC, BITMAP, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{GetCursorInfo, GetIconInfo, GetWindowRect, CURSORINFO, CURSOR_SHOWING, ICONINFO};

/// A imagem de um cursor: tamanho, ponto quente e os pixels em BGRA (alfa normal, não pré-multiplicado).
struct Imagem {
    w: i32,
    h: i32,
    quente: (i32, i32),
    bgra: Vec<u8>,
}

/// Lembra a imagem do último cursor (pela alça do Windows) para não ler de novo a cada quadro.
#[derive(Default)]
pub struct Cursor {
    alca: isize,
    imagem: Option<Imagem>,
}

/// Lê um bitmap do Windows como BGRA de cima para baixo.
unsafe fn bits(hbm: *mut core::ffi::c_void) -> Option<(i32, i32, Vec<u8>)> {
    let mut b: BITMAP = core::mem::zeroed();
    if GetObjectW(hbm, core::mem::size_of::<BITMAP>() as i32, &mut b as *mut _ as *mut _) == 0 || b.bmWidth <= 0 || b.bmHeight <= 0 {
        return None;
    }
    let (w, h) = (b.bmWidth, b.bmHeight);
    let mut info: BITMAPINFO = core::mem::zeroed();
    info.bmiHeader = BITMAPINFOHEADER { biSize: core::mem::size_of::<BITMAPINFOHEADER>() as u32, biWidth: w, biHeight: -h, biPlanes: 1, biBitCount: 32, biCompression: BI_RGB, ..core::mem::zeroed() };
    let mut px = vec![0u8; (w * h * 4) as usize];
    let dc = GetDC(core::ptr::null_mut());
    let n = GetDIBits(dc, hbm, 0, h as u32, px.as_mut_ptr() as *mut _, &mut info, DIB_RGB_COLORS);
    ReleaseDC(core::ptr::null_mut(), dc);
    (n > 0).then_some((w, h, px))
}

unsafe fn ler_imagem(alca: *mut core::ffi::c_void) -> Option<Imagem> {
    let mut ii: ICONINFO = core::mem::zeroed();
    if GetIconInfo(alca, &mut ii) == 0 {
        return None;
    }
    let mascara = if ii.hbmMask.is_null() { None } else { bits(ii.hbmMask) };
    let cor = if ii.hbmColor.is_null() { None } else { bits(ii.hbmColor) };
    if !ii.hbmMask.is_null() {
        DeleteObject(ii.hbmMask);
    }
    if !ii.hbmColor.is_null() {
        DeleteObject(ii.hbmColor);
    }
    let quente = (ii.xHotspot as i32, ii.yHotspot as i32);
    match (cor, mascara) {
        // cursor colorido: com alfa próprio, ou (cursores antigos) com a máscara dizendo o que é transparente
        (Some((w, h, mut px)), m) => {
            let tem_alfa = px.chunks(4).any(|p| p[3] != 0);
            if !tem_alfa {
                for (i, p) in px.chunks_mut(4).enumerate() {
                    let transparente = m.as_ref().is_some_and(|(mw, _, mp)| {
                        let (x, y) = (i as i32 % w, i as i32 / w);
                        x < *mw && mp.get(((y * mw + x) * 4) as usize).is_some_and(|v| *v > 127)
                    });
                    p[3] = if transparente { 0 } else { 255 };
                }
            }
            Some(Imagem { w, h, quente, bgra: px })
        }
        // cursor monocromático: a máscara tem a metade de cima (E) e a de baixo (OU exclusivo)
        (None, Some((w, h2, mp))) => {
            let h = h2 / 2;
            let mut px = vec![0u8; (w * h * 4) as usize];
            for y in 0..h {
                for x in 0..w {
                    let e = mp[((y * w + x) * 4) as usize] > 127;
                    let ou = mp[(((y + h) * w + x) * 4) as usize] > 127;
                    let o = ((y * w + x) * 4) as usize;
                    let (v, a) = match (e, ou) {
                        (true, false) => (0, 0),
                        (false, false) => (0, 255),
                        (false, true) => (255, 255),
                        // "inverter o que está embaixo": um contorno escuro é o mais legível
                        (true, true) => (0, 255),
                    };
                    px[o..o + 4].copy_from_slice(&[v, v, v, a]);
                }
            }
            Some(Imagem { w, h, quente, bgra: px })
        }
        _ => None,
    }
}

/// Onde o quadro capturado fica no desktop: a janela (pelo HWND, que é o id da fonte) ou o canto do quadro.
pub fn origem(janela: Option<u64>, quadro: (i32, i32)) -> (i32, i32) {
    if let Some(hwnd) = janela {
        let mut r: RECT = unsafe { core::mem::zeroed() };
        if unsafe { GetWindowRect(hwnd as usize as *mut _, &mut r) } != 0 {
            return (r.left, r.top);
        }
    }
    quadro
}

impl Cursor {
    /// Desenha o cursor num quadro BGRA (`dados`, `stride`, `w`x`h`, cujo canto está em `origem` no desktop) e
    /// devolve o retalho já desenhado: posição (par) e BGRA de `pw`x`ph`. `None` se o cursor está escondido ou fora.
    pub fn retalho(&mut self, dados: &[u8], stride: usize, w: i32, h: i32, origem: (i32, i32)) -> Option<(i32, i32, i32, i32, Vec<u8>)> {
        let mut ci: CURSORINFO = unsafe { core::mem::zeroed() };
        ci.cbSize = core::mem::size_of::<CURSORINFO>() as u32;
        if unsafe { GetCursorInfo(&mut ci) } == 0 || ci.flags & CURSOR_SHOWING == 0 || ci.hCursor.is_null() {
            return None;
        }
        if ci.hCursor as isize != self.alca {
            self.alca = ci.hCursor as isize;
            self.imagem = unsafe { ler_imagem(ci.hCursor) };
        }
        let img = self.imagem.as_ref()?;
        let POINT { x: mx, y: my } = ci.ptScreenPos;
        if std::env::var("BRITTICO_VOZ_LOG").is_ok() {
            use std::sync::atomic::{AtomicU64, Ordering};
            static N: AtomicU64 = AtomicU64::new(0);
            if N.fetch_add(1, Ordering::Relaxed) % 120 == 0 {
                eprintln!("[voz] cursor em {mx},{my}, imagem {}x{} (quente {:?}), quadro {w}x{h} com origem {:?}", img.w, img.h, img.quente, origem);
            }
        }
        // canto do cursor dentro do quadro
        let (cx, cy) = (mx - img.quente.0 - origem.0, my - img.quente.1 - origem.1);
        // o retalho: o cursor arredondado para posições e tamanhos pares (o I420 anda de 2 em 2), dentro do quadro
        let x0 = (cx.max(0) & !1).min(w);
        let y0 = (cy.max(0) & !1).min(h);
        let x1 = ((cx + img.w + 1).min(w) & !1).max(x0);
        let y1 = ((cy + img.h + 1).min(h) & !1).max(y0);
        let (pw, ph) = (x1 - x0, y1 - y0);
        if pw < 2 || ph < 2 {
            return None;
        }
        let mut px = vec![0u8; (pw * ph * 4) as usize];
        for y in 0..ph {
            let o = (y0 + y) as usize * stride + x0 as usize * 4;
            px[(y * pw * 4) as usize..((y + 1) * pw * 4) as usize].copy_from_slice(&dados[o..o + (pw * 4) as usize]);
        }
        for y in 0..ph {
            for x in 0..pw {
                let (ix, iy) = (x0 + x - cx, y0 + y - cy);
                if ix < 0 || iy < 0 || ix >= img.w || iy >= img.h {
                    continue;
                }
                let s = ((iy * img.w + ix) * 4) as usize;
                let a = img.bgra[s + 3] as u32;
                if a == 0 {
                    continue;
                }
                let d = ((y * pw + x) * 4) as usize;
                for c in 0..3 {
                    px[d + c] = ((img.bgra[s + c] as u32 * a + px[d + c] as u32 * (255 - a)) / 255) as u8;
                }
            }
        }
        Some((x0, y0, pw, ph, px))
    }
}

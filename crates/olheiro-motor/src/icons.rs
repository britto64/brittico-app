//! Os ícones dos campeões da partida, para o olheiro reconhecê-los no minimapa.
//!
//! O minimapa desenha o ícone BASE de cada campeão, qualquer que seja a skin (gravações reais da referência), então basta o ícone base,
//! que o Community Dragon serve. Cada ícone é baixado uma vez e fica no disco. Os pedidos mostram, ao Community Dragon, quais campeões a
//! partida tem, mas não dizem de quem é a partida nem quem joga.

use crate::net;
use crate::session::Icons;
use olheiro_core::templates::icon_templates;
use std::path::PathBuf;
use std::time::Duration;

const CDRAGON: &str = "https://raw.communitydragon.org/latest/game/assets/characters";

/// Campeões cujo ícone base ainda leva o codinome antigo no nome do arquivo (o do Community Dragon; lista da referência).
fn file_alias(alias: &str) -> &str {
    match alias {
        "anivia" => "cryophoenix",
        "blitzcrank" => "steamgolem",
        "chogath" => "greenterror",
        "orianna" => "oriana",
        "rammus" => "armordillo",
        "shaco" => "jester",
        "zilean" => "chronokeeper",
        other => other,
    }
}

/// O nome do arquivo do ícone base dentro da listagem da pasta `hud/` do campeão (`_circle_0.png` ou `_circle.png`).
pub fn base_icon_file(listing: &str, alias: &str) -> Option<String> {
    let base = file_alias(alias);
    [format!("{base}_circle_0.png"), format!("{base}_circle.png")].into_iter().find(|f| listing.contains(&format!("\"{f}\"")))
}

pub struct CdragonIcons {
    cache: PathBuf,
}

impl CdragonIcons {
    pub fn new() -> Self {
        CdragonIcons { cache: crate::ajustes::dir().join("icons") }
    }

    fn png(&self, alias: &str) -> Result<Vec<u8>, String> {
        let file = self.cache.join(format!("{alias}.png"));
        if let Ok(b) = std::fs::read(&file) {
            return Ok(b);
        }
        let agent = net::web(Duration::from_secs(15));
        let listing_url = format!("{CDRAGON}/{alias}/hud/");
        let listing = agent.get(&listing_url).call().map_err(|e| e.to_string()).and_then(|r| net::read_text(r, 2 * 1024 * 1024).ok_or("resposta vazia".to_string()))?;
        let name = base_icon_file(&listing, alias).ok_or_else(|| format!("sem ícone base de {alias}"))?;
        let resp = agent.get(format!("{listing_url}{name}")).call().map_err(|e| e.to_string())?;
        if resp.status().as_u16() != 200 {
            return Err(format!("HTTP {} no ícone de {alias}", resp.status().as_u16()));
        }
        let mut bytes = Vec::new();
        use std::io::Read;
        resp.into_body().into_reader().take(2 * 1024 * 1024).read_to_end(&mut bytes).map_err(|e| e.to_string())?;
        let _ = std::fs::create_dir_all(&self.cache);
        let _ = std::fs::write(&file, &bytes);
        Ok(bytes)
    }
}

impl Default for CdragonIcons {
    fn default() -> Self {
        Self::new()
    }
}

/// PNG → modelos de comparação (os três zooms de um ícone).
pub fn templates_from_png(bytes: &[u8]) -> Result<Vec<Vec<f32>>, String> {
    let img = image::load_from_memory_with_format(bytes, image::ImageFormat::Png).map_err(|e| e.to_string())?.to_rgba8();
    Ok(icon_templates(img.as_raw(), img.width() as usize, img.height() as usize))
}

impl Icons for CdragonIcons {
    /// Baixa em paralelo (um por campeão). Quem não deu certo fica de fora e é dito no registro: sem o ícone de alguém o olheiro
    /// não o reconhece (e a sessão recusa nomear, para o ícone dele não ser confundido com o de outro).
    fn load(&mut self, champs: &[(usize, String)]) -> Vec<(usize, Vec<Vec<f32>>)> {
        let me = &*self;
        std::thread::scope(|s| {
            let handles: Vec<_> = champs
                .iter()
                .map(|(id, alias)| {
                    let (id, alias) = (*id, alias.clone());
                    s.spawn(move || match me.png(&alias).and_then(|b| templates_from_png(&b)) {
                        Ok(t) => Some((id, t)),
                        Err(e) => {
                            eprintln!("[icones] {alias}: {e}");
                            None
                        }
                    })
                })
                .collect();
            handles.into_iter().filter_map(|h| h.join().ok().flatten()).collect()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn acha_o_icone_base_na_listagem() {
        let l = r#"<a href="ahri_circle_0.png">ahri_circle_0.png</a> "ahri_circle_0.png" "ahri_circle_14.png""#;
        assert_eq!(base_icon_file(l, "ahri").as_deref(), Some("ahri_circle_0.png"));
        assert_eq!(base_icon_file(r#""garen_circle.png""#, "garen").as_deref(), Some("garen_circle.png"));
        assert_eq!(base_icon_file(r#""ahri_circle_14.png""#, "ahri"), None, "uma skin não é o ícone base");
    }

    #[test]
    fn os_sete_codinomes_antigos() {
        assert_eq!(base_icon_file(r#""cryophoenix_circle.png""#, "anivia").as_deref(), Some("cryophoenix_circle.png"));
        assert_eq!(base_icon_file(r#""steamgolem_circle.png""#, "blitzcrank").as_deref(), Some("steamgolem_circle.png"));
        assert_eq!(file_alias("zed"), "zed");
    }

    #[test]
    fn um_png_de_verdade_vira_tres_modelos() {
        // 8x8 branco opaco, gerado pelo próprio `image`
        let mut buf = std::io::Cursor::new(Vec::new());
        image::RgbaImage::from_pixel(8, 8, image::Rgba([200, 100, 50, 255])).write_to(&mut buf, image::ImageFormat::Png).unwrap();
        let t = templates_from_png(buf.get_ref()).unwrap();
        assert_eq!(t.len(), 3);
        assert!(templates_from_png(b"nao e png").is_err());
    }
}

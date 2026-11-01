//! O registro do agente: `olheiro.log` na pasta de configuração.
//!
//! Na bandeja do Windows não há terminal, e sem isto o que o agente conta (conectou, caiu e por quê, partida detectada) some. O arquivo
//! é só texto e fica no computador da pessoa; ela o manda quando alguém pede ajuda. Passou de `MAX_BYTES`, fica só o final.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_BYTES: u64 = 256 * 1024;
const KEEP_BYTES: usize = 96 * 1024;

pub fn path() -> std::path::PathBuf {
    crate::ajustes::dir().join("olheiro.log")
}

/// Acrescenta uma linha, com a data e a hora (UTC). Falha em silêncio: o registro nunca pode derrubar o agente.
pub fn write(line: &str) {
    append(&path(), line, SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs()));
}

fn append(file: &Path, line: &str, unix_secs: u64) {
    if let Some(dir) = file.parent() {
        let _ = fs::create_dir_all(dir);
    }
    if fs::metadata(file).is_ok_and(|m| m.len() > MAX_BYTES) {
        trim(file);
    }
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(file) {
        let _ = writeln!(f, "{}  {}", stamp(unix_secs), line.replace('\n', " "));
    }
}

/// Fica com o final do arquivo, a partir da primeira linha inteira.
fn trim(file: &Path) {
    let Ok(text) = fs::read_to_string(file) else { return };
    let from = text.len().saturating_sub(KEEP_BYTES);
    let from = (from..text.len()).find(|&i| text.is_char_boundary(i)).unwrap_or(text.len());
    let tail = &text[from..];
    let tail = tail.find('\n').map_or(tail, |i| &tail[i + 1..]);
    let _ = fs::write(file, tail);
}

/// `2026-10-21 07:15:03` (UTC), sem depender de um crate de datas.
fn stamp(unix_secs: u64) -> String {
    let (days, rest) = ((unix_secs / 86_400) as i64, unix_secs % 86_400);
    // dias desde 1970-01-01 -> data civil (o algoritmo de Howard Hinnant)
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02} {:02}:{:02}:{:02}", rest / 3600, rest % 3600 / 60, rest % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_data_sai_certa() {
        assert_eq!(stamp(0), "1970-01-01 00:00:00");
        assert_eq!(stamp(1_792_566_903), "2026-10-21 07:15:03");
        assert_eq!(stamp(951_782_400), "2000-02-29 00:00:00", "ano bissexto");
    }

    #[test]
    fn grava_linhas_e_apara_o_arquivo_que_cresce_demais() {
        let dir = std::env::temp_dir().join(format!("olheiro-log-{}", std::process::id()));
        let file = dir.join("olheiro.log");
        append(&file, "primeira\ncom quebra", 0);
        let text = fs::read_to_string(&file).unwrap();
        assert_eq!(text, "1970-01-01 00:00:00  primeira com quebra\n");
        for i in 0..8_000 {
            append(&file, &format!("linha {i} {}", "x".repeat(40)), 0);
        }
        let size = fs::metadata(&file).unwrap().len();
        assert!(size < MAX_BYTES + 200, "o arquivo não pode crescer sem fim ({size} bytes)");
        let last = fs::read_to_string(&file).unwrap();
        assert!(last.trim_end().ends_with(&format!("linha 7999 {}", "x".repeat(40))), "o final fica");
        assert!(last.starts_with("1970-01-01"), "e começa numa linha inteira");
        let _ = fs::remove_dir_all(&dir);
    }
}

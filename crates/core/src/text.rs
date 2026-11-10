//! Messages in Korean with English fallback, number/date formatting, DOS 8.3 display names and
//! decoding of old Korean text (KS completion code, Johab) and CP437.
use std::sync::atomic::{AtomicBool, Ordering};
use unicode_width::UnicodeWidthChar;

static KOREAN: AtomicBool = AtomicBool::new(true);

pub fn set_korean(on: bool) {
    KOREAN.store(on, Ordering::Relaxed)
}
pub fn korean() -> bool {
    KOREAN.load(Ordering::Relaxed)
}
/// Korean text, or the English text when the UI language is English.
pub fn t<'a>(ko: &'a str, en: &'a str) -> &'a str {
    if korean() {
        ko
    } else {
        en
    }
}

/// Language from MDIR_LANG / LANG: Korean unless English is asked for.
pub fn init_lang_from_env() {
    if let Ok(v) = std::env::var("MDIR_LANG") {
        set_korean(!v.to_ascii_lowercase().starts_with("en"));
    }
}

pub fn commas(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// Size in at most `w` cells: plain with commas when it fits, else K or M.
pub fn size_fit(n: u64, w: usize) -> String {
    let s = commas(n);
    if s.len() <= w {
        return s;
    }
    let k = commas(n.div_ceil(1024)) + "K";
    if k.len() <= w {
        return k;
    }
    commas(n.div_ceil(1024 * 1024)) + "M"
}

pub fn fmt_date(t: std::time::SystemTime) -> String {
    let d: chrono::DateTime<chrono::Local> = t.into();
    d.format("%y-%m-%d").to_string()
}

pub fn fmt_time(t: std::time::SystemTime, h24: bool) -> String {
    use chrono::Timelike;
    let d: chrono::DateTime<chrono::Local> = t.into();
    if h24 {
        format!("{:>2}:{:02}", d.hour(), d.minute())
    } else {
        let (pm, h) = d.hour12();
        format!("{:>2}:{:02}{}", h, d.minute(), if pm { 'p' } else { 'a' })
    }
}

/// Display width of a string.
pub fn width(s: &str) -> usize {
    s.chars().map(|c| c.width().unwrap_or(0)).sum()
}

/// Cuts `s` to at most `w` cells and pads with spaces to exactly `w`.
pub fn fit(s: &str, w: usize) -> String {
    let mut out = String::new();
    let mut used = 0;
    for c in s.chars() {
        let cw = c.width().unwrap_or(0);
        if used + cw > w {
            break;
        }
        out.push(c);
        used += cw;
    }
    while used < w {
        out.push(' ');
        used += 1;
    }
    out
}

pub fn fit_right(s: &str, w: usize) -> String {
    let n = width(s);
    if n >= w {
        fit(s, w)
    } else {
        format!("{}{}", " ".repeat(w - n), s)
    }
}

/// DOS-style split: extension is after the last dot, unless the name starts with its only dot.
pub fn split_ext(name: &str) -> (&str, &str) {
    match name.rfind('.') {
        Some(0) | None => (name, ""),
        Some(i) => (&name[..i], &name[i + 1..]),
    }
}

fn cut(s: &str, w: usize) -> String {
    if width(s) <= w {
        return s.to_string();
    }
    let mut o = fit(s, w - 1).trim_end().to_string();
    while width(&o) < w - 1 {
        o.push(' ');
    }
    o.push('~');
    o
}

/// "NAME     EXT" in 12 cells, like the DOS short-name listing; longer names are cut with '~'.
pub fn name83(name: &str, lower: bool) -> String {
    let name = if lower { name.to_lowercase() } else { name.to_string() };
    let (b, e) = split_ext(&name);
    format!("{} {}", fit(&cut(b, 8), 8), fit(&cut(e, 3), 3))
}

const CP437_HI: &str = "ÇüéâäàåçêëèïîìÄÅÉæÆôöòûùÿÖÜ¢£¥₧ƒáíóúñÑªº¿⌐¬½¼¡«»░▒▓│┤╡╢╖╕╣║╗╝╜╛┐└┴┬├─┼╞╟╚╔╩╦╠═╬╧╨╤╥╙╘╒╓╫╪┘┌█▄▌▐▀αßΓπΣσµτΦΘΩδ∞φε∩≡±≥≤⌠⌡÷≈°∙·√ⁿ²■\u{a0}";

pub fn cp437(d: &[u8]) -> String {
    let hi: Vec<char> = CP437_HI.chars().collect();
    d.iter().map(|&b| if b < 128 { b as char } else { hi[(b - 128) as usize] }).collect()
}

/// Johab (상용조합형) Hangul syllable → Unicode.
fn johab_pair(a: u8, b: u8) -> Option<char> {
    let v = ((a as u16) << 8) | b as u16;
    if v & 0x8000 == 0 {
        return None;
    }
    let cho = ((v >> 10) & 31) as i32;
    let jung = ((v >> 5) & 31) as i32;
    let jong = (v & 31) as i32;
    let cho = if (2..=20).contains(&cho) { cho - 2 } else { return None };
    let jung = match jung {
        3..=7 => jung - 3,
        10..=15 => jung - 5,
        18..=23 => jung - 7,
        26..=29 => jung - 9,
        _ => return None,
    };
    let jong = match jong {
        1 => 0,
        2..=17 => jong - 1,
        19..=29 => jong - 2,
        _ => return None,
    };
    char::from_u32(0xAC00 + ((cho * 21 + jung) * 28 + jong) as u32)
}

pub fn johab(d: &[u8]) -> (String, usize) {
    let mut s = String::new();
    let mut bad = 0;
    let mut i = 0;
    while i < d.len() {
        let c = d[i];
        if c < 0x80 {
            s.push(c as char);
            i += 1;
        } else if i + 1 < d.len() {
            match johab_pair(c, d[i + 1]) {
                Some(ch) => s.push(ch),
                None => {
                    bad += 1;
                    s.push('?')
                }
            }
            i += 2;
        } else {
            i += 1;
        }
    }
    (s, bad)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Code {
    Utf8,
    Ks,
    Johab,
    English,
}

impl Code {
    pub fn label(self) -> &'static str {
        match self {
            Code::Utf8 => "UTF-8",
            Code::Ks => t("완성형", "KS"),
            Code::Johab => t("조합형", "Johab"),
            Code::English => t("영문", "English"),
        }
    }
}

pub fn decode_as(d: &[u8], code: Code) -> String {
    match code {
        Code::Utf8 => String::from_utf8_lossy(d).into_owned(),
        Code::Ks => encoding_rs::EUC_KR.decode_without_bom_handling(d).0.into_owned(),
        Code::Johab => johab(d).0,
        Code::English => cp437(d),
    }
}

/// Recognises UTF-8, KS (EUC-KR/CP949), Johab or plain 8-bit text, as 보라 does.
pub fn detect(d: &[u8]) -> Code {
    if std::str::from_utf8(d).is_ok() {
        return Code::Utf8;
    }
    let (_, ks_err) = encoding_rs::EUC_KR.decode_without_bom_handling(d);
    if !ks_err {
        return Code::Ks;
    }
    let (_, bad) = johab(d);
    let high = d.iter().filter(|&&b| b >= 0x80).count();
    if bad * 10 < high / 2 {
        Code::Johab
    } else {
        Code::English
    }
}

/// Simple DOS wildcard match ('*' and '?'), case-insensitive. "a" means "a*.*", ".doc" means "*.doc".
pub fn wild(pat: &str, name: &str) -> bool {
    let pat = expand_spec(pat);
    let p: Vec<char> = pat.to_lowercase().chars().collect();
    let n: Vec<char> = name.to_lowercase().chars().collect();
    // "*.*" matches names without a dot too
    if pat == "*.*" {
        return true;
    }
    fn m(p: &[char], n: &[char]) -> bool {
        match p.first() {
            None => n.is_empty(),
            Some('*') => (0..=n.len()).any(|i| m(&p[1..], &n[i..])),
            Some('?') => !n.is_empty() && m(&p[1..], &n[1..]),
            Some(c) => n.first() == Some(c) && m(&p[1..], &n[1..]),
        }
    }
    if m(&p, &n) {
        return true;
    }
    // "name.*" also matches "name" (no extension), as in DOS
    if let Some(stripped) = pat.strip_suffix(".*") {
        let p2: Vec<char> = stripped.to_lowercase().chars().collect();
        return !name.contains('.') && m(&p2, &n);
    }
    false
}

/// Normalises one file spec the way MdirIII reads the filter box.
pub fn expand_spec(s: &str) -> String {
    let s = s.trim();
    if s.is_empty() {
        return "*.*".into();
    }
    if s.starts_with('.') && !s.contains('*') && !s.contains('?') {
        return format!("*{}", s);
    }
    if !s.contains('*') && !s.contains('?') && !s.contains('.') {
        return format!("{}*.*", s);
    }
    if s.ends_with('.') {
        return s.trim_end_matches('.').to_string();
    }
    s.to_string()
}

/// Local date/time fields to a SystemTime (archive entries carry DOS date/time).
pub fn local_time(y: i32, mo: u32, d: u32, h: u32, mi: u32, s: u32) -> std::time::SystemTime {
    use chrono::TimeZone;
    chrono::Local
        .with_ymd_and_hms(y, mo.max(1), d.max(1), h, mi, s)
        .earliest()
        .map(|t| t.into())
        .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
}

/// (year, month, day, hour, minute, second) of a SystemTime in local time.
pub fn local_fields(t: std::time::SystemTime) -> (i32, u32, u32, u32, u32, u32) {
    use chrono::{Datelike, Timelike};
    let d: chrono::DateTime<chrono::Local> = t.into();
    (d.year(), d.month(), d.day(), d.hour(), d.minute(), d.second())
}

/// Unicode text to KS (EUC-KR / CP949) bytes.
pub fn encode_ks(s: &str) -> Vec<u8> {
    encoding_rs::EUC_KR.encode(s).0.into_owned()
}

/// In KS and Johab text every symbol took two columns. Symbols that Unicode draws one column wide
/// (lines, blocks, ▦ ▒ ①...) get a second cell so old screens and logos keep their shape.
pub fn widen(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + s.len() / 4);
    for c in s.chars() {
        out.push(c);
        if (c as u32) < 0x80 || c.width().unwrap_or(0) != 1 {
            continue;
        }
        let fill = match c {
            '─' | '┌' | '└' | '├' | '┬' | '┴' | '┼' | '┍' | '┕' | '┝' | '┯' | '┷' | '┿' | '┠' | '┨' | '┰' | '┸' | '╂' => '─',
            '━' | '┏' | '┗' | '┣' | '┳' | '┻' | '╋' | '┎' | '┖' | '┞' | '┟' | '┡' | '┢' | '┭' | '┮' | '┱' | '┲' | '┵' | '┶' | '┹' | '┺' | '┽' | '┾' | '╀' | '╁' | '╃' | '╄' | '╅' | '╆' | '╇' | '╈' | '╉' | '╊' => '━',
            '\u{2580}'..='\u{25ff}' => c,
            _ => ' ',
        };
        out.push(fill);
    }
    out
}

#[cfg(test)]
mod tt {
    use super::*;
    #[test]
    fn fmt() {
        assert_eq!(commas(52925), "52,925");
        assert_eq!(commas(1234567), "1,234,567");
        assert_eq!(name83("COMMAND.COM", false), "COMMAND  COM");
        assert_eq!(name83("verylongname.text", false), "verylon~ te~");
        assert_eq!(fit("가나다", 5), "가나 ");
    }
    #[test]
    fn wildcards() {
        assert!(wild("*.doc", "M.DOC"));
        assert!(wild(".doc", "a.doc"));
        assert!(wild("a", "abc.txt"));
        assert!(!wild("a", "bcd.txt"));
        assert!(wild("a.", "a"));
        assert!(wild("*.*", "noext"));
        assert!(wild("?.*", "x.c"));
    }
    #[test]
    fn hangul_codes() {
        // "한" in KS = B0 FA? use encoder round trip instead
        let (ks, _, _) = encoding_rs::EUC_KR.encode("한글 text");
        assert_eq!(detect(&ks), Code::Ks);
        assert_eq!(decode_as(&ks, Code::Ks), "한글 text");
        // Johab "가" = 0x8861
        assert_eq!(johab(&[0x88, 0x61]).0, "가");
        assert_eq!(cp437(&[0xDA, 0xC4, 0xBF]), "┌─┐");
    }
}

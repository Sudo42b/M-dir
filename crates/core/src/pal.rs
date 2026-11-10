//! DOS text attributes (fg | bg<<4) and the colour sets kept in MdirIII *.COL files.
use ratatui::style::{Color, Style};

/// Standard VGA text-mode palette.
pub const VGA: [(u8, u8, u8); 16] = [
    (0x00, 0x00, 0x00), (0x00, 0x00, 0xAA), (0x00, 0xAA, 0x00), (0x00, 0xAA, 0xAA),
    (0xAA, 0x00, 0x00), (0xAA, 0x00, 0xAA), (0xAA, 0x55, 0x00), (0xAA, 0xAA, 0xAA),
    (0x55, 0x55, 0x55), (0x55, 0x55, 0xFF), (0x55, 0xFF, 0x55), (0x55, 0xFF, 0xFF),
    (0xFF, 0x55, 0x55), (0xFF, 0x55, 0xFF), (0xFF, 0xFF, 0x55), (0xFF, 0xFF, 0xFF),
];

pub fn color(i: u8) -> Color {
    let (r, g, b) = VGA[(i & 15) as usize];
    Color::Rgb(r, g, b)
}

/// Attribute byte to style. Bit 7 is read as "bright background" (blink disabled), as MdirIII does.
pub fn st(a: u8) -> Style {
    Style::default().fg(color(a & 15)).bg(color(a >> 4))
}

/// Selection-bar attribute (M.EXE 0x33c8): black text on the item's colour; 0x70 when that is black.
pub fn bar(a: u8) -> u8 {
    match (a & 15) << 4 {
        0 => 0x70,
        b => b,
    }
}

// Indexes of the 14 colour items, in MSET's colour-menu order (BLACK.COL offset 0x48, pairs).
pub const MCD: usize = 0;
pub const MCD_STATUS: usize = 1;
pub const DIR: usize = 2;
pub const DRIVE: usize = 3;
pub const LINE: usize = 4;
pub const FKEY: usize = 5;
pub const FKEY_HI: usize = 6;
pub const COUNT: usize = 7;
pub const COUNT_HI: usize = 8;
pub const FILE: usize = 9;
pub const HELP: usize = 10;
pub const SEARCH: usize = 11;
pub const SEARCH_MCD: usize = 12;
pub const DIALOG: usize = 13;
pub const ITEM_NAMES: [(&str, &str); 14] = [
    ("MCD", "MCD"), ("MCD 상태선", "MCD status"), ("디렉토리명", "Directory"), ("드라이브명", "Drive"),
    ("메인화면 선", "Lines"), ("펑션키 보통", "Fkey normal"), ("펑션키 강조", "Fkey highlight"),
    ("파일수 보통", "Count normal"), ("파일수 강조", "Count highlight"), ("나머지 파일", "Other files"),
    ("도움말", "Help"), ("빨리찾기메인", "Search main"), ("빨리찾기MCD", "Search MCD"), ("대화상자", "Dialog"),
];

#[derive(Clone, Debug)]
pub struct Colors {
    pub items: [u8; 14],
    pub ext: Vec<(String, u8)>,
}

/// Bytes 0x00..0x48 of a .COL file: the signature line, then format data other MdirIII tools expect.
const COL_HEAD: &[u8] = b"%Mdir III Color Configuration File...\x1a  ^\0 / ^P\x03ZIP\x03ARJ\x03ARC\x03ICE\x03LHA\x03OMP\x03";
const COL_LEN: usize = 883;

/// Extension groups shared by the two built-in colour sets (foreground colour per group).
const EXT_GROUPS: &[(u8, &[&str])] = &[
    (0x0a, &["EXE"]),
    (0x0b, &["COM"]),
    (0x0e, &["BAT", "BTM"]),
    (0x0d, &["LZH", "ZIP", "ARJ", "ARC", "ICE", "LHA", "OMP", "ZOO", "UC2", "SQZ", "RAR", "HPK"]),
    (0x03, &["DOC", "HWP", "GWP", "TXT", "CAP", "LST", "1ST", "ME"]),
    (0x04, &["BAK", "$$$", "TMP", "OLZ"]),
    (0x09, &["PAS", "C", "CPP", "ASM", "BAS", "CBL"]),
    (0x01, &["H", "OBJ", "LIB"]),
    (0x02, &["GIF", "PCX", "JPG", "FLI", "ANM", "BMP", "LBM", "ICO", "MPG", "FLC"]),
    (0x06, &["IMS", "STM", "ROL", "BNK", "VOC", "MOD", "NST", "MID", "WRK", "PCM", "OKP", "CMF", "WAV", "S3M", "SOP", "OKM", "OKA"]),
];

fn ext_table(bg: u8) -> Vec<(String, u8)> {
    EXT_GROUPS.iter().flat_map(|(fg, es)| es.iter().map(move |e| (e.to_string(), fg | bg))).collect()
}

impl Colors {
    /// Black background (the default set).
    pub fn black() -> Colors {
        Colors { items: [0x9f, 0xf4, 0x0c, 0x06, 0x03, 0x3f, 0x3e, 0x70, 0x7f, 0x07, 0x6e, 0x1f, 0x5f, 0x9f], ext: ext_table(0x00) }
    }
    /// Blue background.
    pub fn blue() -> Colors {
        Colors { items: [0x9f, 0xf4, 0x1c, 0x16, 0x03, 0xf0, 0xf4, 0x70, 0x7f, 0x17, 0x3e, 0x9f, 0x5f, 0x0f], ext: ext_table(0x10) }
    }
}

impl Default for Colors {
    fn default() -> Self {
        Colors::black()
    }
}

impl Colors {
    /// Reads a "%Mdir III Color Configuration File" (.COL). Layout found by comparing BLACK.COL with BLUE.COL.
    pub fn parse(d: &[u8]) -> Option<Colors> {
        if d.len() < 0x220 || !d.starts_with(b"%Mdir III Color") {
            return None;
        }
        let mut items = [7u8; 14];
        for (i, it) in items.iter_mut().enumerate() {
            *it = d[0x48 + i * 2];
        }
        let n = d[0x84] as usize;
        let mut names = Vec::new();
        let mut p = 0x11b;
        while p + 4 <= d.len() && d[p] == 3 && names.len() < n {
            names.push(String::from_utf8_lossy(&d[p + 1..p + 4]).trim().to_string());
            p += 4;
        }
        let ext = names.into_iter().enumerate().map(|(i, s)| (s, d[0x85 + i])).collect();
        Some(Colors { items, ext })
    }

    pub fn write(&self) -> Vec<u8> {
        let mut d = vec![0u8; COL_LEN];
        d[..COL_HEAD.len()].copy_from_slice(COL_HEAD);
        d[0x48..0x84].fill(7);
        for (i, a) in self.items.iter().enumerate() {
            d[0x48 + i * 2] = *a;
        }
        d[0x84..0x11b].fill(0);
        d[0x11b..].fill(0);
        let n = self.ext.len().min(64);
        d[0x84] = n as u8;
        for (i, (e, a)) in self.ext.iter().take(n).enumerate() {
            d[0x85 + i] = *a;
            let p = 0x11b + i * 4;
            d[p] = 3;
            let b = format!("{:<3}", e.to_ascii_uppercase());
            d[p + 1..p + 4].copy_from_slice(&b.as_bytes()[..3]);
        }
        d
    }

    pub fn ext_color(&self, ext: &str) -> Option<u8> {
        self.ext.iter().find(|(e, _)| e.eq_ignore_ascii_case(ext)).map(|(_, a)| *a)
    }
}

#[cfg(test)]
mod t {
    use super::*;
    #[test]
    fn black_col() {
        let c = Colors::default();
        assert_eq!(c.items[DIR], 0x0c);
        assert_eq!(c.items[DIALOG], 0x9f);
        assert_eq!(c.ext_color("exe"), Some(0x0a));
        assert_eq!(c.ext_color("BAT"), Some(0x0e));
        let back = Colors::parse(&c.write()).unwrap();
        assert_eq!(back.items, c.items);
        assert_eq!(back.ext.len(), c.ext.len());
        let blue = Colors::parse(&Colors::blue().write()).unwrap();
        assert_eq!(blue.ext_color("zip"), Some(0x1d));
    }
}

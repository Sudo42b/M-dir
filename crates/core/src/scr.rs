//! Text-mode drawing on a ratatui Buffer, in DOS terms: (x, y, text, attribute).
use crate::pal::st;
use ratatui::buffer::Buffer;
use unicode_width::UnicodeWidthStr;

pub fn w(s: &str) -> u16 {
    UnicodeWidthStr::width(s) as u16
}

/// Width of a string written with ^highlight^ markers.
pub fn hw(s: &str) -> u16 {
    w(&s.replace('^', ""))
}

pub fn put(b: &mut Buffer, x: u16, y: u16, s: &str, a: u8) -> u16 {
    let area = b.area;
    if y >= area.bottom() || x >= area.right() {
        return x;
    }
    let max = (area.right() - x) as usize;
    let (nx, _) = b.set_stringn(x, y, s, max, st(a));
    nx
}

/// Writes `s` clipped/padded to exactly `width` cells.
pub fn putw(b: &mut Buffer, x: u16, y: u16, s: &str, width: u16, a: u8) {
    fill(b, x, y, width, 1, ' ', a);
    let area = b.area;
    if y >= area.bottom() || x >= area.right() {
        return;
    }
    let max = width.min(area.right() - x) as usize;
    b.set_stringn(x, y, s, max, st(a));
}

/// `^` toggles between the normal and the highlight attribute, as in M.HLP and the menus.
pub fn put_hl(b: &mut Buffer, x: u16, y: u16, s: &str, norm: u8, hi: u8) -> u16 {
    let mut x = x;
    for (i, part) in s.split('^').enumerate() {
        x = put(b, x, y, part, if i % 2 == 1 { hi } else { norm });
    }
    x
}

pub fn fill(b: &mut Buffer, x: u16, y: u16, wd: u16, h: u16, ch: char, a: u8) {
    let area = b.area;
    let mut tmp = [0u8; 4];
    let sym = ch.encode_utf8(&mut tmp);
    for yy in y..(y + h).min(area.bottom()) {
        for xx in x..(x + wd).min(area.right()) {
            if let Some(c) = b.cell_mut((xx, yy)) {
                c.set_symbol(sym).set_style(st(a));
            }
        }
    }
}

pub fn hline(b: &mut Buffer, x: u16, y: u16, wd: u16, ch: char, a: u8) {
    fill(b, x, y, wd, 1, ch, a)
}

/// Single (false) or double (true) frame with a cleared inside.
pub fn frame(b: &mut Buffer, x: u16, y: u16, wd: u16, h: u16, a: u8, double: bool) {
    if wd < 2 || h < 2 {
        return;
    }
    let (tl, tr, bl, br, hz, vt) = if double {
        ('╔', '╗', '╚', '╝', '═', '║')
    } else {
        ('┌', '┐', '└', '┘', '─', '│')
    };
    fill(b, x, y, wd, h, ' ', a);
    hline(b, x + 1, y, wd - 2, hz, a);
    hline(b, x + 1, y + h - 1, wd - 2, hz, a);
    fill(b, x, y + 1, 1, h - 2, vt, a);
    fill(b, x + wd - 1, y + 1, 1, h - 2, vt, a);
    put(b, x, y, &tl.to_string(), a);
    put(b, x + wd - 1, y, &tr.to_string(), a);
    put(b, x, y + h - 1, &bl.to_string(), a);
    put(b, x + wd - 1, y + h - 1, &br.to_string(), a);
}

/// Darkens the cells right of and below a box, like the DOS shadow.
pub fn shadow(b: &mut Buffer, x: u16, y: u16, wd: u16, h: u16) {
    let area = b.area;
    let dark = st(0x08);
    for yy in (y + 1)..=(y + h) {
        for xx in [x + wd, x + wd + 1] {
            if xx < area.right() && yy < area.bottom() {
                if let Some(c) = b.cell_mut((xx, yy)) {
                    c.set_style(dark);
                }
            }
        }
    }
    let yy = y + h;
    if yy < area.bottom() {
        for xx in (x + 2)..(x + wd + 2).min(area.right()) {
            if let Some(c) = b.cell_mut((xx, yy)) {
                c.set_style(dark);
            }
        }
    }
}

/// A framed box centred on the buffer; returns its top-left corner.
pub fn center_box(b: &mut Buffer, wd: u16, h: u16, a: u8, title: &str) -> (u16, u16) {
    let area = b.area;
    let wd = wd.min(area.width);
    let h = h.min(area.height);
    let x = area.x + (area.width - wd) / 2;
    let y = area.y + (area.height - h) / 2;
    frame(b, x, y, wd, h, a, false);
    shadow(b, x, y, wd, h);
    if !title.is_empty() {
        let tw = hw(title) + 2;
        put_hl(b, x + (wd.saturating_sub(tw)) / 2, y, &format!(" {} ", title), a, hi_of(a));
    }
    (x, y)
}

/// Highlight attribute for text on a dialog background: yellow (or white on bright backgrounds).
pub fn hi_of(a: u8) -> u8 {
    let bg = a & 0xf0;
    if bg == 0xe0 || bg == 0x60 {
        bg | 0x0f
    } else {
        bg | 0x0e
    }
}

/// Plain text of a buffer row range (used by self-tests to check what is on screen).
pub fn row_text(b: &Buffer, y: u16) -> String {
    let a = b.area;
    let mut s = String::new();
    let mut skip = false;
    for x in a.left()..a.right() {
        let sym = b[(x, y)].symbol();
        if skip {
            skip = false;
            continue;
        }
        skip = w(sym) >= 2;
        s.push_str(sym);
    }
    s
}

pub fn screen_text(b: &Buffer) -> String {
    (b.area.top()..b.area.bottom()).map(|y| row_text(b, y)).collect::<Vec<_>>().join("\n")
}
